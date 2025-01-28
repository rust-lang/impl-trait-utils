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
    parse::{Parse, ParseStream}, parse_macro_input, parse_quote, punctuated::Punctuated, token::Plus, Error, FnArg, GenericParam, Ident, ItemTrait, Pat, PatIdent, PatType, Receiver, Result, ReturnType, Signature, Token, TraitBound, TraitItem, TraitItemConst, TraitItemFn, TraitItemType, Type, TypeGenerics, TypeImplTrait, TypeParam, TypeParamBound, TypeReference, WhereClause
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

enum MakeVariant {
    // Creates a variant of a trait under a new name with additional bounds while preserving the original trait.
    Create {
        name: Ident,
        _colon: Token![:],
        bounds: Punctuated<TraitBound, Plus>,
    },
    // Rewrites the original trait into a new trait with additional bounds.
    Rewrite {
        bounds: Punctuated<TraitBound, Plus>,
    },
}

impl Parse for MakeVariant {
    fn parse(input: ParseStream) -> Result<Self> {
        let variant = if input.peek(Ident) && input.peek2(Token![:]) {
            MakeVariant::Create {
                name: input.parse()?,
                _colon: input.parse()?,
                bounds: input.parse_terminated(TraitBound::parse, Token![+])?,
            }
        } else {
            MakeVariant::Rewrite {
                bounds: input.parse_terminated(TraitBound::parse, Token![+])?,
            }
        };
        Ok(variant)
    }
}

pub fn make(
    attr: proc_macro::TokenStream,
    item: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    let attrs = parse_macro_input!(attr as Attrs);
    let item = parse_macro_input!(item as ItemTrait);

    match attrs.variant {
        MakeVariant::Create { name, bounds, .. } => {
            let maybe_allow_async_lint = if bounds
                .iter()
                .any(|b| b.path.segments.last().unwrap().ident == "Send")
            {
                quote! { #[allow(async_fn_in_trait)] }
            } else {
                quote! {}
            };

            let variant = mk_variant(&name, bounds, &item);
            let blanket_impl = mk_blanket_impl(&name, &item);

            quote! {
                #maybe_allow_async_lint
                #item

                #variant

                #blanket_impl
            }
            .into()
        }
        MakeVariant::Rewrite { bounds, .. } => {
            let variant = mk_variant(&item.ident, bounds, &item);
            quote! {
                #variant
            }
            .into()
        }
    }
}

fn mk_variant(
    variant: &Ident,
    with_bounds: Punctuated<TraitBound, Plus>,
    tr: &ItemTrait,
) -> TokenStream {
    let bounds: Vec<_> = with_bounds
        .into_iter()
        .map(|b| TypeParamBound::Trait(b.clone()))
        .collect();
    let variant = ItemTrait {
        ident: variant.clone(),
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

// Transforms one item declaration within the definition if it has `async fn` and/or `-> impl Trait` return types by adding new bounds.
fn transform_item(item: &TraitItem, bounds: &Vec<TypeParamBound>) -> TraitItem {
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

                parse_quote! { { async move { #(#items)* #block} } }
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
    let (blanket_impl_generics, _ty, blanket_where_clause) = &mut blanket_generics.split_for_impl();
    let self_is_sync = tr.items.iter().any(|item| {
        matches!(
            item,
            TraitItem::Fn(TraitItemFn {
                default: Some(_),
                ..
            })
        )
    });

    let mut blanket_where_clause = blanket_where_clause
        .map(|w| w.predicates.clone())
        .unwrap_or_default();

    if self_is_sync {
        blanket_where_clause.push(parse_quote! { for<'s> &'s Self: Send });
    }

    quote! {
        impl #blanket_impl_generics #orig #orig_ty_generics for #blanket
            where
                #blanket_where_clause
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

fn add_receiver_bounds(sig: &mut Signature) {
    let Some(FnArg::Receiver(Receiver { ty, reference, .. })) = sig.inputs.first_mut() else {
        return;
    };
    let Type::Reference(
        recv_ty @ TypeReference {
            mutability: None, ..
        },
    ) = &mut **ty
    else {
        return;
    };
    let Some((_and, lt)) = reference else {
        return;
    };

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
    recv_ty.lifetime = Some(lifetime.clone());
    *lt = Some(lifetime);
    let predicate = parse_quote! { #recv_ty: Send };

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
