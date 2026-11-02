//! End-to-end demo: procedural pixels → core pipeline → bridge → `Image`.
//!
//! The left card shows a gradient the core built, filtered and encoded
//! without touching the network. The right card has no source and renders
//! its [`ImageFallback`] placeholder instead.

use gpui_image::bridge::{self, ImageFormat};
use gpui_image::component::{Image, ImageFallback};
use gpui_image::core::{EncodedFormat, Filter, ImageBuffer};
use gpui_kit::component::{Size, Sizable as _, spinner::Spinner};
use gpui_kit::{App, Context, IntoElement, Render, Styled as _, Window, WindowOptions, div};

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

struct Demo {
    source: gpui_kit::ImageSource,
}

impl Render for Demo {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .gap_4()
            .p_4()
            .child(
                Image::new()
                    .src(self.source.clone())
                    .object_fit(gpui_kit::ObjectFit::Cover)
                    .aspect_ratio(1.),
            )
            .child(
                Image::new()
                    .with_size(Size::Large)
                    .placeholder(ImageFallback::new().child(Spinner::new().small())),
            )
    }
}

fn main() {
    gpui_kit::application().run(|cx: &mut App| {
        gpui_kit::init(cx);
        let bytes = procedural_png();
        let source = bridge::image_source(bytes, ImageFormat::Png);
        gpui_kit::open_window(WindowOptions::default(), cx, |_, cx| {
            cx.new(|_| Demo { source })
        })
        .expect("open the image demo window");
    });
}
