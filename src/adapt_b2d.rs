/****************************************************************
 * $ID: adapt_b2d.rs  	Thu 20 Nov 2025 16:50:16+0800           *
 *                                                              *
 * Maintainer: 范美辉 (MeiHui FAN) <mhfan@ustc.edu>              *
 * Copyright (c) 2025 M.H.Fan, All rights reserved.             *
 ****************************************************************/

use crate::{helpers::{Vec2D, RGBA}, pathm::{PathBuilder, BezPath},
    schema::{FillRule, LineJoin, LineCap}, render::RenderContext,
    style::{StyleConv, MatrixConv, FSOpts},
};
use intvg::blend2d::{BLPoint, BLPath, BLMatrix2D, BLContext, BLRgba32, BLImage,
    BLSolidColor, BLGradient, BLLinearGradientValues, BLRadialGradientValues,
    BLFillRule::*, BLStrokeJoin::*, BLStrokeCap::*,
};

impl RenderContext for BLContext {
    type ImageID = BLImage;
    type TM2D = BLMatrix2D;
    type VGStyle = BLStyle;
    type VGPath  = BLPath;

    fn get_size(&self) -> (u32, u32) {
        let sz = self.get_target_size();
        (sz.width() as _, sz.height() as _)
    }

    fn clear_rect_with(&mut self, x: u32, y: u32, w: u32, h: u32, color: RGBA) {
        self.fill_rect_i_rgba32(&(x, y, w, h).into(), color.into());
        //self.clear_rect_d(&(x, y, w, h).into()); //self.clear_all();
    }
    fn reset_transform(&mut self, trfm: Option<&Self::TM2D>) {
        self.reset_transform(trfm);     //self.set_global_alpha(1.);
    }   // XXX: BLContext.set_fill/stroke_alpha()
    fn apply_transform(&mut self, trfm: &Self::TM2D, opacity: Option<f32>) -> Self::TM2D {
        let last_trfm = self.get_transform(1);
        if let Some(opacity) = opacity { self.set_global_alpha(opacity as _) }
        self.apply_transform(trfm);     last_trfm
    }

    fn fill_stroke(&mut self, path: &Self::VGPath,
        style: &core::cell::RefCell<(Self::VGStyle, FSOpts)>) {

        match &style.borrow().1 {
            FSOpts::Fill(rule) => {
                self.set_fill_rule(match rule {
                    FillRule::NonZero => BL_FILL_RULE_NON_ZERO,
                    FillRule::EvenOdd => BL_FILL_RULE_EVEN_ODD,
                });

                match &style.borrow().0 {
                    BLStyle::Solid(color) => self.set_fill_style(color),
                    BLStyle::Gradient(grad) => self.set_fill_style(grad),
                }   self.fill_geometry(path);
            }

            FSOpts::Stroke { width, limit,
                join, cap, dash } => {
                self.set_stroke_width(*width as _);
                self.set_stroke_miter_limit(*limit as _);

                self.set_stroke_join(match join {
                    LineJoin::Miter => BL_STROKE_JOIN_MITER_CLIP,
                    LineJoin::Round => BL_STROKE_JOIN_ROUND,
                    LineJoin::Bevel => BL_STROKE_JOIN_BEVEL,
                });
                self.set_stroke_caps(match cap {
                    LineCap::Butt   => BL_STROKE_CAP_BUTT,
                    LineCap::Round  => BL_STROKE_CAP_ROUND,
                    LineCap::Square => BL_STROKE_CAP_SQUARE,
                });

                if 2 < dash.len() {
                    self.set_stroke_dash(dash[0] as _,
                        &dash.iter().skip(1).map(|&x| x as _).collect::<Vec<_>>());
                    //self.stroke_geometry(&path.make_dash(dash[0], &dash[1..]));
                }

                match &style.borrow().0 {
                    BLStyle::Solid(color) => self.set_stroke_style(color),
                    BLStyle::Gradient(grad) => self.set_stroke_style(grad),
                }   self.stroke_geometry(path);
            }
        }
    }
}

impl PathBuilder for BLPath {
    #[inline] fn new(capacity: u32) -> Self {
        let mut path = Self::new();
        if capacity != 0 { path.reserve((2 * capacity) as _); }     path
    }   // different commands vary in size for BLPath
    #[inline] fn close(&mut self) { self.close() }
    #[inline] fn current_pos(&self) -> Option<Vec2D> {
        self.get_last_vertex().map(|pt| Vec2D { x: pt.x() as _, y: pt.y() as _ })
    }

    #[inline] fn move_to(&mut self, end: Vec2D) { self.move_to(end.into()) }
    #[inline] fn line_to(&mut self, end: Vec2D) { self.line_to(end.into()) }
    #[inline] fn cubic_to(&mut self, ocp: Vec2D, icp: Vec2D, end: Vec2D) {
        self.cubic_to(ocp.into(), icp.into(), end.into())
    }
    #[inline] fn quad_to(&mut self, cp: Vec2D, end: Vec2D) {
        self.quad_to(cp.into(), end.into())
    }
    #[inline] fn add_arc(&mut self, center: Vec2D, radii: Vec2D, start: f32, sweep: f32) {
        self.arc_to(center.into(), (radii.x as _, radii.y as _), start as _, sweep as _)
    }
    #[inline] fn elliptic_arc_to(&mut self, radii: Vec2D,
        x_rot: f32, large: bool, sweep: bool, end: Vec2D) {
        self.elliptic_arc_to((radii.x as _, radii.y as _),
            x_rot as _, large, sweep, end.into())
    }

    fn to_kurbo(&self) -> BezPath {   use intvg::blend2d::BLPathItem::*;
        let mut pb = BezPath::with_capacity(self.get_size() as _);
        self.iter().for_each(|item| match item {
            MoveTo(end) => pb.move_to((end.x(), end.y())),
            LineTo(end) => pb.line_to((end.x(), end.y())),
            QuadTo(cp, end) =>
                pb.quad_to((cp.x(), cp.y()), (end.x(), end.y())),
            CubicTo(ocp, icp, end) =>
                pb.curve_to((ocp.x(), ocp.y()), (icp.x(), icp.y()), (end.x(), end.y())),
            Close => pb.close(),
        }); pb
    }
}
impl From<Vec2D> for BLPoint { #[inline] fn from(pt: Vec2D) -> Self { (pt.x, pt.y).into() } }

impl MatrixConv for BLMatrix2D {
    /*  | a b 0 |   BLMatrix2D::transform (A' = B * A)
        | c d 0 |
        | e f 1 | */
    #[inline] fn identity() -> Self { Self::identity() }
    #[inline] fn rotate(&mut self, angle: f32) { self.post_rotate(angle as _, None) }
    #[inline] fn translate(&mut self, pos: Vec2D) { self.post_translate(pos.into()) }
    #[inline] fn skew_x(&mut self, sk: f32) { self.post_skew((sk as _, 0.)) }
    #[inline] fn scale(&mut self, sl: Vec2D) { self.post_scale((sl.x as _, sl.y as _)) }
    #[inline] fn premul(&mut self, tm: &Self) { self.transform(tm) }
}

impl StyleConv for BLStyle {
    #[inline] fn solid_color(color: RGBA) -> Self {
        Self::Solid(BLSolidColor::init_rgba32(color.into()))
    }

    #[inline] fn linear_gradient(sp: Vec2D, ep: Vec2D, stops: &[(f32, RGBA)]) -> Self {
        let stops = stops.iter().map(|&(offset, color)|
                (offset, color.into()).into()).collect::<Vec<_>>();
        Self::Gradient(BLGradient::new(&BLLinearGradientValues::
            new(sp.into(), ep.into())).with_stops(&stops))
    }

    #[inline] fn radial_gradient(cp: Vec2D, fp: Vec2D, radii: (f32, f32),
            stops: &[(f32, RGBA)]) -> Self {
        let stops = stops.iter().map(|&(offset, color)|
                (offset, color.into()).into()).collect::<Vec<_>>();
        Self::Gradient(BLGradient::new(&BLRadialGradientValues::
            new(cp.into(), fp.into(), (radii.0 as _, radii.1 as _))).with_stops(&stops))
    }
}

pub enum BLStyle { Solid(BLSolidColor), Gradient(BLGradient), }
impl From<RGBA> for BLRgba32 {
    #[inline] fn from(color: RGBA) -> Self { (color.r, color.g, color.b, color.a).into() }
}

