/****************************************************************
 * $ID: pathm.rs  	Fri 14 Nov 2025 09:16:08+0800               *
 *                                                              *
 * Maintainer: 范美辉 (MeiHui FAN) <mhfan@ustc.edu>              *
 * Copyright (c) 2025 M.H.Fan, All rights reserved.             *
 ****************************************************************/

use std::f32::consts::PI;
use crate::{schema::*, helpers::*};

#[cfg(feature = "b2d")] use intvg::blend2d::{BLPoint, BLPath};
#[cfg(feature = "b2d")] impl From<Point> for BLPoint {
    #[inline] fn from(p: Point) -> Self { (p.x, p.y).into() }
}
#[cfg(feature = "b2d")] impl PathBuilder for BLPath {
    #[inline] fn new(capacity: u32) -> Self {   // XXX: different commands vary in size?
        let mut path = Self::new();
        if capacity != 0 { path.reserve(capacity as _); }   path
    }
    #[inline] fn close(&mut self) { self.close() }
    #[inline] fn current_position(&self) -> Option<Point> {
        self.get_last_vertex().map(|p| Point { x: p.x() as _, y: p.y() as _ })
    }

    #[inline] fn move_to(&mut self, end: Point) { self.move_to(end.into()) }
    #[inline] fn line_to(&mut self, end: Point) { self.line_to(end.into()) }
    #[inline] fn cubic_to(&mut self, ocp: Point, icp: Point, end: Point) {
        self.cubic_to(ocp.into(), icp.into(), end.into())
    }
    #[inline] fn quad_to(&mut self, cp: Point, end: Point) {
        self.quad_to(cp.into(), end.into())
    }
    #[inline] fn add_arc(&mut self, center: Point, radii: Vec2D, start: f64, sweep: f64) {
        self.arc_to(center.into(), radii.into(), start, sweep)
    }
    #[inline] fn elliptic_arc_to(&mut self, radii: Vec2D,
        x_rot: f64, large: bool, sweep: bool, end: Point) {
        self.elliptic_arc_to(radii.into(), x_rot, large, sweep, end.into())
    }
}

impl PathBuilder for femtovg::Path {    // TODO: reserve capacity, get_last_point
    #[inline] fn current_position(&self) -> Option<Point> { todo!() }
    #[inline] fn new(_capacity: u32) -> Self { Self::new() }
    #[inline] fn close(&mut self) { self.close() }

    #[inline] fn move_to(&mut self, end: Point) { self.move_to(end.x, end.y) }
    #[inline] fn line_to(&mut self, end: Point) { self.line_to(end.x, end.y) }
    #[inline] fn cubic_to(&mut self, ocp: Point, icp: Point, end: Point) {
        self.bezier_to(ocp.x, ocp.y, icp.x, icp.y, end.x, end.y)
    }
    #[inline] fn quad_to(&mut self, cp: Point, end: Point) {
        self.quad_to(cp.x, cp.y, end.x, end.y)
    }
    #[inline] fn add_arc(&mut self, center: Point, radii: Vec2D, start: f64, sweep: f64) {
        self.arc(center.x, center.y, (radii.x + radii.y) / 2.,
            start as _, sweep as _, femtovg::Solidity::Solid)   // XXX:
        //self.arc_to(x1, y1, x2, y2, (radii.x + radii.y) / 2.);
    }
    #[inline] fn elliptic_arc_to(&mut self, _radii: Vec2D,
        _x_rot: f64, _large: bool, _sweep: bool, _end: Point) { todo!() }
}

impl From<Point> for kurbo::Vec2 {
    fn from(val: Point) -> Self { Self::new(val.x as _, val.y as _) }
}
impl PathBuilder for kurbo::BezPath {
    #[inline] fn new(capacity: u32) -> Self {
        if capacity == 0 { Self::new() } else { Self::with_capacity(capacity as _) }
    }
    #[inline] fn close(&mut self) { self.close_path() }
    #[inline] fn current_position(&self) -> Option<Point> {
        self.current_position().map(|p| Point { x: p.x as _, y: p.y as _ })
    }

    #[inline] fn move_to(&mut self, end: Point) { self.move_to(end) }
    #[inline] fn line_to(&mut self, end: Point) { self.line_to(end) }
    #[inline] fn cubic_to(&mut self, ocp: Point, icp: Point, end: Point) {
        self.curve_to(ocp, icp, end)
    }
    #[inline] fn quad_to(&mut self, cp: Point, end: Point) { self.quad_to(cp, end) }
    #[inline] fn add_arc(&mut self, center: Point, radii: Vec2D, start: f64, sweep: f64) {
        let arc = kurbo::Arc::new(center, radii, start, sweep, 0.);
            arc.to_cubic_beziers(ACCURACY_TOLERANCE as _,
            |ocp, icp, end| self.curve_to(ocp, icp, end))
    }
    #[inline] fn elliptic_arc_to(&mut self, radii: Vec2D,
        x_rot: f64, large: bool, sweep: bool, end: Point) {
        let svg_arc = kurbo::SvgArc {
            to: end.into(), radii: radii.into(),
            x_rotation: x_rot, large_arc: large, sweep,
            from: self.current_position().unwrap_or_default(),
        };
        if let Some(arc) = kurbo::Arc::from_svg_arc(&svg_arc) {
            arc.to_cubic_beziers(ACCURACY_TOLERANCE as _,
                |ocp, icp, end| self.curve_to(ocp, icp, end))
        } else { self.line_to(end) }
    }
}

type Point = Vec2D;
pub trait PathBuilder {     //type Point; type Path;
    fn new(capacity: u32) -> Self;
    fn close(&mut self);

    fn move_to(&mut self, end: Point);
    fn line_to(&mut self, end: Point);
    fn quad_to(&mut self,  cp: Point, end: Point);
    fn cubic_to(&mut self, ocp: Point, icp: Point, end: Point);
    fn curve_to(&mut self, ocp: Point, icp: Point, end: Point) {
        self.cubic_to(ocp, icp, end)
    }

    fn current_position(&self) -> Option<Point>;
    fn elliptic_arc_to(&mut self, radii: Vec2D,
        x_rot: f64, large: bool, sweep: bool, end: Point);  // x_rot in radians
    fn add_arc(&mut self, center: Point, radii: Vec2D, start: f64, sweep: f64);
}

pub trait PathFactory { fn to_path<PB: PathBuilder>(&self, fnth: f32) -> PB; }
