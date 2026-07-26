
//! Rive vertex decoding and backend-neutral path construction.

use std::f32;

use super::{decode::{self, Object, object_ids, property_ids},
    display_list::{CornerRadii, Path, PathCommand, Point, Rect},
    runtime::{Result, RuntimeError, boolean, float, uint},
};

#[derive(Clone, Copy)] pub(super) struct Vertex {
    pub position: Point,
    pub incoming: Option<Point>,
    pub outgoing: Option<Point>,
    pub radius: f32,
}

pub(super) fn vertex(object: &Object) -> decode::Result<Option<Vertex>> {
    // Rive stores cubic controls as polar offsets; normalize all vertex variants to points.
    let point = || -> decode::Result<Point> { Ok(Point {
        x: float(object, property_ids::VERTEX_X)?,
        y: float(object, property_ids::VERTEX_Y)?,
    }) };
    let control = |position: Point, rotation: f32, distance: f32| Point {
        x: position.x + rotation.cos() * distance,
        y: position.y + rotation.sin() * distance,
    };
    let (position, incoming, outgoing, radius) = match object.type_id.0 {
        object_ids::STRAIGHT_VERTEX =>
            (point()?, None, None, float(object, property_ids::RADIUS)?),
        object_ids::CUBIC_DETACHED_VERTEX => {
            let position = point()?;
            let incoming = control(position,
                float(object, property_ids::INROTATION)?,
                float(object, property_ids::CUBICDETACHEDVERTEX_INDISTANCE)?);
            let outgoing = control(position,
                float(object, property_ids::OUTROTATION)?,
                float(object, property_ids::CUBICDETACHEDVERTEX_OUTDISTANCE)?);
            (position, Some(incoming), Some(outgoing), 0.0)
        }
        object_ids::CUBIC_ASYMMETRIC_VERTEX => {
            let position = point()?;
            let rotation = float(object, property_ids::CUBICASYMMETRICVERTEX_ROTATION)?;
            let incoming = control(position, rotation,
                -float(object, property_ids::CUBICASYMMETRICVERTEX_INDISTANCE)?);
            let outgoing = control(position, rotation,
                 float(object, property_ids::CUBICASYMMETRICVERTEX_OUTDISTANCE)?);
            (position, Some(incoming), Some(outgoing), 0.0)
        }
        object_ids::CUBIC_MIRRORED_VERTEX => {
            let position = point()?;
            let rotation = float(object, property_ids::CUBICMIRROREDVERTEX_ROTATION)?;
            let distance = float(object, property_ids::CUBICMIRROREDVERTEX_DISTANCE)?;
            (position, Some(control(position, rotation, -distance)),
                       Some(control(position, rotation,  distance)), 0.0)
        }   _ => return Ok(None),
    };  Ok(Some(Vertex { position, incoming, outgoing, radius }))
}

pub(super) fn build_path(vertices: &[Vertex], closed: bool) -> Path {
    if vertices.len() < 2 { return Path::default() }
    let rendered: Vec<_> = vertices.iter().enumerate()
        .map(|(index, &vertex)| rounded_vertex(vertices, index, closed, vertex)).collect();
    let first = rendered[0];
    let mut commands = Vec::with_capacity(vertices.len() * 2 + usize::from(closed));
    commands.push(PathCommand::MoveTo(first.entry));
    push_corner(&mut commands, first);
    for pair in rendered.windows(2) {
        push_segment(&mut commands, pair[0], pair[1]);
        push_corner (&mut commands, pair[1]);
    }
    if closed {
        let last = *rendered.last().unwrap();
        if  last.outgoing.is_some() || first.incoming.is_some() {
            push_segment(&mut commands, last, first);
        }   commands.push(PathCommand::Close);
    }   Path { cmd: commands.into() }
}

#[derive(Clone, Copy)] struct RenderVertex {
    entry: Point, exit: Point,
    incoming: Option<Point>, outgoing: Option<Point>,
    corner: Option<(Point, Point)>,
}

fn rounded_vertex(vertices: &[Vertex], index: usize, closed: bool,
    vertex: Vertex) -> RenderVertex {
    // A rounded vertex expands into entry/exit points plus one cubic corner. Open endpoints
    // remain unchanged because they do not have two adjacent segments.
    let plain = || RenderVertex { entry: vertex.position, exit: vertex.position,
        incoming: vertex.incoming, outgoing: vertex.outgoing, corner: None };
    if vertex.radius == 0.0 || (!closed && (index == 0 || index + 1 == vertices.len())) {
        return plain()
    }

    let prev = vertices[(index + vertices.len() - 1) % vertices.len()];
    let next = vertices[(index + 1) % vertices.len()];
    let mut to_prev = offset(prev.outgoing.unwrap_or(prev.position), vertex.position);
    let mut to_next = offset(next.incoming.unwrap_or(next.position), vertex.position);
    let (prev_len, next_len) = (normalize(&mut to_prev), normalize(&mut to_next));
    if prev_len == 0.0 || next_len == 0.0 { return plain() }

    let radius = vertex.radius.abs().min(prev_len / 2.0).min(next_len / 2.0);
    let ideal = ideal_control_distance(to_prev, to_next, radius);
    let entry = add_scaled(vertex.position, to_prev, radius);
    let exit  = add_scaled(vertex.position, to_next, radius);
    let mut ctrl1 = add_scaled(vertex.position, to_prev, radius - ideal);
    let mut ctrl2 = add_scaled(vertex.position, to_next, radius - ideal);
    if vertex.radius < 0.0 {
        rotate_corner(exit, entry, vertex.position, &mut ctrl1, &mut ctrl2);
    }
    RenderVertex { entry, exit, incoming: None, outgoing: None, corner: Some((ctrl1, ctrl2)) }
}

fn push_segment(commands: &mut Vec<PathCommand>, from: RenderVertex, to: RenderVertex) {
    if from.outgoing.is_some() || to.incoming.is_some() {
        commands.push(PathCommand::CubicTo {
            ctrl1: from.outgoing.unwrap_or(from.exit),
            ctrl2:   to.incoming.unwrap_or(to.entry), to: to.entry,
        });
    } else {
        commands.push(PathCommand::LineTo(to.entry));
    }
}

fn push_corner(commands: &mut Vec<PathCommand>, vertex: RenderVertex) {
    if let Some((ctrl1, ctrl2)) = vertex.corner {
        commands.push(PathCommand::CubicTo { ctrl1, ctrl2, to: vertex.exit });
    }
}

fn offset(point: Point, origin: Point) -> Point {
    Point { x: point.x - origin.x, y: point.y - origin.y }
}

fn add_scaled(point: Point, vector: Point, scale: f32) -> Point {
    Point { x: point.x + vector.x * scale, y: point.y + vector.y * scale }
}

fn normalize(vector: &mut Point) -> f32 {
    let length = vector.x.hypot(vector.y);
    if  length != 0.0 {
        vector.x /= length;
        vector.y /= length;
    }   length
}

fn ideal_control_distance(prev: Point, next: Point, radius: f32) -> f32 {
    // Cubic approximation of the circular arc subtended by the adjacent normalized edges.
    let angle = (prev.x * next.y - prev.y * next.x)
          .atan2(prev.x * next.x + prev.y * next.y).abs();
    4.0 / 3.0 * (angle / 4.0).tan() * radius *
    if angle < f32::consts::FRAC_PI_2 { 1.0 + angle.cos() } else { 2.0 - angle.sin() }
}

fn rotate_corner(next: Point, prev: Point, point: Point,
    outgoing: &mut Point, incoming: &mut Point) {
    let (v1, v2) = (offset(prev, next), offset(point, next));
    let angle = (v1.x * v2.y - v1.y * v2.x).atan2(v1.x * v2.x + v1.y * v2.y);
    *outgoing = rotate_around(*outgoing, prev,  angle * 2.0);
    *incoming = rotate_around(*incoming, next, -angle * 2.0);
}

fn rotate_around(point: Point, origin: Point, angle: f32) -> Point {
    let (sin, cos) = angle.sin_cos();
    let point = offset(point, origin);
    Point { x: point.x * cos - point.y * sin + origin.x,
            y: point.x * sin + point.y * cos + origin.y }
}

pub(super) fn bounds(object: &Object) -> decode::Result<Rect> {
    let width  = float(object, property_ids::PARAMETRICPATH_WIDTH)?;
    let height = float(object, property_ids::PARAMETRICPATH_HEIGHT)?;
    let origin_x = float(object, property_ids::PARAMETRICPATH_ORIGINX)?;
    let origin_y = float(object, property_ids::PARAMETRICPATH_ORIGINY)?;
    Ok(Rect { x: -width * origin_x, y: -height * origin_y, w: width, h: height, })
}

pub(super) fn parametric_path(object: &Object) -> Result<Path> {
    // TODO: Animated procedural parameters must rebuild or cache geometry per frame.
    let rect = bounds(object)?;
    if object.type_id.0 == object_ids::TRIANGLE {
        return Ok(build_path(&[
            straight_vertex(rect.x + rect.w / 2.0, rect.y, 0.0),
            straight_vertex(rect.x + rect.w, rect.y + rect.h, 0.0),
            straight_vertex(rect.x, rect.y + rect.h, 0.0),
        ], true))
    }

    let points = uint(object, property_ids::POINTS)?;
    let (vertex_count, inner_radius) = if object.type_id.0 == object_ids::STAR {
        (points.saturating_mul(2), float(object, property_ids::INNERRADIUS)?)
    } else { (points, 1.0) };
    if u32::from(u16::MAX) < vertex_count {
        return Err(RuntimeError::TooManyVertices(vertex_count))
    }
    let vertex_count = vertex_count as usize;
    let (half_width, half_height) = (rect.w / 2.0, rect.h / 2.0);
    let center = Point { x: rect.x + half_width, y: rect.y + half_height };
    let radius = float(object, property_ids::CORNERRADIUS)?;
    let mut vertices = Vec::with_capacity(vertex_count);
    for index in 0..vertex_count {
        let angle = -f32::consts::FRAC_PI_2 +
                     f32::consts::TAU * index as f32 / vertex_count as f32;
        let scale = if object.type_id.0 == object_ids::STAR && index % 2 == 1 {
            inner_radius
        } else { 1.0 };
        vertices.push(straight_vertex(
            center.x + angle.cos() * half_width  * scale,
            center.y + angle.sin() * half_height * scale, radius));
    }
    Ok(build_path(&vertices, true))
}

pub(super) fn straight_vertex(x: f32, y: f32, radius: f32) -> Vertex {
    Vertex { position: Point { x, y }, incoming: None, outgoing: None, radius }
}

pub(super) fn rectangle_radii(object: &Object) -> decode::Result<CornerRadii> {
    let top_left = float(object, property_ids::RECTANGLE_CORNERRADIUSTL)?;
    let linked = boolean(object, property_ids::RECTANGLE_LINKCORNERRADIUS)?;
    let radius = |prop_id| float(object, prop_id);
    Ok(if linked { CornerRadii { tl: top_left, tr: top_left,
            br: top_left, bl: top_left,
    } } else { CornerRadii { tl: top_left,
               tr: radius(property_ids::RECTANGLE_CORNERRADIUSTR)?,
            br: radius(property_ids::RECTANGLE_CORNERRADIUSBR)?,
            bl:  radius(property_ids::RECTANGLE_CORNERRADIUSBL)?,
    } })
}

#[cfg(test)] mod tests { use super::*;

    fn straight(x: f32, y: f32, radius: f32) -> Vertex {
        Vertex { position: Point { x, y }, incoming: None, outgoing: None, radius }
    }

    #[test] fn rounds_interior_straight_vertex() {
        let path = build_path(&[straight(0.0, 0.0, 0.0),
            straight(10.0, 0.0, 2.0), straight(10.0, 10.0, 0.0)], false);
        assert_eq!(path.cmd[1], PathCommand::LineTo(Point { x: 8.0, y: 0.0 }));
        let PathCommand::CubicTo { ctrl1, ctrl2, to } = path.cmd[2] else { panic!() };
        assert!((ctrl1.x - 9.104_569).abs() < 1e-5 && ctrl1.y == 0.0);
        assert!(ctrl2.x == 10.0 && (ctrl2.y - 0.895_431).abs() < 1e-5);
        assert_eq!(to, Point { x: 10.0, y: 2.0 });
    }

    #[test] fn clamps_radius_and_leaves_open_endpoints_square() {
        let rounded = build_path(&[straight(0.0, 0.0, 0.0),
            straight(10.0, 0.0, 100.0), straight(10.0, 10.0, 0.0)], false);
        assert_eq!(rounded.cmd[1], PathCommand::LineTo(Point { x: 5.0, y: 0.0 }));
        let PathCommand::CubicTo { to, .. } = rounded.cmd[2] else { panic!() };
        assert_eq!(to, Point { x: 10.0, y: 5.0 });

        let endpoints = build_path(&[straight(0.0, 0.0, 2.0),
            straight(10.0, 0.0, 0.0), straight(10.0, 10.0, 2.0)], false);
        assert_eq!(&*endpoints.cmd, &[
            PathCommand::MoveTo(Point { x: 0.0, y: 0.0 }),
            PathCommand::LineTo(Point { x: 10.0, y: 0.0 }),
            PathCommand::LineTo(Point { x: 10.0, y: 10.0 }),
        ]);
    }

    #[test] fn negative_radius_reverses_corner_controls() {
        let vertices = |radius| [straight(0.0, 0.0, 0.0),
            straight(10.0, 0.0, radius), straight(10.0, 10.0, 0.0)];
        let (positive, negative) = (build_path(&vertices(2.0), false),
            build_path(&vertices(-2.0), false));
        let (PathCommand::CubicTo { ctrl1: pos1, ctrl2: pos2, .. },
             PathCommand::CubicTo { ctrl1: neg1, ctrl2: neg2, .. }) =
            (positive.cmd[2], negative.cmd[2]) else { panic!() };
        assert_ne!((pos1, pos2), (neg1, neg2));
        assert!(neg1.x.is_finite() && neg1.y.is_finite() &&
                neg2.x.is_finite() && neg2.y.is_finite());
    }
}
