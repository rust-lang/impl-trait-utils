[![Latest Version]][crates.io] [![Documentation]][docs.rs] [![GHA Status]][GitHub Actions] ![License]

Utilities for working with `impl Trait`s in Rust.

## `trait_variant`

`trait_variant` generates a specialized version of a base trait that uses `async fn` and/or `-> impl Trait`.

For example, if you want a [`Send`][rust-std-send]able version of your trait, you'd write:

```rust
#[trait_variant::make(IntFactory: Send)]
trait LocalIntFactory {
    async fn make(&self) -> i32;
    fn stream(&self) -> impl Iterator<Item = i32>;
    fn call(&self) -> u32;
}
```

The `trait_variant::make` would generate an additional trait called `IntFactory`:

```rust
use core::future::Future;

trait IntFactory: Send {
   fn make(&self) -> impl Future<Output = i32> + Send;
   fn stream(&self) -> impl Iterator<Item = i32> + Send;
   fn call(&self) -> u32;
}
```

Implementers can choose to implement either `LocalIntFactory` or `IntFactory` as appropriate.

For more details, see the docs for [`trait_variant::make`].

[`trait_variant::make`]: https://docs.rs/trait-variant/latest/trait_variant/attr.make.html

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
[rust-std-send]: https://doc.rust-lang.org/std/marker/trait.Send.html
