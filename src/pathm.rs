/****************************************************************
 * $ID: pathm.rs  	Fri 14 Nov 2025 09:16:08+0800               *
 *                                                              *
 * Maintainer: 范美辉 (MeiHui FAN) <mhfan@ustc.edu>              *
 * Copyright (c) 2025 M.H.Fan, All rights reserved.             *
 ****************************************************************/

use core::f32::consts::PI;
use crate::{schema::*, helpers::*};

// https://docs.rs/kurbo/latest/kurbo/offset/index.html
// https://github.com/nical/lyon/blob/main/crates/algorithms/src/walk.rs
// https://www.reddit.com/r/rust/comments/12do1dq/rendering_text_along_a_curve/
// https://developer.mozilla.org/en-US/docs/Web/SVG/Reference/Element/textPath
#[allow(unused)] fn walk_along_path() { }   // TODO:

pub use kurbo::BezPath;
impl From<Vec2D> for kurbo::Vec2 {
    fn from(val: Vec2D) -> Self { Self::new(val.x as _, val.y as _) }
}
impl PathBuilder for BezPath {
    #[inline] fn new(capacity: u32) -> Self {
        if capacity == 0 { Self::new() } else { Self::with_capacity(capacity as _) }
    }
    #[inline] fn close(&mut self) { self.close_path() }
    #[inline] fn current_pos(&self) -> Option<Vec2D> {
        self.current_position().map(|p| Vec2D::from((p.x as _, p.y as _)))
    }

    #[inline] fn move_to(&mut self, end: Vec2D) { self.move_to(end) }
    #[inline] fn line_to(&mut self, end: Vec2D) { self.line_to(end) }
    #[inline] fn cubic_to(&mut self, ocp: Vec2D, icp: Vec2D, end: Vec2D) {
        self.curve_to(ocp, icp, end)
    }
    #[inline] fn quad_to(&mut self, cp: Vec2D, end: Vec2D) { self.quad_to(cp, end) }

    #[inline] fn from_kurbo(path: BezPath) -> Self { path }
    #[inline] fn to_kurbo(&self) -> BezPath { self.clone() }    // XXX: how to avoid clone?
}

pub trait PathBuilder {     //type Point; type Path;
    fn new(capacity: u32) -> Self;
    fn close(&mut self);

    fn move_to(&mut self, end: Vec2D);
    fn line_to(&mut self, end: Vec2D);
    fn quad_to(&mut self,  cp: Vec2D, end: Vec2D);  // elevating curve order
        //self.cubic_to(cp + (current_pos - cp) / 3, cp + (end - cp) / 3, end)
    fn cubic_to(&mut self, ocp: Vec2D, icp: Vec2D, end: Vec2D);
    #[inline] fn curve_to(&mut self, ocp: Vec2D, icp: Vec2D, end: Vec2D) {
        self.cubic_to(ocp, icp, end)
    }

    fn current_pos(&self) -> Option<Vec2D>;
    fn to_kurbo(&self) -> BezPath;

    #[inline] fn add_arc(&mut self, center: Vec2D, radii: Vec2D, start: f32, sweep: f32) {
        kurbo::Arc::new(center, radii, start as _, sweep as _, 0.)  // in radians
            .to_cubic_beziers(ACCURACY_TOLERANCE, |ocp, icp, end|
                self.curve_to(ocp.into(), icp.into(), end.into()))
    }

    #[inline] fn elliptic_arc_to(&mut self, radii: Vec2D,   // x_rot must be in radians
        x_rot: f32, large: bool, sweep: bool, end: Vec2D) {
        let svg_arc = kurbo::SvgArc {
            to: end.into(), radii: radii.into(),
            x_rotation: x_rot as _, large_arc: large, sweep,
            from: self.current_pos().unwrap().into(),   // XXX:
        };
        if let Some(arc) = kurbo::Arc::from_svg_arc(&svg_arc) {
            arc.to_cubic_beziers(ACCURACY_TOLERANCE, |ocp, icp, end|
                 self.curve_to(ocp.into(), icp.into(), end.into()))
        } else { self.line_to(end) }
    }

    fn from_kurbo(path: BezPath) -> Self where Self: Sized {
        let mut pb = Self::new(path.elements().len() as _);

        #[allow(non_local_definitions)] impl From<kurbo::Point> for Vec2D {
            fn from(pt: kurbo::Point) -> Self { Self { x: pt.x as _, y: pt.y as _ } }
        }   use kurbo::PathEl::*;

        path.iter().for_each(|el| match el {
            MoveTo(pt) => pb.move_to(pt.into()),
            LineTo(pt) => pb.line_to(pt.into()),
            CurveTo(ot, it, pt) =>
                pb.cubic_to(ot.into(), it.into(), pt.into()),
            QuadTo(ct, pt) => pb.quad_to(ct.into(), pt.into()),
            ClosePath => pb.close(),
        }); pb
    }

    #[inline] fn make_dash(&self, offset: f32, pattern: &[f32]) -> Self where Self: Sized {
        Self::from_kurbo(kurbo::dash(self.to_kurbo().iter(), offset as _,
            &pattern.iter().map(|&v| v as f64).collect::<Vec<_>>()).collect())
    }

    // https://lottiefiles.github.io/lottie-docs/scripts/lottie_bezier.js
    // or use curve_length(curve, merr) and subdivide(t, seg) of flo_curves
    fn trim_path(&self, start: f64, mut trim: f64) -> Self where Self: Sized {
        let path = self.to_kurbo();

        use kurbo::{ParamCurve, ParamCurveArclen};
        //let segments = kurbo::segments(path.iter());
        let (mut tri0, mut suml) = (0., path.segments().fold(0.,
            |acc, seg| acc + seg.arclen(ACCURACY_TOLERANCE)));
        if 1. < start + trim { tri0 = start + trim - 1.; trim = 1. - start; }
        let (start, mut trim) = (suml * start, suml * trim);
        tri0 *= suml;  suml = 0.;

        Self::from_kurbo(BezPath::from_path_segments(path.segments().filter_map(|seg| {
            let len = seg.arclen(ACCURACY_TOLERANCE);

            let range = if suml <= start && start < suml + len {
                let start = start - suml;   let end = start + trim;
                if  end < len { trim = 0.;      start / len .. end / len
                } else { trim -= len - start;   start / len .. 1. }
            } else if start < suml && 0. < trim {
                if trim < len { let end = trim / len;   trim = 0.;  0.0 .. end
                } else { trim -= len;   0.0 .. 1. }
            } else if 0. < tri0 {   // rewound part
                if tri0 < len { let end = tri0 / len;   tri0 = 0.;  0.0 .. end
                } else { tri0 -= len;   0.0 .. 1. }
            } else {     suml += len;   return None };
            suml += len;    Some(seg.subsegment(range))
        })))
    }

}

pub trait PathFactory { fn to_path<PB: PathBuilder>(&self, fnth: f32) -> PB; }

impl PathFactory for Rectangle {
    fn to_path<PB: PathBuilder>(&self, fnth: f32) -> PB {
        let center = self. pos.get_value(fnth);
        let halves = self.size.get_value(fnth) / 2.;
        let radius = self.rcr.get_value(fnth).min(halves.x).min(halves.y);
        let (elt, erb) = (center - halves, center + halves);

        // Note that unlike other shapes, on lottie web when the `d` attribute is missing,
        // the rectangle defaults as being reversed.
        //let is_ccw = self.base.dir.map_or(true, |d| matches!(d, ShapeDirection::Reversed));

        if radius < ACCURACY_TOLERANCE as _ {
            let mut path = PB::new(5);
            //path.rect(elt.x, elt.y, size.x, size.y); 	return path;
            path.move_to((erb.x, elt.y).into());    // from top-right going clockwise
            if self.base.is_ccw() {
                path.line_to(elt); path.line_to((elt.x, erb.y).into()); path.line_to(erb);
            } else {
                path.line_to(erb); path.line_to((elt.x, erb.y).into()); path.line_to(elt);
            }   path.close();   	 return path;
        }   let mut path = PB::new(10);

        //path.rounded_rect(elt.x, elt.y, size.x, size.y, radius); 	return path;
        let (clt, crb) = (elt + radius, erb - radius);
            path.move_to((erb.x, clt.y).into());

        /* let tangent = radius * 0.5519;   // approximate with cubic Bezier curve
		let (tlt, trb) = (clt - tangent, crb + tangent);

        if self.base.is_ccw() {
            path.cubic_to((erb.x, tlt.y).into(),
                          (trb.x, elt.y).into(), (crb.x, elt.y).into());
            path. line_to((clt.x, elt.y).into());
            path.cubic_to((tlt.x, elt.y).into(),
                          (elt.x, tlt.y).into(), (elt.x, clt.y).into());
            path. line_to((elt.x, crb.y).into());
            path.cubic_to((elt.x, trb.y).into(),
                          (tlt.x, erb.y).into(), (clt.x, erb.y).into());
            path. line_to((crb.x, erb.y).into());
            path.cubic_to((trb.x, erb.y).into(),
                          (erb.x, trb.y).into(), (erb.x, crb.y).into());
            //path. line_to((erb.x, clt.y).into());
        } else {
            path. line_to((erb.x, crb.y).into());
            path.cubic_to((erb.x, trb.y).into(),
                          (trb.x, erb.y).into(), (crb.x, erb.y).into());
            path. line_to((clt.x, erb.y).into());
            path.cubic_to((tlt.x, erb.y).into(),
                          (elt.x, trb.y).into(), (elt.x, crb.y).into());
            path. line_to((elt.x, clt.y).into());
            path.cubic_to((elt.x, tlt.y).into(),
                          (tlt.x, elt.y).into(), (clt.x, elt.y).into());
            path. line_to((crb.x, elt.y).into());
            path.cubic_to((trb.x, elt.y).into(),
                          (erb.x, tlt.y).into(), (erb.x, clt.y).into());
        }   path.close(); 	return path; */

        let radii = (radius, radius).into();
        if self.base.is_ccw() {
            //path.arc_to((erb.x, elt.y).into(), (crb.x, elt.y).into(), radii);
            path.add_arc((crb.x, clt.y).into(), radii,  0.,  (PI / 2.) as _);
            path.line_to((clt.x, elt.y).into());

            //path.arc_to(elt, (elt.x, clt.y).into(), radii);
            path.add_arc(clt, radii,  (PI / 2.) as _,  PI as _);
            path.line_to((elt.x, crb.y).into());

            //path.arc_to((elt.x, erb.y).into(), (clt.x, erb.y).into(), radii);
            path.add_arc((clt.x, crb.y).into(), radii,  PI as _, -(PI / 2.) as _);
            path.line_to((crb.x, erb.y).into());

            //path.arc_to(erb, (erb.x, crb.y).into(), radii);
            path.add_arc(crb, radii, -(PI / 2.) as _,  0.);
            //path.line_to((erb.x, clt.y).into());
        } else {
            path.line_to((erb.x, crb.y).into());
            path.add_arc(crb, radii,  0., -(PI / 2.) as _);
            //path.arc_to(erb, (crb.x, erb.y).into(), radii);

            path.line_to((clt.x, erb.y).into());
            path.add_arc((clt.x, crb.y).into(), radii, -(PI / 2.) as _,  PI as _);
            //path.arc_to((elt.x, erb.y).into(), (elt.x, crb.y).into(), radii);

            path.line_to((elt.x, clt.y).into());
            path.add_arc(clt, radii,  PI as _,  (PI / 2.) as _);
            //path.arc_to(elt, (clt.x, elt.y).into(), radii);

            path.line_to((crb.x, elt.y).into());
            path.add_arc((crb.x, clt.y).into(), radii,  (PI / 2.) as _,  0.);
            //path.arc_to((erb.x, elt.y).into(), (erb.x, clt.y).into(), radii);
        }   path.close();   path
    }
}

impl PathFactory for Polystar {
    fn to_path<PB: PathBuilder>(&self, fnth: f32) -> PB {
        let center = self.pos.get_value(fnth);
        let (or, nvp) = (self.or.get_value(fnth), self.pt.get_value(fnth));
        let orr = self.os.get_value(fnth) * PI / 2. / 100. / nvp;  // XXX:

        let ir  = self.ir.as_ref().map_or(0.,   // self.sy == StarType::Star
            |ir| ir.get_value(fnth));
        let irr = self.is.as_ref().map_or(0.,
            |is| is.get_value(fnth) * PI / 2. / 100. / nvp);

        let is_star = matches!(self.sy, StarType::Star);
        let mut angle = -PI / 2. + self.rotation.get_value(fnth).to_radians();
        let angle_step = if is_star { PI } else { PI * 2. } /
            if self.base.is_ccw() { -nvp } else { nvp };
        let nvp = nvp as u32;

		let rp = Vec2D::from_polar(angle) * or;
		let pt = center + rp; 	//let rp = rp * orr;
        let mut path = PB::new(2 + if is_star { nvp * 2 } else { nvp });
        path.move_to(pt);

        let rd = Vec2D::from((rp.y, -rp.x));
        let mut lot = pt - rd * orr;

        let mut add_bezier_to = |radius, rr| {
            angle += angle_step;

			let rp = Vec2D::from_polar(angle) * radius;
			let pt = center + rp; 	//let rp = rp * rr;

            let rd = rd * rr;
            path.cubic_to(lot, pt + rd, pt);    lot = pt - rd
        };

        for _ in 0..nvp {
            if is_star { add_bezier_to(ir, irr); } add_bezier_to(or, orr);
        }   path.close();   path
    }
}

impl PathFactory for Ellipse {
    fn to_path<PB: PathBuilder>(&self, fnth: f32) -> PB {
        let mut path = PB::new(6);
        let center = self. pos.get_value(fnth);
        let radii  = self.size.get_value(fnth) / 2.;
        //path.ellipse(center, radii);  return path;

        //  Approximate a circle with cubic Bézier curves
        //  https://spencermortensen.com/articles/bezier-circle/
        let tangent = radii * 0.5519;   // a magic number
        let (elt, tlt) = (center - radii, center - tangent);
        let (erb, trb) = (center + radii, center + tangent);
        path.move_to((center.x, elt.y).into());

        if self.base.is_ccw() {
            path.cubic_to((tlt.x, elt.y).into(),
                          (elt.x, tlt.y).into(), (elt.x, center.y).into());
            path.cubic_to((elt.x, trb.y).into(),
                          (tlt.x, erb.y).into(), (center.x, erb.y).into());
            path.cubic_to((trb.x, erb.y).into(),
                          (erb.x, trb.y).into(), (erb.x, center.y).into());
            path.cubic_to((erb.x, tlt.y).into(),
                          (trb.x, elt.y).into(), (center.x, elt.y).into());
        } else {
            path.cubic_to((trb.x, elt.y).into(),
                          (erb.x, tlt.y).into(), (erb.x, center.y).into());
            path.cubic_to((erb.x, trb.y).into(),
                          (trb.x, erb.y).into(), (center.x, erb.y).into());
            path.cubic_to((tlt.x, erb.y).into(),
                          (elt.x, trb.y).into(), (elt.x, center.y).into());
            path.cubic_to((elt.x, tlt.y).into(),
                          (tlt.x, elt.y).into(), (center.x, elt.y).into());
        }   path.close();   path
    }
}

impl PathFactory for FreePath {
    fn to_path<PB: PathBuilder>(&self, fnth: f32) -> PB {
        if !self.base.is_ccw() { return self.shape.to_path(fnth); }
        let curv = self.shape.get_value(fnth);
        debug_assert!(curv.vp.len() == curv.it.len() &&
                      curv.it.len() == curv.ot.len() && !curv.vp.is_empty());

        let n = curv.vp.len();
        let pt = *curv.vp.last().unwrap();
        let mut path = PB::new(2 + n as u32);   path.move_to(pt);

        for ((&cvp, &cit), (&lvp, &lot)) in
            curv.vp.iter().zip(curv.it.iter()).rev().skip(1).zip(
            curv.vp.iter().zip(curv.ot.iter()).rev()) {
            path.cubic_to(lvp + lot, cvp + cit, cvp);
        }
        /* let mut i = n - 1;
        while 0 < i { let (j, pt) = (i - 1, curv.vp[i]);
            path.cubic_to(curv.vp[j] + curv.ot[j], pt + curv.it[i], pt);    i -= 1; } */

        if  curv.closed {  let j = n - 1;
            path.cubic_to(curv.vp[0] + curv.ot[0], pt + curv.it[j], pt);
            path.close();
        }   path
    }
}

impl PathFactory for ShapeProperty {    // for mask
    fn to_path<PB: PathBuilder>(&self, fnth: f32) -> PB {
        let curv = self.get_value(fnth);
        debug_assert!(curv.vp.len() == curv.it.len() &&
                      curv.it.len() == curv.ot.len() && !curv.vp.is_empty());

        let n = curv.vp.len();
        let pt = *curv.vp.first().unwrap(); //curv.vp[0];
        let mut path = PB::new(2 + n as u32);   path.move_to(pt);

        /* let _ = curv.vp.iter().zip(curv.it.iter()).cycle().skip(1).take( //.rev()
                curv.vp.len() - if curv.closed { 0 } else { 1 }).zip(
                curv.vp.iter().zip(curv.ot.iter())); */

        for ((&cvp, &cit), (&lvp, &lot)) in
            curv.vp.iter().zip(curv.it.iter()).skip(1).zip(
            curv.vp.iter().zip(curv.ot.iter())) {
            path.cubic_to(lvp + lot, cvp + cit, cvp);
        }
        /* for i in 1..n { let (j, pt) = (i - 1, &curv.vp[i]);
            path.cubic_to(curv.vp[j] + curv.ot[j], pt + curv.it[i], pt); } */

        if  curv.closed {  let j = n - 1;
            path.cubic_to(curv.vp[j] + curv.ot[j], pt + curv.it[0], pt);
            path.close();
        }   path
    }
}

