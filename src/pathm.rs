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

impl PathFactory for Rectangle {
    fn to_path(&self, fnth: f32) -> VGPath {
        let center = self. pos.get_value(fnth);
        let halves = self.size.get_value(fnth) / 2.;
        let radius = self.rcr.get_value(fnth).min(halves.x).min(halves.y);
        let (elt, erb) = (center - halves, center + halves);

        // Note that unlike other shapes, on lottie web when the `d` attribute is missing,
        // the rectangle defaults as being reversed.
        //let is_ccw = self.base.dir.map_or(true, |d| matches!(d, ShapeDirection::Reversed));

        let mut path = VGPath::new();
        if radius < ACCURACY_TOLERANCE {
            //path.rect(elt.x, elt.y, size.x, size.y); 	return path;
            path.move_to(erb.x, elt.y);
            if self.base.is_ccw() {
                path.line_to(elt.x, elt.y); path.line_to(elt.x, erb.y);
				path.line_to(erb.x, erb.y);
            } else {
                path.line_to(erb.x, erb.y); path.line_to(elt.x, erb.y);
				path.line_to(elt.x, elt.y);
            }   path.close();   	 return path;
        }

        //path.rounded_rect(elt.x, elt.y, size.x, size.y, radius); 	return path;
        let (clt, crb) = (elt + radius, erb - radius);
            path.move_to(erb.x, clt.y);

        /* let tangent = radius * 0.5519;   // approximate with cubic Bezier curve
		let (tlt, trb) = (clt - tangent, crb + tangent);

        if self.base.is_ccw() {
            path.bezier_to(erb.x, tlt.y, trb.x, elt.y, crb.x, elt.y); path.line_to(clt.x, elt.y);
            path.bezier_to(tlt.x, elt.y, elt.x, tlt.y, elt.x, clt.y); path.line_to(elt.x, crb.y);
            path.bezier_to(elt.x, trb.y, tlt.x, erb.y, clt.x, erb.y); path.line_to(crb.x, erb.y);
            path.bezier_to(trb.x, erb.y, erb.x, trb.y, erb.x, crb.y); //path.line_to(erb.x, clt.y);
        } else {
            path.line_to(erb.x, crb.y); path.bezier_to(erb.x, trb.y, trb.x, erb.y, crb.x, erb.y);
            path.line_to(clt.x, erb.y); path.bezier_to(tlt.x, erb.y, elt.x, trb.y, elt.x, crb.y);
            path.line_to(elt.x, clt.y); path.bezier_to(elt.x, tlt.y, tlt.x, elt.y, clt.x, elt.y);
            path.line_to(crb.x, elt.y); path.bezier_to(trb.x, elt.y, erb.x, tlt.y, erb.x, clt.y);
        }   path.close(); 	return path; */

        if self.base.is_ccw() {     let dir = femtovg::Solidity::Solid;
            //path.arc_to(erb.x, elt.y, crb.x, elt.y, radius);
            path.arc(crb.x, clt.y, radius,  0.,  PI / 2., dir); path.line_to(clt.x, elt.y);
            //path.arc_to(elt.x, elt.y, elt.x, clt.y, radius);
            path.arc(clt.x, clt.y, radius,  PI / 2.,  PI, dir); path.line_to(elt.x, crb.y);
            //path.arc_to(elt.x, erb.y, clt.x, erb.y, radius);
            path.arc(clt.x, crb.y, radius,  PI, -PI / 2., dir); path.line_to(crb.x, erb.y);
            //path.arc_to(erb.x, erb.y, erb.x, crb.y, radius);
            path.arc(crb.x, crb.y, radius, -PI / 2.,  0., dir); //path.line_to(erb.x, clt.y);
        } else {                    let dir = femtovg::Solidity::Hole; // XXX:
            path.line_to(erb.x, crb.y); path.arc(crb.x, crb.y, radius,  0., -PI / 2., dir);
                                        //path.arc_to(erb.x, erb.y, crb.x, erb.y, radius);
            path.line_to(clt.x, erb.y); path.arc(clt.x, crb.y, radius, -PI / 2.,  PI, dir);
                                        //path.arc_to(elt.x, erb.y, elt.x, crb.y, radius);
            path.line_to(elt.x, clt.y); path.arc(clt.x, clt.y, radius,  PI,  PI / 2., dir);
                                        //path.arc_to(elt.x, elt.y, clt.x, elt.y, radius);
            path.line_to(crb.x, elt.y); path.arc(crb.x, clt.y, radius,  PI / 2.,  0., dir);
                                        //path.arc_to(erb.x, elt.y, erb.x, clt.y, radius);
        }   path.close();   path
    }
}

impl PathFactory for Polystar {
    fn to_path(&self, fnth: f32) -> VGPath {
        let center = self.pos.get_value(fnth);
        let (or, nvp) = (self.or.get_value(fnth), self.pt.get_value(fnth));
        let orr = self.os.get_value(fnth) * PI / 2. / 100. / nvp;  // XXX:

        let ir  = self.ir.as_ref().map_or(0.,   // self.sy == StarType::Star
            |ir| ir.get_value(fnth));
        let irr = self.is.as_ref().map_or(0.,
            |is| is.get_value(fnth) * PI / 2. / 100. / nvp);

        let mut angle = -PI / 2. + self.rotation.get_value(fnth).to_radians();
        let angle_step = if matches!(self.sy, StarType::Star) { PI } else { PI * 2. } /
            if self.base.is_ccw() { -nvp } else { nvp };

		let rp = Vec2D::from_polar(angle) * or;
		let pt = center + rp; 	//let rp = rp * orr;
        let mut path = VGPath::new();   path.move_to(pt.x, pt.y);

        let (mut lotx, mut loty) = (pt.x - rp.y * orr, pt.y + rp.x * orr);
        let  mut add_bezier_to = |radius, rr| {
            angle += angle_step;

			let rp = Vec2D::from_polar(angle) * radius;
			let pt = center + rp; 	//let rp = rp * rr;
            let (rdx, rdy) = (rp.y * rr, -rp.x * rr);

            path.bezier_to(lotx, loty,  pt.x + rdx, pt.y + rdy, pt.x, pt.y); // pt + rd
            (lotx, loty) = (pt.x - rdx, pt.y - rdy); // pt - rd
        };

        for _ in 0..nvp as u32 {
            if matches!(self.sy, StarType::Star) { add_bezier_to(ir, irr); }
            add_bezier_to(or, orr);
        }   path.close();   path
    }
}

impl PathFactory for Ellipse {
    fn to_path(&self, fnth: f32) -> VGPath {
        let mut path = VGPath::new();
        let center = self. pos.get_value(fnth);
        let radii  = self.size.get_value(fnth) / 2.;
        //path.ellipse(center.x, center.y, radii.x, radii.y);   return path;

        //  Approximate a circle with cubic Bézier curves
        //  https://spencermortensen.com/articles/bezier-circle/
        let tangent = radii * 0.5519;   // a magic number
        let (elt, tlt) = (center - radii, center - tangent);
        let (erb, trb) = (center + radii, center + tangent);
        path.move_to(center.x, elt.y);

        if self.base.is_ccw() {
            path.bezier_to(tlt.x, elt.y, elt.x, tlt.y, elt.x, center.y);
            path.bezier_to(elt.x, trb.y, tlt.x, erb.y, center.x, erb.y);
            path.bezier_to(trb.x, erb.y, erb.x, trb.y, erb.x, center.y);
            path.bezier_to(erb.x, tlt.y, trb.x, elt.y, center.x, elt.y);
        } else {
            path.bezier_to(trb.x, elt.y, erb.x, tlt.y, erb.x, center.y);
            path.bezier_to(erb.x, trb.y, trb.x, erb.y, center.x, erb.y);
            path.bezier_to(tlt.x, erb.y, elt.x, trb.y, elt.x, center.y);
            path.bezier_to(elt.x, tlt.y, tlt.x, elt.y, center.x, elt.y);
        }   path.close();   path
    }
}

impl PathFactory for FreePath {
    fn to_path(&self, fnth: f32) -> VGPath {
        if !self.base.is_ccw() { return self.shape.to_path(fnth); }
        let curv = self.shape.get_value(fnth);
        debug_assert!(curv.vp.len() == curv.it.len() &&
                      curv.it.len() == curv.ot.len() && !curv.vp.is_empty());

        let pt = curv.vp.last().unwrap();
        let mut path = VGPath::new();   path.move_to(pt.x, pt.y);

        for ((cvp, cit), (lvp, lot)) in
            curv.vp.iter().zip(curv.it.iter()).rev().skip(1).zip(
            curv.vp.iter().zip(curv.ot.iter()).rev()) {
            path.bezier_to( lvp.x + lot.x, lvp.y + lot.y,
                            cvp.x + cit.x, cvp.y + cit.y, cvp.x, cvp.y);
        }
        /* let mut i = curv.vp.len() - 1;
        while 0 < i { let (j, pt) = (i - 1, &curv.vp[i]);
            path.bezier_to(curv.vp[j].x + curv.ot[j].x, curv.vp[j].y + curv.ot[j].y,
                    pt.x + curv.it[i].x, pt.y + curv.it[i].y, pt.x, pt.y);  i -= 1; } */

        if  curv.closed {  let j = curv.it.len() - 1;
            path.bezier_to(curv.vp[0].x + curv.ot[0].x, curv.vp[0].y + curv.ot[0].y,
                pt.x + curv.it[j].x, pt.y + curv.it[j].y, pt.x, pt.y);
            path.close();
        }   path
    }
}

impl PathFactory for ShapeProperty {    // for mask
    fn to_path(&self, fnth: f32) -> VGPath {
        let curv = self.get_value(fnth);
        debug_assert!(curv.vp.len() == curv.it.len() &&
                      curv.it.len() == curv.ot.len() && !curv.vp.is_empty());

        let pt = curv.vp.first().unwrap(); //&curv.vp[0];
        let mut path = VGPath::new();   path.move_to(pt.x, pt.y);

        /* let _ = curv.vp.iter().zip(curv.it.iter()).cycle().skip(1).take( //.rev()
                curv.vp.len() - if curv.closed { 0 } else { 1 }).zip(
                curv.vp.iter().zip(curv.ot.iter())); */

        for ((cvp, cit), (lvp, lot)) in
            curv.vp.iter().zip(curv.it.iter()).skip(1).zip(
            curv.vp.iter().zip(curv.ot.iter())) {
            path.bezier_to( lvp.x + lot.x, lvp.y + lot.y,
                            cvp.x + cit.x, cvp.y + cit.y, cvp.x, cvp.y);
        }
        /* for i in 1..curv.vp.len() { let (j, pt) = (i - 1, &curv.vp[i]);
            path.bezier_to(curv.vp[j].x + curv.ot[j].x, curv.vp[j].y + curv.ot[j].y,
                    pt.x + curv.it[i].x, pt.y + curv.it[i].y, pt.x, pt.y); } */

        if  curv.closed {  let j = curv.ot.len() - 1;
            path.bezier_to(curv.vp[j].x + curv.ot[j].x, curv.vp[j].y + curv.ot[j].y,
                pt.x + curv.it[0].x, pt.y + curv.it[0].y, pt.x, pt.y);
            path.close();
        }   path
    }
}
