//! Generic image display components: [`Image`] and [`ImageFallback`].
//!
//! GPUI's `img()` primitive shows one image; `Attachment` and `Avatar` wrap it
//! for two specific jobs (attachment previews, user avatars). This crate wraps
//! it for the general job: show any [`ImageSource`] in a box with a chosen
//! [`ObjectFit`], an optional aspect ratio, and a fallback slot for the
//! loading and failure states.
//!
//! The element stays visually unopinionated (no background, border or radius
//! of its own): product styling arrives through `Styled`, exactly like the
//! unstyled roots in `gpui-base`.
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

use gpui::{
    AnyElement, App, Div, ImageSource, InteractiveElement, Interactivity, IntoElement, ObjectFit,
    ParentElement as _, Pixels, RenderOnce, StyleRefinement, Styled, Window, div, img, px,
};
use gpui_component::{Size, Sizable};

/// Default outer-box edge for [`Image`] per [`Size`].
///
/// The box is only a default: any explicit size set through `Styled`
/// (`.size()`, `.w_*()`, …) is refined over it last and wins. `Large` suits
/// list thumbnails; full-bleed photos should set their own bounds.
fn image_box_size(size: Size) -> Pixels {
    match size {
        Size::XSmall => px(24.),
        Size::Small => px(32.),
        Size::Medium => px(48.),
        Size::Large => px(64.),
    }
}

/// A generic image element.
///
/// Shows `src` fitted into a default box (see [`Sizable`], overridable through
/// `Styled`), or the placeholder when no source is set. Mirrors the builder
/// style of `AttachmentMedia` and `Avatar`: `src` / `object_fit` /
/// `placeholder` / `aspect_ratio`.
///
/// ```ignore
/// use gpui_image_component::{Image, ImageFallback};
///
/// Image::new()
///     .src("photo.png")
///     .object_fit(ObjectFit::Cover)
///     .aspect_ratio(16. / 9.)
///     .placeholder(ImageFallback::new().child(loading_spinner()))
/// ```
#[derive(IntoElement)]
pub struct Image {
    base: Div,
    style: StyleRefinement,
    source: Option<ImageSource>,
    object_fit: ObjectFit,
    aspect_ratio: Option<f32>,
    fallback: Option<AnyElement>,
    size: Size,
}

impl Image {
    /// Create an image with no source, `Contain` fit and a medium default box.
    pub fn new() -> Self {
        Self {
            base: div(),
            style: StyleRefinement::default(),
            source: None,
            object_fit: ObjectFit::Contain,
            aspect_ratio: None,
            fallback: None,
            size: Size::Medium,
        }
    }

    /// Set the image source: a path, URI, embedded asset, or bridge output
    /// such as `gpui-image-gpui`'s `still_image`.
    pub fn src(mut self, source: impl Into<ImageSource>) -> Self {
        self.source = Some(source.into());
        self
    }

    /// Set how the bitmap fits its box. Defaults to [`ObjectFit::Contain`].
    pub fn object_fit(mut self, fit: ObjectFit) -> Self {
        self.object_fit = fit;
        self
    }

    /// Fix the width/height ratio of the box (for example `16. / 9.`).
    /// Must be positive; non-positive values are ignored at render time.
    pub fn aspect_ratio(mut self, ratio: f32) -> Self {
        self.aspect_ratio = Some(ratio);
        self
    }

    /// Set the placeholder shown while no source is set (loading, failure,
    /// or empty states). Typically an [`ImageFallback`], but any element
    /// works; it is centered in the box by default.
    pub fn placeholder(mut self, fallback: impl IntoElement) -> Self {
        self.fallback = Some(fallback.into_any_element());
        self
    }

    /// Whether a source is set.
    pub fn has_source(&self) -> bool {
        self.source.is_some()
    }
}

impl Default for Image {
    fn default() -> Self {
        Self::new()
    }
}

impl Sizable for Image {
    fn with_size(mut self, size: impl Into<Size>) -> Self {
        self.size = size.into();
        self
    }
}

impl Styled for Image {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl InteractiveElement for Image {
    fn interactivity(&mut self) -> &mut Interactivity {
        self.base.interactivity()
    }
}

impl RenderOnce for Image {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        let picture = self.source.map(|source| {
            img(source)
                .size_full()
                .object_fit(self.object_fit)
        });
        let centered_fallback = self.fallback.map(|fallback| {
            div()
                .size_full()
                .flex()
                .items_center()
                .justify_center()
                .child(fallback)
                .into_any_element()
        });

        let mut root = self
            .base
            .relative()
            .overflow_hidden()
            .size(image_box_size(self.size));
        if let Some(ratio) = self.aspect_ratio {
            if ratio.is_finite() && ratio > 0.0 {
                root = root.aspect_ratio(ratio);
            }
        }
        match (picture, centered_fallback) {
            (Some(picture), _) => root.child(picture),
            (None, Some(fallback)) => root.child(fallback),
            (None, None) => root,
        }
        .refine_style(&self.style)
    }
}

/// Placeholder slot for [`Image`], shown while no source is set.
///
/// An unstyled centered slot, like `AvatarFallback`: the caller supplies the
/// visible children (spinner, icon, initials) and any product styling.
#[derive(IntoElement)]
pub struct ImageFallback {
    base: Div,
    style: StyleRefinement,
    children: Vec<AnyElement>,
}

impl ImageFallback {
    /// Create an empty fallback slot.
    pub fn new() -> Self {
        Self {
            base: div(),
            style: StyleRefinement::default(),
            children: Vec::new(),
        }
    }
}

impl Default for ImageFallback {
    fn default() -> Self {
        Self::new()
    }
}

impl Styled for ImageFallback {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl gpui::ParentElement for ImageFallback {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.children.extend(elements);
    }
}

impl RenderOnce for ImageFallback {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        self.base.children(self.children).refine_style(&self.style)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_image_box_sizes_grow_with_size() {
        let edges = [
            Size::XSmall,
            Size::Small,
            Size::Medium,
            Size::Large,
        ]
        .map(image_box_size);
        assert!(edges.windows(2).all(|pair| pair[0] < pair[1]));
    }

    #[test]
    fn test_image_builder() {
        assert!(!Image::new().has_source());
        assert!(Image::new().fallback.is_none());

        let image = Image::new()
            .src("photo.png")
            .object_fit(ObjectFit::Cover)
            .aspect_ratio(16. / 9.)
            .with_size(Size::Large)
            .placeholder(ImageFallback::new());
        assert!(image.has_source());
        assert!(image.size == Size::Large);
        assert!((image.aspect_ratio.unwrap() - 16. / 9.).abs() < f32::EPSILON);
        assert!(image.fallback.is_some());
    }

    #[test]
    fn test_image_fallback_builder() {
        let fallback = ImageFallback::new().child(div().size(px(8.)));
        assert_eq!(fallback.children.len(), 1);
    }
}
