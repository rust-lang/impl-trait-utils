// Copyright (c) 2023 Google LLC
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

#![doc = include_str!("../README.md")]

mod variant;

/// Creates a specialized version of a base trait that adds bounds to `async
/// fn` and/or `-> impl Trait` return types.
///
/// ```
/// #[trait_variant::make(IntFactory: Send)]
/// trait LocalIntFactory {
///     async fn make(&self) -> i32;
///     fn stream(&self) -> impl Iterator<Item = i32>;
///     fn call(&self) -> u32;
/// }
/// ```
///
/// The above example causes a second trait called `IntFactory` to be created:
///
/// ```
/// # use core::future::Future;
/// trait IntFactory: Send {
///     fn make(&self) -> impl Future<Output = i32> + Send;
///     fn stream(&self) -> impl Iterator<Item = i32> + Send;
///     fn call(&self) -> u32;
/// }
/// ```
///
/// Note that ordinary methods such as `call` are not affected.
///
/// Implementers of the trait can choose to implement the variant instead of the
/// original trait. The macro creates a blanket impl which ensures that any type
/// which implements the variant also implements the original trait.
///
/// If a non-`Send` variant of the trait is not needed, the name of
/// new variant can simply be omitted.  E.g., this generates a
/// *single* (rather than an additional) trait whose definition
/// matches that in the expansion above:
///
/// #[trait_variant::make(Send)]
/// trait IntFactory {
///     async fn make(&self) -> i32;
///     fn stream(&self) -> impl Iterator<Item = i32>;
///     fn call(&self) -> u32;
/// }
/// ```
#[proc_macro_attribute]
pub fn make(
    attr: proc_macro::TokenStream,
    item: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    variant::make(attr, item)
}
