// Copyright (c) 2023 Google LLC
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use std::iter;

use proc_macro2::TokenStream;
use quote::{quote, ToTokens};
use syn::{
    parse::{Parse, ParseStream},
    parse_macro_input,
    punctuated::Punctuated,
    token::{Comma, Plus},
    Error, FnArg, GenericParam, Generics, Ident, ItemTrait, Lifetime, Pat, PatType, Result,
    ReturnType, Signature, Token, TraitBound, TraitItem, TraitItemConst, TraitItemFn,
    TraitItemType, Type, TypeImplTrait, TypeParamBound,
};

struct Attrs {
    variant: MakeVariant,
}

impl Parse for Attrs {
    fn parse(input: ParseStream) -> Result<Self> {
        Ok(Self {
            variant: MakeVariant::parse(input)?,
        })
    }
}

struct MakeVariant {
    name: Ident,
    #[allow(unused)]
    colon: Token![:],
    bounds: Punctuated<TraitBound, Plus>,
}

impl Parse for MakeVariant {
    fn parse(input: ParseStream) -> Result<Self> {
        Ok(Self {
            name: input.parse()?,
            colon: input.parse()?,
            bounds: input.parse_terminated(TraitBound::parse, Token![+])?,
        })
    }
}

pub fn make(
    attr: proc_macro::TokenStream,
    item: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    let attrs = parse_macro_input!(attr as Attrs);
    let item = parse_macro_input!(item as ItemTrait);

    let maybe_allow_async_lint = if attrs
        .variant
        .bounds
        .iter()
        .any(|b| b.path.segments.last().unwrap().ident == "Send")
    {
        quote! { #[allow(async_fn_in_trait)] }
    } else {
        quote! {}
    };

    let variant = mk_variant(&attrs, &item);
    let blanket_impl = mk_blanket_impl(&attrs, &item);

    quote! {
        #maybe_allow_async_lint
        #item

        #variant

        #blanket_impl
    }
    .into()
}

fn mk_variant(attrs: &Attrs, tr: &ItemTrait) -> TokenStream {
    let MakeVariant {
        ref name,
        colon: _,
        ref bounds,
    } = attrs.variant;
    let bounds: Vec<_> = bounds
        .into_iter()
        .map(|b| TypeParamBound::Trait(b.clone()))
        .collect();
    let variant = ItemTrait {
        ident: name.clone(),
        supertraits: tr.supertraits.iter().chain(&bounds).cloned().collect(),
        items: tr
            .items
            .iter()
            .map(|item| transform_item(item, &bounds))
            .collect(),
        ..tr.clone()
    };
    quote! { #variant }
}

fn transform_item(item: &TraitItem, bounds: &Vec<TypeParamBound>) -> TraitItem {
    // #[make_variant(SendIntFactory: Send)]
    // trait IntFactory {
    //     async fn make(&self, x: u32, y: &str) -> i32;
    //     fn stream(&self) -> impl Iterator<Item = i32>;
    //     fn call(&self) -> u32;
    // }
    //
    // becomes:
    //
    // trait SendIntFactory: Send {
    //     fn make(&self, x: u32, y: &str) -> impl ::core::future::Future<Output = i32> + Send;
    //     fn stream(&self) -> impl Iterator<Item = i32> + Send;
    //     fn call(&self) -> u32;
    // }
    let TraitItem::Fn(fn_item @ TraitItemFn { sig, .. }) = item else {
        return item.clone();
    };
    let (arrow, output) = if sig.asyncness.is_some() {
        let orig = match &sig.output {
            ReturnType::Default => quote! { () },
            ReturnType::Type(_, ty) => quote! { #ty },
        };
        let future = syn::parse2(quote! { ::core::future::Future<Output = #orig> }).unwrap();
        let ty = Type::ImplTrait(TypeImplTrait {
            impl_token: syn::parse2(quote! { impl }).unwrap(),
            bounds: iter::once(TypeParamBound::Trait(future))
                .chain(bounds.iter().cloned())
                .collect(),
        });
        (syn::parse2(quote! { -> }).unwrap(), ty)
    } else {
        match &sig.output {
            ReturnType::Type(arrow, ty) => match &**ty {
                Type::ImplTrait(it) => {
                    let ty = Type::ImplTrait(TypeImplTrait {
                        impl_token: it.impl_token,
                        bounds: it.bounds.iter().chain(bounds).cloned().collect(),
                    });
                    (*arrow, ty)
                }
                _ => return item.clone(),
            },
            ReturnType::Default => return item.clone(),
        }
    };
    TraitItem::Fn(TraitItemFn {
        sig: Signature {
            asyncness: None,
            output: ReturnType::Type(arrow, Box::new(output)),
            ..sig.clone()
        },
        ..fn_item.clone()
    })
}

fn mk_blanket_impl(attrs: &Attrs, tr: &ItemTrait) -> TokenStream {
    let orig = &tr.ident;
    let generics = &tr.generics.params;
    let mut generic_names = tr
        .generics
        .params
        .iter()
        .map(|generic| match generic {
            GenericParam::Lifetime(lt) => GenericParamName::Lifetime(&lt.lifetime),
            GenericParam::Type(ty) => GenericParamName::Type(&ty.ident),
            GenericParam::Const(co) => GenericParamName::Const(&co.ident),
        })
        .collect::<Punctuated<_, Comma>>();
    let trailing_comma = if !generic_names.is_empty() {
        generic_names.push_punct(Comma::default());
        quote! { , }
    } else {
        quote! {}
    };
    let variant = &attrs.variant.name;
    let items = tr
        .items
        .iter()
        .map(|item| blanket_impl_item(item, variant, &generic_names));
    let where_clauses = tr.generics.where_clause.as_ref().map(|wh| &wh.predicates);
    quote! {
        impl<#generics #trailing_comma TraitVariantBlanketType> #orig<#generic_names>
        for TraitVariantBlanketType
        where TraitVariantBlanketType: #variant<#generic_names>, #where_clauses
        {
            #(#items)*
        }
    }
}

enum GenericParamName<'s> {
    Lifetime(&'s Lifetime),
    Type(&'s Ident),
    Const(&'s Ident),
}

impl ToTokens for GenericParamName<'_> {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        match self {
            GenericParamName::Lifetime(lt) => lt.to_tokens(tokens),
            GenericParamName::Type(ty) => ty.to_tokens(tokens),
            GenericParamName::Const(co) => co.to_tokens(tokens),
        }
    }
}

fn blanket_impl_item(
    item: &TraitItem,
    variant: &Ident,
    generic_names: &Punctuated<GenericParamName<'_>, Comma>,
) -> TokenStream {
    // impl<T> IntFactory for T where T: SendIntFactory {
    //     const NAME: &'static str = <Self as SendIntFactory>::NAME;
    //     type MyFut<'a> = <Self as SendIntFactory>::MyFut<'a> where Self: 'a;
    //     async fn make(&self, x: u32, y: &str) -> i32 {
    //         <Self as SendIntFactory>::make(self, x, y).await
    //     }
    // }
    match item {
        TraitItem::Const(TraitItemConst {
            ident,
            generics,
            ty,
            ..
        }) => {
            quote! {
                const #ident #generics: #ty = <Self as #variant<#generic_names>>::#ident;
            }
        }
        TraitItem::Fn(TraitItemFn { sig, .. }) => {
            let ident = &sig.ident;
            let args = sig.inputs.iter().map(|arg| match arg {
                FnArg::Receiver(_) => quote! { self },
                FnArg::Typed(PatType { pat, .. }) => match &**pat {
                    Pat::Ident(arg) => quote! { #arg },
                    _ => Error::new_spanned(pat, "patterns are not supported in arguments")
                        .to_compile_error(),
                },
            });
            let maybe_await = if sig.asyncness.is_some() {
                quote! { .await }
            } else {
                quote! {}
            };
            quote! {
                #sig {
                    <Self as #variant<#generic_names>>::#ident(#(#args),*)#maybe_await
                }
            }
        }
        TraitItem::Type(TraitItemType {
            ident,
            generics:
                Generics {
                    params,
                    where_clause,
                    ..
                },
            ..
        }) => {
            quote! {
                type #ident<#params> = <Self as #variant<#generic_names>>::#ident<#params> #where_clause;
            }
        }
        _ => Error::new_spanned(item, "unsupported item type").into_compile_error(),
    }
}
