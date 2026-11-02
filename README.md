# gpui-image

Image decoding, processing and display for [GPUI](https://gpui.rs) applications.

The crate fills the gap between GPUI's `img()` primitive and application needs:
arbitrary bytes or files can be decoded, transformed (resize / crop / rotate /
flip / filter / thumbnail / format conversion) and handed back to GPUI as an
`ImageSource`, or rendered through the generic `Image` / `ImageFallback`
display components.

## Layout

Two crates: the pure `gpui-image-core` plus this facade, which owns the two
GPUI-bound modules directly.

| Path | Contents | GPUI dependency |
| ---- | -------- | --------------- |
| `gpui-image-core` | Decoding / encoding + processing primitives (pure logic) | none |
| `gpui_image::bridge` | `ImageSource` construction (`still_image`), `EncodedImage` conversion, `ImageCache` | exactly one flavor |
| `gpui_image::component` | Generic `Image` / `ImageFallback` components (`Sizable` / `Styled`) | `component` feature |

## GPUI flavors

Select exactly one flavor: `gpui-kit` (GPUI Kit's `gpui-pre` types, default) or
`gpui-fast` (GPUI Fast). Disable default features when selecting an
alternative. The selected flavor must be the GPUI the application itself builds
against, because the bridge hands GPUI types to the application.

The `gpui-pre` / `gpui-fast` pins always track the latest published GPUI Kit
release; `script/check-gpui-kit-pin.py` verifies the alignment in CI.

## Repository shape

This is an independent repository, mounted at `crates/gpui-image` of
`gpui-kit` through `.gitmodules` (the same pattern as `gpui-tray` and
`gpui-autostart`). It owns its `[workspace]` and `Cargo.lock`, is not a member
of the host workspace, and never modifies `gpui-base` — so syncing the host
with upstream is a submodule pointer bump with no textual conflicts.

## Example

```rust,ignore
use gpui_image::core::{EncodedFormat, Filter, ImageBuffer};

let buffer = ImageBuffer::decode(&bytes)?
    .thumbnail(512)
    .filter(Filter::unsharp(1.0, 5));
let encoded = buffer.to_encoded(EncodedFormat::Png)?;
let source = gpui_image::bridge::ImageSource::from(encoded);
```

See `examples/image_demo` for a runnable application and
`docs/plan/image-processing-display.md` in the host repository for the full
design (in Chinese).
