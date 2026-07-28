/****************************************************************
 * $ID: blend2d.rs  	Thu 20 Nov 2025 16:50:16+0800           *
 *                                                              *
 * Maintainer: 范美辉 (MeiHui FAN) <mhfan@ustc.edu>              *
 * Copyright (c) 2025 M.H.Fan, All rights reserved.             *
 ****************************************************************/

use crate::core::{helpers::{Vec2D, RGBA}, pathm::{PathBuilder, BezPath},
    schema::{FillRule, LineJoin, LineCap}, render::RenderContext,
    style::{StyleConv, MatrixConv, FSOpts},
};
use intvg::blend2d::{BLPoint, BLPath, BLMatrix2D, BLContext, BLRgba32, BLImage,
    BLSolidColor, BLGradient, BLLinearGradientValues, BLRadialGradientValues,
    B2DStyle, BLFillRule::*, BLStrokeJoin::*, BLStrokeCap::*,
};

impl RenderContext for BLContext {
    type ImageID = BLImage;
    type TM2D = BLMatrix2D;
    type VGStyle = BLStyle;
    type VGPath  = BLPath;
    type State = (Self::TM2D, f64);

    fn get_size(&self) -> (u32, u32) {
        let sz = self.get_target_size();
        (sz.width() as _, sz.height() as _)
    }

    fn clear_rect_with(&mut self, x: u32, y: u32, w: u32, h: u32, color: RGBA) {
        if color.a < u8::MAX {
            self.clear_rect_d(&(x as f64, y as f64, w as f64, h as f64).into())
                .expect("failed to clear Blend2D rectangle");
        }
        if color.a != 0 {
            self.fill_rect_i_rgba32(&(x, y, w, h).into(), color.into())
                .expect("failed to fill Blend2D background");
        }
    }
    fn save_state(&mut self) -> Self::State {
        //self.save().expect("failed to save Blend2D state");
        (self.user_transform(), self.get_global_alpha())
    }
    fn restore_state(&mut self, (transform, alpha): Self::State) {
        //self.restore().expect("failed to restore Blend2D state");
        self.reset_transform(Some(&transform)); self.set_global_alpha(alpha);
    }
    fn apply_transform(&mut self, trfm: &Self::TM2D, opacity: Option<f32>) {
        if let Some(opacity) = opacity { self.set_global_alpha(opacity as _) }
        self.reset_transform(Some(trfm));
    }

    fn fill_stroke(&mut self, path: &Self::VGPath,
        relative: Option<&Self::TM2D>,
        style: &core::cell::RefCell<(Self::VGStyle, FSOpts)>) {
        let transformed = relative.map(|transform| {
            let mut result = BLPath::new();
            result.add_transformed_path(path, transform)
                .expect("failed to transform Blend2D path");
            result
        });
        let path = transformed.as_ref().unwrap_or(path);
        let style = style.borrow();
        match &style.1 {
            FSOpts::Fill(rule) => {
                self.set_fill_rule(match rule {
                    FillRule::NonZero => BL_FILL_RULE_NON_ZERO,
                    FillRule::EvenOdd => BL_FILL_RULE_EVEN_ODD,
                });

                self.set_fill_style(style.0.as_b2d_style());
                self.fill_geometry(path).expect("failed to fill Blend2D path");
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

                if dash.1.is_empty() {
                    self.set_stroke_dash(0., &[])
                        .expect("failed to clear Blend2D stroke dash");
                } else {
                    self.set_stroke_dash(dash.0 as _,
                        &dash.1.iter().map(|&x| x as _).collect::<Vec<_>>())
                        .expect("failed to set Blend2D stroke dash");
                }

                self.set_stroke_style(style.0.as_b2d_style());
                self.stroke_geometry(path).expect("failed to stroke Blend2D path");
            }
        }
    }
}

impl PathBuilder for BLPath {
    fn new(capacity: u32) -> Self {
        let mut path = Self::new();
        if capacity != 0 {
            path.reserve((2 * capacity) as _).expect("failed to reserve Blend2D path");
        }   path
    }   // different commands vary in size for BLPath
    fn close(&mut self) { self.close() }
    fn current_pos(&self) -> Option<Vec2D> {
        self.get_last_vertex().ok()
            .map(|pt| Vec2D { x: pt.x() as _, y: pt.y() as _ })
    }

    fn move_to(&mut self, end: Vec2D) { self.move_to(end.into()) }
    fn line_to(&mut self, end: Vec2D) { self.line_to(end.into()) }
    fn cubic_to(&mut self, ocp: Vec2D, icp: Vec2D, end: Vec2D) {
        self.cubic_to(ocp.into(), icp.into(), end.into())
    }
    fn quad_to(&mut self, cp: Vec2D, end: Vec2D) {
        self.quad_to(cp.into(), end.into())
    }
    fn add_arc(&mut self, center: Vec2D, radii: Vec2D, start: f32, sweep: f32) {
        self.arc_to(center.into(), (radii.x as _, radii.y as _), start as _, sweep as _)
            .expect("failed to append Blend2D arc")
    }
    fn elliptic_arc_to(&mut self, radii: Vec2D,
        x_rot: f32, large: bool, sweep: bool, end: Vec2D) {
        self.elliptic_arc_to((radii.x as _, radii.y as _),
                x_rot as _, large, sweep, end.into())
            .expect("failed to append Blend2D elliptic arc")
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
impl From<Vec2D> for BLPoint { fn from(pt: Vec2D) -> Self { (pt.x, pt.y).into() } }

impl MatrixConv for BLMatrix2D {
    /*  | a b 0 |   BLMatrix2D::transform (A' = B * A)
        | c d 0 |
        | e f 1 | */
    fn identity() -> Self { Self::identity() }
    fn rotate(&mut self, angle: f32) { self.post_rotate(angle as _, None) }
    fn translate(&mut self, pos: Vec2D) { self.post_translate(pos.into()) }
    fn skew_x(&mut self, sk: f32) { self.post_skew((sk as _, 0.)) }
    fn scale(&mut self, sl: Vec2D) { self.post_scale((sl.x as _, sl.y as _)) }
    fn premul(&mut self, tm: &Self) { self.transform(tm) }
}

impl StyleConv for BLStyle {
    fn solid_color(color: RGBA) -> Self {
        Self::Solid(BLSolidColor::init_rgba32(color.into())
            .expect("failed to create Blend2D solid color"))
    }

    fn linear_gradient(sp: Vec2D, ep: Vec2D, stops: &[(f32, RGBA)]) -> Self {
        let stops = stops.iter().map(|&(offset, color)|
                (offset, color.into()).into()).collect::<Vec<_>>();
        Self::Gradient(BLGradient::new(&BLLinearGradientValues::new(sp.into(), ep.into()))
            .and_then(|gradient| gradient.with_stops(&stops))
            .expect("failed to create Blend2D linear gradient"))
    }

    fn radial_gradient(cp: Vec2D, fp: Vec2D, radii: (f32, f32),
            stops: &[(f32, RGBA)]) -> Self {
        let stops = stops.iter().map(|&(offset, color)|
                (offset, color.into()).into()).collect::<Vec<_>>();
        Self::Gradient(BLGradient::new(&BLRadialGradientValues::
            new(cp.into(), fp.into(), (radii.0 as _, radii.1 as _)))
            .and_then(|gradient| gradient.with_stops(&stops))
            .expect("failed to create Blend2D radial gradient"))
    }
}

pub enum BLStyle { Solid(BLSolidColor), Gradient(BLGradient), }
impl BLStyle {
    fn as_b2d_style(&self) -> &dyn B2DStyle {
        match self {
            Self::Solid(style) => style,
            Self::Gradient(style) => style,
        }
    }
}
impl From<RGBA> for BLRgba32 {
    fn from(color: RGBA) -> Self { (color.r, color.g, color.b, color.a).into() }
}
