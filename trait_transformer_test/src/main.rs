// Copyright (c) 2023 Google LLC
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use trait_transformer::trait_transformer;

#[trait_transformer(SendIntFactory: Send)]
trait IntFactory {
    async fn make(&self) -> i32;
    // ..or..
    fn stream(&self) -> impl Iterator<Item = i32>;
    fn call(&self) -> u32;
}

fn thing(factory: impl SendIntFactory) {
    tokio::task::spawn(|| async {
        factory.make().await;
    })
}

struct MyFactory;

impl IntFactory for MyFactory {
    async fn make(&self) -> i32 {
        todo!();
    }

    fn stream(&self) -> impl Iterator<Item = i32> {
        todo!();
    }

    fn call(&self) -> u32 {
        0
    }
}

fn main() {
    let my_factory = MyFactory;
    thing(my_factory);
}
