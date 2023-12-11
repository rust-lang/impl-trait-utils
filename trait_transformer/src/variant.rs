// Copyright (c) 2023 Google LLC
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
    parse::{Parse, ParseStream},
    parse_macro_input,
    punctuated::Punctuated,
    token::Plus,
    Ident, ItemTrait, Result, ReturnType, Signature, Token, TraitBound, TraitItem, TraitItemFn,
    Type, TypeImplTrait, TypeParamBound,
};

struct Attrs {
    variant: Variant,
}

impl Parse for Attrs {
    fn parse(input: ParseStream) -> Result<Self> {
        Ok(Self {
            variant: Variant::parse(input)?,
        })
    }
}

struct Variant {
    name: Ident,
    #[allow(unused)]
    colon: Token![:],
    bounds: Punctuated<TraitBound, Plus>,
}

impl Parse for Variant {
    fn parse(input: ParseStream) -> Result<Self> {
        Ok(Self {
            name: input.parse()?,
            colon: input.parse()?,
            bounds: input.parse_terminated(TraitBound::parse, Token![+])?,
        })
    }
}

pub fn variant(
    attr: proc_macro::TokenStream,
    item: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    let attrs = parse_macro_input!(attr as Attrs);
    let item = parse_macro_input!(item as ItemTrait);

    let variant = make_variant(&attrs, &item);
    let output = quote! {
        #item
        #variant
    };

    output.into()
}

fn make_variant(attrs: &Attrs, tr: &ItemTrait) -> TokenStream {
    let Variant {
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
                        impl_token: it.impl_token.clone(),
                        bounds: it.bounds.iter().chain(bounds).cloned().collect(),
                    });
                    (arrow.clone(), ty)
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