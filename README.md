# `impl_trait_utils`

Utilities for working with impl traits in Rust.

## `trait_transformer`

Trait transformer is an experimental crate that generates specialized versions of a base trait. For example, if you want a `Send`able version of your trait, you'd write:

```rust
#[trait_transformer(SendIntFactory: Send)]
trait IntFactory {
    async fn make(&self) -> i32;
    // ..or..
    fn stream(&self) -> impl Iterator<Item = i32>;
    fn call(&self) -> u32;
}
```

Which creates a new `SendIntFactory: IntFactory + Send` trait and additionally bounds `SendIntFactory::make(): Send` and `SendIntFactory::stream(): Send`. The generated sytax is still experimental, as it relies on the nightly and unstable `async_fn_in_trait`, `return_position_impl_trait_in_trait`, and `return_type_notation` features.

#### License and usage notes

Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE) or
[MIT license](LICENSE-MIT) at your option.
