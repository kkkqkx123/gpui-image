//! GPUI rendering bridge: turn core output into something `img()` can show.
//!
//! GPUI has no `ImageSource::Bytes` variant: already-encoded bytes travel as
//! [`ImageSource::Image`], whose payload (`gpui::platform::Image`) this module
//! constructs with a process-unique id. Remote locations, files and embedded
//! assets keep using [`ImageSource::Resource`]; this bridge only covers the
//! "bytes we decoded or processed ourselves" path, plus an in-memory
//! [`ImageCache`] for derived thumbnails.
//!
//! ## GPUI flavors
//!
//! Select exactly one flavor: `gpui-kit` (GPUI Kit's `gpui-pre` types) or
//! `gpui-fast` (GPUI Fast). The selected flavor must be the GPUI the
//! application itself builds against.

#[cfg(all(feature = "gpui-kit", feature = "gpui-fast"))]
compile_error!(
    "features `gpui-kit` and `gpui-fast` are mutually exclusive; use default-features = false and select exactly one backend"
);

#[cfg(not(any(feature = "gpui-kit", feature = "gpui-fast")))]
compile_error!("enable exactly one of the `gpui-kit` or `gpui-fast` features");

// Both flavors arrive under a renamed dependency, so the selected one takes
// the name `gpui` here.
#[cfg(all(feature = "gpui-kit", not(feature = "gpui-fast")))]
extern crate gpui_kit_backend as gpui;
#[cfg(all(feature = "gpui-fast", not(feature = "gpui-kit")))]
extern crate gpui_fast_backend as gpui;

use std::{
    collections::HashMap,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
};

use gpui::{Image, ImageFormat as GpuiImageFormat, ImageSource};

pub use gpui::ImageFormat;
pub use gpui_image_core::{EncodedFormat, EncodedImage};

/// Counter for bridge-allocated image ids. GPUI keys its texture cache by id,
/// so every [`still_image`] gets a fresh one; re-displaying the same bytes
/// without caching is a cache miss by design (see [`ImageCache`]).
static NEXT_IMAGE_ID: AtomicU64 = AtomicU64::new(1);

/// Wrap already-encoded bytes so GPUI's `img()` can display them.
///
/// Each call mints a fresh image id. Callers that re-render the same bytes
/// should keep the returned [`ImageSource`] in an [`ImageCache`] instead of
/// calling this on every frame.
pub fn still_image(bytes: Vec<u8>, format: GpuiImageFormat) -> ImageSource {
    ImageSource::Image(Arc::new(Image {
        format,
        bytes,
        id: NEXT_IMAGE_ID.fetch_add(1, Ordering::Relaxed),
    }))
}

/// Map a core encoding to the GPUI image format the renderer understands.
pub fn gpui_format(format: EncodedFormat) -> GpuiImageFormat {
    match format {
        EncodedFormat::Png => GpuiImageFormat::Png,
        EncodedFormat::Jpeg => GpuiImageFormat::Jpeg,
        EncodedFormat::Gif => GpuiImageFormat::Gif,
        EncodedFormat::WebP => GpuiImageFormat::Webp,
        EncodedFormat::Bmp => GpuiImageFormat::Bmp,
        EncodedFormat::Tiff => GpuiImageFormat::Tiff,
        EncodedFormat::Svg => GpuiImageFormat::Svg,
    }
}

/// Turn core pipeline output into something `img()` can display.
///
/// A free function rather than `From<EncodedImage>`: both the trait and the
/// target type are foreign here, so the impl would break the orphan rule.
/// SVG passthroughs map to [`GpuiImageFormat::Svg`], which GPUI renders itself.
pub fn image_source(encoded: EncodedImage) -> ImageSource {
    let format = gpui_format(encoded.format());
    still_image(encoded.into_bytes(), format)
}

/// In-memory memo from an application key to a display-ready [`ImageSource`].
///
/// The cache owns no loading policy: fetching bytes (disk, network, asset) is
/// the caller's job; the cache only remembers what the bridge already built,
/// typically thumbnails or otherwise derived images that are expensive to
/// re-encode. It needs no GPUI `App`: entries are plain data.
pub struct ImageCache {
    entries: HashMap<String, ImageSource>,
}

impl ImageCache {
    /// Create an empty cache.
    pub fn new() -> Self {
        Self {
            entries: HashMap::new(),
        }
    }

    /// Look up a cached source without building it.
    pub fn get(&self, key: &str) -> Option<ImageSource> {
        self.entries.get(key).cloned()
    }

    /// Return the cached source for `key`, building and storing it on a miss.
    pub fn get_or_insert(
        &mut self,
        key: &str,
        make: impl FnOnce() -> ImageSource,
    ) -> ImageSource {
        if let Some(source) = self.entries.get(key) {
            return source.clone();
        }
        let source = make();
        self.entries.insert(key.to_owned(), source.clone());
        source
    }

    /// Store a source, returning the previous one for the key, if any.
    pub fn insert(
        &mut self,
        key: impl Into<String>,
        source: ImageSource,
    ) -> Option<ImageSource> {
        self.entries.insert(key.into(), source)
    }

    /// Drop the entry for `key`, returning it if present.
    pub fn remove(&mut self, key: &str) -> Option<ImageSource> {
        self.entries.remove(key)
    }

    /// Number of cached entries.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the cache holds nothing.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Drop every entry.
    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

impl Default for ImageCache {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn png_source() -> ImageSource {
        still_image(vec![1, 2, 3], GpuiImageFormat::Png)
    }

    fn image_id(source: &ImageSource) -> Option<u64> {
        match source {
            ImageSource::Image(image) => Some(image.id),
            _ => None,
        }
    }

    #[test]
    fn test_still_image_carries_bytes_and_format() {
        let bytes = vec![137, 80, 78, 71];
        let source = still_image(bytes.clone(), GpuiImageFormat::Png);
        match source {
            ImageSource::Image(image) => {
                assert_eq!(image.bytes, bytes);
                assert_eq!(image.format, GpuiImageFormat::Png);
            }
            _ => panic!("expected ImageSource::Image"),
        }
    }

    #[test]
    fn test_still_image_mints_unique_ids() {
        let first = image_id(&png_source()).expect("image id");
        let second = image_id(&png_source()).expect("image id");
        assert_ne!(first, second);
    }

    #[test]
    fn test_image_source_converts_with_mapped_format() {
        for (encoded_format, gpui_format) in [
            (EncodedFormat::Png, GpuiImageFormat::Png),
            (EncodedFormat::Jpeg, GpuiImageFormat::Jpeg),
            (EncodedFormat::Gif, GpuiImageFormat::Gif),
            (EncodedFormat::WebP, GpuiImageFormat::Webp),
            (EncodedFormat::Bmp, GpuiImageFormat::Bmp),
            (EncodedFormat::Tiff, GpuiImageFormat::Tiff),
            (EncodedFormat::Svg, GpuiImageFormat::Svg),
        ] {
            let encoded = EncodedImage::passthrough(vec![9, 9], encoded_format);
            let source = image_source(encoded);
            match source {
                ImageSource::Image(image) => {
                    assert_eq!(image.format, gpui_format);
                    assert_eq!(image.bytes, vec![9, 9]);
                }
                _ => panic!("expected ImageSource::Image"),
            }
        }
    }

    #[test]
    fn test_cache_builds_once_and_reuses() {
        let mut cache = ImageCache::new();
        assert!(cache.is_empty());

        let mut builds = 0;
        let first = cache.get_or_insert("thumb", || {
            builds += 1;
            png_source()
        });
        let second = cache.get_or_insert("thumb", || {
            builds += 1;
            png_source()
        });
        assert_eq!(builds, 1);
        assert_eq!(image_id(&first), image_id(&second));
        assert_eq!(cache.len(), 1);

        assert!(cache.get("missing").is_none());
        cache.remove("thumb");
        assert!(cache.is_empty());
        cache.insert("thumb", png_source());
        cache.clear();
        assert!(cache.is_empty());
    }
}
