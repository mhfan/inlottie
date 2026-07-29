/****************************************************************
 * $ID: femtovg.rs  	Thu 27 Nov 2025 13:01:22+0800           *
 *                                                              *
 * Maintainer: 范美辉 (MeiHui FAN) <mhfan@ustc.edu>              *
 * Copyright (c) 2025 M.H.Fan, All rights reserved.             *
 ****************************************************************/

use crate::core::{CompositeContext, helpers::{Vec2D, RGBA},
    pathm::{PathBuilder, BezPath, PathFactory},
    schema::{VisualLayer, MatteMode, MaskMode, FillRule, LineJoin, LineCap},
    style::{StyleConv, MatrixConv, TM2DwO, FSOpts}, render::RenderContext
};
use femtovg::{PixelFormat, ImageId, ImageFlags, RenderTarget, Color as VGColor,
    CompositeOperation as CompOp, renderer::SurfacelessRenderer};
const CLEAR_COLOR: VGColor = VGColor::rgbaf(0., 0., 0., 0.);
const  MASK_COLOR: VGColor = VGColor::rgbaf(1., 1., 1., 1.);

pub struct FemtovgContext<'a, T: SurfacelessRenderer> {
    canvas: &'a mut femtovg::Canvas<T>, target: RenderTarget,
}
pub struct Offscreen { image: ImageId, parent: RenderTarget }

impl<'a, T: SurfacelessRenderer> FemtovgContext<'a, T> {
    pub fn new(canvas: &'a mut femtovg::Canvas<T>) -> Self {
        canvas.set_render_target(RenderTarget::Screen);
        Self { canvas, target:   RenderTarget::Screen }
    }
    fn set_target(&mut self, target: RenderTarget) {
        self.canvas.set_render_target(target);
        self.target = target;
    }
}
impl<T: SurfacelessRenderer> core::ops::Deref for FemtovgContext<'_, T> {
    fn deref(&self) -> &Self::Target { self.canvas }
    type Target = femtovg::Canvas<T>;
}
impl<T: SurfacelessRenderer> core::ops::DerefMut for FemtovgContext<'_, T> {
    fn deref_mut(&mut self) -> &mut Self::Target { self.canvas }
}

impl PathBuilder for femtovg::Path {
    fn new(_capacity: u32) -> Self { Self::new() }    // XXX: can't make reservation
    fn close(&mut self) { self.close() }

    fn move_to (&mut self, end: Vec2D) { self.move_to(end.x, end.y) }
    fn line_to (&mut self, end: Vec2D) { self.line_to(end.x, end.y) }
    fn cubic_to(&mut self, ocp: Vec2D, icp: Vec2D, end: Vec2D) {
        self.bezier_to(ocp.x, ocp.y, icp.x, icp.y, end.x, end.y)
    }
    fn quad_to (&mut self, cpt: Vec2D, end: Vec2D) {
        self.quad_to  (cpt.x, cpt.y, end.x, end.y)
    }
    fn add_arc(&mut self, center: Vec2D, radii: Vec2D, start: f32, sweep: f32) {
        self.arc(center.x, center.y, (radii.x + radii.y) / 2.,
            start as _, sweep as _, femtovg::Solidity::Solid)   // XXX:
        //self.arc_to(x1, y1, x2, y2, (radii.x + radii.y) / 2.);
    }

    fn current_pos(&self) -> Option<Vec2D> {  use femtovg::Verb::*;
        match self.verbs().last()? {
            MoveTo(x, y) => Some((x, y).into()),
            LineTo(x, y) => Some((x, y).into()),
            BezierTo(_, _, _, _, x, y) => Some((x, y).into()),  _ => None,
        }
    }

    fn to_kurbo(&self) -> BezPath {     use femtovg::Verb::*;
        let mut pb = BezPath::with_capacity(self.verbs().count());
        self.verbs().for_each(|verb| match verb {
            MoveTo(x, y) => pb.move_to((x, y)),
            LineTo(x, y) => pb.line_to((x, y)),
            BezierTo(ox, oy, ix, iy, x, y) => pb.curve_to((ox, oy), (ix, iy), (x, y)),
            Solid | Hole => unreachable!(),
            Close => pb.close(),
        }); pb
    }
}

impl MatrixConv for femtovg::Transform2D {
    /*  |a c e|              Transform2D::multiply (A' = B * A)
        |b d f|
        |0 0 1| */
    fn identity() -> Self { Self::identity() }
    fn skew_x(&mut self, sk: f32) { self.skew_x(sk) }
    fn rotate(&mut self, angle: f32) { self.rotate(angle) }
    fn translate(&mut self, pos: Vec2D) { self.translate(pos.x, pos.y) }
    fn scale(&mut self, sl: Vec2D) { self.scale(sl.x, sl.y) }
    fn premul(&mut self, tm: &Self) { self.premultiply(tm) }
}

impl StyleConv for femtovg::Paint {
    fn solid_color(color: RGBA) -> Self { Self::color(color.into()) }
    fn linear_gradient(sp: Vec2D, ep: Vec2D, stops: &[(f32, RGBA)]) -> Self {
        Self::linear_gradient_stops(sp.x, sp.y, ep.x, ep.y,
            stops.iter().map(|&(offset, color)| (offset, color.into())))
    }

    fn radial_gradient(cp: Vec2D, _fp: Vec2D, radii: (f32, f32),
            stops: &[(f32, RGBA)]) -> Self {
        Self::radial_gradient_stops(cp.x, cp.y, radii.0, radii.1,
            stops.iter().map(|&(offset, color)| (offset, color.into())))
    }
    fn configure(&mut self, options: &FSOpts) {
        use femtovg::{FillRule as FFR, LineCap as FLC, LineJoin as FLJ};
        match options {
            FSOpts::Fill(rule) => self.set_fill_rule(match rule {
                FillRule::NonZero => FFR::NonZero,
                FillRule::EvenOdd => FFR::EvenOdd,
            }),
            FSOpts::Stroke { width, limit, join, cap, dash } => {
                self.set_line_width(*width);
                self.set_miter_limit(*limit);
                self.set_line_join(match join {
                    LineJoin::Miter => FLJ::Miter,
                    LineJoin::Round => FLJ::Round,
                    LineJoin::Bevel => FLJ::Bevel,
                });
                self.set_line_cap(match cap {
                    LineCap::Butt   => FLC::Butt,
                    LineCap::Round  => FLC::Round,
                    LineCap::Square => FLC::Square,
                });
                self.set_line_dash_offset(dash.0);
                self.set_line_dash(&dash.1);
            }
        }
    }
}
impl From<RGBA> for VGColor {
    fn from(color: RGBA) -> Self { Self::rgba(color.r, color.g, color.b, color.a) }
}

impl<T: SurfacelessRenderer> RenderContext for FemtovgContext<'_, T> {
    type TM2D = femtovg::Transform2D;
    type Error = femtovg::ErrorKind;
    type VGStyle = femtovg::Paint;
    type VGPath  = femtovg::Path;
    type State = ();

    fn get_size(&self) -> (u32, u32) { (self.width(), self.height()) }
    fn clear_rect_with(&mut self, x: u32, y: u32, w: u32, h: u32,
        color: RGBA) -> Result<(), Self::Error> {
        self.clear_rect(x, y, w, h, color.into()); Ok(())
    }
    fn save_state(&mut self) -> Result<Self::State, Self::Error> { self.save(); Ok(()) }
    fn restore_state(&mut self, (): Self::State) -> Result<(), Self::Error> {
        self.restore();     Ok(())
    }
    fn apply_transform(&mut self, trfm: &Self::TM2D,
        opacity: Option<f32>) -> Result<(), Self::Error> {
        if let Some(opacity) = opacity { self.set_global_alpha(opacity) }
        self.set_transform(trfm); Ok(())
    }

    fn fill_stroke(&mut self, path: &Self::VGPath, relative: Option<&Self::TM2D>,
        style: &(Self::VGStyle, FSOpts)) -> Result<(), Self::Error> {
        let transformed = relative.map(|transform| {
            let mut result = Self::VGPath::new();
            path.verbs().for_each(|verb| { use femtovg::Verb::*;
                let point = |x, y| transform.transform_point(x, y);
                match verb {
                    MoveTo(x, y) => { let p = point(x, y); result.move_to(p.0, p.1); }
                    LineTo(x, y) => { let p = point(x, y); result.line_to(p.0, p.1); }
                    BezierTo(x1, y1, x2, y2, x, y) => {
                        let (p1, p2, p) = (point(x1, y1), point(x2, y2), point(x, y));
                        result.bezier_to(p1.0, p1.1, p2.0, p2.1, p.0, p.1);
                    }
                    Solid => result.solidity(femtovg::Solidity::Solid),
                    Hole  => result.solidity(femtovg::Solidity::Hole),
                    Close => result.close(),
                }
            }); result
        });
        let path = transformed.as_ref().unwrap_or(path);
        match &style.1 {
            FSOpts::Fill(_) => self.fill_path(path, &style.0),
            FSOpts::Stroke { .. } => self.stroke_path(path, &style.0),
        }   Ok(())
    }
}

impl<T: SurfacelessRenderer> CompositeContext for FemtovgContext<'_, T> {
    type Offscreen = Offscreen;
    type Image = ImageId;

    fn begin_offscreen(&mut self) -> Result<Self::Offscreen, Self::Error> {
        let (w, h) = (self.width(), self.height());
        let image = self.create_image_empty(w as _, h as _,
            PixelFormat::Rgba8, ImageFlags::FLIP_Y)?;
        let parent = self.target;
        self.set_target(RenderTarget::Image(image));
        self.clear_rect(0, 0, w, h, CLEAR_COLOR);
        Ok(Offscreen { image, parent })
    }

    fn abort_offscreen(&mut self, target: Self::Offscreen) {
        self.set_target(target.parent);     self.flush();
        self.delete_image(target.image);
    }

    fn end_offscreen(&mut self, target: Self::Offscreen) -> Result<Self::Image, Self::Error> {
        self.set_target(target.parent); Ok(target.image)
    }

    fn apply_masks(&mut self, content: Self::Image, vl: &VisualLayer,
        ltm: &TM2DwO<Self::TM2D>, fnth: f32) -> Result<Self::Image, Self::Error> {
        let (w, h, parent) = (self.width(), self.height(), self.target);
        let accum = match self.create_image_empty(w as _, h as _,
            PixelFormat::Rgba8, ImageFlags::FLIP_Y) {
            Err(error) => { self.delete_image(content); return Err(error) }
             Ok(image) => image,
        };
        let mut images = Vec::with_capacity(1 + vl.masks.len() * 2);
        self.save();    images.push(accum);
        self.reset_transform(); self.set_global_alpha(1.);
        self.set_target(RenderTarget::Image(accum));
        self.clear_rect(0, 0, w, h, CLEAR_COLOR);

        let (mut initialized, bounds) = (false, full_path(w, h));
        let result = (|| {
            for mask in &vl.masks {
                if matches!(mask.mode, MaskMode::None) { continue }
                if matches!(mask.mode, MaskMode::Lighten | MaskMode::Darken) {
                    return Err(Self::Error::UnsupportedOperation)
                }
                let part = self.create_image_empty(w as _, h as _,
                    PixelFormat::Rgba8, ImageFlags::FLIP_Y)?;
                images.push(part); self.set_target(RenderTarget::Image(part));
                self.clear_rect(0, 0, w, h, CLEAR_COLOR);
                self.global_composite_operation(CompOp::SourceOver);

                let mut path: Self::VGPath = mask.shape.to_path(fnth);
                if let Some(expand) = &mask.expand {
                    path.offset_path(expand.get_value(fnth), LineJoin::Round, 4.);
                }
                let opacity = mask.opacity.as_ref().map_or(1.,
                    |opacity| opacity.get_value(fnth) / 100.);
                self.save(); self.set_transform(&ltm.0); self.set_global_alpha(1.);
                self.fill_path(&path, &Self::VGStyle::color(MASK_COLOR)); self.restore();

                let source = if mask.inv {
                    let inverse = self.create_image_empty(w as _, h as _,
                        PixelFormat::Rgba8, ImageFlags::FLIP_Y)?;
                    images.push(inverse); self.set_target(RenderTarget::Image(inverse));
                    self.clear_rect(0, 0, w, h, MASK_COLOR);
                    self.global_composite_operation(CompOp::DestinationOut);
                    self.fill_path(&bounds,
                        &Self::VGStyle::image(part, 0., 0., w as _, h as _, 0., 1.));
                    inverse
                } else { part };

                self.set_target(RenderTarget::Image(accum));
                if !initialized && matches!(mask.mode,
                    MaskMode::Subtract | MaskMode::Intersect) {
                    self.clear_rect(0, 0, w, h, MASK_COLOR);
                }
                self.set_global_alpha(opacity);
                self.global_composite_operation(match mask.mode {
                    MaskMode::Add        => CompOp::SourceOver,
                    MaskMode::Subtract   => CompOp::DestinationOut,
                    MaskMode::Intersect  => CompOp::DestinationIn,
                    MaskMode::Difference => CompOp::Xor,
                    MaskMode::None | MaskMode::Lighten | MaskMode::Darken => unreachable!(),
                });
                self.fill_path(&bounds,
                    &Self::VGStyle::image(source, 0., 0., w as _, h as _, 0., 1.));
                initialized = true;
            }
            if initialized {
                self.set_global_alpha(1.);
                self.set_target(RenderTarget::Image(content));
                self.global_composite_operation(CompOp::DestinationIn);
                self.fill_path(&bounds,
                    &Self::VGStyle::image(accum, 0., 0., w as _, h as _, 0., 1.));
                self.flush();
            }   Ok(content)
        })();

        self.set_target(parent);    self.flush();
        for image in images { self.delete_image(image); }
        self.restore();
        if  result.is_err() { self.delete_image(content); }
            result
    }

    fn apply_matte(&mut self, content: Self::Image, matte: Self::Image,
        mode: MatteMode) -> Result<Self::Image, Self::Error> {
        if matches!(mode, MatteMode::Luma | MatteMode::InvertedLuma) {
            self.flush();
            self.delete_image(content); self.delete_image(matte);
            return Err(Self::Error::UnsupportedOperation)
        }
        let (w, h, parent) = (self.width(), self.height(), self.target);
        if !matches!(mode, MatteMode::Normal) {
            self.save(); self.reset_transform(); self.set_global_alpha(1.);
            self.set_target(RenderTarget::Image(content));
            self.global_composite_operation(match mode {
                MatteMode::Alpha => CompOp::DestinationIn,
                MatteMode::InvertedAlpha => CompOp::DestinationOut,
                _ => unreachable!(),
            });
            self.fill_path(&full_path(w, h), &Self::VGStyle::image(
                matte, 0., 0., w as _, h as _, 0., 1.));
            self.flush(); self.set_target(parent); self.restore();
        }   self.delete_image(matte);   Ok(content)
    }

    fn present(&mut self, image: Self::Image) -> Result<(), Self::Error> {
        let (w, h) = (self.width(), self.height());
        self.save(); self.reset_transform(); self.set_global_alpha(1.);
        self.global_composite_operation(CompOp::SourceOver);
        self.fill_path(&full_path(w, h), &Self::VGStyle::image(
            image, 0., 0., w as _, h as _, 0., 1.));
        self.flush(); self.restore();
        self.delete_image(image);   Ok(())
    }
    fn discard(&mut self, image: Self::Image) { self.flush(); self.delete_image(image); }
}

fn full_path(w: u32, h: u32) -> femtovg::Path {
    let mut path = femtovg::Path::new();
    path.rect(0., 0., w as _, h as _); path
}
