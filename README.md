# impl-trait-utils

[![Latest Version]][crates.io] [![Documentation]][docs.rs] [![GHA Status]][GitHub Actions] ![License]

Utilities for working with impl traits in Rust.

## `make_variant`

`make_variant` generates a specialized version of a base trait that uses `async fn` and/or `-> impl Trait`. For example, if you want a `Send`able version of your trait, you'd write:

```rust
#[trait_variant::make_variant(SendIntFactory: Send)]
trait IntFactory {
    async fn make(&self) -> i32;
    // ..or..
    fn stream(&self) -> impl Iterator<Item = i32>;
    fn call(&self) -> u32;
}
```

Which creates a new `SendIntFactory: IntFactory + Send` trait and additionally bounds `SendIntFactory::make(): Send` and `SendIntFactory::stream(): Send`. Ordinary methods are not affected.

Implementers of the trait can choose to implement the variant instead of the original trait. The macro creates a blanket impl which ensures that any type which implements the variant also implements the original trait.

## `trait_transformer`

`trait_transformer` does the same thing as `make_variant`, but using experimental nightly-only syntax that depends on the `return_type_notation` feature. It may be used to experiment with new kinds of trait transformations in the future.

#### License and usage notes

Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE) or
[MIT license](LICENSE-MIT) at your option.

[GitHub Actions]: https://github.com/rust-lang/impl-trait-utils/actions
[GHA Status]: https://github.com/rust-lang/impl-trait-utils/actions/workflows/rust.yml/badge.svg
[crates.io]: https://crates.io/crates/trait-variant
[Latest Version]: https://img.shields.io/crates/v/trait-variant.svg
[Documentation]: https://img.shields.io/docsrs/trait-variant
[docs.rs]: https://docs.rs/trait-variant
[License]: https://img.shields.io/crates/l/trait-variant.svg
