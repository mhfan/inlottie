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

const DEFAULT_BM: peniko::BlendMode = peniko::BlendMode {
    mix: peniko::Mix::Clip,  compose: peniko::Compose::SrcOver,
};

fn render_group<F: FnMut(&mut Scene, &usvg::Node)>(scene: &mut Scene,
    group: &usvg::Group, ts: &usvg::Transform, error_handler: &mut F) {
    for node in group.children() {
        match node {
            usvg::Node::Group(group) => {
                let mut pushed_clip = false;
                if let Some(clip_path) = group.clip_path() {
                    if let Some(usvg::Node::Path(clip_path)) =
                        clip_path.root().children().first() {
                        // XXX: support clip-path with a single path only
                        scene.push_layer(DEFAULT_BM, 1.0, util::to_affine(ts),
                            &util::to_bez_path(clip_path));     pushed_clip = true;
                    }
                }   // TODO: deal with group.mask()/filters()

                render_group(scene, group, &ts.pre_concat(group.transform()), error_handler);
                if pushed_clip { scene.pop_layer(); }
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
                        scene.draw_image(&image, util::to_affine(ts));
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
use vello::peniko::{self, Brush, Color, Fill, Image};
use vello::kurbo::{Affine, BezPath, Point, Rect, Stroke};

#[inline] pub fn to_affine(ts: &usvg::Transform) -> Affine {
    Affine::new([ts.sx, ts.kx, ts.ky, ts.sy, ts.tx, ts.ty].map(f64::from))
}

pub fn to_stroke(stroke: &usvg::Stroke) -> Stroke {
    use usvg::{LineCap, LineJoin};  use vello::kurbo::{Cap, Join};

    let mut conv_stroke = Stroke::new(stroke.width().get() as _)
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
        conv_stroke = conv_stroke.with_dashes(stroke.dashoffset() as _,
            dash_array.iter().map(|x| *x as f64));
    }   conv_stroke
}

pub fn to_bez_path(path: &usvg::Path) -> BezPath {
    let mut local_path = BezPath::new();
    use usvg::tiny_skia_path::PathSegment;

    for elt in path.data().segments() {
        match elt {
            PathSegment::MoveTo(p) => local_path.move_to(Point::new(p.x as _, p.y as _)),
            PathSegment::LineTo(p) => local_path.line_to(Point::new(p.x as _, p.y as _)),
            PathSegment::QuadTo(p1, p2) =>
                local_path .quad_to(Point::new(p1.x as _, p1.y as _),
                                    Point::new(p2.x as _, p2.y as _)),
            PathSegment::CubicTo(p1, p2, p3) =>
                local_path.curve_to(Point::new(p1.x as _, p1.y as _),
                                    Point::new(p2.x as _, p2.y as _),
                                    Point::new(p3.x as _, p3.y as _)),
            PathSegment::Close => local_path.close_path(),
        }
    }   local_path
}

pub fn to_brush(paint: &usvg::Paint, opacity: usvg::Opacity) -> Option<(Brush, Affine)> {
    use peniko::{ColorStop, Gradient};
    #[inline] fn convert_stops(stops: &[usvg::Stop], opacity: usvg::Opacity) -> Vec<ColorStop> {
        stops.iter().map(|stop| ColorStop {     offset: stop.offset().get(),
            color: Color::rgba8(stop.color().red, stop.color().green, stop.color().blue,
            (stop.opacity() * opacity).to_u8())
        }).collect()
    }

    match paint {
        usvg::Paint::Color(color) => Some((Brush::Solid(Color::rgba8(
            color.red, color.green, color.blue, opacity.to_u8())), Affine::IDENTITY)),
        usvg::Paint::LinearGradient(gr) => {
            let gradient = Gradient::new_linear(
                Point::new(gr.x1() as _, gr.y1() as _),
                Point::new(gr.x2() as _, gr.y2() as _)
            ).with_stops(convert_stops(gr.stops(), opacity).as_slice());
            Some((Brush::Gradient(gradient), to_affine(&gr.transform())))
        }
        usvg::Paint::RadialGradient(gr) => {
            let gradient = Gradient::new_two_point_radial(
                Point::new(gr.cx() as _, gr.cy() as _), 0.,
                Point::new(gr.fx() as _, gr.fy() as _), gr.r().get(),
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
    scene.fill(Fill::NonZero, Affine::IDENTITY, Color::RED.multiply_alpha(0.5), None, &rect);
}

pub fn decode_raw_raster_image(img: &usvg::ImageKind) -> Result<Image, image::ImageError> {
    let image = match img {
        usvg::ImageKind::WEBP(data) =>
            image::load_from_memory_with_format(data, image::ImageFormat::WebP),
        usvg::ImageKind::JPEG(data) =>
            image::load_from_memory_with_format(data, image::ImageFormat::Jpeg),
        usvg::ImageKind::PNG(data) =>
            image::load_from_memory_with_format(data, image::ImageFormat::Png),
        usvg::ImageKind::GIF(data) =>
            image::load_from_memory_with_format(data, image::ImageFormat::Gif),
        usvg::ImageKind::SVG(_) => unreachable!(),
    }?.into_rgba8();

    let (width, height) = (image.width(), image.height());
    Ok(Image::new(peniko::Blob::new(std::sync::Arc::new(image.into_vec())),
        peniko::Format::Rgba8, width, height))
}

}
