/****************************************************************
 * $ID: render.rs  	Fri 03 May 2024 22:07:36+0800               *
 *                                                              *
 * Maintainer: 范美辉 (MeiHui FAN) <mhfan@ustc.edu>              *
 * Copyright (c) 2024 M.H.Fan, All rights reserved.             *
 ****************************************************************/

use core::cell::RefCell;
use crate::{helpers::{RGBA, IntBool, ACCURACY_TOLERANCE},
    schema::{Animation, AssetItem, LayerItem, ShapeItem, VisualLayer,
        TrimPath, TrimMultiple, MatteMode, FillRule},
    style::{StyleConv, MatrixConv, TM2DwO, FSOpts}, pathm::{PathBuilder, PathFactory}
};

impl Animation {    /// https://lottiefiles.github.io/lottie-docs/rendering/
    //fn get_duration(&self) -> f32 { (self.op - self.ip) / self.fr }
    pub fn render_next_frame<RC: RenderContext>(&mut self,
        rctx: &mut RC, elapsed: f32) -> bool {
        //debug_assert!(0. < self.fr && 0. <= self.ip && 1. < self.op - self.ip);

            self.elapsed += elapsed * self.fr;
        if  self.elapsed < 1. && self.ip < self.fnth { return false }

        if  2. <= self.elapsed {    // advance/skip elapsed frames
            let elapsed = (self.elapsed - 1.).floor();
            self.fnth = (self.fnth + elapsed) % self.op;
            self.elapsed -= elapsed;
        }

        let sz = rctx.get_size();
        rctx.clear_rect_with(0, 0, sz.0, sz.1, RGBA::new_f32(0.4, 0.4, 0.4, 1.));
        self.render_layers(rctx, &TM2DwO::default(), &self.layers, self.fnth);

        self.elapsed -= 1.;       self.fnth += 1.;
        if self.op <= self.fnth { self.fnth  = 0.; }    true
    }

    /// The render order goes from the last element to the first,
    /// items in list coming first will be rendered on top.
    fn render_layers<RC: RenderContext>(&self, rctx: &mut RC,
        ptm: &TM2DwO<RC::TM2D>, layers: &[LayerItem], fnth: f32) {
        let mut matte = None;

        for layer in layers.iter().rev() { match layer {
            LayerItem::Shape(shpl) => if !shpl.vl.should_hide(fnth) {
                let ltm =
                    shpl.vl.get_matrix(layers, fnth).compose(ptm);
                let (draws, ctm) =
                    convert_shapes(&shpl.shapes, fnth, shpl.vl.ao);

                rctx.prepare_matte(&shpl.vl, &mut matte);
                rctx.render_shapes(&ctm.compose(&ltm), &draws);
                rctx.compose_matte(&shpl.vl, &mut matte, &ltm, fnth);
            }
            LayerItem::PrecompLayer(pcl) => if !pcl.vl.should_hide(fnth) {
                if let Some(pcomp) = self.assets.iter().find_map(|asset|
                    match asset { AssetItem::Precomp(pcomp)
                        if pcomp.base.id == pcl.rid => Some(pcomp), _ => None }) {
                    let fnth = (fnth - pcl.vl.base.st) / pcl.vl.base.sr;

                    let fnth = pcl.tm.as_ref().map_or(fnth, // handle time remapping
                        |tm| tm.get_value(fnth) * pcomp.fr);
                    let ltm =
                        pcl.vl.get_matrix(layers, fnth).compose(ptm);

                    rctx.prepare_matte(&pcl.vl, &mut matte);
                    self.render_layers(rctx, &ltm, &pcomp.layers, fnth);
                    rctx.compose_matte(&pcl.vl, &mut matte, &ltm, fnth);
                }   // XXX: clipping(pcl.w, pcl.h)?
            }
            LayerItem::SolidColor(scl) => if !scl.vl.should_hide(fnth) {
                let ltm =
                    scl.vl.get_matrix(layers, fnth).compose(ptm);

                let mut path = RC::VGPath::new(5);
                path.rect((self.w as f32 - scl.sw) / 2., // 0., 0.,
                          (self.h as f32 - scl.sh) / 2., scl.sw, scl.sh);

                rctx.prepare_matte(&scl.vl, &mut matte);
                rctx.render_shapes(&ltm, &[DrawItem::Shape(path.into()),
                    DrawItem::Style(RefCell::new((RC::VGStyle::solid_color(scl.sc),
                        FSOpts::Fill(FillRule::NonZero))).into())]);
                rctx.compose_matte(&scl.vl, &mut matte, &ltm, fnth);
            }
            LayerItem::Image(_) | LayerItem::Text(_)  | LayerItem::Data(_)  |
            LayerItem::Audio(_) | LayerItem::Camera(_) => dbg!(),     // TODO:

            //LayerItem::Null(_) => (),    // used as a parent, nothing to do
            _ => (),
        } }
    }
}

pub trait RenderContext {
    type VGPath: PathBuilder;
    type VGStyle: StyleConv;    // (VGBrush/VGPaint, FSOpts)
    type TM2D: MatrixConv + Clone;
    type ImageID;

    //fn set_comp_op(&mut self, op: CompOp);

    fn get_size(&self) -> (u32, u32);
    fn prepare_matte(&mut self, _: &VisualLayer, _: &mut Option<TrackMatte<Self::ImageID>>) {}
    fn compose_matte(&mut self, _: &VisualLayer, _: &mut Option<TrackMatte<Self::ImageID>>,
        _: &TM2DwO<Self::TM2D>, _: f32) {}
    fn clear_rect_with(&mut self, x: u32, y: u32, w: u32, h: u32, color: RGBA);

    fn reset_transform(&mut self, trfm: Option<&Self::TM2D>);   // XXX: save/restore
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
            |(idx, item)| match item {
            DrawItem::Style(style) =>
                self.traverse_shapes(ptm, &draws[0..idx], style),
            DrawItem::Group(grp, rep) =>
                rep.iter().rev().for_each(|gtm|
                    self.render_shapes(&gtm.clone().compose(ptm), grp)),
            _ => (), // skip/ignore Shape
        });
    }
}

pub struct TrackMatte<T> {
    pub mode: MatteMode, pub mlid: Option<u32>, pub imgid: T, pub mskid: Option<T>
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

