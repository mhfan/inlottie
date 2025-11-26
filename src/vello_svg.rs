// Copyright 2023 the Vello Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Render an SVG document [`usvg::Tree`] to a Vello [`Scene`](vello::Scene).
//!
//! This currently lacks support for a [number of important](crate#unsupported-features)
//! SVG features.
//!
//! This is also intended to be the preferred integration between Vello and [usvg],
//! so [consider contributing](https://github.com/linebender/vello_svg)
//! if you need a feature which is missing.
//!
//! This crate also re-exports [`usvg`] and [`vello`], so you can easily
//! use the specific versions that are compatible with Vello SVG.
//!
//! # Unsupported features
//!
//! Missing features include:
//! - text (supported by usvg text flatten feature)
//! - group opacity
//! - mix-blend-modes
//! - clipping (?)
//! - masking
//! - filter effects
//! - group background
//! - path shape-rendering
//! - patterns

//pub mod util;
pub use vello;  // Re-export vello.
pub use usvg;   // Re-export usvg.
use vello::{Scene, peniko};

/// Append an [`usvg::Tree`] to a vello [`Scene`](vello::Scene), with default error handling.
///
/// This will draw a red box over (some) unsupported elements.
#[inline] pub fn render_tree(scene: &mut Scene, svg: &usvg::Tree) {
    render_tree_with(scene, svg, &mut util::default_error_handler);
}

/// Append an [`usvg::Tree`] to a vello [`Scene`](vello::Scene),
/// with user-provided error handling logic.
///
/// See the [module level documentation](crate#unsupported-features)
/// for a list of some unsupported svg features
#[inline] pub fn render_tree_with<F: FnMut(&mut Scene, &usvg::Node)>(
    scene: &mut Scene, svg: &usvg::Tree, error_handler: &mut F) {
    render_group(scene, svg.root(), &usvg::Transform::identity(), error_handler);
}

fn render_group<F: FnMut(&mut Scene, &usvg::Node)>(scene: &mut Scene,
    group: &usvg::Group, ts: &usvg::Transform, error_handler: &mut F) {
    for node in group.children() {
        //let trfm = util::to_affine(ts) * util::to_affine(&node.abs_transform());
        match node {
            usvg::Node::Group(group) => {
                let mix = match group.blend_mode() {
                    usvg::BlendMode::Normal     => peniko::Mix::Normal,
                    usvg::BlendMode::Multiply   => peniko::Mix::Multiply,
                    usvg::BlendMode::Screen     => peniko::Mix::Screen,
                    usvg::BlendMode::Overlay    => peniko::Mix::Overlay,
                    usvg::BlendMode::Darken     => peniko::Mix::Darken,
                    usvg::BlendMode::Lighten    => peniko::Mix::Lighten,
                    usvg::BlendMode::ColorDodge => peniko::Mix::ColorDodge,
                    usvg::BlendMode::ColorBurn  => peniko::Mix::ColorBurn,
                    usvg::BlendMode::HardLight  => peniko::Mix::HardLight,
                    usvg::BlendMode::SoftLight  => peniko::Mix::SoftLight,
                    usvg::BlendMode::Difference => peniko::Mix::Difference,
                    usvg::BlendMode::Exclusion  => peniko::Mix::Exclusion,
                    usvg::BlendMode::Hue        => peniko::Mix::Hue,
                    usvg::BlendMode::Saturation => peniko::Mix::Saturation,
                    usvg::BlendMode::Color      => peniko::Mix::Color,
                    usvg::BlendMode::Luminosity => peniko::Mix::Luminosity,
                };  // TODO: deal with group.mask()/filters()

                let clipped = match group.clip_path()
                    .and_then(|path| path.root().children().first()) {
                    Some(usvg::Node::Path(clip_path)) => {
                        let local_path = util::to_bez_path(clip_path);
                        scene.push_layer(peniko::BlendMode { mix,
                                compose: peniko::Compose::SrcOver, },
                            group.opacity().get(), util::to_affine(ts), &local_path);   true
                    }   _ => false,
                };  // support clip-path with a single path

                render_group(scene, group, &usvg::Transform::identity(), error_handler);
                if clipped { scene.pop_layer(); }
            }
            usvg::Node::Path(path) => if path.is_visible() {
                let local_path = util::to_bez_path(path);
                let trfm = util::to_affine(ts);

                let do_fill =
                    |scene: &mut Scene, error_handler: &mut F| {
                    if let Some(fill) = path.fill() {
                        if let Some((brush, brush_trfm)) =
                            util::to_brush(fill.paint(), fill.opacity()) {
                            scene.fill(match fill.rule() {
                                usvg::FillRule::NonZero => peniko::Fill::NonZero,
                                usvg::FillRule::EvenOdd => peniko::Fill::EvenOdd,
                            },  trfm, &brush, Some(brush_trfm), &local_path);
                        } else { error_handler(scene, node); }
                    }
                };
                let do_stroke =
                    |scene: &mut Scene, error_handler: &mut F| {
                    if let Some(stroke) = path.stroke() {
                        if let Some((brush, brush_trfm)) =
                            util::to_brush(stroke.paint(), stroke.opacity()) {
                            scene.stroke(&util::to_stroke(stroke),
                                trfm, &brush, Some(brush_trfm), &local_path);
                        } else { error_handler(scene, node); }
                    }
                };

                match path.paint_order() {
                    usvg::PaintOrder::FillAndStroke => {
                        do_fill  (scene, error_handler);
                        do_stroke(scene, error_handler);
                    }
                    usvg::PaintOrder::StrokeAndFill => {
                        do_stroke(scene, error_handler);
                        do_fill  (scene, error_handler);
                    }
                }
            }
            usvg::Node::Image(img) => if img.is_visible() {
                match img.kind() {
                    usvg::ImageKind::GIF(_) | usvg::ImageKind::WEBP(_) |
                    usvg::ImageKind::PNG(_) | usvg::ImageKind::JPEG(_) => {
                        let Ok(image) = util::decode_raw_raster_image(img.kind())
                        else { error_handler(scene, node); continue };
                        scene.draw_image(&util::into_image(image), util::to_affine(ts));
                    }
                    usvg::ImageKind::SVG(svg) =>
                        render_group(scene, svg.root(), ts, error_handler),
                }
            }
            usvg::Node::Text(text) => { let group = text.flattened();
                render_group(scene, group, &ts.pre_concat(group.transform()), error_handler);
            }
        }
    }
}

mod util {
use vello::kurbo::{Affine, BezPath, Rect, Stroke};
use vello::peniko::{self, Brush, Color, Fill, color::palette};

#[inline] pub fn to_affine(ts: &usvg::Transform) -> Affine {
    Affine::new([ts.sx, ts.ky, ts.kx, ts.sy, ts.tx, ts.ty].map(f64::from))
}

pub fn to_stroke(stroke: &usvg::Stroke) -> Stroke {
    use usvg::{LineCap, LineJoin};  use vello::kurbo::{Cap, Join};

    let conv_stroke = Stroke::new(stroke.width().get() as _)
          .with_caps(match stroke.linecap() {
            LineCap::Butt   => Cap::Butt,
            LineCap::Round  => Cap::Round,
            LineCap::Square => Cap::Square,
        }).with_join(match stroke.linejoin() {
            LineJoin::Miter | LineJoin::MiterClip => Join::Miter,
            LineJoin::Round => Join::Round,
            LineJoin::Bevel => Join::Bevel,
        }).with_miter_limit(stroke.miterlimit().get() as _);

    if let Some(dash_array) = stroke.dasharray() {
        conv_stroke.with_dashes(stroke.dashoffset() as _,
            dash_array.iter().map(|&x| x as f64))
    } else { conv_stroke }
}

pub fn to_bez_path(path: &usvg::Path) -> BezPath {
    let mut local_path = BezPath::new();
    use usvg::tiny_skia_path::PathSegment::*;

    for elt in path.data().segments() {
        match elt {
            MoveTo(p) => local_path.move_to((p.x, p.y)),
            LineTo(p) => local_path.line_to((p.x, p.y)),
            QuadTo(p1, p2) => local_path.quad_to((p1.x, p1.y), (p2.x, p2.y)),
            CubicTo(p1, p2, p3) =>
                local_path.curve_to((p1.x, p1.y), (p2.x, p2.y), (p3.x, p3.y)),
            Close => local_path.close_path(),
        }
    }   local_path
}

pub fn to_brush(paint: &usvg::Paint, opacity: usvg::Opacity) -> Option<(Brush, Affine)> {
    use peniko::{ColorStop, Gradient};
    #[inline] fn convert_stops(stops: &[usvg::Stop], opacity: usvg::Opacity) -> Vec<ColorStop> {
        stops.iter().map(|stop| ColorStop {     offset: stop.offset().get(),
            color: Color::from_rgba8(stop.color().red, stop.color().green, stop.color().blue,
                (stop.opacity() * opacity).to_u8()).into()
        }).collect()
    }

    match paint {
        usvg::Paint::Color(color) => Some((Brush::Solid(Color::from_rgba8(
            color.red, color.green, color.blue, opacity.to_u8())), Affine::IDENTITY)),
        usvg::Paint::LinearGradient(gr) => {
            let gradient = Gradient::new_linear(
                (gr.x1(), gr.y1()), (gr.x2(), gr.y2())
            ).with_stops(convert_stops(gr.stops(), opacity).as_slice());
            Some((Brush::Gradient(gradient), to_affine(&gr.transform())))
        }
        usvg::Paint::RadialGradient(gr) => {
            let gradient = Gradient::new_two_point_radial(
                (gr.cx(), gr.cy()), 0., (gr.fx(), gr.fy()), gr.r().get(),
            ).with_stops(convert_stops(gr.stops(), opacity).as_slice());
            Some((Brush::Gradient(gradient), to_affine(&gr.transform())))
        }
        usvg::Paint::Pattern(_) => None, // TODO:
        // https://github.com/RazrFalcon/resvg/blob/master/crates/resvg/src/path.rs#L179
    }
}

/// Error handler function for [`super::render_tree_with`]
/// which draws a transparent red box instead of unsupported SVG features
pub fn default_error_handler(scene: &mut vello::Scene, node: &usvg::Node) {
    let bb = node.bounding_box();
    let rect = Rect { x0: bb.left()   as _, y0: bb.top()    as _,
                            x1: bb.right()  as _, y1: bb.bottom() as _, };
    scene.fill(Fill::NonZero, Affine::IDENTITY,
        palette::css::RED.multiply_alpha(0.5), None, &rect);
}

pub fn into_image(image: image::ImageBuffer<image::Rgba<u8>, Vec<u8>>) -> peniko::ImageBrush {
    let (width, height) = (image.width(), image.height());
    peniko::ImageData {
        data: peniko::Blob::new(std::sync::Arc::new(image.into_vec())),
        alpha_type: peniko::ImageAlphaType::AlphaPremultiplied,
        format: peniko::ImageFormat::Rgba8, width, height,
    }.into()
}

pub fn decode_raw_raster_image(img: &usvg::ImageKind) ->
    Result<image::RgbaImage, image::ImageError> {
    let (data, format) = match img {
        usvg::ImageKind::JPEG(data) => (data, image::ImageFormat::Jpeg),
        usvg::ImageKind::PNG (data) => (data, image::ImageFormat::Png),
        usvg::ImageKind::GIF (data) => (data, image::ImageFormat::Gif),
        usvg::ImageKind::WEBP(data) => (data, image::ImageFormat::WebP),
        usvg::ImageKind::SVG(_) => unreachable!(),
    };

    image::load_from_memory_with_format(data, format)
        .map(|dyn_img| dyn_img.into_rgba8())
}

}
