/****************************************************************
 * $ID: style.rs  	Tue 18 Nov 2025 15:30:11+0800               *
 *                                                              *
 * Maintainer: 范美辉 (MeiHui FAN) <mhfan@ustc.edu>              *
 * Copyright (c) 2025 M.H.Fan, All rights reserved.             *
 ****************************************************************/

use crate::{helpers::{Vec2D, RGBA, IntBool, math},
    schema::{Transform, Translation, TransRotation, VisualLayer, LayerItem,
        FillStrokeGrad, ColorGrad, FillStroke, FillRule, GradientType,
        Repeater, Composite, LineJoin, LineCap, StrokeDashType}
};

impl MatrixConv for kurbo::Affine {
    /*  | a c e |          Affine::Mul (self * other)
        | b d f |
        | 0 0 1 | */
    #[inline] fn identity() -> Self { Self::IDENTITY }
    #[inline] fn rotate(&mut self, angle: f32) { *self = self.then_rotate(angle as _) }
    #[inline] fn translate(&mut self, pos: Vec2D) { *self = Self::translate(pos) * *self }
    #[inline] fn skew_x(&mut self, sk: f32) { *self = Self::skew(sk.tan() as _, 0.) * *self }
    #[inline] fn scale(&mut self, sl: Vec2D) {      // Affine didn't do tan() inside
        *self = self.then_scale_non_uniform(sl.x as _, sl.y as _)
    }
    #[inline] fn premul(&mut self, tm: &Self) { *self *= *tm }
}

#[cfg(feature = "vello")] impl StyleConv for peniko::Brush {
    #[inline] fn solid_color(color: RGBA) -> Self { Self::Solid(color.into()) }
    #[inline] fn linear_gradient(sp: Vec2D, ep: Vec2D,
            stops: &[(f32, RGBA)]) -> Self {
        let stops = stops.iter().map(|&(offset, color)|
            (offset, DynamicColor::from_alpha_color(color.into())).into())
            .collect::<Vec<ColorStop>>();
        Self::Gradient(peniko::Gradient::new_linear(sp, ep).with_stops(stops.as_slice()))
    }
    #[inline] fn radial_gradient(cp: Vec2D, fp: Vec2D, radii: (f32, f32),
            stops: &[(f32, RGBA)]) -> Self {
        let stops = stops.iter().map(|&(offset, color)|
            (offset, DynamicColor::from_alpha_color(color.into())).into())
            .collect::<Vec<ColorStop>>();
        Self::Gradient(peniko::Gradient::new_two_point_radial(cp, radii.0, fp, radii.1)
            .with_stops(stops.as_slice()))
    }
}
#[cfg(feature = "vello")] use vello::peniko::{self, ColorStop, color::DynamicColor};
#[cfg(feature = "vello")] impl From<RGBA> for peniko::Color {
    #[inline] fn from(color: RGBA) -> Self {
        Self::from_rgba8(color.r, color.g, color.b, color.a)
    }
}

/** ```
    let  a = [1.0, 2.0, 3.0, 4.0, 5.0, 6.0];
    let  b = [7.0, 8.0, 9.0, 10.0, 11.0, 12.0];
    let ab = [31.0, 46.0, 39.0, 58.0, 52.0, 76.0];
    assert_eq!(Affine::new(a) * Affine::new(b), Affine::new(ab));

    let to_f32 = |x: f64| x as f32;     // Into::into
    let mut t = TM2D(a.map(to_f32));    t.premultiply(&TM2D(b.map(to_f32)));
    assert_eq!(t, TM2D(ab.map(to_f32)));    use femtovg::Transform2D as TM2D;

    use intvg::blend2d::BLMatrix2D;     use kurbo::Affine;
    let mut t = BLMatrix2D::new(a);     t.transform(&BLMatrix2D::new(b));
    assert_eq!(t.get_values(), BLMatrix2D::new(ab).get_values());

    fn operate_matrix(t: &mut impl MatrixConv) {
        t.translate(-Vec2D::from((1., 2.))); t.scale((3., 4.).into());
        t.rotate(-0.5); t.skew_x(-0.6); t.rotate(0.5);
        t.rotate(-0.7); t.translate((8., 9.).into());
    }

    use inlottie::{style::MatrixConv, helpers::Vec2D, adapt_b2d, adapt_nvg};
    let mut t1 =     Affine::identity(); operate_matrix(&mut t1);
    let mut t2 =       TM2D::identity(); operate_matrix(&mut t2);
    let mut t3 = BLMatrix2D::identity(); operate_matrix(&mut t3);

    println!("{t1:?}\n{t2:?}\nBLMatrix2D{:?}", t3.get_values());    //assert!(false);
    t1.as_coeffs().iter().zip(t2.0.iter()).zip(t3.get_values().iter()).all(|((&v1, &v2), &v3)|
        (v1 - v2 as f64).abs() < f64::EPSILON && (v1 - v3).abs() < f64::EPSILON);
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
    #[inline] fn default() -> Self { Self(MC::identity(), 1.) }
}
impl<MC: MatrixConv> TM2DwO<MC> {
    #[inline] pub fn compose(mut self, other: &Self) -> Self {
        self.0.premul(&other.0);    self.1 *= other.1;  self
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
                if  ao.as_bool() &&  apos.animated.as_bool() {
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

impl VisualLayer {
    pub fn get_matrix<MC: MatrixConv>(&self, layers: &[LayerItem], fnth: f32) -> TM2DwO<MC> {
        let ctm = self.ks.to_matrix(fnth, self.ao);
        if let Some(pid) = self.base.parent {
            let ptm = layers.iter().find_map(|layer|
                layer.visual_layer().and_then(|vl|
                    vl.base.ind.and_then(|ind| if ind == pid {
                        Some(vl.ks.to_matrix(fnth, vl.ao)) } else { None })));

            if let Some(ptm) = &ptm { ctm.compose(ptm) } else { unreachable!() }
        } else { ctm }
    }
}

impl Repeater {
    pub fn get_matrix<MC: MatrixConv>(&self, fnth: f32) -> Vec<TM2DwO<MC>> {
        let mut opacity = self.tr.so.as_ref().map_or(1.,
            |so| so.get_value(fnth) / 100.);
        let  offset = self.offset.as_ref().map_or(0.,
            |offset| offset.get_value(fnth));   // range: [-1, 2]?

        let cnt = self.cnt.get_value(fnth) as u32;
        let delta = if 1 < cnt { (self.tr.eo.as_ref().map_or(1., |eo|
            eo.get_value(fnth) / 100.) - opacity) / (cnt - 1) as f32 } else { 0. };
        let mut coll = Vec::with_capacity(cnt as usize);

        let trfm = &self.tr.trfm;
        let  anchor = trfm.anchor.as_ref().map_or(Vec2D { x: 0., y: 0. },
            |anchor| anchor.get_value(fnth));
        let scale = trfm.scale.as_ref()
            .map(|scale| scale.get_value(fnth) / 100.);

        let rot = match &trfm.extra {
            TransRotation::Normal2D { rotation } =>
                rotation.as_ref().map(|rdeg|
                    rdeg.get_value(fnth).to_radians()),
            TransRotation::Split3D(_) => unimplemented!(), //debug_assert!(ddd),
        };

        let pos = match &trfm.position {
            Some(Translation::Normal(apos)) => apos.get_value(fnth),
            Some(Translation::Split(sv)) => {   debug_assert!(sv.split);
                Vec2D { x: sv.x.get_value(fnth), y: sv.y.get_value(fnth) }
            }   _ => Vec2D { x: 0., y: 0. },
        };  // XXX: shouldn't need to deal with auto orient and skew_x?

        for i in 0..cnt {
            let offset = offset + if matches!(self.order,
                Composite::Below) { i } else { cnt - 1 - i } as f32;
            let mut trfm = MC::identity();

            trfm.translate(-anchor);
            if let Some(scale) = scale {
                trfm.scale((scale.x.powf(offset), scale.y.powf(offset)).into());
            };  if let Some(rot) = rot { trfm.rotate(rot * offset); }
            trfm.translate(pos * offset + anchor);

            coll.push(TM2DwO(trfm, opacity));
            opacity += delta;
        }   coll
    }
}

pub trait StyleConv {
    fn solid_color(color: RGBA) -> Self;
    fn linear_gradient(sp: Vec2D, ep: Vec2D, stops: &[(f32, RGBA)]) -> Self;
    fn radial_gradient(cp: Vec2D, fp: Vec2D, radii: (f32, f32),
        stops: &[(f32, RGBA)]) -> Self;
}

pub enum FSOpts {   Fill(FillRule),
    /// dash\[0\] is offset indeed; XXX: use SmallVec for dash?
    Stroke { width: f32, limit: f32, join: LineJoin, cap: LineCap, dash: Vec<f32>, }
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
                let mut stops = grad.stops.cl.get_value(fnth).0;
                debug_assert!(stops.len() as u32 == grad.stops.cnt);
                stops.iter_mut().for_each(|(_, rgba)|
                    rgba.a = (opacity * rgba.a as f32 + 0.5) as _);

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

    fn get_dash(&self, fnth: f32) -> Vec<f32> {
        let (mut dpat, mut sum) = (vec![], 0.);
        if let FillStroke::Stroke(stroke) = &self.base {
            let len = stroke.dash.len();
            if  len < 3 { return dpat }

            dpat.reserve(len);   dpat.push(0.);
            stroke.dash.iter().for_each(|sd| {
                let value = sd.value.get_value(fnth);
                match sd.r#type {   // Offset should be at end of the array?
                    StrokeDashType::Offset => dpat[0] = value,
                    StrokeDashType::Length | StrokeDashType::Gap => {
                        if value < 0. { dpat.clear(); return }
                        dpat.push(value);   sum += value;

                        debug_assert!(dpat.len() % 2 ==
                            if matches!(sd.r#type, StrokeDashType::Gap) { 1 } else { 0 });
                    }   // Length and Gap should be alternating and positive
                }
            });
        }

        if sum < f32::EPSILON { dpat.clear(); }   dpat
        //if dpat.len() % 2 == 0 { dpat.extend_from_slice(&dpat[1..].clone()); } // XXX:
    }
}

