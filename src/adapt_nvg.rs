/****************************************************************
 * $ID: adapt_nvg.rs  	Thu 27 Nov 2025 13:01:22+0800           *
 *                                                              *
 * Maintainer: 范美辉 (MeiHui FAN) <mhfan@ustc.edu>              *
 * Copyright (c) 2025 M.H.Fan, All rights reserved.             *
 ****************************************************************/

use crate::{helpers::{Vec2D, RGBA}, pathm::{PathBuilder, BezPath, PathFactory},
    schema::{VisualLayer, MatteMode, MaskMode, FillRule, LineJoin, LineCap},
    style::{StyleConv, MatrixConv, TM2DwO, FSOpts},
    render::{RenderContext, TrackMatte}
};
use femtovg::{PixelFormat, ImageFlags, RenderTarget,
    CompositeOperation as CompOp, Color as VGColor};
const CLEAR_COLOR: VGColor = VGColor::rgbaf(0., 0., 0., 0.);

impl PathBuilder for femtovg::Path {
    #[inline] fn new(_capacity: u32) -> Self { Self::new() }    // XXX: can't make reservation
    #[inline] fn close(&mut self) { self.close() }

    #[inline] fn move_to(&mut self, end: Vec2D) { self.move_to(end.x, end.y) }
    #[inline] fn line_to(&mut self, end: Vec2D) { self.line_to(end.x, end.y) }
    #[inline] fn cubic_to(&mut self, ocp: Vec2D, icp: Vec2D, end: Vec2D) {
        self.bezier_to(ocp.x, ocp.y, icp.x, icp.y, end.x, end.y)
    }
    #[inline] fn quad_to(&mut self, cp: Vec2D, end: Vec2D) {
        self.quad_to(cp.x, cp.y, end.x, end.y)
    }
    #[inline] fn add_arc(&mut self, center: Vec2D, radii: Vec2D, start: f32, sweep: f32) {
        self.arc(center.x, center.y, (radii.x + radii.y) / 2.,
            start as _, sweep as _, femtovg::Solidity::Solid)   // XXX:
        //self.arc_to(x1, y1, x2, y2, (radii.x + radii.y) / 2.);
    }

    #[inline] fn current_pos(&self) -> Option<Vec2D> {  use femtovg::Verb::*;
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
            BezierTo(ox, oy, ix, iy, x, y) =>
                pb.curve_to((ox, oy), (ix, iy), (x, y)),
            Solid | Hole => unreachable!(),
            Close => pb.close(),
        }); pb
    }
}

impl MatrixConv for femtovg::Transform2D {
    /*  |a c e|              Transform2D::multiply (A' = B * A)
        |b d f|
        |0 0 1| */
    #[inline] fn identity() -> Self { Self::identity() }
    #[inline] fn skew_x(&mut self, sk: f32) { self.skew_x(sk) }
    #[inline] fn rotate(&mut self, angle: f32) { self.rotate(angle) }
    #[inline] fn translate(&mut self, pos: Vec2D) { self.translate(pos.x, pos.y) }
    #[inline] fn scale(&mut self, sl: Vec2D) { self.scale(sl.x, sl.y) }
    #[inline] fn premul(&mut self, tm: &Self) { self.premultiply(tm) }
}

impl StyleConv for femtovg::Paint {
    #[inline] fn solid_color(color: RGBA) -> Self { Self::color(color.into()) }
    #[inline] fn linear_gradient(sp: Vec2D, ep: Vec2D, stops: &[(f32, RGBA)]) -> Self {
        Self::linear_gradient_stops(sp.x, sp.y, ep.x, ep.y,
            stops.iter().map(|&(offset, color)| (offset, color.into())))
    }

    #[inline] fn radial_gradient(cp: Vec2D, _fp: Vec2D, radii: (f32, f32),
            stops: &[(f32, RGBA)]) -> Self {
        Self::radial_gradient_stops(cp.x, cp.y, radii.0, radii.1,
            stops.iter().map(|&(offset, color)| (offset, color.into())))
    }
}
impl From<RGBA> for VGColor {
    #[inline] fn from(color: RGBA) -> Self { Self::rgba(color.r, color.g, color.b, color.a) }
}

impl<T: femtovg::renderer::SurfacelessRenderer> RenderContext for femtovg::Canvas<T> {
    type TM2D = femtovg::Transform2D;
    type ImageID = femtovg::ImageId;
    type VGStyle = femtovg::Paint;
    type VGPath  = femtovg::Path;

    fn get_size(&self) -> (u32, u32) { (self.width(), self.height()) }
    fn clear_rect_with(&mut self, x: u32, y: u32, w: u32, h: u32, color: RGBA) {
        self.clear_rect(x, y, w, h, color.into());
    }
    fn reset_transform(&mut self, trfm: Option<&Self::TM2D>) {
        self.reset_transform();     //self.set_global_alpha(1.);
        if let Some(trfm) = trfm { self.set_transform(trfm) }
    }
    fn apply_transform(&mut self, trfm: &Self::TM2D, opacity: Option<f32>) -> Self::TM2D {
        let last_trfm = self.transform();
        if let Some(opacity) = opacity { self.set_global_alpha(opacity) }
        self.set_transform(trfm);   last_trfm
    }

    fn fill_stroke(&mut self, path: &Self::VGPath,
        style: &core::cell::RefCell<(Self::VGStyle, FSOpts)>) {
        use femtovg::{FillRule as FFR, LineCap as FLC, LineJoin as FLJ};

        match &style.borrow().1 {
            FSOpts::Fill(rule) => {
                let paint = &mut style.borrow_mut().0;
                paint.set_fill_rule(match rule {
                    FillRule::NonZero => FFR::NonZero,
                    FillRule::EvenOdd => FFR::EvenOdd,
                }); self.fill_path(path, paint);
            }

            FSOpts::Stroke { width, limit,
                join, cap, dash } => {
                let paint = &mut style.borrow_mut().0;
                paint.set_line_width (*width);
                paint.set_miter_limit(*limit);

                paint.set_line_join(match join {
                    LineJoin::Miter => FLJ::Miter, LineJoin::Round => FLJ::Round,
                    LineJoin::Bevel => FLJ::Bevel,
                });
                paint.set_line_cap(match cap {
                    LineCap::Butt   => FLC::Butt,   LineCap::Round => FLC::Round,
                    LineCap::Square => FLC::Square,
                });

                if dash.len() < 3 { self.stroke_path(path, paint); } else {
                    self.stroke_path(&path.make_dash(dash[0], &dash[1..]), paint);
                }
            }
        }
    }

    fn prepare_matte(&mut self,
        vl: &VisualLayer, matte: &mut Option<TrackMatte<Self::ImageID>>) {
        if vl.tt.is_none() && matte.is_none() { return }

        // XXX: limit image to viewport/viewbox
        let (w, h) = (self.width(), self.height());
        let (lx, ty) = self.transform().transform_point(0., 0.);
        let (lx, ty) = (lx as u32, ty as u32);

        if vl.tt.is_some() || vl.has_mask {
            let imgid = self.create_image_empty(w as _, h as _,
                PixelFormat::Rgba8, ImageFlags::FLIP_Y).unwrap();
            self.set_render_target(RenderTarget::Image(imgid));
            self.clear_rect(lx, ty, w - lx * 2, h - ty * 2, CLEAR_COLOR);

            *matte = Some(TrackMatte { mode: vl.tt.unwrap_or(MatteMode::Normal),
                mlid: vl.tp, imgid, mskid: None }); 	return
        } else if vl.td.is_some_and(|td| !td.as_bool()) { return }

        let matte = matte.as_mut().unwrap();
        if vl.base.ind.is_some_and(|ind|
            matte.mlid.is_some_and(|mlid| ind != mlid)) { return }

        let mskid = self.create_image_empty(w as _, h as _,
            PixelFormat::Rgba8, ImageFlags::FLIP_Y).unwrap();
        self.set_render_target(RenderTarget::Image(mskid));
        self.clear_rect(lx, ty, w - lx * 2, h - ty * 2, CLEAR_COLOR);
        matte.mskid = Some(mskid);
    }

    fn compose_matte(&mut self, vl: &VisualLayer,
        matte: &mut Option<TrackMatte<Self::ImageID>>, ltm: &TM2DwO<Self::TM2D>, fnth: f32) {
        if (vl.tt.is_some() || matte.is_none() ||
            vl.td.is_some_and(|td| !td.as_bool())) && !vl.has_mask { return }

        let track = matte.as_mut().unwrap();
        if  vl.base.ind.is_some_and(|ind|
            track.mlid.is_some_and(|mlid| ind != mlid)) { return }
        let (imgid, mut path) = (track.imgid, Self::VGPath::new());

        // XXX: limit image to viewport/viewbox
        //let (w, h) = self.image_size(imgid).unwrap();
        let (w, h) = (self.width(), self.height());
        let (lx, ty) = self.transform().transform_point(0., 0.);
        path.rect(lx, ty, w as f32 - lx * 2., h as f32 - ty * 2.);

        if  vl.has_mask {
            let mskid = self.create_image_empty(w as _, h as _,
                PixelFormat::Rgba8, ImageFlags::FLIP_Y).unwrap();
            self.set_render_target(RenderTarget::Image(mskid));
            let paint = Self::VGStyle::image(mskid, 0., 0., w as _, h as _, 0., 1.);
            let mut mpaint = Self::VGStyle::color(CLEAR_COLOR);

            vl.masks.iter().for_each(|mask| {
                let mut path: Self::VGPath = mask.shape.to_path(fnth);
                if mask.inv { path.solidity(femtovg::Solidity::Hole); }
                if let Some(_expand) = &mask.expand { todo!() }

                let  opacity = mask.opacity.as_ref().map_or(1.,
                    |opacity| opacity.get_value(fnth) / 100.);
                mpaint.set_color(VGColor::rgbaf(0., 0., 0., opacity));

                self.clear_rect(lx as _, ty as _,
                    w - lx as u32 * 2, h - ty as u32 * 2, CLEAR_COLOR);

                let last_trfm = self.apply_transform(&ltm.0, Some(ltm.1));
                self.fill_path(&path, &mpaint);
                self.reset_transform();    self.set_transform(&last_trfm);  // XXX:

                let cop = match mask.mode {
                    MaskMode::Add       => Some(CompOp::DestinationIn),
                    MaskMode::Subtract  => Some(CompOp::DestinationOut),
                    MaskMode::Intersect => Some(CompOp::DestinationAtop),
                    MaskMode::Lighten   => Some(CompOp::Lighter),
                    MaskMode::Darken | MaskMode::Difference => unimplemented!(),
                    MaskMode::None => None,
                };

                if let Some(cop) = cop {
                    self.global_composite_operation (cop);
                    self.set_render_target(RenderTarget::Image(imgid));
                    self.fill_path(&path, &paint);  self.flush();
                } 	self.set_render_target(RenderTarget::Image(mskid));
            }); 	self.delete_image(mskid);
        }

        if let Some(mskid) = track.mskid {
            // https://developer.mozilla.org/en-US/docs/Web/API/CanvasRenderingContext2D/
            let cop = match track.mode {
                MatteMode::Alpha =>         Some(CompOp::DestinationIn),
                MatteMode::InvertedAlpha => Some(CompOp::DestinationOut),
                MatteMode::Luma | MatteMode::InvertedLuma => unimplemented!(),
                MatteMode::Normal => None,
            };

            if let Some(cop) = cop {
                self.global_composite_operation (cop);
                self.set_render_target(RenderTarget::Image(imgid));

                self.fill_path(&path, &Self::VGStyle::image(mskid,
                    0., 0., w as _, h as _, 0., 1.));   self.flush();
            } 	self.delete_image(mskid);
        }

        self.set_render_target(RenderTarget::Screen);
        self.global_composite_operation(CompOp::SourceOver);
        self.fill_path(&path, &Self::VGStyle::image(imgid, 0., 0., w as _, h as _, 0., 1.));
        self.flush();   self.delete_image(imgid); 	*matte = None;
    }
}

