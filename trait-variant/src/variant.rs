// Copyright (c) 2023 Google LLC
// Copyright (c) 2023 Various contributors (see git history)
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use std::iter;

use proc_macro2::TokenStream;
use quote::quote;
use syn::{
    parse::{discouraged::Speculative as _, Parse, ParseStream},
    parse_macro_input, parse_quote,
    punctuated::Punctuated,
    token::Plus,
    Error, FnArg, GenericParam, Ident, ItemTrait, Pat, PatType, Result, ReturnType, Signature,
    Token, TraitBound, TraitItem, TraitItemConst, TraitItemFn, TraitItemType, Type, TypeGenerics,
    TypeImplTrait, TypeParam, TypeParamBound,
};

#[derive(Clone)]
struct Variant {
    name: Option<Ident>,
    _colon: Option<Token![:]>,
    bounds: Punctuated<TraitBound, Plus>,
}

fn parse_bounds_only(input: ParseStream) -> Result<Option<Variant>> {
    let fork = input.fork();
    let colon: Option<Token![:]> = fork.parse()?;
    let bounds = match fork.parse_terminated(TraitBound::parse, Token![+]) {
        Ok(x) => Ok(x),
        Err(e) if colon.is_some() => Err(e),
        Err(_) => return Ok(None),
    };
    input.advance_to(&fork);
    Ok(Some(Variant {
        name: None,
        _colon: colon,
        bounds: bounds?,
    }))
}

fn parse_fallback(input: ParseStream) -> Result<Variant> {
    let name: Ident = input.parse()?;
    let colon: Token![:] = input.parse()?;
    let bounds = input.parse_terminated(TraitBound::parse, Token![+])?;
    Ok(Variant {
        name: Some(name),
        _colon: Some(colon),
        bounds,
    })
}

impl Parse for Variant {
    fn parse(input: ParseStream) -> Result<Self> {
        match parse_bounds_only(input)? {
            Some(x) => Ok(x),
            None => parse_fallback(input),
        }
    }
}

pub fn make(
    attr: proc_macro::TokenStream,
    item: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    let variant = parse_macro_input!(attr as Variant);
    let item = parse_macro_input!(item as ItemTrait);

    let maybe_allow_async_lint = if variant
        .bounds
        .iter()
        .any(|b| b.path.segments.last().unwrap().ident == "Send")
    {
        quote! { #[allow(async_fn_in_trait)] }
    } else {
        quote! {}
    };

    let variant_name = variant.clone().name.unwrap_or(item.clone().ident);
    let variant_def = mk_variant(&variant_name, &variant.bounds, &item);
    if variant_name == item.ident {
        return variant_def.into();
    }
    let blanket_impl = Some(mk_blanket_impl(&variant_name, &item));
    quote! {
        #maybe_allow_async_lint
        #item

        #variant_def

        #blanket_impl
    }
    .into()
}

fn mk_variant(name: &Ident, bounds: &Punctuated<TraitBound, Plus>, tr: &ItemTrait) -> TokenStream {
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

fn mk_blanket_impl(variant: &Ident, tr: &ItemTrait) -> TokenStream {
    let orig = &tr.ident;
    let (_impl, orig_ty_generics, _where) = &tr.generics.split_for_impl();
    let items = tr
        .items
        .iter()
        .map(|item| blanket_impl_item(item, variant, orig_ty_generics));
    let blanket_bound: TypeParam =
        parse_quote!(TraitVariantBlanketType: #variant #orig_ty_generics);
    let blanket = &blanket_bound.ident.clone();
    let mut blanket_generics = tr.generics.clone();
    blanket_generics
        .params
        .push(GenericParam::Type(blanket_bound));
    let (blanket_impl_generics, _ty, blanket_where_clause) = &blanket_generics.split_for_impl();
    quote! {
        impl #blanket_impl_generics #orig #orig_ty_generics for #blanket #blanket_where_clause
        {
            #(#items)*
        }
    }
}

fn blanket_impl_item(
    item: &TraitItem,
    variant: &Ident,
    trait_ty_generics: &TypeGenerics<'_>,
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
                const #ident #generics: #ty = <Self as #variant #trait_ty_generics>::#ident;
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
                    <Self as #variant #trait_ty_generics>::#ident(#(#args),*)#maybe_await
                }
            }
        }
        TraitItem::Type(TraitItemType {
            ident, generics, ..
        }) => {
            let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();
            quote! {
                type #ident #impl_generics = <Self as #variant #trait_ty_generics>::#ident #ty_generics #where_clause;
            }
        }
        _ => Error::new_spanned(item, "unsupported item type").into_compile_error(),
    }
}
