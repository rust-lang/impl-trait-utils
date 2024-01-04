// Copyright (c) 2023 Google LLC
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use std::iter;

use proc_macro2::{Span, TokenStream};
use quote::quote;
use syn::{
    parse::{Parse, ParseStream},
    parse_macro_input,
    punctuated::Punctuated,
    token::Plus,
    Error, FnArg, Generics, Ident, ItemTrait, Pat, PatIdent, PatType, Receiver, Result, ReturnType,
    Signature, Token, TraitBound, TraitItem, TraitItemConst, TraitItemFn, TraitItemType, Type,
    TypeImplTrait, TypeParamBound, WhereClause,
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
    let TraitItem::Fn(fn_item @ TraitItemFn { sig, default, .. }) = item else {
        return item.clone();
    };
    let (sig, default) = if sig.asyncness.is_some() {
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
        let mut sig = sig.clone();
        if default.is_some() {
            add_receiver_bounds(&mut sig);
        }

        (
            Signature {
                asyncness: None,
                output: ReturnType::Type(syn::parse2(quote! { -> }).unwrap(), Box::new(ty)),
                ..sig.clone()
            },
            fn_item.default.as_ref().map(|b| {
                let items = sig.inputs.iter().map(|i| match i {
                    FnArg::Receiver(Receiver { self_token, .. }) => {
                        quote! { let __self = #self_token; }
                    }
                    FnArg::Typed(PatType { pat, .. }) => match pat.as_ref() {
                        Pat::Ident(PatIdent { ident, .. }) => quote! { let #ident = #ident; },
                        _ => todo!(),
                    },
                });

                struct ReplaceSelfVisitor;
                impl syn::visit_mut::VisitMut for ReplaceSelfVisitor {
                    fn visit_ident_mut(&mut self, ident: &mut syn::Ident) {
                        if ident == "self" {
                            *ident = syn::Ident::new("__self", ident.span());
                        }
                        syn::visit_mut::visit_ident_mut(self, ident);
                    }
                }

                let mut block = b.clone();
                syn::visit_mut::visit_block_mut(&mut ReplaceSelfVisitor, &mut block);

                syn::parse2(quote! { { async move { #(#items)* #block} } })
                    .expect("valid async block")
            }),
        )
    } else {
        match &sig.output {
            ReturnType::Type(arrow, ty) => match &**ty {
                Type::ImplTrait(it) => {
                    let ty = Type::ImplTrait(TypeImplTrait {
                        impl_token: it.impl_token,
                        bounds: it.bounds.iter().chain(bounds).cloned().collect(),
                    });
                    (
                        Signature {
                            output: ReturnType::Type(*arrow, Box::new(ty)),
                            ..sig.clone()
                        },
                        fn_item.default.clone(),
                    )
                }
                _ => return item.clone(),
            },
            ReturnType::Default => return item.clone(),
        }
    };
    TraitItem::Fn(TraitItemFn {
        sig,
        default,
        ..fn_item.clone()
    })
}

fn mk_blanket_impl(attrs: &Attrs, tr: &ItemTrait) -> TokenStream {
    let orig = &tr.ident;
    let variant = &attrs.variant.name;
    let items = tr.items.iter().map(|item| blanket_impl_item(item, variant));
    let self_is_sync = tr
        .items
        .iter()
        .any(|item| {
            matches!(
                item,
                TraitItem::Fn(TraitItemFn {
                    default: Some(_),
                    ..
                })
            )
        })
        .then(|| quote! { Self: Sync })
        .unwrap_or_default();
    quote! {
        impl<T> #orig for T
        where
            T: #variant,
            #self_is_sync
        {
            #(#items)*
        }
    }
}

fn blanket_impl_item(item: &TraitItem, variant: &Ident) -> TokenStream {
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
                const #ident #generics: #ty = <Self as #variant>::#ident;
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
                    <Self as #variant>::#ident(#(#args),*)#maybe_await
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
                type #ident<#params> = <Self as #variant>::#ident<#params> #where_clause;
            }
        }
        _ => Error::new_spanned(item, "unsupported item type").into_compile_error(),
    }
}

fn add_receiver_bounds(sig: &mut Signature) {
    if let Some(FnArg::Receiver(Receiver { ty, reference, .. })) = sig.inputs.first_mut() {
        let predicate =
            if let (Type::Reference(reference), Some((_and, lt))) = (&mut **ty, reference) {
                let lifetime = syn::Lifetime {
                    apostrophe: Span::mixed_site(),
                    ident: Ident::new("the_self_lt", Span::mixed_site()),
                };
                sig.generics.params.insert(
                    0,
                    syn::GenericParam::Lifetime(syn::LifetimeParam {
                        lifetime: lifetime.clone(),
                        colon_token: None,
                        bounds: Default::default(),
                        attrs: Default::default(),
                    }),
                );
                reference.lifetime = Some(lifetime.clone());
                let predicate = syn::parse2(quote! { #reference: Send }).unwrap();
                *lt = Some(lifetime);
                predicate
            } else {
                syn::parse2(quote! { #ty: Send }).unwrap()
            };

        if let Some(wh) = &mut sig.generics.where_clause {
            wh.predicates.push(predicate);
        } else {
            let where_clause = WhereClause {
                where_token: Token![where](Span::mixed_site()),
                predicates: Punctuated::from_iter([predicate]),
            };
            sig.generics.where_clause = Some(where_clause);
        }
    }
}
