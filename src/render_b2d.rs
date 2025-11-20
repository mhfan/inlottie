/****************************************************************
 * $ID: render_b2d.rs  	Thu 20 Nov 2025 16:50:16+0800           *
 *                                                              *
 * Maintainer: 范美辉 (MeiHui FAN) <mhfan@ustc.edu>              *
 * Copyright (c) 2025 M.H.Fan, All rights reserved.             *
 ****************************************************************/

use crate::{helpers::*, pathm::*, style::*};

use intvg::blend2d::{BLPoint, BLPath, BLMatrix2D, BLRgba32, BLSolidColor,
    BLGradient, BLLinearGradientValues, BLRadialGradientValues};

impl From<Vec2D> for BLPoint {
    #[inline] fn from(pt: Vec2D) -> Self { (pt.x, pt.y).into() }
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
        self.elliptic_arc_to((radii.x as _, radii.y as _), x_rot as _, large, sweep, end.into())
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

impl MatrixConv for BLMatrix2D {
    #[inline] fn identity() -> Self { Self::identity() }
    #[inline] fn rotate(&mut self, angle: f32) { self.rotate(angle as _, None) }
    #[inline] fn translate(&mut self, pos: Vec2D) { self.translate(pos.into()) }
    #[inline] fn skew_x(&mut self, sk: f32) { self.skew((sk as _, 0.)) }
    #[inline] fn scale(&mut self, sl: Vec2D) { self.scale((sl.x as _, sl.y as _)) }
    #[inline] fn multiply(&mut self, tm: &Self) { self.transform(tm) }
}

impl From<RGBA> for BLRgba32 {
    #[inline] fn from(color: RGBA) -> Self { (color.r, color.g, color.b, color.a).into() }
}
pub enum BLStyle { Solid(BLSolidColor), Gradient(BLGradient), }
impl StyleConv for BLStyle {
    #[inline] fn solid_color(&mut self, color: RGBA) -> Self {
        Self::Solid(BLSolidColor::init_rgba32(color.into()))
    }

    #[inline] fn linear_gradient(&mut self, sp: Vec2D, ep: Vec2D,
            stops: &[(f32, RGBA)]) -> Self {
        let stops = stops.iter().map(|&(offset, color)|
                (offset, color.into()).into()).collect::<Vec<_>>();
        Self::Gradient(BLGradient::new(&BLLinearGradientValues::
            new(sp.into(), ep.into())).with_stops(&stops))
    }

    #[inline] fn radial_gradient(&mut self, cp: Vec2D, fp: Vec2D, radii: (f32, f32),
            stops: &[(f32, RGBA)]) -> Self {
        let stops = stops.iter().map(|&(offset, color)|
                (offset, color.into()).into()).collect::<Vec<_>>();
        Self::Gradient(BLGradient::new(&BLRadialGradientValues::
            new(cp.into(), fp.into(), (radii.0 as _, radii.1 as _))).with_stops(&stops))
    }
}

