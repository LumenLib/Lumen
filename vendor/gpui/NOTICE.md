# GPUI — Modified Version

This directory contains a vendored and modified copy of [GPUI](https://github.com/zed-industries/gpui),
originally authored by:

```
Copyright 2022 – 2025 Zed Industries, Inc.
Licensed under Apache 2.0 (see LICENSE-APACHE)
```

## Modifications

The following changes have been made by the Lumen project:

- Atlas memory management: added `MAX_POLYCHROME_TEXTURES` cap (4 textures)
  and force-eviction of the oldest texture when the cap is reached
  (files: `metal_atlas.rs`, `directx_atlas.rs`)

- Added `Thumbnail` atlas texture kind and `is_thumbnail` field for small images
  (files: `assets.rs`, `platform.rs`, `metal_atlas.rs`)

- Removed `examples/`, `tests/`, `docs/` directories

- Removed `test-support` feature, all `#[cfg(test)]` and `#[cfg(any(test, feature = "test-support"))]`
  conditional blocks, test infrastructure files (`platform/test.rs`, `app/test_context.rs`),
  test modules, test-only trait methods (`as_test`), and test-only public API
  (`TestAppContext`, `TestPlatform`, `TestDispatcher`, `TestWindow`, `VisualTestContext`,
  `smol_timeout`, `block_test`, etc.)

- Removed `dev-dependencies` from `Cargo.toml`
