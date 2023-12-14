// Copyright (c) 2023 Google LLC
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use std::future::Future;

use trait_transformer::make_variant;

#[make_variant(SendIntFactory: Send)]
trait IntFactory {
    const NAME: &'static str;

    type MyFut<'a>: Future
    where
        Self: 'a;

    async fn make(&self, x: u32, y: &str) -> i32;
    fn stream(&self) -> impl Iterator<Item = i32>;
    fn call(&self) -> u32;
    fn another_async(&self, input: Result<(), &str>) -> Self::MyFut<'_>;
}

fn main() {}
