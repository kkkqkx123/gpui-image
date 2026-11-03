//! End-to-end demo: procedural pixels → core pipeline → bridge → `Image`.
//!
//! Without arguments, the left card shows a gradient the core built, filtered
//! and encoded without touching the network. The right card has no source and
//! renders its [`ImageFallback`] placeholder instead.
//!
//! For real-file verification, pass a path (and optionally a fit mode):
//!
//! ```sh
//! cargo run -p image_demo -- --path <image-file>            # decode from disk
//! cargo run -p image_demo -- --path <broken-file>           # shows the fallback
//! cargo run -p image_demo -- --path <img> --fit contain     # object-fit modes
//! ```
//!
//! A broken or non-image file exercises the real decode-failure path (distinct
//! from the no-source fallback above): the left card then shows the
//! [`ImageFallback`] placeholder too.

use gpui_image::bridge;
use gpui_image::component::{Image, ImageFallback};
use gpui_image::core::{EncodedFormat, EncodedImage, Filter, ImageBuffer};
use gpui_kit::component::{Size, Sizable as _, spinner::Spinner};
use gpui_kit::{
    App, AppContext as _, Context, IntoElement, ObjectFit, ParentElement as _, Render, Styled as _,
    Window, WindowOptions, div,
};

/// Build a 128x128 diagonal gradient, filter it, and PNG-encode it.
fn procedural_png() -> Vec<u8> {
    const SIDE: u32 = 128;
    let mut pixels = Vec::with_capacity((SIDE * SIDE * 4) as usize);
    for y in 0..SIDE {
        for x in 0..SIDE {
            pixels.extend_from_slice(&[
                (x * 255 / (SIDE - 1)) as u8,
                (y * 255 / (SIDE - 1)) as u8,
                128,
                255,
            ]);
        }
    }
    ImageBuffer::from_rgba8(SIDE, SIDE, pixels)
        .expect("procedural pixels match their dimensions")
        .filter(Filter::unsharp(1.0, 5))
        .encode(EncodedFormat::Png)
        .expect("encode procedural png")
}

/// Load an image file from disk through the real decode pipeline and
/// re-encode it to PNG for the bridge. `Err` on any failure (missing file,
/// unsupported format, corrupt data) — the caller shows the fallback.
#[allow(unused_mut)]
fn load_from_disk(path: &std::path::Path) -> Result<Vec<u8>, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("read {}: {e}", path.display()))?;
    let decoded =
        ImageBuffer::decode(&bytes).map_err(|e| format!("decode {}: {e}", path.display()))?;
    decoded
        .encode(EncodedFormat::Png)
        .map_err(|e| format!("re-encode {}: {e}", path.display()))
}

/// Parse a fit-mode name into the GPUI `ObjectFit` the `Image` accepts.
fn parse_fit(name: &str) -> Result<ObjectFit, String> {
    match name {
        "cover" => Ok(ObjectFit::Cover),
        "contain" => Ok(ObjectFit::Contain),
        "fill" => Ok(ObjectFit::Fill),
        "scale-down" => Ok(ObjectFit::ScaleDown),
        other => Err(format!(
            "unknown fit {other:?}: expected cover, contain, fill or scale-down"
        )),
    }
}

struct Demo {
    source: Option<gpui_kit::ImageSource>,
    /// The fit mode by name: `ObjectFit` is neither `Copy` nor `Clone`, so it
    /// is rebuilt from this name on every render.
    fit_name: &'static str,
    caption: String,
}

impl Render for Demo {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let fit = parse_fit(self.fit_name).expect("stored fit name is valid");
        let mut card = Image::new().object_fit(fit).aspect_ratio(1.);
        if let Some(source) = self.source.clone() {
            card = card.src(source);
        } else {
            card = card.placeholder(ImageFallback::new().child(Spinner::new().small()));
        }
        div()
            .flex()
            .flex_col()
            .gap_2()
            .p_4()
            .size_full()
            .child(self.caption.clone())
            .child(
                div()
                    .flex()
                    .gap_4()
                    .child(card)
                    .child(
                        // The always-fallback card: no source at all.
                        Image::new()
                            .with_size(Size::Large)
                            .placeholder(ImageFallback::new().child(Spinner::new().small())),
                    ),
            )
    }
}

fn main() {
    let mut args = std::env::args().skip(1);
    let mut path: Option<std::path::PathBuf> = None;
    let mut fit_name = "cover";
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--path" => match args.next() {
                Some(v) => path = Some(std::path::PathBuf::from(v)),
                None => {
                    eprintln!("--path requires a file path");
                    std::process::exit(2);
                }
            },
            "--fit" => {
                let Some(parsed) = args.next() else {
                    eprintln!("--fit requires cover|contain|fill|scale-down");
                    std::process::exit(2);
                };
                if let Err(error) = parse_fit(&parsed) {
                    eprintln!("{error}");
                    std::process::exit(2);
                }
                fit_name = Box::leak(parsed.into_boxed_str()) as &str;
            }
            other => {
                eprintln!("unknown argument {other:?}; usage: image_demo [--path FILE] [--fit cover|contain|fill|scale-down]");
                std::process::exit(2);
            }
        }
    }

    gpui_kit::application().run(move |cx: &mut App| {
        gpui_kit::init(cx);

        let (source, caption) = match &path {
            Some(path) => match load_from_disk(path) {
                Ok(bytes) => {
                    let encoded = EncodedImage::passthrough(bytes, EncodedFormat::Png);
                    (
                        Some(bridge::image_source(encoded)),
                        format!("{} (fit: {fit_name})", path.display()),
                    )
                }
                Err(error) => {
                    println!("decode failed on purpose-path: {error}; showing the fallback");
                    (None, format!("DECODE FAILED: {error}"))
                }
            },
            None => {
                let bytes = procedural_png();
                let encoded = EncodedImage::passthrough(bytes, EncodedFormat::Png);
                (
                    Some(bridge::image_source(encoded)),
                    "procedural gradient (default demo)".to_string(),
                )
            }
        };

        gpui_kit::open_window(WindowOptions::default(), cx, |_, cx| {
            cx.new(|_| Demo { source, fit_name, caption })
        })
        .expect("open the image demo window");
    });
}
