//! GPUI Image: decoding, processing and display for GPUI applications.
//!
//! The facade re-exports the pure core crate and contains the two GPUI-bound
//! modules directly:
//!
//! | Path          | Contents                 | GPUI dependency    |
//! | ------------- | ------------------------ | -------------------- |
//! | [`core`]      | decoding / processing    | none               |
//! | [`bridge`]    | `ImageSource` + cache    | exactly one flavor |
//! | [`component`] | `Image` / `ImageFallback`| `component` feature|
//!
//! ## GPUI flavors
//!
//! Select exactly one flavor: `gpui-kit` (GPUI Kit's `gpui-pre` types) or
//! `gpui-fast` (GPUI Fast). The selected flavor must be the GPUI the
//! application itself builds against, because every bridge operation hands
//! GPUI types (`ImageSource`, `ObjectFit`) to the application.
//!
//! ```ignore
//! // Decode, process, display.
//! use gpui_image::core::{EncodedFormat, Filter, ImageBuffer};
//!
//! let buffer = ImageBuffer::decode(&bytes)?
//!     .thumbnail(512)
//!     .filter(Filter::unsharp(1.0, 5));
//! let encoded = buffer.to_encoded(EncodedFormat::Png)?;
//! let source = gpui_image::bridge::image_source(encoded);
//! ```

/// Pure decoding / processing primitives. No GPUI dependency.
pub use gpui_image_core as core;

/// GPUI rendering bridge: `ImageSource` construction and caching.
pub mod bridge;

/// Generic display components (`Image` / `ImageFallback`).
#[cfg(feature = "component")]
pub mod component;
