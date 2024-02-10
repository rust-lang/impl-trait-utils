// Copyright (c) 2023 Google LLC
// Copyright (c) 2023 Various contributors (see git history)
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

#[trait_variant::make(Send + Sync)]
pub trait Trait {
    const CONST: &'static ();
    type Gat<'a>
    where
        Self: 'a;
    async fn assoc_async_fn_no_ret(a: (), b: ());
    async fn assoc_async_method_no_ret(&self, a: (), b: ());
    async fn assoc_async_fn(a: (), b: ()) -> ();
    async fn assoc_async_method(&self, a: (), b: ()) -> ();
    fn assoc_sync_fn_no_ret(a: (), b: ());
    fn assoc_sync_method_no_ret(&self, a: (), b: ());
    fn assoc_sync_fn(a: (), b: ()) -> ();
    fn assoc_sync_method(&self, a: (), b: ()) -> ();
    // FIXME: See #17.
    //async fn dft_assoc_async_fn_no_ret(_a: (), _b: ()) {}
    //async fn dft_assoc_async_method_no_ret(&self, _a: (), _b: ()) {}
    //async fn dft_assoc_async_fn(_a: (), _b: ()) -> () {}
    //async fn dft_assoc_async_method(&self, _a: (), _b: ()) -> () {}
    fn dft_assoc_sync_fn_no_ret(_a: (), _b: ()) {}
    fn dft_assoc_sync_method_no_ret(&self, _a: (), _b: ()) {}
    fn dft_assoc_sync_fn(_a: (), _b: ()) -> () {}
    fn dft_assoc_sync_method(&self, _a: (), _b: ()) -> () {}
}

impl Trait for () {
    const CONST: &'static () = &();
    type Gat<'a> = ();
    async fn assoc_async_fn_no_ret(_a: (), _b: ()) {}
    async fn assoc_async_method_no_ret(&self, _a: (), _b: ()) {}
    async fn assoc_async_fn(_a: (), _b: ()) -> () {}
    async fn assoc_async_method(&self, _a: (), _b: ()) -> () {}
    fn assoc_sync_fn_no_ret(_a: (), _b: ()) {}
    fn assoc_sync_method_no_ret(&self, _a: (), _b: ()) {}
    fn assoc_sync_fn(_a: (), _b: ()) -> () {}
    fn assoc_sync_method(&self, _a: (), _b: ()) -> () {}
}

fn is_bounded<T: Send + Sync>(_: T) {}

#[test]
fn test() {
    fn inner<T: Trait>(x: T) {
        let (a, b) = ((), ());
        is_bounded(<T as Trait>::assoc_async_fn_no_ret(a, b));
        is_bounded(<T as Trait>::assoc_async_method_no_ret(&x, a, b));
        is_bounded(<T as Trait>::assoc_async_fn(a, b));
        is_bounded(<T as Trait>::assoc_async_method(&x, a, b));
        // FIXME: See #17.
        //is_bounded(<T as Trait>::dft_assoc_async_fn_no_ret(a, b));
        //is_bounded(<T as Trait>::dft_assoc_async_method_no_ret(&x, a, b));
        //is_bounded(<T as Trait>::dft_assoc_async_fn(a, b));
        //is_bounded(<T as Trait>::dft_assoc_async_method(&x, a, b));
    }
    inner(());
}
