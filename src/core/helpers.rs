
use serde::{de::Error, Deserialize, Deserializer, Serialize, Serializer};

//  https://rust-lang.github.io/api-guidelines/

//  Represents boolean values as an integer. 0 is false, 1 is true.
#[derive(Clone, Copy, Default, PartialEq, Deserialize, Serialize)]
#[serde(transparent)] pub struct IntBool(u8);

impl IntBool { #[inline] pub fn as_bool(&self) -> bool { (*self).into() } }
impl From<IntBool> for bool { #[inline] fn from(value: IntBool) -> Self { value.0 != 0 } }
impl From<bool> for IntBool { fn from(value: bool) -> Self { Self(if value { 1 } else { 0 }) } }

/* #[derive(Clone, Copy)] pub struct Rgb  { pub r: u8, pub g: u8, pub b: u8 }
impl Rgb {  #[inline] pub fn new_u8 (r:  u8, g:  u8, b:  u8) -> Self { Self { r, g, b } }
            #[inline] pub fn new_f32(r: f32, g: f32, b: f32) -> Self { Self {
        r: (r * 255. + 0.5) as _, g: (g * 255. + 0.5) as _, b: (b * 255. + 0.5) as _
    } }
} */

#[derive(Clone, Copy)] pub struct RGBA { pub r: u8, pub g: u8, pub b: u8, pub a: u8 }
impl Default for RGBA { #[inline] fn default() -> Self { Self { r: 0, g: 0, b: 0, a: 255 } } }

impl RGBA {
    #[inline] pub fn new_u8 (r:  u8, g:  u8, b:  u8, a:  u8) -> Self { Self { r, g, b, a } }
    #[inline] pub fn new_f32(r: f32, g: f32, b: f32, a: f32) -> Self { Self {
        r: (r * 255. + 0.5) as _, g: (g * 255. + 0.5) as _,
        b: (b * 255. + 0.5) as _, a: (a * 255. + 0.5) as _
    } }
}

impl<'de> Deserialize<'de> for RGBA {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let v = Vec::<f32>::deserialize(deserializer)?;     // use SmallVec?
        if !(3..=4).contains(&v.len()) {
            return Err(D::Error::invalid_length(v.len(), &"an RGB or RGBA array"));
        }
        Ok(Self::new_f32(v[0], v[1], v[2], v.get(3).cloned().unwrap_or(1.)))
    }
}

impl Serialize for RGBA {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut v = vec![self.r as f32 / 255.,
            self.g as f32 / 255.0, self.b as f32 / 255.];   // use SmallVec?
        if  self.a < 255 {  v.push(self.a as f32 / 255.); }    v.serialize(serializer)
    }
}

impl std::str::FromStr for RGBA { type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if !matches!(s.len(), 7 | 9) {
            return Err("expected #RRGGBB or #RRGGBBAA".to_owned());
        }
        let v = u32::from_str_radix(s.strip_prefix('#')
            .ok_or("not prefixed with '#'".to_owned())?, 16)
            .map_err(|err| err.to_string())?;

        let v = if s.len() == 7 { (v << 8) | 0xff } else { v };
        Ok(Self::new_u8((v >> 24) as _, ((v >> 16) & 0xff) as _,
                       ((v >>  8) & 0xff) as _, (v & 0xff) as _))
    }
}

impl core::fmt::Display for RGBA {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, r"#{:02x}{:02x}{:02x}", self.r, self.g, self.b)?;
        if self.a < 255 { write!(f, r"{:02x}", self.a)?; }  Ok(())
    }
}

#[inline] pub(crate) fn str_to_rgba<'de, D: Deserializer<'de>>(deserializer: D) ->
    Result<RGBA, D::Error> { String::deserialize(deserializer)?.parse().map_err(D::Error::custom) }

#[inline] pub(crate) fn str_from_rgba<S: Serializer>(c: &RGBA, serializer: S) ->
    Result<S::Ok, S::Error> { serializer.serialize_str(&c.to_string()) }

#[derive(Clone, Copy)] pub struct Vec2D { pub x: f32, pub y: f32 }
//impl From<Vec2D> for (f32, f32) { fn from(val: Vec2D) -> Self { (val.x, val.y) } }
impl From<(f32, f32)> for Vec2D {   // for Point/Size/Position/Scale
    #[inline] fn from((x, y): (f32, f32)) -> Self { Self { x, y } }
}

impl<'de> Deserialize<'de> for Vec2D {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let v = Vec::<f32>::deserialize(deserializer)?;     // use SmallVec?
        if !(1..=3).contains(&v.len()) {
            return Err(D::Error::invalid_length(v.len(), &"a 1- to 3-element vector"));
        }
        Ok(Self { x: v[0], y: v.get(1).cloned().unwrap_or(0.) })
    }
}

impl Serialize for Vec2D {
    #[inline] fn serialize<S: Serializer>(&self, serializer: S) ->
        Result<S::Ok, S::Error> { [self.x, self.y].serialize(serializer) }
}

#[derive(Clone)] pub struct ColorList(pub Vec<(f32, RGBA)>); // (offset, color) for Gradient

impl  core::ops::Deref for ColorList {  type Target = [(f32, RGBA)];
    #[inline] fn deref(&self) -> &Self::Target { &self.0 }
}

impl<'de> Deserialize<'de> for ColorList {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let data = Vec::<f32>::deserialize(deserializer)?;
        let len = data.len();   let cnt = len / 6;  // XXX:

        let cnt = if len.is_multiple_of(6) && !(len.is_multiple_of(4) && (0..cnt)
            .any(|i| data[i * 4] != data[cnt * 4 + i * 2])) { cnt } else { len / 4 };

        Ok(Self(if len == cnt * 4 { // RGB color
            data.chunks(4).map(|chunk| (chunk[0],
                RGBA::new_f32(chunk[1], chunk[2], chunk[3], 1.))).collect()
        } else  if len == cnt * (4 + 2) {   let cnt = cnt * 4;  // RGBA color
            data[0..cnt].chunks(4).zip(data[cnt..].chunks(2))
                .map(|(chunk, opacity)| (chunk[0], // == opacity[0]
                RGBA::new_f32(chunk[1], chunk[2], chunk[3], opacity[1]))).collect()
        } else {    // issue_1732.json
            eprintln!("Inconsistent ColorList: {cnt} * 4 != {}", data.len());
            data.chunks_exact(4).map(|chunk| (chunk[0],
                RGBA::new_f32(chunk[1], chunk[2], chunk[3], 1.))).collect()
        }))
    }
}

impl Serialize for ColorList {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut data = self.0.iter().flat_map(|&(offset, color)|
            [offset, color.r as f32 / 255., color.g as f32 / 255.,
                     color.b as f32 / 255.]).collect::<Vec<_>>();

        if  self.0.iter().any(|&(_, color)| color.a < 255) {
            data.extend(self.0.iter().flat_map(|&(offset, color)|
                [offset, color.a as f32 / 255.]));
        }   data.serialize(serializer)
    }
}

use crate::core::schema::*;

pub(crate) mod defaults { #![allow(unused)]
    pub fn time_stretch() -> f32 { 1.0 }
    pub fn animation_wh() -> u32 { 512 }
    pub fn animation_fr() -> f32 { 60. }
    pub fn animation_vs() -> String { "5.5.2".to_owned() }

    pub fn effect_en() -> super::IntBool { true.into() }
    pub fn precomp_op() -> f32 { 99999. }

    pub fn font_size()  -> f32 { 10. }
    //pub fn font_family() -> String { "sans".to_owned() }
    //pub fn font_style()  -> String { "Regular".to_owned() }
    //pub fn font_name()   -> String { "sans-Regular".to_owned() }

    use super::{Value, Animated2D};
    pub fn opacity() -> Value { Value::from_value(100.) }
    pub fn animated2d() -> Animated2D { Animated2D::from_value((100., 100.).into()) }

    #[inline] pub fn is_default<T: Default + PartialEq>(v: &T) -> bool { *v == T::default() }
}

pub use crate::core::schema_impl::{AnyAsset, AnyValue, UnresolvedSlot};

#[cfg(test)] mod test { use super::*;
    #[test] fn rgba_text_roundtrip_and_validation() {
        let color = RGBA::new_u8(1, 2, 15, 16);
        assert_eq!(color.to_string(), "#01020f10");
        let parsed: RGBA = color.to_string().parse().unwrap();
        assert_eq!((parsed.r, parsed.g, parsed.b, parsed.a), (1, 2, 15, 16));
        assert!("#123".parse::<RGBA>().is_err());
    }

    #[test] fn vector_deserialization_rejects_invalid_lengths() {
        assert!(serde_json::from_str::<RGBA>("[]").is_err());
        assert!(serde_json::from_str::<RGBA>("[1, 1]").is_err());
        assert!(serde_json::from_str::<Vec2D>("[]").is_err());
        assert!(serde_json::from_str::<Vec2D>("[1, 2, 3, 4]").is_err());
    }

    #[test] fn rgba_lerp_handles_decreasing_channels() {
        let from = RGBA::new_u8(255, 200, 100, 50);
        let to = RGBA::new_u8(0, 100, 50, 0);
        let color = math::Tween::lerp(&from, &to, 0.5);
        assert_eq!((color.r, color.g, color.b, color.a), (127, 150, 75, 25));
    }
}

pub const ACCURACY_TOLERANCE: f64 = 1e-2;

pub mod math {  use super::*;

/** Fast arctangent approximations by iterative algorithms, the coordinated rotation
    digital computer (**CORDIC**) algorithm (requiring only shifts and add operations).

 - https://geekshavefeelings.com/posts/fixed-point-atan2
 - https://github.com/quartiq/idsp/blob/main/src/atan2.rs
 - https://www-labs.iro.umontreal.ca/~mignotte/IFT2425/Documents/EfficientApproximationArctgFunction.pdf
 - https://ieeexplore.ieee.org/book/6241055
 - https://en.wikipedia.org/wiki/Fast_inverse_square_root
 - https://en.wikipedia.org/wiki/Methods_of_computing_square_roots
 - https://en.wikipedia.org/wiki/CORDIC, https://en.wikipedia.org/wiki/Atan2
 - https://math.stackexchange.com/questions/1098487/atan2-faster-approximation

```
    use core::f32::consts::PI;
    use inlottie::core::helpers::math::fast_atan2;
    assert_eq!(fast_atan2( 0.,  0.),   0.);

    assert_eq!(fast_atan2( 0.,  1.),   0.);
    assert_eq!(fast_atan2( 0., -1.),   PI);
    assert_eq!(fast_atan2( 1.,  0.),   PI / 2.);
    assert_eq!(fast_atan2(-1.,  0.),  -PI / 2.);

    assert!  ((fast_atan2( 1.,  1.) -  PI / 4.).abs() < f32::EPSILON);
    assert!  ((fast_atan2(-1.,  1.) - -PI / 4.).abs() < f32::EPSILON);
    assert_eq!(fast_atan2(-1., -1.),  -PI * 3. / 4.);
    assert_eq!(fast_atan2( 1., -1.),   PI * 3. / 4.);

    [(1., 2.), (-1., 2.), (1., -2.), (-1., -2.), (2., 1.), (-2., 1.), (2., -1.), (-2., -1.)]
    .into_iter().for_each(|(x, y)| assert!((fast_atan2(y, x) - y.atan2(x)).abs() < 0.0038));
``` */
pub fn fast_atan2(y: f32, x: f32) -> f32 {  use core::f32::consts::PI;
    if x == 0. { return if 0. < y { PI / 2. } else if y < 0. { -PI / 2. } else { 0. } }
    else if y == 0. { return if 0. < x { 0. } else { PI } }

    let flag = y.abs() < x.abs();
    let slope = if flag { y / x } else { x / y };   // valid range: [-1, 1]
    let hatan = (PI / 4. + 0.273 - 0.273 * slope.abs()) * slope; // max error ~0.0038
        //(PI / 4. + 0.2447 - (0.2447 - 0.0663 + 0.0663 * slope.abs()) * slope.abs()) * slope;
        // http://nghiaho.com/?p=997, max error ~0.0015, 3x faster than standard C atan

    //if 1. < slope { PI / 2. - hatan } else if slope < -1. { -PI / 2. - hatan } else { hatan }
    if flag { hatan + if 0. < x { 0. } else if 0. < y { PI } else { -PI }
    } else { (if 0. < y { PI / 2. } else { -PI / 2. }) - hatan }
}

use core::ops::{Div, Mul, Add, Sub, Neg};
impl Div<f32> for Vec2D {  type Output =  Self;
    #[inline] fn div(self, scale: f32) -> Self {
        Self { x: self.x / scale, y: self.y / scale }
    }
}
impl Mul<f32> for Vec2D {  type Output =  Self;
    #[inline] fn mul(self, scale: f32) -> Self {
        Self { x: self.x * scale, y: self.y * scale }
    }
}
impl Add<f32> for Vec2D {  type Output =   Self;
    #[inline] fn add(self, offset: f32) -> Self {
        Self { x: self.x + offset, y: self.y + offset }
    }
}
impl Sub<f32> for Vec2D {  type Output =   Self;
    #[inline] fn sub(self, offset: f32) -> Self {
        Self { x: self.x - offset, y: self.y - offset }
    }
}
impl Add for Vec2D {  type Output = Self;
    #[inline] fn add(self, rhs: Self) -> Self {
        Self { x: self.x + rhs.x, y: self.y + rhs.y }
    }
}
impl Sub for Vec2D {  type Output = Self;
    #[inline] fn sub(self, rhs: Self) -> Self {
        Self { x: self.x - rhs.x, y: self.y - rhs.y }
    }
}
impl Neg for Vec2D {  type Output = Self;
    #[inline] fn neg(self) -> Self { Self { x: -self.x, y: -self.y } }
}
impl Vec2D {
    #[inline] pub fn from_polar(angle: f32) -> Self { Self { x: angle.cos(), y: angle.sin() } }
}

/** https://github.com/hannesmann/keyframe, https://github.com/gre/bezier-easing,
    https://github.com/hlhr202/bezier-easing-rs
```
    use inlottie::core::helpers::math::CubicBezierEasing;
    let easing = CubicBezierEasing::new((0., 0.), (1., 0.5));
    assert_eq!(easing.curve(0.5), 0.3125);
    assert_eq!(easing.get_y(0.5), 0.3125);
    assert_eq!(easing.get_y(1.), 1.);
    assert_eq!(easing.get_y(0.), 0.);

    let easing = CubicBezierEasing::from((0.6, 0.1, 0.9, 0.4));
    assert!((easing.get_y(0.7)  - easing.curve(0.7)).abs() < f32::EPSILON);
    assert_eq!(easing.get_y(0.3), easing.curve(0.3));
    assert!(easing.get_y(0.2) > easing.get_y(0.1));
    assert!(easing.get_y(0.8) > easing.get_y(0.2));
``` */
pub struct CubicBezierEasing { p1: (f32, f32), p2: (f32, f32), }

impl CubicBezierEasing {    // https://pomax.github.io/bezierinfo
    // B(t) = p0 * (1 - t)^3 + p1 * 3 * (1 - t)^2 * t + p2 * 3 * (1 - t) * t^2 + p3 * t^3
    // x(t) =  a * (1 - t)^3 +  b * 3 * (1 - t)^2 * t +  c * 3 * (1 - t) * t^2 +  d * t^3
    //      = (3b - 3c + d - a) * t^3 + (3a - 6b + 3c) * t^2 + (3b - 3a) * t + a
    //
    // Regarding easing curve: a = x0 = 0., b = x1, c = x2, d = x3 = 1., so:
    //  A = 3b - 3c + d - a  = 3 * x1 - 3 * x2 + 1.
    //  B = 3a - 6b + 3c     = 3 * x2 - 6 * x1
    //  C = 3b - 3a          = 3 * x1
    #[inline] fn a(x1: f32, x2: f32) -> f32 { 3.0 * x1 - 3.0 * x2 + 1.0 }
    #[inline] fn b(x1: f32, x2: f32) -> f32 { 3.0 * x2 - 6.0 * x1 }
    #[inline] fn c(x1: f32) -> f32 { 3.0 * x1 }

    fn at(t: f32, x1: f32, x2: f32) -> f32 {
        ((Self::a(x1, x2) * t + Self::b(x1, x2)) * t + Self::c(x1)) * t
    }
    fn slope(t: f32, x1: f32, x2: f32) -> f32 {   // derivative
        3.0 * Self::a(x1, x2) * t * t + 2.0 * Self::b(x1, x2) * t + Self::c(x1)
    }

    fn calc_t(x: f32, x1: f32, x2: f32) -> f32 {    // Newton-Raphson iteration
        let mut guess_t = x;    for _ in 0..5 {
            let current_slope = Self::slope(guess_t, x1, x2);
            if  current_slope < f32::EPSILON { break }
            let delta = (Self::at(guess_t, x1, x2) - x) / current_slope;
            guess_t -= delta;   //if delta.abs() < 1e-5 { break }
        }   guess_t
    }

    /* fn binary_subdivide(x: f32, mut a: f32, mut b: f32, x1: f32, x2: f32) -> f32 {
        let (mut current_x, mut current_t) = (0.0f32, 0.);
        let (mut has_run_once, mut i) = (false, 0);
        while !has_run_once || 0.0000001 < current_x.abs() && i + 1 < 10 {
            current_t = a + (b - a) / 2.0;  has_run_once = true;
            current_x = Self::at(current_t, x1, x2) - x;
            if current_x > 0.0 { b = current_t; } else { a = current_t; }   i += 1;
        }   current_t
    } */

    pub fn get_y(&self, x: f32) -> f32 {
        if x == 0. || x == 1. { x } else {
            //if self.p1.0 == self.p1.1 && self.p2.0 == self.p2.1 { return x }
            Self::at(Self::calc_t(x, self.p1.0, self.p2.0), self.p1.1, self.p2.1)
        }
    }
    #[inline] pub fn new(p1: (f32, f32), p2: (f32, f32)) -> Self { Self { p1, p2 } }

    pub fn linear()      -> Self { Self::new((0.00, 0.0), (1.00, 1.0)) }
    pub fn ease()        -> Self { Self::new((0.25, 0.1), (0.25, 1.0)) }
    pub fn ease_in()     -> Self { Self::new((0.42, 0.0), (1.00, 1.0)) }
    pub fn ease_out()    -> Self { Self::new((0.00, 0.0), (0.58, 1.0)) }
    pub fn ease_in_out() -> Self { Self::new((0.42, 0.0), (0.58, 1.0)) }

    pub fn curve(&self, x: f32) -> f32 {
        let cp1 = (self.p1.0 as _, self.p1.1 as _);
        let cp2 = (self.p2.0 as _, self.p2.1 as _);
        /* use flo_curves::{bezier::Curve, BezierCurve, BezierCurveFactory, Coord2};
        let curve = Curve::from_points( (0., 0.).into(), (Coord2::from(cp1),
            Coord2::from(cp2)), (1., 1.).into());
        let intersect = curve_intersects_line(&curve,
            &((x as _, 0.).into(), ((x as _, 1.).into())));
        return if intersect.is_empty() { 0. } else { intersect[0].2.1 as _ }; */

        use kurbo::{CubicBez, ParamCurve, PathSeg};
        let sline = kurbo::Line::new((x as _, 0.), (x as _, 1.));
        let curve = CubicBez::new((0., 0.), cp1, cp2, (1., 1.));
        let intersect = PathSeg::Cubic(curve).intersect_line(sline);
        if  intersect.is_empty() { 0. } else { sline.eval(intersect[0].line_t).y as _ }
                                             //curve.eval(intersect[0].segment_t).y as _
    }
}

impl From<(f32, f32, f32, f32)> for CubicBezierEasing {
    #[inline] fn from(cp: (f32, f32, f32, f32)) -> Self { Self::new((cp.0, cp.1), (cp.2, cp.3)) }
}

/*  https://www.w3.org/TR/css-easing-1/#cubic-bezier-easing-functions
    http://robertpenner.com/easing/, https://lib.rs/keywords/easing,
    https://github.com/orhanbalci/rust-easing, https://github.com/sanbox-irl/tween */
impl From<[f32; 4]> for CubicBezierEasing {
    #[inline] fn from(cp: [f32; 4]) -> Self { Self::new((cp[0], cp[1]), (cp[2], cp[3])) }
}

pub trait Tween { fn lerp(&self, other: &Self, t: f32) -> Self; // Linear intERPolation
    fn bezc(&self, _: &Self, _: f32, _: &PositionExtra) -> Self
        where Self: Sized { unreachable!() }    // Cubic Bezier interpolation
}

impl Tween for f32 {
    #[inline] fn lerp(&self, other: &Self, t: f32) -> Self { self + (other - self) * t }
    //#[inline] fn lerp(&self, other: &Self, t: f32) -> Self { self * (1. - t) + other * t }
}

impl Tween for Vec2D {
    fn lerp(&self, other: &Self, t: f32) -> Self {
        Self {  x: self.x + (other.x - self.x) * t,
                y: self.y + (other.y - self.y) * t, }
    }

    fn bezc(&self, other: &Self, t: f32, extra: &PositionExtra) -> Self {
        /* impl From<&Vec2D> for Coord2 {
            #[inline] fn from(val: &Vec2D) -> Self { Self { x: val.x as _, y: val.y as _ } }
        }   use flo_curves::{bezier::Curve, BezierCurve, BezierCurveFactory, Coord2};
        let bezier = Curve::from_points((*self).into(), ((*self + extra.to).into(),
            (*other + extra.ti).into()), (*other).into());

        let (mut tmin, mut tmax) = (0., 1.);
        let tlen = bezier.estimate_length() * t as f64;
        while ACCURACY_TOLERANCE < tmax - tmin {
            let tmid = (tmin + tmax) / 2.;
            if bezier.subdivide(tmid).1.estimate_length() < tlen {
                tmin = tmid; } else { tmax = tmid; }
        }   let pt = bezier.point_at_pos((tmin + tmax) / 2.); */

        #[allow(non_local_definitions)] impl From<Vec2D> for Point {
            #[inline] fn from(val: Vec2D) -> Self { (val.x, val.y).into() }
        }   use kurbo::{CubicBez, ParamCurve, ParamCurveArclen, Point};
        let curve = CubicBez::new::<Point>((*self).into(), (*self + extra.to).into(),
            (*other + extra.ti).into(), (*other).into());

        let (mut tmin, mut tmax) = (0., 1.);
        let tlen = curve.arclen(ACCURACY_TOLERANCE) * t as f64;
        while ACCURACY_TOLERANCE < tmax - tmin {
            let tmid = (tmin + tmax) / 2.;
            if curve.subsegment(0.0..tmid).arclen(ACCURACY_TOLERANCE) < tlen {
                tmin = tmid; } else { tmax = tmid; }
        }   let pt = curve.eval((tmin + tmax) / 2.);
        //let pt = curve.eval(curve.inv_arclen(tlen, ACCURACY_TOLERANCE));

        Self { x: pt.x as _, y: pt.y as _ } //(pt.x as _, pt.y as _).into()
    }
}

impl Tween for RGBA {
    fn lerp(&self, other: &Self, t: f32) -> Self {
        let channel = |from: u8, to: u8| (from as f32 + (to as f32 - from as f32) * t) as u8;
        Self {  r: channel(self.r, other.r),
                g: channel(self.g, other.g),
                b: channel(self.b, other.b),
                a: channel(self.a, other.a),
        }
    }
}

impl Tween for Bezier {
    fn lerp(&self, other: &Self, t: f32) -> Self {
        let closure =
            |val: (&Vec2D, &Vec2D)| val.0.lerp(val.1, t);
        Self { closed: self.closed,
            vp: self.vp.iter().zip(other.vp.iter()).map(closure).collect(),
            it: self.it.iter().zip(other.it.iter()).map(closure).collect(),
            ot: self.ot.iter().zip(other.ot.iter()).map(closure).collect(),
        }
    }
}

/* impl Tween for Vec<Bezier> {
    fn lerp(&self, other: &Self, t: f32) -> Self {
        self.iter().zip(other.iter())
            .map(|val| val.0.lerp(val.1, t)).collect()
    }
} */

impl Tween for ColorList {
    fn lerp(&self, other: &Self, t: f32) -> Self {
        Self(self.0.iter().zip(other.0.iter()).map(|(first, second)|
            (first.0 + (second.0 - first.0) * t, first.1.lerp(&second.1, t))).collect())
    }
}

impl Tween for Vec<f32> {   // aka MultiD
    fn lerp(&self, other: &Self, t: f32) -> Self {
        self.iter().zip(other.iter()).map(|val| //val.0.lerp(val.1, t)
            *val.0 + (*val.1 - *val.0) * t).collect()
    }
}

}
