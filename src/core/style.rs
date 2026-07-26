/****************************************************************
 * $ID: style.rs  	Tue 18 Nov 2025 15:30:11+0800               *
 *                                                              *
 * Maintainer: 范美辉 (MeiHui FAN) <mhfan@ustc.edu>              *
 * Copyright (c) 2025 M.H.Fan, All rights reserved.             *
 ****************************************************************/

use crate::core::{helpers::{Vec2D, RGBA, IntBool, math},
    schema::{Transform, Translation, TransRotation,
        FillStrokeGrad, ColorGrad, FillStroke, FillRule, GradientType, GradientColors,
        Repeater, Composite, LineJoin, LineCap, StrokeDashType}
};

impl MatrixConv for kurbo::Affine {
    /*  | a c e |          Affine::Mul (self * other)
        | b d f |
        | 0 0 1 | */
    fn identity() -> Self { Self::IDENTITY }
    fn rotate(&mut self, angle: f32) { *self = self.then_rotate(angle as _) }
    fn translate(&mut self, pos: Vec2D) { *self = Self::translate(pos) * *self }
    fn skew_x(&mut self, sk: f32) { *self = Self::skew(sk.tan() as _, 0.) * *self }
    fn scale(&mut self, sl: Vec2D) {      // Affine didn't do tan() inside
        *self = self.then_scale_non_uniform(sl.x as _, sl.y as _)
    }
    fn premul(&mut self, tm: &Self) { *self *= *tm }
}

#[cfg(feature = "vello")] impl StyleConv for peniko::Brush {
    fn solid_color(color: RGBA) -> Self { Self::Solid(color.into()) }
    fn linear_gradient(sp: Vec2D, ep: Vec2D, stops: &[(f32, RGBA)]) -> Self {
        Self::Gradient(peniko::Gradient::new_linear(
            (sp.x, sp.y), (ep.x, ep.y)).with_stops(VelloStops(stops)))
    }
    fn radial_gradient(cp: Vec2D, fp: Vec2D, radii: (f32, f32),
            stops: &[(f32, RGBA)]) -> Self {
        Self::Gradient(peniko::Gradient::new_two_point_radial(
            (cp.x, cp.y), radii.0, (fp.x, fp.y), radii.1)
            .with_stops(VelloStops(stops)))
    }
}
#[cfg(feature = "vello")] struct VelloStops<'a>(&'a [(f32, RGBA)]);
#[cfg(feature = "vello")] impl peniko::ColorStopsSource for VelloStops<'_> {
    fn collect_stops(self, target: &mut peniko::ColorStops) {
        target.extend(self.0.iter().map(|&(offset, color)|
            (offset, DynamicColor::from_alpha_color(color.into())).into()));
    }
}
#[cfg(feature = "vello")] use vello::peniko::{self, color::DynamicColor};
#[cfg(feature = "vello")] impl From<RGBA> for peniko::Color {
    fn from(color: RGBA) -> Self {
        Self::from_rgba8(color.r, color.g, color.b, color.a)
    }
}

/** ```
//#[cfg(test)] mod tests { use super::*;
//    #[test] fn test_matrix_transform() {
        let  a = [1.0, 2.0, 3.0, 4.0, 5.0, 6.0];
        let  b = [7.0, 8.0, 9.0, 10.0, 11.0, 12.0];
        let ab = [31.0, 46.0, 39.0, 58.0, 52.0, 76.0];

        fn operate_matrix(t: &mut impl MatrixConv) {
            t.translate(-Vec2D::from((1., 2.))); t.scale((3., 4.).into());
            t.rotate(-0.5); t.skew_x(-0.6); t.rotate(0.5);
            t.rotate(-0.7); t.translate((8., 9.).into());
        }   use kurbo::Affine;

        assert_eq!(Affine::new(a) * Affine::new(b), Affine::new(ab));
        let mut t1 =     Affine::identity(); operate_matrix(&mut t1);
        use inlottie::core::{style::MatrixConv, helpers::Vec2D};

        let to_f32 = |x: f64| x as f32;     // Into::into
        let mut t = TM2D(a.map(to_f32));    t.premultiply(&TM2D(b.map(to_f32)));
        assert_eq!(t, TM2D(ab.map(to_f32)));    use femtovg::Transform2D as TM2D;

        let mut t2 =       TM2D::identity(); operate_matrix(&mut t2);
        assert!(t1.as_coeffs().iter().zip(t2.0.iter())
            .all(|(&v1, &v2)| (v1 - v2 as f64).abs() < 1e-5));

        #[cfg(feature = "b2d")] {
            use intvg::blend2d::BLMatrix2D;
            let mut t  = BLMatrix2D::new(a);    t.transform(&BLMatrix2D::new(b));
            assert_eq!(t.get_values(), BLMatrix2D::new(ab).get_values());

            let mut t3 = BLMatrix2D::identity(); operate_matrix(&mut t3);
            println!("{t1:?}\n{t2:?}\nBLMatrix2D{:?}", t3.get_values());
            assert!(t1.as_coeffs().iter().zip(t3.get_values().iter())
                .all(|(&v1, &v2)| (v1 - v2).abs() < 1e-5)); // XXX: f64::EPSILON
        }
//    }
//}
 ``` */
pub trait MatrixConv {
    fn identity() -> Self;
    fn premul(&mut self, tm: &Self);
    fn rotate(&mut self, angle: f32);
    fn translate(&mut self, pos: Vec2D);
    fn skew_x(&mut self, sk: f32);
    fn scale(&mut self, sl: Vec2D);
}

#[derive(Clone)] pub struct TM2DwO<MC: MatrixConv>(pub MC, pub f32);
impl<MC: MatrixConv> Default for TM2DwO<MC> {
    fn default() -> Self { Self(MC::identity(), 1.) }
}
impl<MC: MatrixConv> TM2DwO<MC> {
    pub fn compose(mut self, other: &Self) -> Self {
        self.0.premul(&other.0);    self.1 *= other.1;  self
    }
    pub fn compose_matrix(mut self, other: &MC) -> Self {
        self.0.premul(other);  self
    }
}

impl Transform {
    /// https://lottie.github.io/lottie-spec/latest/single-page/#specs-helpers-transform
    ///
    /// Multiplications are RIGHT multiplications (Next = Previous * StepOperation).
    ///
    /// If your transform is transposed (`tx`, `ty` are on the last column),
    /// perform LEFT multiplication instead. Perform the following operations on a
    /// matrix starting from the identity matrix (or the parent object's transform matrix):
    pub fn to_matrix<MC: MatrixConv>(&self, fnth: f32, ao: IntBool) -> TM2DwO<MC> {
        let opacity = self.opacity.as_ref().map_or(1.,
            |o| o.get_value(fnth) / 100.); // FIXME: for canvas global?

        let mut trfm = MC::identity();
        if  let Some(anchor) = &self.anchor {
            trfm.translate(-anchor.get_value(fnth));
        }

        if  let Some(scale) = &self.scale {
            let scale = scale.get_value(fnth) / 100.;
            //if scale.x == 0. { scale.x = f32::EPSILON; } // workaround for some lottie file?
            //if scale.y == 0. { scale.y = f32::EPSILON; }
            trfm.scale(scale);
        }

        if  let Some(skew) = &self.skew {
            let axis = self.skew_axis.as_ref()
                .map(|axis| axis.get_value(fnth).to_radians());
            if let Some(axis) = axis { trfm.rotate(-axis); }

            let skew = -skew.get_value(fnth).clamp(-85., 85.);  // SKEW_LIMIT
            trfm.skew_x(skew.to_radians());     // do tan() inside

            if let Some(axis) = axis { trfm.rotate( axis); }
        }

        match &self.extra {
            TransRotation::Normal2D { rotation: Some(rdeg) } =>
                trfm.rotate(rdeg.get_value(fnth).to_radians()),
            TransRotation::Split3D(_) => unimplemented!(), //debug_assert!(ddd),
            _ => (),
        }

        match &self.position {
            Some(Translation::Normal(apos)) => {
                let pos  = apos.get_value(fnth);
                if  ao.as_bool() && apos.is_animated() {
                    let orient = pos - apos.get_value(fnth - 1.);
                    trfm.rotate(math::fast_atan2(orient.y, orient.x));
                }   trfm.translate(pos);
            }

            Some(Translation::Split(sv)) => {   debug_assert!(sv.split);
                let pos = Vec2D { x: sv.x.get_value(fnth), y: sv.y.get_value(fnth) };
                if  ao.as_bool() {
                    let orient = pos -
                        Vec2D { x: sv.x.get_value(fnth - 1.), y: sv.y.get_value(fnth - 1.) };
                    trfm.rotate(math::fast_atan2(orient.y, orient.x));
                }   trfm.translate(pos);
                if sv.z.is_some() { unimplemented!(); }
            }   _ => (),
        }   TM2DwO(trfm, opacity)
    }
}

impl Repeater {
    pub fn get_matrix<MC: MatrixConv>(&self, fnth: f32) -> Vec<TM2DwO<MC>> {
        let copies = self.cnt.get_value(fnth);
        if !copies.is_finite() || copies <= 0. { return Vec::new() }
        // lottie-web creates ceil(copies) full instances; a fractional final copy
        // does not receive an additional coverage-opacity multiplier.
        let cnt = copies.ceil().min(10000.) as u32;
        let mut coll = Vec::with_capacity(cnt as usize);

        let start_opacity = self.tr.so.as_ref().map_or(1., |so| so.get_value(fnth) / 100.);
        let opacity_delta = if 1 < cnt {
            (self.tr.eo.as_ref().map_or(1., |eo| eo.get_value(fnth) / 100.)
                - start_opacity) / (cnt - 1) as f32
        } else { 0. };
        let  offset = self.offset.as_ref().map_or(0.,
            |offset| offset.get_value(fnth));
        let  offset = if offset.is_finite() { offset } else { 0. };

        let trfm = &self.tr.trfm;
        let  anchor = trfm.anchor.as_ref().map_or(Vec2D { x: 0., y: 0. },
            |anchor| anchor.get_value(fnth));
        let scale = trfm.scale.as_ref()
            .map(|scale| scale.get_value(fnth) / 100.);

        let rot = match &trfm.extra {
            TransRotation::Normal2D { rotation } =>
                rotation.as_ref().map(|rdeg| rdeg.get_value(fnth).to_radians()),
            TransRotation::Split3D(_) => unimplemented!(), //debug_assert!(ddd),
        };

        let pos = match &trfm.position {
            Some(Translation::Normal(apos)) => apos.get_value(fnth),
            Some(Translation::Split(sv)) => {   debug_assert!(sv.split);
                Vec2D { x: sv.x.get_value(fnth), y: sv.y.get_value(fnth) }
            }   _ => Vec2D { x: 0., y: 0. },
        };
        let skew = trfm.skew.as_ref().map(|skew|
            -skew.get_value(fnth).clamp(-85., 85.).to_radians());
        let skew_axis = trfm.skew_axis.as_ref()
            .map(|axis| axis.get_value(fnth).to_radians());

        for i in 0..cnt {
            let copy = if matches!(self.order, Composite::Below) { i } else { cnt - 1 - i };
            let amount = offset + copy as f32;
            let mut trfm = MC::identity();

            trfm.translate(-anchor);
            if let Some(scale) = scale {
                trfm.scale(Vec2D {
                    x: repeater_scale(scale.x, amount),
                    y: repeater_scale(scale.y, amount),
                });
            }
            if let Some(skew) = skew {
                if let Some(axis) = skew_axis { trfm.rotate(-axis); }
                trfm.skew_x(skew * amount);
                if let Some(axis) = skew_axis { trfm.rotate(axis); }
            }
            if let Some(rot) = rot { trfm.rotate(rot * amount); }
            trfm.translate(pos * amount + anchor);

            coll.push(TM2DwO(trfm, start_opacity + opacity_delta * copy as f32));
        }   coll
    }
}

fn repeater_scale(scale: f32, amount: f32) -> f32 {
    let whole = amount.trunc() as i32;
    let fraction = amount.fract().abs();
    let partial = 1. + (scale - 1.) * fraction;
    if amount < 0. { scale.powi(whole) / partial } else { scale.powi(whole) * partial }
}

pub trait StyleConv {
    fn solid_color(color: RGBA) -> Self;
    fn linear_gradient(sp: Vec2D, ep: Vec2D, stops: &[(f32, RGBA)]) -> Self;
    fn radial_gradient(cp: Vec2D, fp: Vec2D, radii: (f32, f32),
        stops: &[(f32, RGBA)]) -> Self;
}

pub enum FSOpts {   Fill(FillRule),     // XXX: use SmallVec for dash?
    Stroke { width: f32, limit: f32, join: LineJoin, cap: LineCap, dash: (f32, Vec<f32>) }
}

impl GradientColors {
    fn resolve(&self, fnth: f32, opacity: f32) -> Vec<(f32, RGBA)> {
        let data = self.cl.get_value_cow(fnth);
        let color_count = self.cnt as usize;
        let color_len = color_count * 4;
        let alpha = &data[color_len..];
        let alpha_count = alpha.len() / 2;
        let (mut colors, mut alphas) = (0, 0);
        let mut stops = Vec::with_capacity(color_count + alpha_count);

        while colors < color_count || alphas < alpha_count {
            let color_offset = (colors < color_count).then(||  data[colors * 4]);
            let alpha_offset = (alphas < alpha_count).then(|| alpha[alphas * 2]);
            let offset = match (color_offset, alpha_offset) {
                (Some(color), Some(alpha)) => color.min(alpha),
                (Some(color), None) => color,
                (None, Some(alpha)) => alpha,
                (None, None) => unreachable!(),
            };
            if color_offset == Some(offset) { colors += 1; }
            if alpha_offset == Some(offset) { alphas += 1; }
            if stops.last().is_none_or(|&(last, _)| last != offset) {
                let (lo, hi, factor) = gradient_segment(&data[..color_len], 4, offset);
                let lerp = |channel: usize| {
                    let first =  data[lo * 4 + channel];
                        first + (data[hi * 4 + channel] - first) * factor
                };
                let alpha = if alpha_count == 0 { 1. } else {
                    let (lo, hi, factor) = gradient_segment(alpha, 2, offset);
                    let first =  alpha[lo * 2 + 1];
                        first + (alpha[hi * 2 + 1] - first) * factor
                };
                stops.push((offset, RGBA::new_f32(
                    lerp(1), lerp(2), lerp(3), alpha * opacity)));
            }
        }   stops
    }
}

fn gradient_segment(data: &[f32], stride: usize, offset: f32) -> (usize, usize, f32) {
    let count = data.len() / stride;
    let (mut lower, mut upper) = (0, count);
    while lower < upper {
        let middle = lower + (upper - lower) / 2;
        if data[middle * stride] <= offset { lower = middle + 1 } else { upper = middle }
    }
    if upper == 0 { return (0, 0, 0.) }
    if upper == count { return (count - 1, count - 1, 0.) }
    let lower = upper - 1;
    let span = data[upper * stride] - data[lower * stride];
    (lower, upper, if span == 0. { 0. } else {
        (offset - data[lower * stride]) / span
    })
}

impl FillStrokeGrad {
    pub fn to_style<SC: StyleConv>(&self, fnth: f32) -> (SC, FSOpts) {
        let opacity = self.opacity.get_value(fnth) / 100.;
        let style = match &self.grad {
            ColorGrad::Color { color } => {
                let mut rgba = color.get_value(fnth);  // RGB indeed
                rgba.a = (opacity * 255.) as _;     SC::solid_color(rgba)
            }
            ColorGrad::Gradient(grad) => {
                let (sp, ep) = (grad.sp.get_value(fnth), grad.ep.get_value(fnth));
                let stops = grad.stops.resolve(fnth, opacity);

                if matches!(grad.r#type, GradientType::Radial) {
                    let (dx, dy) = (ep.x - sp.x, ep.y - sp.y);
                    let radius = dx.hypot(dy);

                    let hl = grad.hl.as_ref().map_or(0., |hl|
                        hl.get_value(fnth).clamp(f32::EPSILON - 100.,
                            100. - f32::EPSILON) * radius / 100.);
                    let ha = grad.ha.as_ref().map_or(0., |ha|
                        ha.get_value(fnth).to_radians()) + math::fast_atan2(dy, dx);
                    let fp = Vec2D::from_polar(ha) * hl + sp;

                    // Lottie doesn't have any focal radius concept
                         SC::radial_gradient(sp, fp, (0., radius), &stops)
                } else { SC::linear_gradient(sp, ep, &stops) }
            }
        };

        let fso = match &self.base {
            FillStroke::FillRule { rule } => FSOpts::Fill(*rule),
            FillStroke::Stroke(stroke) => {
                let width = stroke.width.get_value(fnth);
                let limit = stroke.ml2.as_ref().map_or(stroke.ml,
                    |ml| ml.get_value(fnth));
                FSOpts::Stroke { width, limit, join: stroke.lj, cap: stroke.lc,
                    dash: self.get_dash(fnth) }
            }
        };

        (style, fso)
    }

    fn get_dash(&self, fnth: f32) -> (f32, Vec<f32>) {
        let FillStroke::Stroke(stroke) = &self.base else { return (0., Vec::new()) };
        let mut pattern = Vec::with_capacity(stroke.dash.len());
        let (mut offset, mut sum) = (0., 0.);

        for (index, dash) in stroke.dash.iter().enumerate() {
            let value = dash.value.get_value(fnth);
            match dash.r#type {
                StrokeDashType::Offset if index + 1 == stroke.dash.len() => offset = value,
                StrokeDashType::Offset => return (0., Vec::new()),
                kind => {
                    let expected = if pattern.len() % 2 == 0 {
                             StrokeDashType::Length
                    } else { StrokeDashType::Gap };
                    if !matches!((kind, expected),
                        (StrokeDashType::Length, StrokeDashType::Length) |
                        (StrokeDashType::Gap, StrokeDashType::Gap)) || value < 0. {
                        return (0., Vec::new())
                    }
                    pattern.push(value); sum += value;
                }
            }
        }
        if pattern.is_empty() || sum == 0. { return (0., Vec::new()) }
        if pattern.len() % 2 == 1 { pattern.extend_from_within(..); }

        (offset, pattern)
    }
}

#[cfg(test)] mod tests {
    use crate::core::schema::{GradientColors, Repeater, ShapeItem};

    fn stroke_dash(entries: &str) -> (f32, Vec<f32>) {
        let json = format!(r#"{{
            "ty":"st","o":{{"k":100}},"c":{{"k":[1,0,0]}},"w":{{"k":1}},
            "lc":1,"lj":1,"ml":4,"d":[{entries}]
        }}"#);
        let ShapeItem::Stroke(stroke) = serde_json::from_str(&json).unwrap() else { panic!() };
        stroke.get_dash(0.)
    }

    fn repeater(copies: f32, order: u8, offset: f32, transform: &str) -> Box<Repeater> {
        let json = format!(r#"{{
            "ty":"rp","c":{{"k":{copies}}},"m":{order},"o":{{"k":{offset}}},
            "tr":{{{transform}}}
        }}"#);
        let ShapeItem::Repeater(repeater) = serde_json::from_str(&json).unwrap()
            else { panic!() };
        repeater
    }

    #[test] fn repeater_ceil_copies_and_interpolates_opacity_by_copy_index() {
        let below = repeater(2.5, 1, 0.,
            r#""so":{"k":20},"eo":{"k":80}"#);
        let matrices = below.get_matrix::<kurbo::Affine>(0.);
        assert_eq!(matrices.len(), 3);
        assert_eq!(matrices.iter().map(|matrix| matrix.1).collect::<Vec<_>>(),
            [0.2, 0.5, 0.8]);

        let above = repeater(2.5, 2, 0.,
            r#""so":{"k":20},"eo":{"k":80}"#);
        assert_eq!(above.get_matrix::<kurbo::Affine>(0.).iter()
            .map(|matrix| matrix.1).collect::<Vec<_>>(), [0.8, 0.5, 0.2]);
    }

    #[test] fn repeater_handles_fractional_negative_offset_and_split_position() {
        let repeater = repeater(2., 1, -0.5, concat!(
            r#""s":{"k":[200,200]}"#,
            r#","p":{"s":true,"x":{"k":4},"y":{"k":6}}"#,
            r#","sk":{"k":10},"sa":{"k":30}"#,
        ));
        let matrix = &repeater.get_matrix::<kurbo::Affine>(0.)[0].0;
        let coeffs = matrix.as_coeffs();
        assert!(coeffs.iter().all(|value| value.is_finite()));
        assert_ne!(coeffs[1], 0.);
        assert!(coeffs[4] < 0. && coeffs[5] < 0.);
    }

    #[test] fn repeater_bounds_abnormal_copy_counts() {
        assert!(repeater(-1., 1, 0., "")
            .get_matrix::<kurbo::Affine>(0.).is_empty());
        assert_eq!(repeater(10_001., 1, 0., "")
            .get_matrix::<kurbo::Affine>(0.).len(), 10_000);
    }

    #[test] fn dash_expands_odd_patterns_and_keeps_trailing_offset() {
        assert_eq!(stroke_dash(r#"{"n":"d","v":{"k":4}}"#), (0., vec![4., 4.]));
        assert_eq!(stroke_dash(concat!(
            r#"{"n":"d","v":{"k":4}},{"n":"g","v":{"k":8}},"#,
            r#"{"n":"d","v":{"k":16}},{"n":"o","v":{"k":-3}}"#,
        )), (-3., vec![4., 8., 16., 4., 8., 16.]));
    }

    #[test] fn dash_rejects_invalid_sequences_and_lengths() {
        assert!(stroke_dash(r#"{"n":"g","v":{"k":4}}"#).1.is_empty());
        assert!(stroke_dash(concat!(
            r#"{"n":"d","v":{"k":4}},{"n":"o","v":{"k":1}},"#,
            r#"{"n":"g","v":{"k":8}}"#,
        )).1.is_empty());
        assert!(stroke_dash(r#"{"n":"d","v":{"k":-1}}"#).1.is_empty());
        assert!(stroke_dash(r#"{"n":"d","v":{"k":0}}"#).1.is_empty());
    }

    #[test] fn gradient_merges_independent_color_and_opacity_offsets() {
        let gradient: GradientColors = serde_json::from_str(concat!(
            r#"{"p":2,"k":{"k":["#,
            r#"0,1,0,0,1,0,0,1,"#,
            r#"0.25,0,0.75,1]}}"#,
        )).unwrap();
        let stops = gradient.resolve(0., 1.);

        assert_eq!(stops.iter().map(|stop| stop.0).collect::<Vec<_>>(),
            [0., 0.25, 0.75, 1.]);
        assert_eq!((stops[0].1.r, stops[0].1.b, stops[0].1.a), (255, 0, 0));
        assert_eq!((stops[1].1.r, stops[1].1.b, stops[1].1.a), (191, 64, 0));
        assert_eq!((stops[2].1.r, stops[2].1.b, stops[2].1.a), (64, 191, 255));
        assert_eq!((stops[3].1.r, stops[3].1.b, stops[3].1.a), (0, 255, 255));
    }

    #[test] fn gradient_rejects_malformed_data() {
        assert!(serde_json::from_str::<GradientColors>(
            r#"{"p":2,"k":{"k":[0,1,0,0,1,0,0,1,0]}}"#).is_err());
        assert!(serde_json::from_str::<GradientColors>(
            r#"{"p":2,"k":{"k":[0,1,0,0,-0.1,0,0,1]}}"#).is_err());
    }

    #[test] fn gradient_supports_rgb_only_and_animated_stops() {
        let plain: GradientColors = serde_json::from_str(
            r#"{"p":2,"k":{"k":[0,1,0,0,1,0,0,1]}}"#).unwrap();
        assert!(plain.resolve(0., 1.).iter().all(|stop| stop.1.a == 255));

        let one_alpha: GradientColors = serde_json::from_str(
            r#"{"p":2,"k":{"k":[0,1,0,0,1,0,0,1,0.5,0.25]}}"#).unwrap();
        assert!(one_alpha.resolve(0., 1.).iter().all(|stop| stop.1.a == 64));

        let animated: GradientColors = serde_json::from_str(concat!(
            r#"{"p":2,"k":{"k":["#,
            r#"{"t":0,"s":[0,1,0,0,1,0,0,1]},"#,
            r#"{"t":10,"s":[0,0,1,0,1,1,1,0]}"#,
            r#"]}}"#,
        )).unwrap();
        let stops = animated.resolve(5., 1.);
        assert_eq!((stops[0].1.r, stops[0].1.g, stops[0].1.b), (128, 128, 0));

        let encoded = serde_json::to_string(&animated).unwrap();
        let decoded: GradientColors = serde_json::from_str(&encoded).unwrap();
        assert_eq!(decoded.cl.get_value(5.), animated.cl.get_value(5.));

        let eased: GradientColors = serde_json::from_str(concat!(
            r#"{"p":2,"k":{"k":[{"t":0,"s":[0,1,0,0,1,0,0,1],"#,
            r#""o":{"x":0,"y":[0,1]},"i":{"x":1,"y":[0,1]}},"#,
            r#"{"t":10,"s":[0,0,1,0,1,1,1,0]}]}}"#,
        )).unwrap();
        let color = eased.resolve(5., 1.)[0].1;
        assert_eq!((color.r, color.g, color.b), (32, 223, 0));
    }
}
