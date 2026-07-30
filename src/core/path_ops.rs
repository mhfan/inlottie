//! Internal contour modifiers and measured-path operations.

use super::{helpers::ACCURACY_TOLERANCE, schema::LineJoin};
use kurbo::{BezPath, ParamCurve, ParamCurveArclen};

pub(super) fn for_each_contour(path: &BezPath,
    mut f: impl FnMut(&[kurbo::PathEl], bool)) {
    use kurbo::PathEl::*;
    let (elements, mut start) = (path.elements(), None);
    for (index, element) in elements.iter().enumerate() { match element {
        MoveTo(_) => {
            if let Some(from) = start.replace(index) { f(&elements[from..index], false) }
        }
        ClosePath => {
            if let Some(from) = start.take() { f(&elements[from..=index], true) }
        }   _ => {}
    }}  if let Some(from) = start { f(&elements[from..], false) }
}

pub(super) fn round_contour(elements: &[kurbo::PathEl], closed: bool, radius: f64,
    points: &mut Vec<kurbo::Point>, output: &mut BezPath) {
    use kurbo::PathEl::*;
    points.clear();
    let mut straight = true;
    for &element in elements { match element {
        MoveTo(point) | LineTo(point) => points.push(point),
        QuadTo(control, point) => {
            straight &= points.last().is_some_and(|&start|
                control == start || control == point);
            points.push(point);
        }
        CurveTo(first, second, point) => {
            straight &= points.last().is_some_and(|&start|
                first == start && second == point);
            points.push(point);
        }
        ClosePath => {}
    }}
    if !straight  { output.extend(elements.iter().copied()); return }
    if closed && points.len() > 1 && points.last() == points.first() { points.pop(); }
    let count = points.len();
    if count < 2 { output.extend(elements.iter().copied()); return }

    let corner = |index: usize| {
        let current = points[index];
        let (incoming, outgoing) = (points[(index + count - 1) % count] - current,
            points[(index + 1) % count] - current);
        let (incoming_len, outgoing_len) = (incoming.hypot(), outgoing.hypot());
        let distance = radius.min(incoming_len * 0.5).min(outgoing_len * 0.5);
        if distance == 0. { (current, current) } else {
            (current + incoming * (distance / incoming_len),
             current + outgoing * (distance / outgoing_len))
        }
    };

    if closed {
        output.move_to(corner(0).1);
        for index in 1..=count {
            let index = index % count;
            let (entry, exit) = corner(index);
            output.line_to(entry); output.quad_to(points[index], exit);
        }   output.close_path();
    } else {
        output.move_to(points[0]);
        for index in 1..count - 1 {
            let (entry, exit) = corner(index);
            output.line_to(entry); output.quad_to(points[index], exit);
        }   output.line_to(points[count - 1]);
    }
}

pub(super) fn flatten_contour(elements: &[kurbo::PathEl], points: &mut Vec<kurbo::Point>) {
    fn flatten(segment: kurbo::PathSeg, points: &mut Vec<kurbo::Point>, depth: u8) {
        let (start, end, middle) =
            (segment.eval(0.), segment.eval(1.), segment.eval(0.5));
        let chord = end - start;
        let distance = if chord.hypot2() == 0. { (middle - start).hypot()
        } else { chord.cross(middle - start).abs() / chord.hypot() };
        if  distance <= ACCURACY_TOLERANCE || depth == 12 { points.push(end); } else {
            flatten(segment.subsegment(0. .. 0.5), points, depth + 1);
            flatten(segment.subsegment(0.5 .. 1.), points, depth + 1);
        }
    }

    let mut current = None;
    for &element in elements { match element {
        MoveTo(point) => { points.push(point); current = Some(point); }
        LineTo(point) => { points.push(point); current = Some(point); }
        QuadTo(control, point) => {
            if let Some(start) = current {
                flatten(kurbo::QuadBez::new(start, control, point).into(), points, 0);
            }   current = Some(point);
        }
        CurveTo(first, second, point) => {
            if let Some(start) = current {
                flatten(kurbo::CubicBez::new(start, first, second, point).into(), points, 0);
            }   current = Some(point);
        }
        ClosePath => {}
    } } use kurbo::PathEl::*;
}

pub(super) fn offset_contour(points: &mut Vec<kurbo::Point>, closed: bool,
    amount: f64, join: LineJoin, miter_limit: f64, output: &mut BezPath) {
    points.dedup();     use kurbo::{Point, Vec2};
    if closed && points.len() > 1 && points.last() == points.first() { points.pop(); }
    if points.len() < 2 { return }

    let count = points.len();
    let signed_area = if closed {
        (0..count).map(|index| {
            let (a, b) = (points[index], points[(index + 1) % count]);
            a.x * b.y - b.x * a.y
        }).sum()
    } else { 0. };
    let distance = if closed && 0. < signed_area { -amount } else { amount };
    let direction = |from: Point, to: Point| (to - from).normalize();
    let normal = |dir: Vec2| Vec2::new(-dir.y, dir.x) * distance;
    let join_points = |index: usize| {
        let (previous, point, next) = (points[(index + count - 1) % count],
            points[index], points[(index + 1) % count]);
        let (before, after) = (direction(previous, point), direction(point, next));
        (point, before, after, point + normal(before), point + normal(after))
    };
    let valid_miter = |point: Point, first: Point, before: Vec2,
        second: Point, after: Vec2| {
        let cross = before.cross(after);
        (cross.abs() > 1e-9).then(||
            first + before * ((second - first).cross(after) / cross))
            .filter(|miter|
                (*miter - point).hypot() <= distance.abs() * miter_limit.max(1.))
    };

    let append_join = |path: &mut BezPath, index: usize| {
        let (point, before, after, first, second) = join_points(index);
        match join {
            LineJoin::Miter => {
                if let Some(miter) = valid_miter(point, first, before, second, after) {
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
                kurbo::Arc::new(point, (distance.abs(), distance.abs()), start, sweep, 0.)
                    .to_cubic_beziers(ACCURACY_TOLERANCE,
                        |first, second, end| path.curve_to(first, second, end));
            }
        }
    };

    if closed {
        let (point, before, after, first, second) = join_points(0);
        output.move_to(if matches!(join, LineJoin::Miter) {
            valid_miter(point, first, before, second, after).unwrap_or(first)
        } else { first });
        for index in 0..count { append_join(output, index); }
        output.close_path();
    } else {
        output.move_to(points[0] + normal(direction(points[0], points[1])));
        for index in 1..count - 1 { append_join(output, index); }
        output.line_to(points[count - 1] +
            normal(direction(points[count - 2], points[count - 1])));
    }
}

pub(crate) struct MeasuredPath {
    path: BezPath, pub length: f64,
    segments: Vec<(kurbo::PathSeg, f64)>,
}

impl MeasuredPath {
    pub fn new(path: BezPath) -> Self {
        let mut segments = Vec::with_capacity(path.elements().len().saturating_sub(1));
        let mut length = 0.;
        for seg in path.segments() {
            let len = seg.arclen(ACCURACY_TOLERANCE);
            segments.push((seg, len)); length += len;
        }   Self { path, segments, length }
    }

    pub fn trim_ranges(&self, ranges: &[(f64, f64)]) -> BezPath {
        if ranges.len() == 1 && ranges[0].0 <= 0. && 1. <= ranges[0].1 {
            return self.path.clone()
        }
        if self.length == 0. { return BezPath::new() }
        let mut output = Vec::with_capacity(self.segments.len() * ranges.len());

        for &(from, to) in ranges {
            let (from, to, mut offset) = (from.clamp(0., 1.) * self.length,
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
