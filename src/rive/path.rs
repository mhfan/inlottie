
//! Rive vertex decoding and backend-neutral path construction.

use std::f32;
use super::{decode::{self, Object, object_ids, property_ids},
    display_list::{CornerRadii, Geometry, Path, PathCommand, Point, Rect},
    runtime::{Result, RuntimeError, boolean, float, uint},
};

#[derive(Debug)] pub(super) enum GeomParams {
    Ellipse   { width: f32, height: f32, origin_x: f32, origin_y: f32 },
    Rectangle { width: f32, height: f32, origin_x: f32, origin_y: f32,
        radii: CornerRadii, linked: bool },
    Parametric { type_id: u32, width: f32, height: f32, origin_x: f32,
        origin_y: f32, points: u16, inner_radius: f32, corner_radius: f32 },
}

impl GeomParams {
    pub fn from_object(object: &Object) -> Result<Option<Self>> {
        let type_id = object.type_id.0;
        if !matches!(type_id, object_ids::ELLIPSE | object_ids::RECTANGLE |
            object_ids::TRIANGLE | object_ids::POLYGON | object_ids::STAR) {
            return Ok(None)
        }
        let (width, height, origin_x, origin_y) = (
            float(object, property_ids::PARAMETRICPATH_WIDTH)?,
            float(object, property_ids::PARAMETRICPATH_HEIGHT)?,
            float(object, property_ids::PARAMETRICPATH_ORIGINX)?,
            float(object, property_ids::PARAMETRICPATH_ORIGINY)?,
        );
        Ok(Some(match type_id {
            object_ids::ELLIPSE => Self::Ellipse { width, height, origin_x, origin_y },
            object_ids::RECTANGLE => Self::Rectangle {
                width, height, origin_x, origin_y,
                radii: rectangle_radii(object)?,
                linked: boolean(object, property_ids::RECTANGLE_LINKCORNERRADIUS)?,
            },
            type_id @ (object_ids::TRIANGLE | object_ids::POLYGON | object_ids::STAR) => {
                let points = uint(object, property_ids::POINTS)?;
                let count = if type_id == object_ids::STAR {
                    points.saturating_mul(2)
                } else { points };
                if u32::from(u16::MAX) < count {
                    return Err(RuntimeError::TooManyVertices(count))
                }
                Self::Parametric { type_id, width, height, origin_x, origin_y,
                    points: points as u16,
                    inner_radius: float(object, property_ids::INNERRADIUS)?,
                    corner_radius: float(object, property_ids::CORNERRADIUS)?,
                }
            }   _ => return Ok(None),
        }))
    }

    /// Returns `None` for unrelated properties and whether geometry changed otherwise.
    pub fn set_float(&mut self, prop_id: u32, value: f32) -> Option<bool> { match self {
            Self::Ellipse   { width, height, origin_x, origin_y } =>
                set_bounds(width, height, origin_x, origin_y, prop_id, value),
            Self::Rectangle { width, height, origin_x, origin_y, radii, linked } => {
                if let changed @ Some(_) =
                   set_bounds(width, height, origin_x, origin_y, prop_id, value) { changed
                } else { match prop_id {
                    property_ids::RECTANGLE_CORNERRADIUSTL => {
                        let changed = radii.tl != value || *linked && (radii.tr != value ||
                                radii.br != value || radii.bl != value);
                        radii.tl = value;
                        if *linked { radii.tr = value; radii.br = value; radii.bl = value }
                        Some(changed)
                    }
                    property_ids::RECTANGLE_CORNERRADIUSTR =>
                        Some(set_value(&mut radii.tr, value)),
                    property_ids::RECTANGLE_CORNERRADIUSBR =>
                        Some(set_value(&mut radii.br, value)),
                    property_ids::RECTANGLE_CORNERRADIUSBL =>
                        Some(set_value(&mut radii.bl, value)),
                    _ => None,
                } }
            }
            Self::Parametric { width, height, origin_x, origin_y,
                inner_radius, corner_radius, .. } => {
                if let changed @ Some(_) =
                    set_bounds(width, height, origin_x, origin_y, prop_id, value) {
                    changed
                } else { match prop_id {
                    property_ids::INNERRADIUS => Some(set_value(inner_radius, value)),
                    property_ids::CORNERRADIUS => Some(set_value(corner_radius, value)),
                    _ => None,
                } }
            }
    } }

    pub fn set_bool(&mut self, prop_id: u32, value: bool) -> Option<bool> {
        let Self::Rectangle { radii, linked, .. } = self else { return None };
        if prop_id != property_ids::RECTANGLE_LINKCORNERRADIUS { return None }
        let changed = *linked != value || value &&
            (radii.tr != radii.tl || radii.br != radii.tl || radii.bl != radii.tl);
        *linked = value;
        if value { radii.tr = radii.tl; radii.br = radii.tl; radii.bl = radii.tl }
        Some(changed)
    }

    pub fn set_uint(&mut self, prop_id: u32, value: u32) -> Option<bool> {
        let Self::Parametric { type_id, points, .. } = self else { return None };
        if prop_id != property_ids::POINTS { return None }
        let count = if *type_id == object_ids::STAR { value.saturating_mul(2)
        } else { value };
        if u32::from(u16::MAX) < count { return Some(false) }
        Some(set_value(points, value as u16))
    }

    pub fn geometry(&self) -> Geometry { match *self {
            Self::Ellipse { width, height, origin_x, origin_y } =>
                Geometry::Ellipse(rect(width, height, origin_x, origin_y)),
            Self::Rectangle { width, height, origin_x, origin_y, radii, .. } =>
                Geometry::RoundedRect {
                    rect: rect(width, height, origin_x, origin_y), radii,
                },
            Self::Parametric { type_id, width, height, origin_x, origin_y,
                points, inner_radius, corner_radius } =>
                Geometry::Path(parametric(type_id,
                    rect(width, height, origin_x, origin_y), points,
                    inner_radius, corner_radius)),
    } }
}

fn set_bounds(width: &mut f32, height: &mut f32, origin_x: &mut f32,
    origin_y: &mut f32, prop_id: u32, value: f32) -> Option<bool> {
    match prop_id {
        property_ids::PARAMETRICPATH_WIDTH => Some(set_value(width, value)),
        property_ids::PARAMETRICPATH_HEIGHT => Some(set_value(height, value)),
        property_ids::PARAMETRICPATH_ORIGINX => Some(set_value(origin_x, value)),
        property_ids::PARAMETRICPATH_ORIGINY => Some(set_value(origin_y, value)),
        _ => None,
    }
}

fn set_value<T: PartialEq>(target: &mut T, value: T) -> bool {
    let changed = *target != value;
    *target = value;   changed
}

fn rect(width: f32, height: f32, origin_x: f32, origin_y: f32) -> Rect {
    Rect { x: -width * origin_x, y: -height * origin_y, w: width, h: height }
}

#[derive(Debug, Clone, Copy)] pub(super) struct Vertex {
    pub position: Point,
    pub incoming: Option<Point>,
    pub outgoing: Option<Point>,
    pub radius: f32,
}

#[derive(Debug)] enum VertexKind {
    Straight { radius: f32 },
    Detached { in_rotation: f32, in_distance: f32,
        out_rotation: f32, out_distance: f32 },
    Asymmetric { rotation: f32, in_distance: f32, out_distance: f32 },
    Mirrored { rotation: f32, distance: f32 },
}

#[derive(Debug)] pub(super) struct VertexParams {
    x: f32, y: f32, kind: VertexKind,
}

impl VertexParams {
    pub fn from_object(object: &Object) -> decode::Result<Option<Self>> {
        let kind = match object.type_id.0 {
            object_ids::STRAIGHT_VERTEX => VertexKind::Straight {
                radius: float(object, property_ids::RADIUS)?,
            },
            object_ids::CUBIC_DETACHED_VERTEX => VertexKind::Detached {
                in_rotation: float(object, property_ids::INROTATION)?,
                in_distance: float(object,
                    property_ids::CUBICDETACHEDVERTEX_INDISTANCE)?,
                out_rotation: float(object, property_ids::OUTROTATION)?,
                out_distance: float(object,
                    property_ids::CUBICDETACHEDVERTEX_OUTDISTANCE)?,
            },
            object_ids::CUBIC_ASYMMETRIC_VERTEX => VertexKind::Asymmetric {
                rotation: float(object,
                    property_ids::CUBICASYMMETRICVERTEX_ROTATION)?,
                in_distance: float(object,
                    property_ids::CUBICASYMMETRICVERTEX_INDISTANCE)?,
                out_distance: float(object,
                    property_ids::CUBICASYMMETRICVERTEX_OUTDISTANCE)?,
            },
            object_ids::CUBIC_MIRRORED_VERTEX => VertexKind::Mirrored {
                rotation: float(object, property_ids::CUBICMIRROREDVERTEX_ROTATION)?,
                distance: float(object, property_ids::CUBICMIRROREDVERTEX_DISTANCE)?,
            },
            _ => return Ok(None),
        };
        Ok(Some(Self { x: float(object, property_ids::VERTEX_X)?,
            y: float(object, property_ids::VERTEX_Y)?, kind }))
    }

    pub fn set(&mut self, prop_id: u32, value: f32) -> Option<bool> {
        match prop_id {
            property_ids::VERTEX_X => return Some(set_value(&mut self.x, value)),
            property_ids::VERTEX_Y => return Some(set_value(&mut self.y, value)),
            _ => {}
        }
        Some(match (&mut self.kind, prop_id) {
            (VertexKind::Straight { radius }, property_ids::RADIUS) =>
                set_value(radius, value),
            (VertexKind::Detached { in_rotation, .. }, property_ids::INROTATION) =>
                set_value(in_rotation, value),
            (VertexKind::Detached { in_distance, .. },
                property_ids::CUBICDETACHEDVERTEX_INDISTANCE) =>
                set_value(in_distance, value),
            (VertexKind::Detached { out_rotation, .. }, property_ids::OUTROTATION) =>
                set_value(out_rotation, value),
            (VertexKind::Detached { out_distance, .. },
                property_ids::CUBICDETACHEDVERTEX_OUTDISTANCE) =>
                set_value(out_distance, value),
            (VertexKind::Asymmetric { rotation, .. },
                property_ids::CUBICASYMMETRICVERTEX_ROTATION) =>
                set_value(rotation, value),
            (VertexKind::Asymmetric { in_distance, .. },
                property_ids::CUBICASYMMETRICVERTEX_INDISTANCE) =>
                set_value(in_distance, value),
            (VertexKind::Asymmetric { out_distance, .. },
                property_ids::CUBICASYMMETRICVERTEX_OUTDISTANCE) =>
                set_value(out_distance, value),
            (VertexKind::Mirrored { rotation, .. },
                property_ids::CUBICMIRROREDVERTEX_ROTATION) =>
                set_value(rotation, value),
            (VertexKind::Mirrored { distance, .. },
                property_ids::CUBICMIRROREDVERTEX_DISTANCE) =>
                set_value(distance, value),
            _ => return None,
        })
    }

    pub fn vertex(&self) -> Vertex {
        let position = Point { x: self.x, y: self.y };
        let control = |rotation: f32, distance: f32| Point {
            x: position.x + rotation.cos() * distance,
            y: position.y + rotation.sin() * distance,
        };
        let (incoming, outgoing, radius) = match self.kind {
            VertexKind::Straight { radius } => (None, None, radius),
            VertexKind::Detached { in_rotation, in_distance,
                out_rotation, out_distance } =>
                (Some(control( in_rotation,  in_distance)),
                 Some(control(out_rotation, out_distance)), 0.0),
            VertexKind::Asymmetric { rotation, in_distance, out_distance } =>
                (Some(control(rotation, -in_distance)),
                 Some(control(rotation, out_distance)), 0.0),
            VertexKind::Mirrored { rotation, distance } =>
                (Some(control(rotation, -distance)),
                 Some(control(rotation,  distance)), 0.0),
        };  Vertex { position, incoming, outgoing, radius }
    }
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

fn parametric(type_id: u32, rect: Rect, points: u16,
    inner_radius: f32, radius: f32) -> Path {
    if type_id == object_ids::TRIANGLE {
        return build_path(&[
            straight_vertex(rect.x + rect.w / 2.0, rect.y, 0.0),
            straight_vertex(rect.x + rect.w, rect.y + rect.h, 0.0),
            straight_vertex(rect.x, rect.y + rect.h, 0.0),
        ], true)
    }
    let vertex_count = usize::from(if type_id == object_ids::STAR {
        points.saturating_mul(2)
    } else { points });
    let (half_width, half_height) = (rect.w / 2.0, rect.h / 2.0);
    let center = Point { x: rect.x + half_width, y: rect.y + half_height };
    let mut vertices = Vec::with_capacity(vertex_count);
    for index in 0..vertex_count {
        let angle = -f32::consts::FRAC_PI_2 +
                     f32::consts::TAU * index as f32 / vertex_count as f32;
        let scale = if type_id == object_ids::STAR && index % 2 == 1 { inner_radius
        } else { 1.0 };
        vertices.push(straight_vertex(
            center.x + angle.cos() * half_width  * scale,
            center.y + angle.sin() * half_height * scale, radius));
    }   build_path(&vertices, true)
}

pub(super) fn straight_vertex(x: f32, y: f32, radius: f32) -> Vertex {
    Vertex { position: Point { x, y }, incoming: None, outgoing: None, radius }
}

pub(super) fn rectangle_radii(object: &Object) -> decode::Result<CornerRadii> {
    let top_left = float(object, property_ids::RECTANGLE_CORNERRADIUSTL)?;
    let linked = boolean(object, property_ids::RECTANGLE_LINKCORNERRADIUS)?;
    let radius = |prop_id| float(object, prop_id);
    Ok(if linked {  CornerRadii { tl: top_left, tr: top_left, br: top_left, bl: top_left }
    } else {        CornerRadii { tl: top_left,
        tr: radius(property_ids::RECTANGLE_CORNERRADIUSTR)?,
        br: radius(property_ids::RECTANGLE_CORNERRADIUSBR)?,
        bl: radius(property_ids::RECTANGLE_CORNERRADIUSBL)?,
    } })
}

#[cfg(test)] mod tests { use super::*;

    fn straight(x: f32, y: f32, radius: f32) -> Vertex {
        Vertex { position: Point { x, y }, incoming: None, outgoing: None, radius }
    }

    #[test] fn updates_all_vertex_parameter_kinds() {
        let mut straight = VertexParams {
            x: 0.0, y: 0.0, kind: VertexKind::Straight { radius: 0.0 } };
        assert_eq!(straight.set(property_ids::RADIUS, 2.0), Some(true));
        assert_eq!(straight.vertex().radius, 2.0);

        let mut detached = VertexParams { x: 1.0, y: 2.0,
            kind: VertexKind::Detached { in_rotation: 0.0, in_distance: 1.0,
                out_rotation: 0.0, out_distance: 1.0 }
        };
        detached.set(property_ids::INROTATION, f32::consts::FRAC_PI_2);
        detached.set(property_ids::CUBICDETACHEDVERTEX_OUTDISTANCE, 3.0);
        let vertex = detached.vertex();
        assert!((vertex.incoming.unwrap().y - 3.0).abs() < 1e-6);
        assert_eq!(vertex.outgoing.unwrap().x, 4.0);

        let mut asymmetric = VertexParams { x: 0.0, y: 0.0,
            kind: VertexKind::Asymmetric {
                rotation: 0.0, in_distance: 1.0, out_distance: 1.0 } };
        asymmetric.set(property_ids::CUBICASYMMETRICVERTEX_ROTATION,
            f32::consts::FRAC_PI_2);
        asymmetric.set(property_ids::CUBICASYMMETRICVERTEX_INDISTANCE, 2.0);
        let vertex = asymmetric.vertex();
        assert!((vertex.incoming.unwrap().y + 2.0).abs() < 1e-6);

        let mut mirrored = VertexParams { x: 0.0, y: 0.0,
            kind: VertexKind::Mirrored { rotation: 0.0, distance: 1.0 } };
        mirrored.set(property_ids::CUBICMIRROREDVERTEX_ROTATION,
            f32::consts::FRAC_PI_2);
        mirrored.set(property_ids::CUBICMIRROREDVERTEX_DISTANCE, 2.0);
        let vertex = mirrored.vertex();
        assert!((vertex.incoming.unwrap().y + 2.0).abs() < 1e-6);
        assert!((vertex.outgoing.unwrap().y - 2.0).abs() < 1e-6);
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

        let endpoints = build_path(&[ straight( 0.0,  0.0, 2.0),
            straight(10.0, 0.0, 0.0), straight(10.0, 10.0, 2.0)], false);
        assert_eq!(&*endpoints.cmd, &[
            PathCommand::MoveTo(Point { x:  0.0, y:  0.0 }),
            PathCommand::LineTo(Point { x: 10.0, y:  0.0 }),
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
