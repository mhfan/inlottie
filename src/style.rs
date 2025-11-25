/****************************************************************
 * $ID: style.rs  	Tue 18 Nov 2025 15:30:11+0800               *
 *                                                              *
 * Maintainer: 范美辉 (MeiHui FAN) <mhfan@ustc.edu>              *
 * Copyright (c) 2025 M.H.Fan, All rights reserved.             *
 ****************************************************************/

use crate::{schema::*, helpers::*};
use crate::pathm::{PathBuilder, PathFactory};

pub trait RenderContext {
    type VGPath: PathBuilder;
    type VGStyle: StyleConv;    // (VGBrush/VGPaint, FSOpts)
    type TM2D: MatrixConv + Clone;

    //fn set_comp_op(&mut self, op: CompOp);
    fn clear_rect_with(&mut self, x: u32, y: u32, w: u32, h: u32, color: RGBA);

    fn reset_transform(&mut self, trfm: Option<&Self::TM2D>);
    fn apply_transform(&mut self, trfm: &Self::TM2D, opacity: Option<f32>) -> Self::TM2D;
    fn fill_stroke(&mut self, path: &Self::VGPath, style: &RefCell<(Self::VGStyle, FSOpts)>);

    fn traverse_shapes(&mut self, ptm: &TM2DwO<Self::TM2D>,
        draws: &[DrawItem<Self::VGPath, Self::VGStyle, Self::TM2D>],
        style: &RefCell<(Self::VGStyle, FSOpts)>) {

        // XXX: in which case shape/path and style need to apply different transforms?
        let last_trfm = self.apply_transform(&ptm.0, Some(ptm.1));
        draws.iter().rev().for_each(
            |draw| match draw {
            DrawItem::Shape(path) =>
                self.fill_stroke(path, style),
            DrawItem::Group(grp, rep) =>
                rep.iter().rev().for_each(|gtm|
                    self.traverse_shapes(&gtm.clone().compose(ptm), grp, style)),
            _ => (), // skip/ignore Style
        });     self.reset_transform(Some(&last_trfm));
    }

    fn render_shapes(&mut self, ptm: &TM2DwO<Self::TM2D>,
        draws: &[DrawItem<Self::VGPath, Self::VGStyle, Self::TM2D>]) {
        draws.iter().enumerate().rev().for_each(
            |(idx, draw)| match draw {
            DrawItem::Style(style) =>
                self.traverse_shapes(ptm, &draws[0..idx], style),
            DrawItem::Group(grp, rep) =>
                rep.iter().rev().for_each(|gtm|
                    self.render_shapes(&gtm.clone().compose(ptm), grp)),
            _ => (), // skip/ignore Shape
        });
    }
}

/// calculate transform matrix, convert shapes to paths, modify/change the paths,
/// and convert style(fill/stroke/gradient) to draw items, recursively
pub fn convert_shapes<VGPath: PathBuilder, VGPaint: StyleConv, TM2D: MatrixConv>(
    shapes: &[ShapeItem], fnth: f32, ao: IntBool) ->
    (Vec<DrawItem<VGPath, VGPaint, TM2D>>, TM2DwO<TM2D>) {
    let mut draws = Vec::with_capacity(shapes.len());
    let mut ctm = Default::default();

    for shape in shapes.iter() { match shape {
        ShapeItem::Rectangle(rect)    if !rect.base.elem.hd =>
            draws.push(DrawItem::Shape(Box::new(rect.to_path(fnth)))),
        ShapeItem::Polystar(star) if !star.base.elem.hd =>
            draws.push(DrawItem::Shape(Box::new(star.to_path(fnth)))),
        ShapeItem::Ellipse(elps)        if !elps.base.elem.hd =>
            draws.push(DrawItem::Shape(Box::new(elps.to_path(fnth)))),
        ShapeItem::Path(curv)          if !curv.base.elem.hd =>
            draws.push(DrawItem::Shape(Box::new(curv.to_path(fnth)))),

        // styles affect on all preceding paths ever before
        ShapeItem::Fill(fill)   if !fill.elem.hd =>
            draws.push(DrawItem::Style(Box::new(fill.to_style(fnth).into()))),
        ShapeItem::Stroke(line) if !line.elem.hd =>
            draws.push(DrawItem::Style(Box::new(line.to_style(fnth).into()))),
        ShapeItem::GradientFill(grad)   if !grad.elem.hd =>
            draws.push(DrawItem::Style(Box::new(grad.to_style(fnth).into()))),
        ShapeItem::GradientStroke(grad) if !grad.elem.hd =>
            draws.push(DrawItem::Style(Box::new(grad.to_style(fnth).into()))),
        ShapeItem::NoStyle(_) => eprintln!("Nothing to do here?"),

        ShapeItem::Group(group) if !group.elem.hd => {
            let (grp, ctm) =
                convert_shapes(&group.shapes, fnth, ao);
            draws.push(DrawItem::Group(grp, vec![ctm]));
        }

        ShapeItem::Repeater(mdfr) if !mdfr.elem.hd => {
            let grp = core::mem::take(&mut draws);
            draws.push(DrawItem::Group(grp, mdfr.get_matrix(fnth)));
        }

        // other modifiers usually just affect on all preceding paths ever before
        ShapeItem::Trim(mdfr) if !mdfr.elem.hd =>
            trim_shapes(mdfr, &mut draws, fnth),

        ShapeItem::Merge (_) | ShapeItem::OffsetPath (_) |
        ShapeItem::Twist (_) | ShapeItem::PuckerBloat(_) |
        ShapeItem::ZigZag(_) | ShapeItem::RoundedCorners(_) => dbg!(),  // TODO:

        ShapeItem::Transform(ts) if !ts.elem.hd =>
            ctm = ts.trfm.to_matrix(fnth, ao),

        _ => (),
    } }     (draws, ctm)
}

use core::cell::RefCell;
//  https://lottie.github.io/lottie-spec/latest/specs/shapes/#graphic-element
pub enum DrawItem<VGPath: PathBuilder, VGPaint: StyleConv, TM2D: MatrixConv> {
    Shape(Box<VGPath>),                     // DrawItem is a.k.a Graphic Element
    Style(Box<RefCell<(VGPaint, FSOpts)>>), // RefCell interior mutation for femtovg
    Group(Vec<Self>, Vec<TM2DwO<TM2D>>),    // support batch Groups for Repeater
}

fn trim_shapes<VGPath: PathBuilder, VGPaint: StyleConv, TM2D: MatrixConv>(
    mdfr: &TrimPath, draws: &mut [DrawItem<VGPath, VGPaint, TM2D>], fnth: f32) {
    fn traverse_shapes<VGPath: PathBuilder, VGPaint: StyleConv, TM2D: MatrixConv>(draws:
        &mut [DrawItem<VGPath, VGPaint, TM2D>], closure: &mut impl FnMut(&mut VGPath)) {
        draws.iter_mut().for_each(|draw| match draw {
            DrawItem::Group(grp, _) =>
                traverse_shapes(grp, closure),
            DrawItem::Shape(path) => closure(path),
            _ => (), // skip/ignore Style
        });
    }       // XXX: how to treat repeated shapes?

    let offset   = mdfr.offset.get_value(fnth) as f64 / 360.;
    let start    = mdfr. start.get_value(fnth) as f64 / 100.;
    let mut trim = mdfr.   end.get_value(fnth) as f64 / 100. - start;
    if 1. < trim { trim = 1.; } //debug_assert!((0.0..=1.).contains(&trim));
    let start = (start + offset) % 1.;

    if mdfr.multiple.is_some_and(|ml| matches!(ml, TrimMultiple::Simultaneously)) {
        traverse_shapes(draws, &mut |path| *path = path.trim_path(start, trim));
    } else {    use kurbo::ParamCurveArclen;
        let (mut idx, mut suml) = (0u32, 0.);
        let (mut lens, mut tri0) = (vec![], 0.);

        traverse_shapes(draws, &mut |path| {
            let len = kurbo::segments(path.to_kurbo()).fold(0.,
                |acc, seg| acc + seg.arclen(ACCURACY_TOLERANCE));
            lens.push(len);     suml += len;
        });

        if 1. < start + trim { tri0 = start + trim - 1.; trim = 1. - start; }
        let (start, mut trim) = (suml * start, suml * trim);
        tri0 *= suml;   suml = 0.;

        traverse_shapes(draws, &mut |path| {    // same logic as in trim_path
            let len = lens[idx as usize];   idx += 1;

            if suml <= start &&  start < suml + len {   let start = start - suml;
                if  start + trim < len {
                    *path = path.trim_path(start / len, trim  / len);  trim = 0.;
                } else { trim -= len - start;   let start = start / len;
                    *path = path.trim_path(start, 1. - start);
                }
            } else if start < suml && 0. < trim { if trim < len {
                    *path = path.trim_path(0., (trim / len) as _);     trim = 0.;
                } else { trim -= len; }
            } else if 0. < tri0 { if tri0 < len {
                    *path = path.trim_path(0., (tri0 / len) as _);     tri0 = 0.;
                } else { tri0 -= len; }
            } else { *path = VGPath::new(0); }  suml += len;
        });
    }
}

impl MatrixConv for kurbo::Affine {
    #[inline] fn identity() -> Self { Self::IDENTITY }
    #[inline] fn rotate(&mut self, angle: f32) { *self *= Self::rotate(angle as _) }
    #[inline] fn translate(&mut self, pos: Vec2D) { *self *= Self::translate(pos) }
    #[inline] fn skew_x(&mut self, sk: f32) { *self *= Self::skew(sk as _, 0.) }
    #[inline] fn scale(&mut self, sl: Vec2D) {
        *self *= Self::scale_non_uniform(sl.x as _, sl.y as _)
    }
    #[inline] fn multiply(&mut self, tm: &Self) { *self = *tm * *self }     // *self *= *tm
}

#[cfg(feature = "vello")] impl From<RGBA> for vello::peniko::Color {
    #[inline] fn from(color: RGBA) -> Self {
        Self::from_rgba8(color.r, color.g, color.b, color.a)
    }
}
#[cfg(feature = "vello")] impl StyleConv for vello::peniko::Brush {
    #[inline] fn solid_color(color: RGBA) -> Self { Self::Solid(color.into()) }
    #[inline] fn linear_gradient(sp: Vec2D, ep: Vec2D,
            stops: &[(f32, RGBA)]) -> Self {
        use vello::peniko::{Gradient, ColorStop, color::DynamicColor};
        let stops = stops.iter().map(|&(offset, color)|
            (offset, DynamicColor::from_alpha_color(color.into())).into())
            .collect::<Vec<ColorStop>>();
        Self::Gradient(Gradient::new_linear(sp, ep).with_stops(stops.as_slice()))
    }
    #[inline] fn radial_gradient(cp: Vec2D, fp: Vec2D, radii: (f32, f32),
            stops: &[(f32, RGBA)]) -> Self {
        use vello::peniko::{Gradient, ColorStop, color::DynamicColor};
        let stops = stops.iter().map(|&(offset, color)|
            (offset, DynamicColor::from_alpha_color(color.into())).into())
            .collect::<Vec<ColorStop>>();
        Self::Gradient(Gradient::new_two_point_radial(cp, radii.0, fp, radii.1)
            .with_stops(stops.as_slice()))
    }
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
    #[inline] pub fn compose(mut self, other: &Self) -> Self {
        self.0.multiply(&other.0);  self.1 *= other.1;  self
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

            let skew = -skew.get_value(fnth).clamp(-85., 85.);  // SKEW_LIMIT
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

