//! Pure image decoding, encoding and processing primitives.
//!
//! This crate has no GPUI dependency by design: it turns encoded bytes into an
//! [`ImageBuffer`], transforms it through a consuming builder chain, and
//! serializes the result back into an [`EncodedImage`]. The GPUI side
//! (`bridge`) only ever sees the encoded bytes.

use std::{fmt, io::Cursor};

/// Error from decoding, encoding or describing an image.
#[derive(Debug)]
pub enum Error {
    /// The input bytes could not be decoded (unknown or corrupt data,
    /// or the codec for the detected format is not enabled).
    Decode(image::ImageError),
    /// The processed image could not be serialized into the target format.
    Encode(image::ImageError),
    /// The requested operation is not supported for the given format
    /// (for example, encoding to `Svg`, which is a bridge passthrough).
    UnsupportedFormat(&'static str),
    /// The input was empty.
    EmptyInput,
    /// Raw pixel data did not match the declared dimensions.
    InvalidPixels {
        /// Declared width.
        width: u32,
        /// Declared height.
        height: u32,
        /// Actual byte length.
        len: usize,
    },
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Decode(inner) => write!(f, "cannot decode image: {inner}"),
            Error::Encode(inner) => write!(f, "cannot encode image: {inner}"),
            Error::UnsupportedFormat(format) => {
                write!(f, "unsupported image format: {format}")
            }
            Error::EmptyInput => write!(f, "empty image input"),
            Error::InvalidPixels { width, height, len } => write!(
                f,
                "expected {width}x{height} RGBA bytes ({}), got {len}",
                (*width as usize) * (*height as usize) * 4,
            ),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Error::Decode(inner) | Error::Encode(inner) => Some(inner),
            Error::UnsupportedFormat(_) | Error::EmptyInput | Error::InvalidPixels { .. } => None,
        }
    }
}

/// Target (or passthrough) encoding of an image.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum EncodedFormat {
    /// PNG.
    Png,
    /// JPEG.
    Jpeg,
    /// GIF (first frame only when encoding).
    Gif,
    /// WebP.
    WebP,
    /// BMP.
    Bmp,
    /// TIFF.
    Tiff,
    /// SVG. The core never decodes or encodes SVG; the variant exists so SVG
    /// bytes can bypass the core and reach GPUI (which renders SVG itself) as
    /// a passthrough [`EncodedImage`].
    Svg,
}

impl EncodedFormat {
    /// File extension without a leading dot.
    pub const fn extension(self) -> &'static str {
        match self {
            EncodedFormat::Png => "png",
            EncodedFormat::Jpeg => "jpg",
            EncodedFormat::Gif => "gif",
            EncodedFormat::WebP => "webp",
            EncodedFormat::Bmp => "bmp",
            EncodedFormat::Tiff => "tiff",
            EncodedFormat::Svg => "svg",
        }
    }

    /// Canonical MIME type.
    pub const fn mime_type(self) -> &'static str {
        match self {
            EncodedFormat::Png => "image/png",
            EncodedFormat::Jpeg => "image/jpeg",
            EncodedFormat::Gif => "image/gif",
            EncodedFormat::WebP => "image/webp",
            EncodedFormat::Bmp => "image/bmp",
            EncodedFormat::Tiff => "image/tiff",
            EncodedFormat::Svg => "image/svg+xml",
        }
    }

    fn image_format(self) -> Result<image::ImageFormat, Error> {
        match self {
            EncodedFormat::Png => Ok(image::ImageFormat::Png),
            EncodedFormat::Jpeg => Ok(image::ImageFormat::Jpeg),
            EncodedFormat::Gif => Ok(image::ImageFormat::Gif),
            EncodedFormat::WebP => Ok(image::ImageFormat::WebP),
            EncodedFormat::Bmp => Ok(image::ImageFormat::Bmp),
            EncodedFormat::Tiff => Ok(image::ImageFormat::Tiff),
            EncodedFormat::Svg => Err(Error::UnsupportedFormat("svg")),
        }
    }
}

/// Sampling algorithm for [`ImageBuffer::resize`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ResizeFilter {
    /// Nearest neighbor. Fast, blocky.
    Nearest,
    /// Linear triangle filter. Good for upscaling.
    Triangle,
    /// Bicubic Catmull-Rom. Good default for photographs.
    CatmullRom,
    /// Gaussian blur approximation.
    Gaussian,
    /// Lanczos with a 3-lobed kernel. Slowest, highest quality.
    #[default]
    Lanczos3,
}

impl ResizeFilter {
    fn image_filter(self) -> image::imageops::FilterType {
        match self {
            ResizeFilter::Nearest => image::imageops::FilterType::Nearest,
            ResizeFilter::Triangle => image::imageops::FilterType::Triangle,
            ResizeFilter::CatmullRom => image::imageops::FilterType::CatmullRom,
            ResizeFilter::Gaussian => image::imageops::FilterType::Gaussian,
            ResizeFilter::Lanczos3 => image::imageops::FilterType::Lanczos3,
        }
    }
}

/// Clockwise rotation for [`ImageBuffer::rotate`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Rotation {
    /// 90 degrees clockwise.
    Deg90,
    /// 180 degrees.
    Deg180,
    /// 270 degrees clockwise (90 counter-clockwise).
    Deg270,
}

/// Mirror axis for [`ImageBuffer::flip`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Flip {
    /// Mirror left-right.
    Horizontal,
    /// Mirror top-bottom.
    Vertical,
}

/// Pixel filter for [`ImageBuffer::filter`].
#[derive(Clone, Debug, PartialEq)]
pub enum Filter {
    /// Gaussian blur with the given sigma.
    Blur {
        /// Blur radius.
        sigma: f32,
    },
    /// Unsharp mask with the given sigma and threshold.
    Unsharp {
        /// Blur radius.
        sigma: f32,
        /// Minimum brightness difference to sharpen.
        threshold: i32,
    },
    /// Adjust brightness by adding the value to every channel.
    Brightness {
        /// Delta in `[-255, 255]`.
        value: i32,
    },
    /// Adjust contrast around the midpoint.
    Contrast {
        /// Contrast factor; `1.0` is identity.
        value: f32,
    },
    /// Rotate the hue by the given degrees.
    HueRotate {
        /// Degrees in `[0, 360)`.
        degrees: i32,
    },
    /// Convert to grayscale.
    Grayscale,
    /// Invert every channel.
    Invert,
}

impl Filter {
    /// Gaussian blur with the given sigma.
    pub fn blur(sigma: f32) -> Self {
        Self::Blur { sigma }
    }

    /// Unsharp mask with the given sigma and threshold.
    pub fn unsharp(sigma: f32, threshold: i32) -> Self {
        Self::Unsharp { sigma, threshold }
    }

    /// Brighten by the given delta in `[-255, 255]`.
    pub fn brightness(value: i32) -> Self {
        Self::Brightness { value }
    }

    /// Adjust contrast; `1.0` is identity.
    pub fn contrast(value: f32) -> Self {
        Self::Contrast { value }
    }

    /// Rotate the hue by the given degrees.
    pub fn hue_rotate(degrees: i32) -> Self {
        Self::HueRotate { degrees }
    }
}

/// A decoded image plus a consuming transformation chain.
///
/// The buffer owns a `DynamicImage`; every transformation takes `self` by
/// value so pipelines read left to right:
///
/// ```ignore
/// let buffer = ImageBuffer::decode(&bytes)?
///     .thumbnail(512)
///     .filter(Filter::brightness(10))
///     .rotate(Rotation::Deg90);
/// let png = buffer.encode(EncodedFormat::Png)?;
/// ```
pub struct ImageBuffer {
    image: image::DynamicImage,
}

impl ImageBuffer {
    /// Decode image bytes, auto-detecting the format.
    ///
    /// Returns [`Error::EmptyInput`] for empty input and [`Error::Decode`]
    /// when no enabled codec recognizes the data (see crate features).
    pub fn decode(bytes: &[u8]) -> Result<Self, Error> {
        if bytes.is_empty() {
            return Err(Error::EmptyInput);
        }
        let image = image::load_from_memory(bytes).map_err(Error::Decode)?;
        Ok(Self { image })
    }

    /// Build a buffer from raw RGBA pixels (4 bytes per pixel, row-major).
    ///
    /// Useful for procedural images and tests; returns [`Error::InvalidPixels`]
    /// when `pixels.len() != width * height * 4`.
    pub fn from_rgba8(width: u32, height: u32, pixels: Vec<u8>) -> Result<Self, Error> {
        let expected = (width as usize) * (height as usize) * 4;
        if pixels.len() != expected {
            return Err(Error::InvalidPixels {
                width,
                height,
                len: pixels.len(),
            });
        }
        image::RgbaImage::from_raw(width, height, pixels)
            .map(image::DynamicImage::ImageRgba8)
            .map(|image| Self { image })
            .ok_or(Error::InvalidPixels {
                width,
                height,
                len: expected,
            })
    }

    /// Pixel dimensions `(width, height)`.
    pub fn dimensions(&self) -> (u32, u32) {
        (self.image.width(), self.image.height())
    }

    /// Image width in pixels.
    pub fn width(&self) -> u32 {
        self.image.width()
    }

    /// Image height in pixels.
    pub fn height(&self) -> u32 {
        self.image.height()
    }

    /// Whether the buffer holds no pixels.
    pub fn is_empty(&self) -> bool {
        self.image.width() == 0 || self.image.height() == 0
    }

    /// Scale to the exact dimensions with the given sampling filter.
    ///
    /// The aspect ratio is not preserved; use [`ImageBuffer::thumbnail`] for
    /// an aspect-preserving fit. (`image`'s own `resize` preserves the ratio,
    /// so this calls `resize_exact` to honor the contract above.)
    pub fn resize(mut self, width: u32, height: u32, filter: ResizeFilter) -> Self {
        let width = width.max(1);
        let height = height.max(1);
        self.image = self
            .image
            .resize_exact(width, height, filter.image_filter());
        self
    }

    /// Cut out a rectangle. Out-of-bounds coordinates are clamped to the
    /// image, so the call never panics; a fully out-of-bounds rect keeps the
    /// buffer unchanged.
    pub fn crop(mut self, x: u32, y: u32, width: u32, height: u32) -> Self {
        let (image_width, image_height) = (self.image.width(), self.image.height());
        if image_width == 0 || image_height == 0 || width == 0 || height == 0 {
            return self;
        }
        if x >= image_width || y >= image_height {
            return self;
        }
        let x = x.min(image_width - 1);
        let y = y.min(image_height - 1);
        let width = width.min(image_width - x).max(1);
        let height = height.min(image_height - y).max(1);
        self.image = self.image.crop(x, y, width, height);
        self
    }

    /// Rotate clockwise by the given angle.
    pub fn rotate(mut self, rotation: Rotation) -> Self {
        self.image = match rotation {
            Rotation::Deg90 => self.image.rotate90(),
            Rotation::Deg180 => self.image.rotate180(),
            Rotation::Deg270 => self.image.rotate270(),
        };
        self
    }

    /// Mirror along the given axis.
    pub fn flip(mut self, flip: Flip) -> Self {
        self.image = match flip {
            Flip::Horizontal => self.image.fliph(),
            Flip::Vertical => self.image.flipv(),
        };
        self
    }

    /// Apply a pixel filter.
    pub fn filter(mut self, filter: Filter) -> Self {
        self.image = match filter {
            Filter::Blur { sigma } => self.image.blur(sigma),
            Filter::Unsharp { sigma, threshold } => self.image.unsharpen(sigma, threshold),
            Filter::Brightness { value } => self.image.brighten(value),
            Filter::Contrast { value } => self.image.adjust_contrast(value),
            Filter::HueRotate { degrees } => self.image.huerotate(degrees),
            Filter::Grayscale => self.image.grayscale(),
            Filter::Invert => {
                self.image.invert();
                self.image
            }
        };
        self
    }

    /// Scale down so the longest edge is at most `max_edge`, keeping the
    /// aspect ratio. Images already within bounds are returned unchanged.
    pub fn thumbnail(mut self, max_edge: u32) -> Self {
        let max_edge = max_edge.max(1);
        let (width, height) = (self.image.width(), self.image.height());
        let longest = width.max(height);
        if longest <= max_edge || longest == 0 {
            return self;
        }
        let scale = max_edge as f32 / longest as f32;
        let width = ((width as f32 * scale).round() as u32).max(1);
        let height = ((height as f32 * scale).round() as u32).max(1);
        self.image = self.image.thumbnail(width, height);
        self
    }

    /// Normalize the buffer according to its EXIF orientation tag.
    ///
    /// Phase 1 is a passthrough: the buffer is returned unchanged. Phase 2
    /// wires an EXIF parser (feature-gated) and applies the rotation/flip the
    /// tag requires. The method exists now so pipelines do not change shape
    /// when the real implementation lands.
    pub fn auto_orient(self) -> Self {
        // TODO(phase-2): parse the EXIF orientation tag and rotate/flip.
        self
    }

    /// Serialize the buffer into the target format.
    pub fn encode(&self, format: EncodedFormat) -> Result<Vec<u8>, Error> {
        let format = format.image_format()?;
        let mut bytes = Vec::new();
        self.image
            .write_to(&mut Cursor::new(&mut bytes), format)
            .map_err(Error::Encode)?;
        Ok(bytes)
    }

    /// Serialize the buffer into an [`EncodedImage`] that the GPUI bridge
    /// can hand to the renderer.
    pub fn to_encoded(self, format: EncodedFormat) -> Result<EncodedImage, Error> {
        let bytes = self.encode(format)?;
        Ok(EncodedImage { bytes, format })
    }
}

/// Encoded image bytes plus their format: the handoff to `bridge`.
///
/// Besides `ImageBuffer::to_encoded`, SVG documents can bypass the core with
/// [`EncodedImage::passthrough`]: GPUI renders SVG itself, so the core never
/// needs an SVG rasterizer.
pub struct EncodedImage {
    bytes: Vec<u8>,
    format: EncodedFormat,
}

impl EncodedImage {
    /// Wrap already-encoded bytes (for example an SVG document) without
    /// touching the core pipeline.
    pub fn passthrough(bytes: Vec<u8>, format: EncodedFormat) -> Self {
        Self { bytes, format }
    }

    /// The encoded bytes.
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// The format the bytes are encoded in.
    pub fn format(&self) -> EncodedFormat {
        self.format
    }

    /// The encoded bytes, by value.
    pub fn into_bytes(self) -> Vec<u8> {
        self.bytes
    }

    /// Whether there is nothing to show.
    pub fn is_empty(&self) -> bool {
        self.bytes.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{DynamicImage, Rgba, RgbaImage};

    fn checkerboard(width: u32, height: u32) -> ImageBuffer {
        let mut pixels = RgbaImage::new(width, height);
        for (x, y, pixel) in pixels.enumerate_pixels_mut() {
            let light = (x + y) % 2 == 0;
            *pixel = if light {
                Rgba([240, 240, 240, 255])
            } else {
                Rgba([16, 16, 16, 255])
            };
        }
        ImageBuffer {
            image: DynamicImage::ImageRgba8(pixels),
        }
    }

    #[test]
    fn test_decode_rejects_empty_input() {
        assert!(matches!(ImageBuffer::decode(&[]), Err(Error::EmptyInput)));
    }

    #[test]
    fn test_decode_rejects_unknown_bytes() {
        assert!(matches!(
            ImageBuffer::decode(b"definitely not an image"),
            Err(Error::Decode(_))
        ));
    }

    #[test]
    fn test_from_rgba8_validates_length() {
        assert!(ImageBuffer::from_rgba8(2, 2, vec![0; 15]).is_err());
        let buffer = ImageBuffer::from_rgba8(2, 3, vec![128; 24]).expect("valid pixels");
        assert_eq!(buffer.dimensions(), (2, 3));
    }

    #[test]
    fn test_png_round_trip_preserves_dimensions() {
        let buffer = checkerboard(12, 8);
        let bytes = buffer.encode(EncodedFormat::Png).expect("encode png");
        assert_eq!(&bytes[..8], &[137, 80, 78, 71, 13, 10, 26, 10]);
        let decoded = ImageBuffer::decode(&bytes).expect("decode png");
        assert_eq!(decoded.dimensions(), (12, 8));
    }

    #[test]
    fn test_format_descriptors() {
        assert_eq!(EncodedFormat::Png.extension(), "png");
        assert_eq!(EncodedFormat::Jpeg.mime_type(), "image/jpeg");
        assert_eq!(EncodedFormat::Svg.mime_type(), "image/svg+xml");
    }

    #[test]
    fn test_encode_rejects_svg_passthrough_format() {
        let buffer = checkerboard(4, 4);
        let error = buffer.encode(EncodedFormat::Svg).unwrap_err();
        assert!(matches!(error, Error::UnsupportedFormat("svg")));
    }

    #[test]
    fn test_resize_produces_exact_dimensions() {
        let buffer = checkerboard(16, 16).resize(5, 3, ResizeFilter::Nearest);
        assert_eq!(buffer.dimensions(), (5, 3));
    }

    #[test]
    fn test_crop_clamps_out_of_bounds_rect() {
        let buffer = checkerboard(10, 10).crop(8, 8, 100, 100);
        assert_eq!(buffer.dimensions(), (2, 2));

        let unchanged = checkerboard(10, 10).crop(50, 50, 4, 4);
        assert_eq!(unchanged.dimensions(), (10, 10));
    }

    #[test]
    fn test_rotate_swaps_dimensions_on_quarter_turns() {
        assert_eq!(
            checkerboard(12, 8)
                .rotate(Rotation::Deg90)
                .dimensions(),
            (8, 12)
        );
        assert_eq!(
            checkerboard(12, 8)
                .rotate(Rotation::Deg270)
                .dimensions(),
            (8, 12)
        );
        assert_eq!(
            checkerboard(12, 8)
                .rotate(Rotation::Deg180)
                .dimensions(),
            (12, 8)
        );
    }

    #[test]
    fn test_thumbnail_constrains_longest_edge() {
        let small = checkerboard(32, 16).thumbnail(64);
        assert_eq!(small.dimensions(), (32, 16));

        let large = checkerboard(200, 100).thumbnail(50);
        assert_eq!(large.dimensions(), (50, 25));
    }

    #[test]
    fn test_filter_keeps_dimensions() {
        for filter in [
            Filter::blur(1.0),
            Filter::unsharp(1.0, 5),
            Filter::brightness(10),
            Filter::contrast(1.2),
            Filter::hue_rotate(45),
            Filter::Grayscale,
            Filter::Invert,
        ] {
            let buffer = checkerboard(8, 6).filter(filter);
            assert_eq!(buffer.dimensions(), (8, 6));
        }
    }

    #[test]
    fn test_pipeline_decodes_after_processing() {
        let bytes = checkerboard(64, 32)
            .thumbnail(32)
            .filter(Filter::Grayscale)
            .encode(EncodedFormat::Png)
            .expect("encode pipeline result");
        let decoded = ImageBuffer::decode(&bytes).expect("decode pipeline result");
        assert_eq!(decoded.dimensions(), (32, 16));
    }

    #[test]
    fn test_auto_orient_is_currently_a_passthrough() {
        let buffer = checkerboard(6, 4).auto_orient();
        assert_eq!(buffer.dimensions(), (6, 4));
    }
}
