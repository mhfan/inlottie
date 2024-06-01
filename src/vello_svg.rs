// Copyright 2023 the Vello Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Render a [`usvg::Tree`] to a Vello [`Scene`].
//!
//! This currently lacks support for a [number of SVG features](crate#unsupported-features).
//! This is because this integration was developed for examples,
//! which only need to support enough SVG to demonstrate Vello.
//!
//! However, this is also intended to be the preferred integration between Vello and [usvg],
//! so [consider contributing](https://github.com/linebender/vello_svg)
//! if you need a feature which is missing.
//!
//! [`render_tree_with`] is the primary entry point function, which supports choosing
//! the behaviour when [unsupported features](crate#unsupported-features) are detected.
//! In a future release where there are no unsupported features, this may be phased out
//!
//! [`render_tree`] is a convenience wrapper around [`render_tree_with`]
//! which renders an indicator around not yet supported features
//!
//! This crate also re-exports [`usvg`], to make handling dependency versions easier
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

use vello::{Scene, peniko};

/// Re-export vello.
pub use vello;

/// Re-export usvg.
pub use usvg;

/// Append a [`usvg::Tree`] into a Vello [`Scene`], with default error handling
/// This will draw a red box over (some) unsupported elements
///
/// Calls [`render_tree_with`] with an error handler implementing the above.
///
/// See the [module level documentation](crate#unsupported-features)
/// for a list of some unsupported svg features
#[inline] pub fn render_tree(scene: &mut Scene, svg: &usvg::Tree) {
    render_tree_with::<_, std::convert::Infallible>(scene, svg,
        &mut util::default_error_handler).unwrap_or_else(|e| match e {});
}

/// Append a [`usvg::Tree`] into a Vello [`Scene`].
///
/// Calls [`render_tree_with`] with [`util::default_error_handler`].
/// This will draw a red box over unsupported element types.
///
/// See the [module level documentation](crate#unsupported-features)
/// for a list of some unsupported svg features
#[inline] pub fn render_tree_with<F: FnMut(&mut Scene, &usvg::Node) -> Result<(), E>, E>(
    scene: &mut Scene, svg: &usvg::Tree, error_handler: &mut F) -> Result<(), E> {
    render_tree_impl(scene, svg, &svg.view_box(), &usvg::Transform::identity(), error_handler)
}

/// A helper function to render a tree with a given transform.
fn render_tree_impl<F: FnMut(&mut Scene, &usvg::Node) -> Result<(), E>, E>(
    scene: &mut Scene, tree: &usvg::Tree, view_box: &usvg::ViewBox, ts: &usvg::Transform,
    error_handler: &mut F) -> Result<(), E> {
    let ts = ts.pre_concat(view_box.to_transform(tree.size()));

    let flag = !ts.is_identity();
    if  flag { scene.push_layer(DEFAULT_BM, 1.0, util::to_affine(&ts),
            &util::to_rect_shape(&view_box.rect));
    }
    render_group(scene, tree.root(), &ts.pre_concat(tree.root().transform()), error_handler)?;
    if  flag { scene.pop_layer(); }     Ok(())
}

const DEFAULT_BM: peniko::BlendMode = peniko::BlendMode {
    mix: peniko::Mix::Clip,  compose: peniko::Compose::SrcOver,
};

fn render_group<F: FnMut(&mut Scene, &usvg::Node) -> Result<(), E>, E>(
    scene: &mut Scene, group: &usvg::Group, ts: &usvg::Transform, error_handler: &mut F)
    -> Result<(), E> {  let trfm = util::to_affine(ts);

    for node in group.children() {
        match node {
            usvg::Node::Group(group) => {
                let mut pushed_clip = false;
                if let Some(clip_path) = group.clip_path() {
                    if let Some(usvg::Node::Path(clip_path)) =
                        clip_path.root().children().first() {
                        // support clip-path with a single path
                        scene.push_layer(DEFAULT_BM, 1.0, trfm, &util::to_bez_path(clip_path));
                        pushed_clip = true;
                    }
                }

                render_group(scene, group, &ts.pre_concat(group.transform()), error_handler)?;
                if pushed_clip { scene.pop_layer(); }
            }
            usvg::Node::Path(path) => {
                if path.visibility() != usvg::Visibility::Visible { continue }
                let local_path = util::to_bez_path(path);

                let do_fill =
                    |scene: &mut Scene, error_handler: &mut F| {
                    if let Some(fill) = path.fill() {
                        if let Some((brush, brush_trfm)) =
                            util::to_brush(fill.paint(), fill.opacity()) {
                            scene.fill(match fill.rule() {
                                usvg::FillRule::NonZero => peniko::Fill::NonZero,
                                usvg::FillRule::EvenOdd => peniko::Fill::EvenOdd,
                            },  trfm, &brush, Some(brush_trfm), &local_path);
                        } else { return error_handler(scene, node); }
                    }   Ok(())
                };
                let do_stroke =
                    |scene: &mut Scene, error_handler: &mut F| {
                    if let Some(stroke) = path.stroke() {
                        if let Some((brush, brush_trfm)) =
                            util::to_brush(stroke.paint(), stroke.opacity()) {
                            scene.stroke(&util::to_stroke(stroke),
                                trfm, &brush, Some(brush_trfm), &local_path);
                        } else { return error_handler(scene, node); }
                    }   Ok(())
                };
                match path.paint_order() {
                    usvg::PaintOrder::FillAndStroke => {
                        do_fill  (scene, error_handler)?;
                        do_stroke(scene, error_handler)?;
                    }
                    usvg::PaintOrder::StrokeAndFill => {
                        do_stroke(scene, error_handler)?;
                        do_fill  (scene, error_handler)?;
                    }
                }
            }
            usvg::Node::Image(img) => {
                if img.visibility() != usvg::Visibility::Visible { continue }
                match img.kind() {
                    usvg::ImageKind::PNG(_) | usvg::ImageKind::JPEG(_) |
                    usvg::ImageKind::GIF(_) => {
                        let Ok(image) = util::decode_raw_raster_image(img.kind())
                        else { error_handler(scene, node)?; continue };

                        let view_box = img.view_box();
                        let image_ts = view_box.to_transform( // XXX:
                            usvg::Size::from_wh(image.width as _, image.height as _).unwrap());

                        scene.push_layer(DEFAULT_BM, 1.0, trfm,
                            &util::to_rect_shape(&view_box.rect));
                        scene.draw_image(&image, util::to_affine(&ts.pre_concat(image_ts)));
                        scene.pop_layer();
                    }
                    usvg::ImageKind::SVG(svg) =>
                        //render_group(scene, svg, &ts.pre_concat(
                        //    img.view_box().to_transform(svg.size())), error_handler)?,
                        render_tree_impl(scene, svg, &img.view_box(), ts, error_handler)?,
                }
            }
            usvg::Node::Text(text) => { let group = text.flattened();
                render_group(scene, group, &ts.pre_concat(group.transform()), error_handler)?;
            }
        }
    }   Ok(())
}

mod util {
use vello::peniko::{self, Brush, Color, Fill, Image};
use vello::kurbo::{Affine, BezPath, Point, Rect, Stroke};

#[inline] pub fn to_affine(ts: &usvg::Transform) -> Affine {
    Affine::new([ts.sx, ts.kx, ts.ky, ts.sy, ts.tx, ts.ty].map(f64::from))
}

#[inline] pub fn to_rect_shape(rect: &usvg::NonZeroRect) -> Rect {
    Rect::new(rect.left()  as _, rect.top() as _, rect.right() as _, rect.bottom() as _)
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
pub fn default_error_handler(scene: &mut vello::Scene, node: &usvg::Node)
    -> Result<(), std::convert::Infallible> {
    scene.fill(Fill::NonZero, Affine::IDENTITY, Color::RED.with_alpha_factor(0.5),
        None, &to_rect_shape(&node.bounding_box().to_non_zero_rect().unwrap()));
    Ok(())
}

pub fn decode_raw_raster_image(img: &usvg::ImageKind)
    -> Result<Image, image::ImageError> {
    let image = match img {
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
