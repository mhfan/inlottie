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

pub(super) fn flatten_contour(elements: &[kurbo::PathEl],
    tolerance: f64, points: &mut Vec<kurbo::Point>) {
    fn flatten(segment: kurbo::PathSeg, tolerance: f64,
        points: &mut Vec<kurbo::Point>, depth: u8) {
        let (start, end, middle) =
            (segment.eval(0.), segment.eval(1.), segment.eval(0.5));
        let chord = end - start;
        let distance = if chord.hypot2() == 0. { (middle - start).hypot()
        } else { chord.cross(middle - start).abs() / chord.hypot() };
        if  distance <= tolerance || depth == 12 { points.push(end); } else {
            flatten(segment.subsegment(0. .. 0.5), tolerance, points, depth + 1);
            flatten(segment.subsegment(0.5 .. 1.), tolerance, points, depth + 1);
        }
    }

    let mut current = None;
    for &element in elements { match element {
        MoveTo(point) => { points.push(point); current = Some(point); }
        LineTo(point) => { points.push(point); current = Some(point); }
        QuadTo(control, point) => {
            if let Some(start) = current {
                flatten(kurbo::QuadBez::new(start, control, point).into(),
                    tolerance, points, 0);
            }   current = Some(point);
        }
        CurveTo(first, second, point) => {
            if let Some(start) = current {
                flatten(kurbo::CubicBez::new(start, first, second, point).into(),
                    tolerance, points, 0);
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
    path: BezPath, tolerance: f64, pub length: f64,
    segments: Vec<(kurbo::PathSeg, f64)>,
    contours: Vec<(std::ops::Range<usize>, f64, bool)>,
}

impl MeasuredPath {
    pub fn new(path: BezPath, tolerance: f64) -> Self {
        assert!(tolerance.is_finite() && 0. < tolerance,
            "path measurement tolerance must be finite and positive");
        let mut segments = Vec::with_capacity(path.elements().len().saturating_sub(1));
        let (mut length, mut contours) = (0., Vec::new());
        for_each_contour(&path, |elements, closed| {
            let (start, mut contour_length) = (segments.len(), 0.);
            for seg in kurbo::segments(elements.iter().copied()) {
                let len = seg.arclen(tolerance);
                segments.push((seg, len));
                contour_length += len;
            }
            contours.push((start..segments.len(), contour_length, closed));
            length += contour_length;
        });
        Self { path, tolerance, length, segments, contours }
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
                    let range = seg.inv_arclen(lo - offset, self.tolerance)
                             .. seg.inv_arclen(hi - offset, self.tolerance);
                    output.push(seg.subsegment(range));
                }   offset = next;
            }
        }   BezPath::from_path_segments(output.into_iter())
    }

    /// Apply a dash pattern using the segment lengths already measured by this path.
    pub fn dash(&self, offset: f64, pattern: &[f64]) -> BezPath {
        let period: f64 = pattern.iter().sum();
        if  pattern.is_empty() ||     !period.is_finite() || period <= 0. ||
            pattern.iter().any(|value| !value.is_finite() || *value < 0.) {
            return BezPath::new()
        }
        let mut output = BezPath::new();
        for (segments, length, closed) in &self.contours {
            if *length == 0. { continue }
            let (mut index, mut remaining, mut active) =
                (0usize, pattern[0] - offset.rem_euclid(period), true);
            while remaining < 0. {
                index = (index + 1) % pattern.len();
                remaining += pattern[index]; active = !active;
            }

            let (mut cursor, mut ranges) = (0., Vec::<(f64, f64)>::new());
            while cursor < *length {
                if remaining <= 0. {
                    index = (index + 1) % pattern.len();
                    remaining = pattern[index]; active = !active;
                    continue
                }
                let step = remaining.min(*length - cursor);
                if active && 0. < step {
                    if let Some(last) = ranges.last_mut().filter(|last| last.1 == cursor) {
                        last.1 += step;
                    } else {
                        ranges.push((cursor, cursor + step));
                    }
                }
                cursor += step; remaining -= step;
            }
            if ranges.is_empty() { continue }
            if *closed && ranges.len() == 1 && ranges[0] == (0., *length) {
                append_range(&mut output, &self.segments[segments.clone()],
                    0., *length, self.tolerance, false);
                output.close_path();    continue
            }

            // A closed contour whose first and last dashes are active has one dash crossing
            // the seam. Emit the tail and head as one connected subpath.
            let wraps = *closed && ranges.len() > 1 &&
                ranges.first().is_some_and(|range| range.0 == 0.) &&
                ranges. last().is_some_and(|range| range.1 == *length);
            if wraps {
                let tail = ranges.pop().unwrap();
                let head = ranges.remove(0);
                append_range(&mut output, &self.segments[segments.clone()],
                    tail.0, tail.1, self.tolerance, false);
                append_range(&mut output, &self.segments[segments.clone()],
                    head.0, head.1, self.tolerance, true);
            }
            for (from, to) in ranges {
                append_range(&mut output, &self.segments[segments.clone()],
                    from, to, self.tolerance, false);
            }
        }   output
    }
}

fn append_range(output: &mut BezPath, segments: &[(kurbo::PathSeg, f64)],
    from: f64, to: f64, tolerance: f64, connect: bool) {
    let (mut offset, mut first) = (0., true);
    for &(segment, length) in segments {
        let next = offset + length;
        let (lo, hi) = (from.max(offset), to.min(next));
        if lo < hi && 0. < length {
            let range = segment.inv_arclen(lo - offset, tolerance)
                     .. segment.inv_arclen(hi - offset, tolerance);
            let segment = segment.subsegment(range);
            if first && !connect { output.move_to(segment.start()); }
            match segment {
                kurbo::PathSeg::Line(line) => output.line_to(line.p1),
                kurbo::PathSeg::Quad(quad) => output.quad_to(quad.p1, quad.p2),
                kurbo::PathSeg::Cubic(cubic) =>
                    output.curve_to(cubic.p1, cubic.p2, cubic.p3),
            }
            first = false;
        }   offset = next;
    }
}

#[cfg(test)] mod tests { use super::*;
    const TOLERANCE: f64 = 1e-6;

    #[test] fn measured_dash_matches_kurbo_for_open_curves() {
        let (mut path, pattern) = (BezPath::new(), [13., 7., 3., 5.]);
        path.move_to((0., 0.)); path.curve_to((20., 40.), (80., -40.), (100., 0.));
        let expected: BezPath = kurbo::dash(path.iter(), 9., &pattern).collect();
        let actual = MeasuredPath::new(path, TOLERANCE).dash(9., &pattern);
        assert_eq!(actual.segments().count(), expected.segments().count());
        let length = |path: &BezPath| path.segments()
            .map(|segment| segment.arclen(TOLERANCE)).sum::<f64>();
        assert!((length(&actual) - length(&expected)).abs() < 1e-5);
    }

    #[test] fn measured_dash_resets_phase_for_each_contour() {
        let mut path = BezPath::new();
        path.move_to((0., 0.));   path.line_to((20., 0.));
        path.move_to((100., 0.)); path.line_to((120., 0.));
        let dashed = MeasuredPath::new(path, TOLERANCE).dash(3., &[8., 4.]);
        let starts = dashed.elements().iter().filter_map(|element| match element {
            kurbo::PathEl::MoveTo(point) => Some(point.x),
            _ => None,
        }).collect::<Vec<_>>();
        assert_eq!(starts, [0., 9., 100., 109.]);
    }

    #[test] fn measured_dash_joins_a_closed_contour_across_its_seam() {
        let mut path = BezPath::new();
        path.move_to((0., 0.));   path.line_to((10., 0.));
        path.line_to((10., 10.)); path.line_to((0., 10.)); path.close_path();
        let dashed = MeasuredPath::new(path, TOLERANCE).dash(3., &[8., 4.]);
        let moves = dashed.elements().iter()
            .filter(|element| matches!(element, kurbo::PathEl::MoveTo(_))).count();
        assert_eq!(moves, 3);
        assert!(dashed.elements().iter()
            .all(|element| !matches!(element, kurbo::PathEl::ClosePath)));

        let unbroken = MeasuredPath::new(dashed, TOLERANCE);
        assert!(unbroken.length > 0.);
    }

    #[test] fn measured_dash_preserves_an_unbroken_closed_contour() {
        let mut path = BezPath::new();
        path.move_to((0., 0.)); path.line_to((10., 0.));
        path.line_to((10., 10.)); path.close_path();
        let dashed = MeasuredPath::new(path.clone(), TOLERANCE).dash(0., &[100., 1.]);
        assert!(matches!(dashed.elements().last(), Some(kurbo::PathEl::ClosePath)));
        let length = |path: &BezPath| path.segments()
            .map(|segment| segment.arclen(TOLERANCE)).sum::<f64>();
        assert_eq!(length(&dashed), length(&path));
    }
}
