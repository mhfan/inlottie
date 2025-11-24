/****************************************************************
 * $ID: style.rs  	Tue 18 Nov 2025 15:30:11+0800               *
 *                                                              *
 * Maintainer: 范美辉 (MeiHui FAN) <mhfan@ustc.edu>              *
 * Copyright (c) 2025 M.H.Fan, All rights reserved.             *
 ****************************************************************/

use crate::{schema::*, helpers::*};

impl MatrixConv for kurbo::Affine {
    #[inline] fn identity() -> Self { Self::IDENTITY }
    #[inline] fn rotate(&mut self, angle: f32) { *self *= Self::rotate(angle as _) }
    #[inline] fn translate(&mut self, pos: Vec2D) { *self *= Self::translate(pos) }
    #[inline] fn skew_x(&mut self, sk: f32) { *self *= Self::skew(sk as _, 0.) }
    #[inline] fn scale(&mut self, sl: Vec2D) {
        *self *= Self::scale_non_uniform(sl.x as _, sl.y as _)
    }
    #[inline] fn multiply(&mut self, tm: &Self) { *self *= *tm }
}

impl From<RGBA> for peniko::Color {
    #[inline] fn from(color: RGBA) -> Self {
        Self::from_rgba8(color.r, color.g, color.b, color.a)
    }
}
impl StyleConv for peniko::Brush {
    #[inline] fn solid_color(color: RGBA) -> Self { Self::Solid(color.into()) }
    #[inline] fn linear_gradient(sp: Vec2D, ep: Vec2D,
            stops: &[(f32, RGBA)]) -> Self {
        use peniko::{Gradient, ColorStop, color::DynamicColor};
        let stops = stops.iter().map(|&(offset, color)|
            (offset, DynamicColor::from_alpha_color(color.into())).into())
            .collect::<Vec<ColorStop>>();
        Self::Gradient(Gradient::new_linear(sp, ep).with_stops(stops.as_slice()))
    }
    #[inline] fn radial_gradient(cp: Vec2D, fp: Vec2D, radii: (f32, f32),
            stops: &[(f32, RGBA)]) -> Self {
        use peniko::{Gradient, ColorStop, color::DynamicColor};
        let stops = stops.iter().map(|&(offset, color)|
            (offset, DynamicColor::from_alpha_color(color.into())).into())
            .collect::<Vec<ColorStop>>();
        Self::Gradient(Gradient::new_two_point_radial(cp, radii.0, fp, radii.1)
            .with_stops(stops.as_slice()))
    }
}

pub trait RenderContext {
    type VGPath: crate::pathm::PathBuilder;
    type VGStyle: StyleConv;    // (VGBrush/VGPaint, FSOpts)
    type TM2D: MatrixConv;

    fn get_transform(&self) -> Self::TM2D;
    fn reset_transform(&mut self, trfm: Option<&Self::TM2D>);
    fn apply_transform(&mut self, trfm: &Self::TM2D, opacity: Option<f32>); // alpha

    fn   fill_path(&mut self, path: &Self::VGPath, style: &Self::VGStyle, fso: &FSOpts);
    fn stroke_path(&mut self, path: &Self::VGPath, style: &Self::VGStyle, fso: &FSOpts);
}

pub trait MatrixConv {
    fn identity() -> Self;
    fn multiply(&mut self, tm: &Self);
    //fn reset(&mut self, tm: Option<&Self>);

    fn rotate(&mut self, angle: f32);
    fn translate(&mut self, pos: Vec2D);
    fn skew_x(&mut self, sk: f32);
    fn scale(&mut self, sl: Vec2D);
}

#[derive(Clone)] pub struct TM2DwO<MC: MatrixConv>(pub MC, pub f32);
impl<MC: MatrixConv> Default for TM2DwO<MC> {
    fn default() -> Self { Self(MC::identity(), 1.) } }
impl<MC: MatrixConv> TM2DwO<MC> {
    #[inline] pub fn multiply(&mut self, other: &Self) {
            self.0  .multiply(&other.0);  self.1 *= other.1;
    }
}

impl Transform {
    /// Multiplications are RIGHT multiplications (Next = Previous * StepOperation).
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

            let skew = -skew.get_value(fnth); //.clamp(-85., 85.); // SKEW_LIMIT
            trfm.skew_x(skew.to_radians().tan());

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

pub enum FSOpts {   Fill(FillRule),     /// dash\[0\] is offset indeed; use SmallVec for dash?
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

                    //ctx.createRadialGradient(sp.x, sp.y, 0., fp.x, fp.y, radius); // XXX:
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

