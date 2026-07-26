
//! Backend-neutral, immutable drawing data emitted by the retained Rive runtime.

use std::sync::Arc;

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Point { pub x: f32, pub y: f32 }

#[derive(Debug, Clone, Copy, PartialEq)] pub enum PathCommand {
    MoveTo(Point), LineTo(Point), Close,
    CubicTo { ctrl1: Point, ctrl2: Point, to: Point },
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Path { pub cmd: Arc<[PathCommand]> }

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rect { pub x: f32, pub y: f32, pub w: f32, pub h: f32 }

#[derive(Debug, Clone, Copy, PartialEq)] pub struct CornerRadii {
    pub tl: f32, pub tr: f32, pub br: f32, pub bl: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)] pub struct Affine2 {
    pub xx: f32, pub yx: f32, pub xy: f32,
    pub yy: f32, pub tx: f32, pub ty: f32,
}

impl Default for Affine2 {
    fn default() -> Self { Self { xx: 1.0, yx: 0.0, xy: 0.0, yy: 1.0, tx: 0.0, ty: 0.0 } }
}

impl Affine2 {
    pub fn from_transform(x: f32, y: f32, rotation: f32,
        scale_x: f32, scale_y: f32) -> Self {
        let (sin, cos) = rotation.sin_cos();
        Self {  xx:  cos * scale_x, yx: sin * scale_x,
                xy: -sin * scale_y, yy: cos * scale_y, tx: x, ty: y,
        }
    }

    /// Compose transforms so `self.then(local)` applies `local` before `self`.
    pub fn then(self, local: Self) -> Self { Self {
            xx: self.xx * local.xx + self.xy * local.yx,
            yx: self.yx * local.xx + self.yy * local.yx,
            xy: self.xx * local.xy + self.xy * local.yy,
            yy: self.yx * local.xy + self.yy * local.yy,
            tx: self.xx * local.tx + self.xy * local.ty + self.tx,
            ty: self.yx * local.tx + self.yy * local.ty + self.ty,
    } }

    pub fn transform_point(self, point: Point) -> Point { Point {
            x: self.xx * point.x + self.xy * point.y + self.tx,
            y: self.yx * point.x + self.yy * point.y + self.ty,
    } }
}

#[derive(Debug, Clone, PartialEq)] pub enum Geometry {
    RoundedRect { rect: Rect, radii: CornerRadii },
    Path(Path), Ellipse(Rect),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FillRule { NonZero, EvenOdd, Clockwise }

#[derive(Debug, Clone, Copy, PartialEq, Eq)] pub enum StrokeCap { Butt, Round, Square }

#[derive(Debug, Clone, Copy, PartialEq, Eq)] pub enum StrokeJoin { Miter, Round, Bevel }

#[derive(Debug, Clone, Copy, PartialEq, Eq)] pub enum TrimMode { Sequential, Synchronized }

#[derive(Debug, Clone, Copy, PartialEq)] pub struct DashSegment {
    pub len: f32, pub relative: bool,
}

#[derive(Debug, Clone, PartialEq)] pub enum PathEffect {
    Trim { start: f32, end: f32, offset: f32, mode: TrimMode },
    Dash { offset: f32, relative: bool, segments: Arc<[DashSegment]> },
}

#[derive(Debug, Clone, Copy, PartialEq)] pub struct GradientStop {
    pub pos: f32, pub color: u32,
}

#[derive(Debug, Clone, PartialEq)] pub enum Brush { Solid(u32),
    LinearGradient {  start: Point,  end: Point, trfm: Affine2,
        opacity: f32, stops: Arc<[GradientStop]> },
    RadialGradient { center: Point, radius: f32, trfm: Affine2,
        opacity: f32, stops: Arc<[GradientStop]> },
}

#[derive(Debug, Clone, PartialEq)] pub enum Paint {
    Stroke { brush: Brush, width: f32, cap: StrokeCap, join: StrokeJoin,
        trfm_scale: bool, effects: Arc<[PathEffect]> },
    Fill   { brush: Brush, rule: FillRule, effects: Arc<[PathEffect]> },
}

#[derive(Debug, Clone, PartialEq)] pub struct Shape {
    /// Source object index, retained for diagnostics and future hit testing.
    pub obj_idx: u32,
    pub is_hole: bool,
    pub trfm: Affine2,
    pub geom: Geometry,
}

#[derive(Debug, Clone, PartialEq)] pub struct DrawItem {
    /// One paint application over a combined, ordered set of shape contours.
    pub obj_idx: u32, pub opacity: f32, pub paint: Option<Paint>,
    pub shapes: Arc<[Shape]>,
}

#[derive(Debug, Clone, Default, PartialEq)]
/// A frame snapshot; backends may consume it after the Runtime advances again.
pub struct DisplayList { pub items: Vec<DrawItem> }

impl DisplayList {
    pub fn clear(&mut self) { self.items.clear() }
    pub fn iter(&self) -> impl ExactSizeIterator<Item = &DrawItem> {
        self.items.iter()
    }
}
