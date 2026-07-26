/****************************************************************
 * $ID: pathm.rs  	Fri 14 Nov 2025 09:16:08+0800               *
 *                                                              *
 * Maintainer: 范美辉 (MeiHui FAN) <mhfan@ustc.edu>              *
 * Copyright (c) 2025 M.H.Fan, All rights reserved.             *
 ****************************************************************/

use core::f32::consts::PI;
use crate::core::{helpers::{Vec2D, ACCURACY_TOLERANCE},
    schema::{Rectangle, Polystar, Ellipse, FreePath, ShapeProperty, StarType, LineJoin}};

impl From<Vec2D> for kurbo::Vec2 {
    fn from(val: Vec2D) -> Self { Self::new(val.x as _, val.y as _) }
}   pub use   kurbo::BezPath;
impl PathBuilder for BezPath {
    fn new(capacity: u32) -> Self {
        if capacity == 0 { Self::new() } else { Self::with_capacity(capacity as _) }
    }
    fn close(&mut self) { self.close_path() }
    fn current_pos(&self) -> Option<Vec2D> {
        self.current_position().map(|p| Vec2D::from((p.x as _, p.y as _)))
    }

    fn move_to(&mut self, end: Vec2D) { self.move_to(end) }
    fn line_to(&mut self, end: Vec2D) { self.line_to(end) }
    fn cubic_to(&mut self, ocp: Vec2D, icp: Vec2D, end: Vec2D) {
        self.curve_to(ocp, icp, end)
    }
    fn quad_to(&mut self, cp: Vec2D, end: Vec2D) { self.quad_to(cp, end) }

    fn from_kurbo(path: BezPath) -> Self { path }
    fn to_kurbo(&self) -> BezPath { self.clone() }    // XXX: how to avoid clone?
}

pub trait PathBuilder {     //type Point; type Path;
    fn new(capacity: u32) -> Self;
    fn close(&mut self);

    fn move_to(&mut self, end: Vec2D);
    fn line_to(&mut self, end: Vec2D);
    fn quad_to(&mut self,  cp: Vec2D, end: Vec2D);  // elevating curve order
        //self.cubic_to(cp + (current_pos - cp) / 3, cp + (end - cp) / 3, end)
    fn cubic_to(&mut self, ocp: Vec2D, icp: Vec2D, end: Vec2D);
    fn curve_to(&mut self, ocp: Vec2D, icp: Vec2D, end: Vec2D) {
        self.cubic_to(ocp, icp, end)
    }

    fn current_pos(&self) -> Option<Vec2D>;
    fn to_kurbo(&self) -> BezPath;

    fn rect(&mut self, x: f32, y: f32, w: f32, h: f32) {
        self.move_to((x + w, y).into());
        self.line_to((x + w, y + h).into());
        self.line_to((x,     y + h).into());
        self.line_to((x,     y).into());    self.close();
    }
    fn add_arc(&mut self, center: Vec2D, radii: Vec2D, start: f32, sweep: f32) {
        kurbo::Arc::new(center, radii, start as _, sweep as _, 0.)  // in radians
            .to_cubic_beziers(ACCURACY_TOLERANCE, |ocp, icp, end|
                self.curve_to(ocp.into(), icp.into(), end.into()))
    }

    fn elliptic_arc_to(&mut self, radii: Vec2D,   // x_rot must be in radians
        x_rot: f32, large: bool, sweep: bool, end: Vec2D) {
        let svg_arc = kurbo::SvgArc {
            to: end.into(), radii: radii.into(),
            x_rotation: x_rot as _, large_arc: large, sweep,
            from: self.current_pos().map_or(Default::default(), Into::into),
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

    /* kurbo::dash requires &[f64], so this allocates only to convert the pattern.
    fn make_dash(&self, offset: f32, pattern: &[f32]) -> Self where Self: Sized {
        Self::from_kurbo(kurbo::dash(self.to_kurbo().iter(), offset as _,
            &pattern.iter().map(|&v| v as f64).collect::<Vec<_>>()).collect())
    } */

    // https://lottie.github.io/lottie-spec/latest/specs/shapes/#trim-path
    fn trim_path(&mut self, start: f64, trim: f64) where Self: Sized {
        if trim <= 0. { *self = Self::new(0); return }
        if 1. <= trim { return }

        let path = MeasuredPath::new(self.to_kurbo());
        let start = start.rem_euclid(1.);
        let end = start + trim;
        let trimmed = if end <= 1. {
            path.trim_ranges(&[(start, end)])
        } else {
            path.trim_ranges(&[(start, 1.), (0., end - 1.)])
        };  *self = Self::from_kurbo(trimmed);
    }

    fn round_corners(&mut self, radius: f32) where Self: Sized {
        use kurbo::{PathEl::*, Point};
        let radius = f64::from(radius.max(0.));
        if  radius == 0. { return }

        let (source, mut output) = (self.to_kurbo(), BezPath::new());
        let (mut elements, mut points) = (Vec::new(), Vec::new());
        let (mut straight, mut closed) = (true, false);
        let flush = |output: &mut BezPath, elements: &mut Vec<kurbo::PathEl>,
            points: &mut Vec<Point>, straight: &mut bool, closed: &mut bool| {
            if points.len() < 2 || !*straight { output.extend(elements.drain(..)); } else {
                if *closed && points.last() == points.first() { points.pop(); }
                let count = points.len();
                if  count < 2 { output.extend(elements.drain(..)); } else {
                    let corner = |index: usize| {
                        let current = points[index];
                        let previous = points[(index + count - 1) % count];
                        let next = points[(index + 1) % count];
                        let (incoming, outgoing) = (previous - current, next - current);
                        let distance = radius.min(incoming.hypot() * 0.5)
                            .min(outgoing.hypot() * 0.5);
                        if distance == 0. { (current, current) } else {
                            (current + incoming * (distance / incoming.hypot()),
                             current + outgoing * (distance / outgoing.hypot()))
                        }
                    };

                    if *closed {
                        let (_, first) = corner(0);
                        output.move_to(first);
                        for index in 1..=count {
                            let (entry, exit) = corner(index % count);
                            output.line_to(entry);
                            output.quad_to(points[index % count], exit);
                        }   output.close_path();
                    } else {
                        output.move_to(points[0]);
                        for index in 1..count - 1 {
                            let (entry, exit) = corner(index);
                            output.line_to(entry);
                            output.quad_to(points[index], exit);
                        }   output.line_to(points[count - 1]);
                    }   elements.clear();
                }
            }
            points.clear(); *straight = true; *closed = false;
        };

        for element in source.iter() { match element {
            MoveTo(point) => {
                if !elements.is_empty() {
                    flush(&mut output, &mut elements, &mut points,
                        &mut straight, &mut closed);
                }
                elements.push(MoveTo(point)); points.push(point);
            }
            LineTo(point) => {
                elements.push(LineTo(point)); points.push(point);
            }
            QuadTo(control, point) => {
                elements.push(QuadTo(control, point));
                straight &= points.last().is_some_and(|&start| control == start || control == point);
                points.push(point);
            }
            CurveTo(first, second, point) => {
                elements.push(CurveTo(first, second, point));
                straight &= points.last().is_some_and(|&start| first == start && second == point);
                points.push(point);
            }
            ClosePath => {
                elements.push(ClosePath); closed = true;
                flush(&mut output, &mut elements, &mut points, &mut straight, &mut closed);
            }
        }}
        if !elements.is_empty() {
            flush(&mut output, &mut elements, &mut points, &mut straight, &mut closed);
        }   *self = Self::from_kurbo(output);
    }

    fn offset_path(&mut self, amount: f32, join: LineJoin, miter_limit: f32)
        where Self: Sized {
        use kurbo::{ParamCurve, PathEl::*, Point, Vec2};
        let amount = f64::from(amount);
        if  amount == 0. { return }
        let source = self.to_kurbo();

        fn flatten(segment: kurbo::PathSeg, points: &mut Vec<Point>, depth: u8) {
            let (start, end, middle) =
                (segment.eval(0.), segment.eval(1.), segment.eval(0.5));
            let chord = end - start;
            let distance = if chord.hypot2() == 0. { (middle - start).hypot()
            } else { chord.cross(middle - start).abs() / chord.hypot() };
            if distance <= ACCURACY_TOLERANCE || depth == 12 { points.push(end); } else {
                flatten(segment.subsegment(0. .. 0.5), points, depth + 1);
                flatten(segment.subsegment(0.5 .. 1.), points, depth + 1);
            }
        }
        fn intersection(a: Point, av: Vec2, b: Point, bv: Vec2) -> Option<Point> {
            let cross = av.cross(bv);
            (cross.abs() > 1e-9).then(|| a + av * ((b - a).cross(bv) / cross))
        }

        let mut contours: Vec<(Vec<Point>, bool)> = Vec::new();
        let (mut points, mut current) = (Vec::new(), None);
        for element in source.iter() { match element {
            MoveTo(point) => {
                if !points.is_empty() { contours.push((core::mem::take(&mut points), false)); }
                points.push(point); current = Some(point);
            }
            LineTo(point) => { points.push(point); current = Some(point); }
            QuadTo(control, point) => {
                if let Some(start) = current {
                    flatten(kurbo::QuadBez::new(start, control, point).into(), &mut points, 0);
                }
                current = Some(point);
            }
            CurveTo(first, second, point) => {
                if let Some(start) = current {
                    flatten(kurbo::CubicBez::new(start, first, second, point).into(),
                        &mut points, 0);
                }   current = Some(point);
            }
            ClosePath => {
                if points.len() > 1 {
                    contours.push((core::mem::take(&mut points), true));
                }   current = None;
            }
        }}
        if !points.is_empty() { contours.push((points, false)); }

        let mut output = BezPath::new();
        for (mut points, closed) in contours { points.dedup();
            if closed && points.len() > 1 && points.last() == points.first() { points.pop(); }
            if points.len() < 2 { continue }
            let count = points.len();
            let signed_area = if closed {
                (0..count).map(|index| {
                    let (a, b) = (points[index], points[(index + 1) % count]);
                    a.x * b.y - b.x * a.y
                }).sum::<f64>()
            } else { 0. };
            let distance = if closed && 0. < signed_area { -amount } else { amount };
            let direction = |from: Point, to: Point| (to - from).normalize();
            let normal = |dir: Vec2| Vec2::new(-dir.y, dir.x) * distance;

            let append_join = |path: &mut BezPath, index: usize| {
                let previous = points[(index + count - 1) % count];
                let (point, next) = (points[index], points[(index + 1) % count]);
                let (before, after) = (direction(previous, point), direction(point, next));
                let (first, second) = (point + normal(before), point + normal(after));
                match join {
                    LineJoin::Miter => {
                        let miter = intersection(first, before, second, after);
                        if let Some(miter) = miter.filter(|miter|
                            (*miter - point).hypot() <=
                                distance.abs() * f64::from(miter_limit.max(1.))) {
                            path.line_to(miter);
                        } else {
                            path.line_to(first); path.line_to(second);
                        }
                    }
                    LineJoin::Bevel => { path.line_to(first); path.line_to(second); }
                    LineJoin::Round => {
                        path.line_to(first);
                        let start = (first - point).atan2();
                        let mut sweep = (second - point).atan2() - start;
                        if 0. < distance && sweep < 0. { sweep += core::f64::consts::TAU; }
                        if distance < 0. && 0. < sweep { sweep -= core::f64::consts::TAU; }
                        kurbo::Arc::new(point, (distance.abs(), distance.abs()),
                            start, sweep, 0.).to_cubic_beziers(ACCURACY_TOLERANCE,
                            |first, second, end| path.curve_to(first, second, end));
                    }
                }
            };

            if closed {
                let point = points[0];
                let before = direction(points[count - 1], point);
                let after = direction(point, points[1]);
                let (first, second) = (point + normal(before), point + normal(after));
                let start = if matches!(join, LineJoin::Miter) {
                    intersection(first, before, second, after).filter(|miter|
                        (*miter - point).hypot() <=
                            distance.abs() * f64::from(miter_limit.max(1.)))
                        .unwrap_or(first)
                } else { first };
                output.move_to(start);
                for index in 0..count { append_join(&mut output, index); }
                output.close_path();
            } else {
                output.move_to(points[0] + normal(direction(points[0], points[1])));
                for index in 1..count - 1 { append_join(&mut output, index); }
                output.line_to(points[count - 1] +
                    normal(direction(points[count - 2], points[count - 1])));
            }
        }   *self = Self::from_kurbo(output);
    }
}

pub(crate) struct MeasuredPath {
    path: BezPath, pub length: f64,
    segments: Vec<(kurbo::PathSeg, f64)>,
}

use kurbo::{ParamCurve, ParamCurveArclen};
impl MeasuredPath {
    pub fn new(path: BezPath) -> Self {
        let mut segments = Vec::with_capacity(path.elements().len().saturating_sub(1));
        let mut length = 0.;
        for seg in path.segments() {
            let len = seg.arclen(ACCURACY_TOLERANCE);
            segments.push((seg, len)); length += len;
        }
        Self { path, segments, length }
    }

    pub fn trim_ranges(&self, ranges: &[(f64, f64)]) -> BezPath {
        if ranges.len() == 1 && ranges[0].0 <= 0. && 1. <= ranges[0].1 {
            return self.path.clone()
        }
        if self.length == 0. { return BezPath::new() }
        let mut output = Vec::with_capacity(self.segments.len() * ranges.len());

        for &(from, to) in ranges {
            let (from, to, mut offset) = (
                from.clamp(0., 1.) * self.length,
                  to.clamp(0., 1.) * self.length, 0.);
            if to <= from { continue }

            for &(seg, len) in &self.segments {
                let next = offset + len;
                let (lo, hi) = (from.max(offset), to.min(next));
                if lo < hi && 0. < len {
                    let range = seg.inv_arclen(lo - offset, ACCURACY_TOLERANCE)
                             .. seg.inv_arclen(hi - offset, ACCURACY_TOLERANCE);
                    output.push(seg.subsegment(range));
                }   offset = next;
            }
        }   BezPath::from_path_segments(output.into_iter())
    }
}

pub trait PathFactory { fn to_path<PB: PathBuilder>(&self, fnth: f32) -> PB; }

impl PathFactory for Rectangle { #[allow(unreachable_code)]
    fn to_path<PB: PathBuilder>(&self, fnth: f32) -> PB {
        let center = self. pos.get_value(fnth);
        let halves = self.size.get_value(fnth) / 2.;
        let radius = self.rcr.as_ref().map_or(0.,
            |v| v.get_value(fnth).min(halves.x).min(halves.y));
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

        let tangent = radius * 0.552_284_8;
        // 0.5519, approximate with cubic Bezier curve
		let (tlt, trb) = (clt - tangent, crb + tangent);

        if self.base.is_ccw() {
            path.cubic_to((erb.x, tlt.y).into(), (trb.x, elt.y).into(),
                          (crb.x, elt.y).into());
            path. line_to((clt.x, elt.y).into());
            path.cubic_to((tlt.x, elt.y).into(), (elt.x, tlt.y).into(),
                          (elt.x, clt.y).into());
            path. line_to((elt.x, crb.y).into());
            path.cubic_to((elt.x, trb.y).into(), (tlt.x, erb.y).into(),
                          (clt.x, erb.y).into());
            path. line_to((crb.x, erb.y).into());
            path.cubic_to((trb.x, erb.y).into(), (erb.x, trb.y).into(),
                          (erb.x, crb.y).into());
            //path. line_to((erb.x, clt.y).into());
        } else {
            path. line_to((erb.x, crb.y).into());
            path.cubic_to((erb.x, trb.y).into(), (trb.x, erb.y).into(),
                          (crb.x, erb.y).into());
            path. line_to((clt.x, erb.y).into());
            path.cubic_to((tlt.x, erb.y).into(), (elt.x, trb.y).into(),
                          (elt.x, crb.y).into());
            path. line_to((elt.x, clt.y).into());
            path.cubic_to((elt.x, tlt.y).into(), (tlt.x, elt.y).into(),
                          (clt.x, elt.y).into());
            path. line_to((crb.x, elt.y).into());
            path.cubic_to((trb.x, elt.y).into(), (erb.x, tlt.y).into(),
                          (erb.x, clt.y).into());
        }   path.close(); 	return path;

        let (radii, quarter) = ((radius, radius).into(), PI / 2.);
        if self.base.is_ccw() {
            //path.arc_to((erb.x, elt.y).into(), (crb.x, elt.y).into(), radii);
            path.add_arc((crb.x, clt.y).into(), radii, 0., -quarter);
            path.line_to((clt.x, elt.y).into());

            //path.arc_to(elt, (elt.x, clt.y).into(), radii);
            path.add_arc(clt, radii, -quarter, -quarter);
            path.line_to((elt.x, crb.y).into());

            //path.arc_to((elt.x, erb.y).into(), (clt.x, erb.y).into(), radii);
            path.add_arc((clt.x, crb.y).into(), radii, PI as _, -quarter);
            path.line_to((crb.x, erb.y).into());

            //path.arc_to(erb, (erb.x, crb.y).into(), radii);
            path.add_arc(crb, radii, quarter, -quarter);
            //path.line_to((erb.x, clt.y).into());
        } else {
            path.line_to((erb.x, crb.y).into());
            path.add_arc(crb, radii, 0., quarter);
            //path.arc_to(erb, (crb.x, erb.y).into(), radii);

            path.line_to((clt.x, erb.y).into());
            path.add_arc((clt.x, crb.y).into(), radii, quarter, quarter);
            //path.arc_to((elt.x, erb.y).into(), (elt.x, crb.y).into(), radii);

            path.line_to((elt.x, clt.y).into());
            path.add_arc(clt, radii, PI as _, quarter);
            //path.arc_to(elt, (clt.x, elt.y).into(), radii);

            path.line_to((crb.x, elt.y).into());
            path.add_arc((crb.x, clt.y).into(), radii, -quarter, quarter);
            //path.arc_to((erb.x, elt.y).into(), (erb.x, clt.y).into(), radii);
        }   path.close();   path
    }
}

impl PathFactory for Polystar {
    fn to_path<PB: PathBuilder>(&self, fnth: f32) -> PB {
        let points = self.pt.get_value(fnth).round();
        if !points.is_finite() || points < 1. || points > u16::MAX as f32 {
            return PB::new(0)
        }
        let is_star = matches!(self.sy, StarType::Star);
        let center = self.pos.get_value(fnth);

        let vertex_count = points as u32 * if is_star { 2 } else { 1 };
        let (outer_radius, inner_radius) = (self.or.get_value(fnth),
            self.ir.as_ref().map_or(0., |ir| ir.get_value(fnth)));
        let roundness = PI / 2. / 100. / points;
        let (outer_roundness, inner_roundness) = (
            self.os.get_value(fnth) * roundness,
            self.is.as_ref().map_or(0., |is| is.get_value(fnth) * roundness));

        let mut angle = -PI / 2. + self.rotation.get_value(fnth).to_radians();
        let angle_step = if is_star { PI } else { PI * 2. } /
            if self.base.is_ccw() { -points } else { points };
        let direction = angle_step.signum();
        let mut path = PB::new(2 + vertex_count);

        let mut append_vertex = |radius, roundness, (first, last_out)| {
            let radial = Vec2D::from_polar(angle) * radius;
            let point = center + radial;
            let tangent = Vec2D::from((-radial.y, radial.x)) * roundness * direction;
            if first { path.move_to(point); } else {
                path.cubic_to(last_out, point - tangent, point);
            }   angle += angle_step;
            (false, point + tangent)
        };

        let mut state = append_vertex(outer_radius, outer_roundness, (true, (0., 0.).into()));
        for index in 1..vertex_count {
            let inner = is_star && index % 2 == 1;
            state = append_vertex(
                if inner { inner_radius } else { outer_radius },
                if inner { inner_roundness } else { outer_roundness }, state);
        }
        let radial = Vec2D::from_polar(angle) * outer_radius;
        let point = center + radial;
        let tangent = Vec2D::from((-radial.y, radial.x)) * outer_roundness * direction;
        path.cubic_to(state.1, point - tangent, point);
        path.close();   path
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
        let curv = self.shape.get_value_cow(fnth);
        bezier_path(&curv, self.base.is_ccw())
    }
}

impl PathFactory for ShapeProperty {    // for mask
    fn to_path<PB: PathBuilder>(&self, fnth: f32) -> PB {
        let curv = self.get_value_cow(fnth);
        bezier_path(&curv, false)
    }
}

fn bezier_path<PB: PathBuilder>(curve: &crate::core::schema::Bezier, reversed: bool) -> PB {
    let n = curve.vp.len();
    if n == 0 || n != curve.it.len() || n != curve.ot.len() {
        return PB::new(0)
    }

    let first = if reversed { n - 1 } else { 0 };
    let mut path = PB::new(2 + n as u32);
    path.move_to(curve.vp[first]);
    let mut append = |from: usize, to: usize| {
        let (out, incoming) = if reversed {
            (curve.it[from], curve.ot[to])
        } else {
            (curve.ot[from], curve.it[to])
        };
        path.cubic_to(curve.vp[from] + out,
            curve.vp[to] + incoming, curve.vp[to]);
    };

    if reversed {
        for to in (0..n - 1).rev() { append(to + 1, to); }
        if curve.closed { append(0, n - 1); }
    } else {
        for to in 1..n  { append(to - 1, to); }
        if curve.closed { append(n - 1, 0); }
    }
    if curve.closed { path.close(); }
    path
}

#[cfg(test)] mod tests { use super::*;
    use crate::core::schema::{AnimatedProperty, Bezier};
    use kurbo::Shape;

    fn square() -> BezPath {
        let mut path = BezPath::new();
        path.move_to((0., 0.)); path.line_to((10., 0.));
        path.line_to((10., 10.)); path.line_to((0., 10.)); path.close_path();
        path
    }

    #[test] fn geometry_modifiers_leave_zero_values_in_place() {
        let mut path = square();
        let storage = path.elements().as_ptr();
        path.round_corners(0.);
        path.offset_path(0., LineJoin::Round, 4.);
        assert_eq!(storage, path.elements().as_ptr());
    }

    #[test] fn rounded_corners_rounds_closed_straight_contours_and_limits_radius() {
        let mut rounded = square(); rounded.round_corners(20.);
        assert!(matches!(rounded.elements().last(), Some(kurbo::PathEl::ClosePath)));
        assert_eq!(rounded.elements().iter()
            .filter(|element| matches!(element, kurbo::PathEl::QuadTo(..))).count(), 4);
        assert_eq!(rounded.bounding_box(), kurbo::Rect::new(0., 0., 10., 10.));
    }

    #[test] fn offset_path_expands_and_contracts_closed_contours() {
        let (mut expanded, mut contracted) = (square(), square());
        expanded.offset_path(2., LineJoin::Miter, 4.);
        contracted.offset_path(-2., LineJoin::Miter, 4.);
        assert_eq!(expanded.bounding_box(), kurbo::Rect::new(-2., -2., 12., 12.));
        assert_eq!(contracted.bounding_box(), kurbo::Rect::new(2., 2., 8., 8.));
        assert!(matches!(expanded.elements().last(), Some(kurbo::PathEl::ClosePath)));
    }

    #[test] fn offset_path_supports_round_and_bevel_joins_on_curves() {
        let mut path = BezPath::new();
        path.move_to((0., 0.)); path.curve_to((0., 10.), (10., 10.), (10., 0.));
        path.line_to((20., 0.));

        let (mut round, mut bevel) = (path.clone(), path);
        round.offset_path(2., LineJoin::Round, 4.);
        bevel.offset_path(2., LineJoin::Bevel, 4.);
        assert!(round.elements().iter()
            .any(|element| matches!(element, kurbo::PathEl::CurveTo(..))));
        assert!(bevel.elements().iter()
            .all(|element| !matches!(element, kurbo::PathEl::CurveTo(..))));
    }

    #[test] fn rounded_rectangle_has_four_quarter_curves() {
        let rect: Rectangle = serde_json::from_str(
            r#"{"ty":"rc","s":{"k":[100,80]},"p":{"k":[0,0]},"r":{"k":10}}"#,
        ).unwrap();
        let path = rect.to_path::<BezPath>(0.);
        let ends = path.elements().iter().filter_map(|el| match el {
            kurbo::PathEl::CurveTo(_, _, end) => Some(*end), _ => None,
        }).collect::<Vec<_>>();

        assert_eq!(ends.len(), 4);
        for (actual, expected) in ends.iter().zip(
            [(40., 40.), (-50., 30.), (-40., -40.), (50., -30.), ]) {
            assert!((actual.x - expected.0).abs() < 1e-9);
            assert!((actual.y - expected.1).abs() < 1e-9);
        }
    }

    #[test] fn invalid_bezier_produces_an_empty_path() {
        let shape = AnimatedProperty::from_value(Bezier {
            closed: true, vp: Vec::new(), it: Vec::new(), ot: Vec::new(),
        }); assert!(shape.to_path::<BezPath>(0.).is_empty());
    }

    #[test] fn reversed_free_path_swaps_tangent_roles_and_closes() {
        let shape: FreePath = serde_json::from_str(r#"{
            "ty":"sh","d":3,"ks":{"k":{
                "c":true,
                "v":[[0,0],[10,0],[20,0]],
                "i":[[1,2],[3,4],[5,6]],
                "o":[[7,8],[9,10],[11,12]]
            }}
        }"#).unwrap();
        use kurbo::{PathEl::*, Point};
        assert_eq!(shape.to_path::<BezPath>(0.).elements(), &[
            MoveTo(Point::new(20., 0.)),
            CurveTo(Point::new(25., 6.), Point::new(19., 10.), Point::new(10., 0.)),
            CurveTo(Point::new(13., 4.), Point::new(7., 8.), Point::new(0., 0.)),
            CurveTo(Point::new(1., 2.), Point::new(31., 12.), Point::new(20., 0.)),
            ClosePath,
        ]);
    }

    #[test] fn polystar_rounds_point_count_and_rotates_each_tangent() {
        let star: Polystar = serde_json::from_str(r#"{
            "ty":"sr","sy":2,"p":{"k":[0,0]},"pt":{"k":3.6},
            "or":{"k":10},"os":{"k":100},"r":{"k":0}
        }"#).unwrap();
        let path = star.to_path::<BezPath>(0.);
        assert_eq!(path.elements().iter()
            .filter(|element| matches!(element, kurbo::PathEl::CurveTo(..))).count(), 4);

        let kurbo::PathEl::CurveTo(ctrl1, ctrl2, end) = path.elements()[1] else { panic!() };
        let tangent = 10. * PI as f64 / 8.;
        assert!((ctrl1.x - tangent).abs() < 1e-6 && (ctrl1.y + 10.).abs() < 1e-6);
        assert!((ctrl2.x - 10.).abs() < 1e-6 && (ctrl2.y + tangent).abs() < 1e-6);
        assert!((end.x - 10.).abs() < 1e-6 && end.y.abs() < 1e-6);
    }

    #[test] fn polystar_honors_reversed_direction_and_rejects_invalid_counts() {
        let reversed: Polystar = serde_json::from_str(r#"{
            "ty":"sr","d":3,"sy":2,"p":{"k":[0,0]},"pt":{"k":4},
            "or":{"k":10},"os":{"k":100},"r":{"k":0}
        }"#).unwrap();
        let path = reversed.to_path::<BezPath>(0.);
        let kurbo::PathEl::CurveTo(ctrl1, _, end) = path.elements()[1] else { panic!() };
        assert!(ctrl1.x < 0. && end.x < 0.);

        let mut invalid: Polystar = serde_json::from_str(r#"{
            "ty":"sr","sy":2,"p":{"k":[0,0]},"pt":{"k":0},
            "or":{"k":10},"os":{"k":0},"r":{"k":0}
        }"#).unwrap();
        assert!(invalid.to_path::<BezPath>(0.).is_empty());
        invalid.pt = AnimatedProperty::from_value(f32::NAN);
        assert!(invalid.to_path::<BezPath>(0.).is_empty());
        invalid.pt = AnimatedProperty::from_value(u16::MAX as f32 + 1.);
        assert!(invalid.to_path::<BezPath>(0.).is_empty());
    }

    #[test] fn trim_path_wraps_across_the_path_end() {
        let mut path = BezPath::new();
        path.move_to((0., 0.)); path.line_to((100., 0.));
        path.trim_path(0.75, 0.5);
        let segments = path.segments().collect::<Vec<_>>();

        assert_eq! (segments.len(), 2);
        assert_eq!((segments[0].start().x, segments[0].end().x), (75., 100.));
        assert_eq!((segments[1].start().x, segments[1].end().x), (0., 25.));
    }

    #[test] fn trim_path_uses_bezier_arc_length_not_parameter_ratio() {
        use kurbo::{CubicBez, Point};
        let curve = CubicBez::new(
            Point::new(0., 0.),   Point::new(0., 200.),
            Point::new(100., 0.), Point::new(100., 0.));
        let mut path = BezPath::new();
        path.move_to(curve.p0); path.curve_to(curve.p1, curve.p2, curve.p3);

        path.trim_path(0., 0.5);
        let actual = path.segments().next().unwrap().end();
        let t = curve.inv_arclen(
            curve.arclen(ACCURACY_TOLERANCE) / 2., ACCURACY_TOLERANCE);
        let expected = curve.eval(t);
        assert!((actual - expected).hypot() < 1e-6);
        assert!((actual - curve.eval(0.5)).hypot() > 1.);
    }

    #[test] fn full_trim_range_preserves_path_closure() {
        let mut path = BezPath::new();
        path.move_to((0., 0.));   path.line_to((10., 0.));
        path.line_to((10., 10.)); path.close_path();
        let storage = path.elements().as_ptr();
        path.trim_path(0., 1.);
        assert_eq!(storage, path.elements().as_ptr());
        assert!(matches!(path.elements().last(), Some(kurbo::PathEl::ClosePath)));
    }
}
