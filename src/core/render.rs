/****************************************************************
 * $ID: render.rs  	Fri 03 May 2024 22:07:36+0800               *
 *                                                              *
 * Maintainer: 范美辉 (MeiHui FAN) <mhfan@ustc.edu>              *
 * Copyright (c) 2024 M.H.Fan, All rights reserved.             *
 ****************************************************************/

use core::cell::RefCell;
use std::collections::HashMap;
use crate::core::{helpers::{RGBA, IntBool},
    schema::{Animation, AssetItem, LayerItem, ShapeItem, VisualLayer,
        TrimPath, TrimMultiple, MatteMode, FillRule},
    style::{StyleConv, MatrixConv, TM2DwO, FSOpts},
    pathm::{MeasuredPath, PathBuilder, PathFactory}
};

fn layer_world_matrices<MC: MatrixConv + Clone>(
    layers: &[LayerItem], global: f32,
    mut required: impl FnMut(&LayerItem) -> bool) -> Vec<Option<TM2DwO<MC>>> {
    let mut indices = HashMap::<u32, Option<usize>>::with_capacity(layers.len());
    for (index, layer) in layers.iter().enumerate() {
        if let Some(id) = layer.visual_layer().and_then(|vl| vl.base.ind) {
            indices.entry(id).and_modify(|index| *index = None).or_insert(Some(index));
        }
    }

    fn resolve<MC: MatrixConv + Clone>(root: usize, layers: &[LayerItem], global: f32,
        indices: &HashMap<u32, Option<usize>>, states: &mut [u8],
        worlds: &mut [Option<TM2DwO<MC>>], stack: &mut Vec<usize>) {
        if states[root] == 2 { return }

        stack.clear();
        let mut index = root;
        let mut valid = loop { match states[index] {
            0 => {
                states[index] = 1; stack.push(index);
                let Some(vl) = layers[index].visual_layer() else { break false };
                let Some(parent) = vl.base.parent
                    .and_then(|id| indices.get(&id).copied().flatten())
                    else { break true };
                index = parent;
            }
            1 => break false,
            2 => break worlds[index].is_some(),
            _ => unreachable!(),
        } };

        while let Some(index) = stack.pop() {
            if valid {
                let Some(vl) = layers[index].visual_layer() else {
                    states[index] = 2; valid = false; continue
                };
                let Some(local) = vl.base.local_frame(global) else {
                    states[index] = 2; valid = false; continue
                };
                let mut world = vl.ks.to_matrix(local, vl.ao);
                if let Some(parent) = vl.base.parent
                    .and_then(|id| indices.get(&id).copied().flatten()) {
                    let Some(parent) = &worlds[parent] else {
                        states[index] = 2; valid = false; continue
                    };
                    world = world.compose_matrix(&parent.0);
                }
                worlds[index] = Some(world);
            }
            states[index] = 2;
        }
    }

    let mut states = vec![0; layers.len()];
    let mut worlds = vec![None; layers.len()];
    let mut stack = Vec::new();
    for (index, layer) in layers.iter().enumerate() {
        if required(layer) {
            resolve(index, layers, global, &indices, &mut states, &mut worlds, &mut stack);
        }
    }   worlds
}

impl Animation {    /// https://lottiefiles.github.io/lottie-docs/rendering/
    //fn get_duration(&self) -> f32 { (self.op - self.ip) / self.fr }
    /// `clear` selects a frame background; `None` preserves the current render target.
    pub fn render_next_frame<RC: RenderContext>(&mut self,
        rctx: &mut RC, elapsed: f32, clear: Option<RGBA>) -> bool {
        //debug_assert!(0. < self.fr && 0. <= self.ip && 1. < self.op - self.ip);

        if self.fnth < self.ip || self.op <= self.fnth { self.fnth = self.ip; }
            self.elapsed += elapsed * self.fr;
        if  self.elapsed < 1. && self.ip < self.fnth { return false }

        if  2. <= self.elapsed {    // advance/skip elapsed frames
            let elapsed = (self.elapsed - 1.).floor();
            let duration = self.op - self.ip;
            if 0. < duration {
                self.fnth = self.ip + (self.fnth - self.ip + elapsed).rem_euclid(duration);
            }
            self.elapsed -= elapsed;
        }

        if let Some(color) = clear {
            let (width, height) = rctx.get_size();
            rctx.clear_rect_with(0, 0, width, height, color);
        }
        self.render_layers(rctx, &TM2DwO::default(), &self.layers, self.fnth);

        self.elapsed -= 1.;       self.fnth += 1.;
        if self.op <= self.fnth { self.fnth = self.ip; }    true
    }

    /// The render order goes from the last element to the first,
    /// items in list coming first will be rendered on top.
    fn render_layers<RC: RenderContext>(&self, rctx: &mut RC,
        ptm: &TM2DwO<RC::TM2D>, layers: &[LayerItem], fnth: f32) {
        let mut matte = None;
        let matrices = layer_world_matrices::<RC::TM2D>(layers, fnth, |layer| match layer {
            LayerItem::Shape(layer) => !layer.vl.should_hide(fnth),
            LayerItem::PrecompLayer(layer) => !layer.vl.should_hide(fnth),
            LayerItem::SolidColor(layer) => !layer.vl.should_hide(fnth),
            _ => false,
        });

        for (layer, matrix) in layers.iter().zip(matrices).rev() { match layer {
            LayerItem::Shape(shpl) => if let Some(ltm) = matrix {
                let Some(local) = shpl.vl.base.local_frame(fnth) else { continue };
                let ltm = ltm.compose(ptm);
                let (draws, ctm) = convert_shapes(&shpl.shapes, local, shpl.vl.ao);

                rctx.prepare_matte(&shpl.vl, &mut matte);
                rctx.render_shapes(&ctm.compose(&ltm), &draws);
                rctx.compose_matte(&shpl.vl, &mut matte, &ltm, fnth);
            }
            LayerItem::PrecompLayer(pcl) => if let Some(ltm) = matrix {
                if let Some(pcomp) = self.assets.iter().find_map(|asset|
                    match asset { AssetItem::Precomp(pcomp)
                        if pcomp.base.id == pcl.rid => Some(pcomp), _ => None }) {
                    let Some(local) = pcl.vl.base.local_frame(fnth) else { continue };
                    let child_fnth = pcl.tm.as_ref().map_or(local,
                        |tm| tm.get_value(local) * pcomp.fr);
                    let ltm = ltm.compose(ptm);

                    rctx.prepare_matte(&pcl.vl, &mut matte);
                    self.render_layers(rctx, &ltm, &pcomp.layers, child_fnth);
                    rctx.compose_matte(&pcl.vl, &mut matte, &ltm, fnth);
                }   // XXX: clipping(pcl.w, pcl.h)?
            }
            LayerItem::SolidColor(scl) => if let Some(ltm) = matrix {
                let ltm = ltm.compose(ptm);

                let mut path = RC::VGPath::new(5);
                path.rect(0., 0., scl.sw, scl.sh);

                rctx.prepare_matte(&scl.vl, &mut matte);
                rctx.render_shapes(&ltm, &[DrawItem::Shape(path),
                    DrawItem::Style(RefCell::new((RC::VGStyle::solid_color(scl.sc),
                        FSOpts::Fill(FillRule::NonZero))))]);
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

    fn save_state(&mut self);
    fn restore_state(&mut self);
    fn apply_transform(&mut self, trfm: &Self::TM2D, opacity: Option<f32>);
    fn fill_stroke(&mut self, path: &Self::VGPath, style: &RefCell<(Self::VGStyle, FSOpts)>);

    fn traverse_shapes(&mut self, ptm: &TM2DwO<Self::TM2D>,
        draws: &[DrawItem<Self::VGPath, Self::VGStyle, Self::TM2D>],
        style: &RefCell<(Self::VGStyle, FSOpts)>) {

        // XXX: in which case shape/path and style need to apply different transforms?
        self.apply_transform(&ptm.0, Some(ptm.1));
        draws.iter().rev().for_each(|draw| match draw {
            DrawItem::Shape(path) => self.fill_stroke(path, style),
            DrawItem::Group(grp, rep) => rep.iter().rev().for_each(|gtm| {
                self.traverse_shapes(&gtm.clone().compose(ptm), grp, style);
                self.apply_transform(&ptm.0, Some(ptm.1));
            }),
            _ => (), // skip/ignore Style
        });
    }

    fn render_shapes(&mut self, ptm: &TM2DwO<Self::TM2D>,
        draws: &[DrawItem<Self::VGPath, Self::VGStyle, Self::TM2D>]) {
        self.save_state();
        self.render_shapes_inner(ptm, draws);
        self.restore_state();
    }

    fn render_shapes_inner(&mut self, ptm: &TM2DwO<Self::TM2D>,
        draws: &[DrawItem<Self::VGPath, Self::VGStyle, Self::TM2D>]) {
        draws.iter().enumerate().rev().for_each(|(idx, item)| match item {
            DrawItem::Style(style) =>
                self.traverse_shapes(ptm, &draws[0..idx], style),
            DrawItem::Group(grp, rep) => rep.iter().rev().for_each(|gtm|
                    self.render_shapes_inner(&gtm.clone().compose(ptm), grp)),
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
            draws.push(DrawItem::Shape(rect.to_path(fnth))),
        ShapeItem::Polystar(star) if !star.base.elem.hd =>
            draws.push(DrawItem::Shape(star.to_path(fnth))),
        ShapeItem::Ellipse(elps)        if !elps.base.elem.hd =>
            draws.push(DrawItem::Shape(elps.to_path(fnth))),
        ShapeItem::Path(curv)          if !curv.base.elem.hd =>
            draws.push(DrawItem::Shape(curv.to_path(fnth))),

        // styles affect on all preceding paths ever before
        ShapeItem::Fill(fill)   if !fill.elem.hd =>
            draws.push(DrawItem::Style(fill.to_style(fnth).into())),
        ShapeItem::Stroke(line) if !line.elem.hd =>
            draws.push(DrawItem::Style(line.to_style(fnth).into())),
        ShapeItem::GradientFill(grad)   if !grad.elem.hd =>
            draws.push(DrawItem::Style(grad.to_style(fnth).into())),
        ShapeItem::GradientStroke(grad) if !grad.elem.hd =>
            draws.push(DrawItem::Style(grad.to_style(fnth).into())),
        ShapeItem::NoStyle(_) => eprintln!("Nothing to do here?"),

        ShapeItem::Group(group) if !group.elem.hd => {
            let (grp, ctm) = convert_shapes(&group.shapes, fnth, ao);
            draws.push(DrawItem::Group(grp, vec![ctm]));
        }

        ShapeItem::Repeater(mdfr) if !mdfr.elem.hd => {
            let grp = core::mem::take(&mut draws);
            draws.push(DrawItem::Group(grp, mdfr.get_matrix(fnth)));
        }

        // other modifiers usually just affect on all preceding paths ever before
        ShapeItem::Trim(mdfr) if !mdfr.elem.hd => trim_shapes(mdfr, &mut draws, fnth),

        ShapeItem::Merge (_) | ShapeItem::OffsetPath (_) |
        ShapeItem::Twist (_) | ShapeItem::PuckerBloat(_) |
        ShapeItem::ZigZag(_) | ShapeItem::RoundedCorners(_) => dbg!(),  // TODO:

        ShapeItem::Transform(ts) if !ts.elem.hd => ctm = ts.trfm.to_matrix(fnth, ao),

        _ => (),
    } }     (draws, ctm)
}

//  https://lottie.github.io/lottie-spec/latest/specs/shapes/#graphic-element
pub enum DrawItem<VGPath: PathBuilder, VGPaint: StyleConv, TM2D: MatrixConv> {
    Shape(VGPath),                          // DrawItem is a.k.a Graphic Element
    Style(RefCell<(VGPaint, FSOpts)>),      // RefCell interior mutation for femtovg
    Group(Vec<Self>, Vec<TM2DwO<TM2D>>),    // support batch Groups for Repeater
}

fn for_each_path_mut<VGPath: PathBuilder, VGPaint: StyleConv, TM2D: MatrixConv>(
    draws: &mut [DrawItem<VGPath, VGPaint, TM2D>],
    closure: &mut impl FnMut(&mut VGPath)) {
    draws.iter_mut().rev().for_each(|draw| match draw {
        DrawItem::Group(group, _) => for_each_path_mut(group, closure),
        DrawItem::Shape(path) => closure(path),
        DrawItem::Style(_) => (), // skip/ignore Style
    });
}   // XXX: how to treat repeated shapes?

fn trim_shapes<VGPath: PathBuilder, VGPaint: StyleConv, TM2D: MatrixConv>(
    mdfr: &TrimPath, draws: &mut [DrawItem<VGPath, VGPaint, TM2D>], fnth: f32) {
    let (start, trim) = normalize_trim(
        mdfr.start .get_value(fnth) as f64 / 100.,
        mdfr.end   .get_value(fnth) as f64 / 100.,
        mdfr.offset.get_value(fnth) as f64 / 360.,
    );
    if trim <= 0. {
        for_each_path_mut(draws, &mut |path| *path = VGPath::new(0));
        return
    }
    if 1. <= trim { return }

    if mdfr.multiple.is_some_and(|ml| matches!(ml, TrimMultiple::Simultaneously)) {
        for_each_path_mut(draws, &mut |path| *path = path.trim_path(start, trim));
    } else {
        let (mut idx, mut total) = (0usize, 0.);
        let mut paths = Vec::new();

        for_each_path_mut(draws, &mut |path| {
            let path = MeasuredPath::new(path.to_kurbo());
            total += path.length;
            paths.push(path);
        });
        if total == 0. {
            for_each_path_mut(draws, &mut |path| *path = VGPath::new(0));
            return
        }

        let end = start + trim;
        let ranges = if end <= 1. {
            [(start * total, end * total), (0., 0.)]
        } else {
            [(start * total, total), (0., (end - 1.) * total)]
        };
        let mut path_start = 0.;
        for_each_path_mut(draws, &mut |path| {
            let measured = &paths[idx]; idx += 1;
            let length = measured.length;
            let path_end = path_start + length;
            let (mut local, mut count) = ([(0., 0.); 2], 0);
            if 0. < length {
                for &(from, to) in &ranges {
                    let (lo, hi) = (from.max(path_start), to.min(path_end));
                    if lo < hi {
                        local[count] =
                            ((lo - path_start) / length, (hi - path_start) / length);
                        count += 1;
                    }
                }
            }
            *path = VGPath::from_kurbo(measured.trim_ranges(&local[..count]));
            path_start = path_end;
        });
    }
}

fn normalize_trim(start: f64, end: f64, offset: f64) -> (f64, f64) {
    let (start, end) = (start.clamp(0., 1.), end.clamp(0., 1.));
    ((start.min(end) + offset).rem_euclid(1.), (end - start).abs())
}

#[cfg(test)] mod tests { use super::*;
    use crate::core::{helpers::Vec2D, pathm::BezPath};
    use kurbo::ParamCurveArclen;

    struct TestStyle;
    impl StyleConv for TestStyle {
        fn solid_color(_: RGBA) -> Self { Self }
        fn linear_gradient(_: Vec2D, _: Vec2D, _: &[(f32, RGBA)]) -> Self { Self }
        fn radial_gradient(_: Vec2D, _: Vec2D, _: (f32, f32), _: &[(f32, RGBA)]) -> Self { Self }
    }

    #[derive(Default)] struct TestContext {
        clear: Option<RGBA>, clear_count: u32,
    }
    impl RenderContext for TestContext {
        type VGPath = BezPath;
        type VGStyle = TestStyle;
        type TM2D = kurbo::Affine;
        type ImageID = ();

        fn get_size(&self) -> (u32, u32) { (1, 1) }
        fn clear_rect_with(&mut self, _: u32, _: u32, _: u32, _: u32, color: RGBA) {
            self.clear = Some(color); self.clear_count += 1;
        }
        fn save_state(&mut self) {}
        fn restore_state(&mut self) {}
        fn apply_transform(&mut self, _: &Self::TM2D, _: Option<f32>) {}
        fn fill_stroke(&mut self, _: &Self::VGPath, _: &RefCell<(Self::VGStyle, FSOpts)>) {}
    }

    #[test] fn layer_world_matrix_includes_ancestors_and_tolerates_missing_parent() {
        let animation: Animation = serde_json::from_str(r#"{ "layers": [
            {"ty":3,"ind":1,"st":0,"ip":0,"op":10,"ks":{"p":{"k":[10,0]}}},
            {"ty":3,"ind":2,"parent":1,"st":0,"ip":0,"op":10,"ks":{"p":{"k":[0,20]}}},
            {"ty":3,"ind":3,"parent":2,"st":0,"ip":0,"op":10,"ks":{"p":{"k":[3,4]}}},
            {"ty":3,"ind":4,"parent":99,"st":0,"ip":0,"op":10,"ks":{"p":{"k":[5,6]}}}
        ] }"#).unwrap();

        let matrices = layer_world_matrices::<kurbo::Affine>(
            &animation.layers, 0., |_| true);
        assert_eq!(matrices[2].as_ref().unwrap().0.as_coeffs(),
            [1., 0., 0., 1., 13., 24.]);
        assert_eq!(matrices[3].as_ref().unwrap().0.as_coeffs(),
            [1., 0., 0., 1., 5., 6.]);
    }

    #[test] fn parent_layers_contribute_transform_but_not_opacity() {
        let animation: Animation = serde_json::from_str(r#"{ "layers": [
            {"ty":3,"ind":1,"hd":true,"st":0,"ip":0,"op":10,
                "ks":{"p":{"k":[10,0]},"o":{"k":25}}},
            {"ty":3,"ind":2,"parent":1,"st":0,"ip":0,"op":10,
                "ks":{"p":{"k":[0,20]},"o":{"k":50}}},
            {"ty":3,"ind":3,"parent":2,"st":0,"ip":0,"op":10,
                "ks":{"p":{"k":[3,4]},"o":{"k":80}}}
        ] }"#).unwrap();

        let matrices = layer_world_matrices::<kurbo::Affine>(
            &animation.layers, 0., |_| true);
        let child = matrices[2].as_ref().unwrap();
        assert_eq!(child.0.as_coeffs(), [1., 0., 0., 1., 13., 24.]);
        assert_eq!(child.1, 0.8);
    }

    #[test] fn layer_world_matrix_skips_parent_cycles_and_their_descendants() {
        let animation: Animation = serde_json::from_str(r#"{ "layers": [
            {"ty":3,"ind":1,"parent":2,"st":0,"ip":0,"op":10,"ks":{}},
            {"ty":3,"ind":2,"parent":1,"st":0,"ip":0,"op":10,"ks":{}},
            {"ty":3,"ind":3,"parent":1,"st":0,"ip":0,"op":10,"ks":{}},
            {"ty":3,"ind":4,"st":0,"ip":0,"op":10,"ks":{"p":{"k":[5,6]}}}
        ] }"#).unwrap();

        let matrices = layer_world_matrices::<kurbo::Affine>(
            &animation.layers, 0., |_| true);
        assert!(matrices[..3].iter().all(Option::is_none));
        assert_eq!(matrices[3].as_ref().unwrap().0.as_coeffs(),
            [1., 0., 0., 1., 5., 6.]);
    }

    #[test] fn layer_world_matrix_only_evaluates_required_layers_and_their_parents() {
        let animation: Animation = serde_json::from_str(r#"{ "layers": [
            {"ty":3,"ind":1,"st":0,"ip":0,"op":10,"ks":{"p":{"k":[10,0]}}},
            {"ty":3,"ind":2,"parent":1,"st":0,"ip":0,"op":10,"ks":{"p":{"k":[0,20]}}},
            {"ty":3,"ind":3,"st":0,"ip":0,"op":10,"ks":{"p":{"k":[30,40]}}}
        ] }"#).unwrap();

        let matrices = layer_world_matrices::<kurbo::Affine>(
            &animation.layers, 0., |layer|
                layer.visual_layer().is_some_and(|vl| vl.base.ind == Some(2)));
        assert!(matrices[0].is_some());
        assert_eq!(matrices[1].as_ref().unwrap().0.as_coeffs(),
            [1., 0., 0., 1., 10., 20.]);
        assert!(matrices[2].is_none());
    }

    #[test] fn layer_world_matrix_handles_deep_parent_chains_iteratively() {
        use std::fmt::Write;
        const COUNT: u32 = 4096;
        let mut json = String::from("{\"layers\":[");
        for id in 1..=COUNT {
            if 1 < id { json.push(','); }
            write!(json, "{{\"ty\":3,\"ind\":{id},\"st\":0,\"ip\":0,\"op\":10,\"ks\":{{}}")
                .unwrap();
            if 1 < id { write!(json, ",\"parent\":{}", id - 1).unwrap(); }
            json.push('}');
        }
        json.push_str("]}");
        let animation: Animation = serde_json::from_str(&json).unwrap();

        let matrices = layer_world_matrices::<kurbo::Affine>(
            &animation.layers, 0., |layer|
                layer.visual_layer().is_some_and(|vl| vl.base.ind == Some(COUNT)));
        assert!(matrices.iter().all(Option::is_some));
    }

    #[test] fn playback_starts_and_wraps_at_the_in_point() {
        let mut animation: Animation =
            serde_json::from_str(r#"{"ip":10,"op":12,"fr":1,"layers":[]}"#).unwrap();
        let mut context = TestContext::default();

        assert!(animation.render_next_frame(
            &mut context, 1., Some(RGBA::new_u8(0, 0, 0, 0))));
        assert_eq!(animation.fnth, 11.);
        assert!(animation.render_next_frame(
            &mut context, 1., Some(RGBA::new_u8(0, 0, 0, 0))));
        assert_eq!(animation.fnth, 10.);

        assert!(animation.render_next_frame(
            &mut context, 2., Some(RGBA::new_u8(0, 0, 0, 0))));
        assert_eq!(animation.fnth, 10.);
    }

    #[test] fn frame_clear_supports_transparent_color_and_preserve_modes() {
        let mut animation: Animation =
            serde_json::from_str(r#"{"ip":0,"op":10,"fr":1,"layers":[]}"#).unwrap();
        let mut context = TestContext::default();

        assert!(animation.render_next_frame(
            &mut context, 1., Some(RGBA::new_u8(0, 0, 0, 0))));
        let clear = context.clear.unwrap();
        assert_eq!((clear.r, clear.g, clear.b, clear.a), (0, 0, 0, 0));

        let red = RGBA::new_u8(255, 0, 0, 128);
        assert!(animation.render_next_frame(&mut context, 1., Some(red)));
        let clear = context.clear.unwrap();
        assert_eq!((clear.r, clear.g, clear.b, clear.a), (255, 0, 0, 128));

        assert!(animation.render_next_frame(&mut context, 1., None));
        assert_eq!(context.clear_count, 2);
    }

    #[test] fn recursive_render_restores_opacity_between_siblings() {
        #[derive(Default)] struct StateContext {
            opacity: f32, stack: Vec<f32>, drawn: Vec<f32>,
        }
        impl RenderContext for StateContext {
            type VGPath = BezPath;
            type VGStyle = TestStyle;
            type TM2D = kurbo::Affine;
            type ImageID = ();

            fn get_size(&self) -> (u32, u32) { (1, 1) }
            fn clear_rect_with(&mut self, _: u32, _: u32, _: u32, _: u32, _: RGBA) {}
            fn save_state(&mut self) { self.stack.push(self.opacity) }
            fn restore_state(&mut self) { self.opacity = self.stack.pop().unwrap() }
            fn apply_transform(&mut self, _: &Self::TM2D, opacity: Option<f32>) {
                if let Some(opacity) = opacity { self.opacity = opacity }
            }
            fn fill_stroke(&mut self, _: &Self::VGPath,
                _: &RefCell<(Self::VGStyle, FSOpts)>) {
                self.drawn.push(self.opacity);
            }
        }

        let path = || DrawItem::Shape(BezPath::new());
        let draws = vec![
            path(),
            DrawItem::Group(vec![path()],
                vec![TM2DwO(kurbo::Affine::IDENTITY, 0.4)]),
            DrawItem::Style(RefCell::new(
                (TestStyle, FSOpts::Fill(FillRule::NonZero)))),
        ];
        let mut context = StateContext { opacity: 1., ..Default::default() };

        context.render_shapes(
            &TM2DwO(kurbo::Affine::IDENTITY, 0.5), &draws);
        assert_eq!(context.drawn, [0.2, 0.5]);
        assert_eq!(context.opacity, 1.);
        assert!(context.stack.is_empty());
    }

    #[test] fn trim_range_normalizes_direction_and_negative_offset() {
        assert_eq!(normalize_trim(0., 0.5, -0.25), (0.75, 0.5));
        assert_eq!(normalize_trim(0., 0.5,  0.75), (0.75, 0.5));
        assert_eq!(normalize_trim(0.75, 0.25, 0.), (0.25, 0.5));
        assert_eq!(normalize_trim(0., 0.5, -1.25), (0.75, 0.5));
        assert_eq!(normalize_trim(0., 1.5,  0.25), (0.25, 1.0));
        assert_eq!(normalize_trim(-0.5, 0.5, 0.), (0., 0.5));
        assert_eq!(normalize_trim(1.5, 2., 0.), (0., 0.));
    }

    #[test] fn sequential_trim_keeps_both_wrapped_parts_of_one_shape() {
        let trim: TrimPath = serde_json::from_str(r#"{
            "nm":"","ln":"","cl":"",
            "s":{"a":0,"k":0},"e":{"a":0,"k":50},"o":{"a":0,"k":270},"m":2
        }"#).unwrap();
        let mut path = BezPath::new();
        path.move_to((0., 0.)); path.line_to((100., 0.));
        let mut draws: Vec<DrawItem<BezPath, TestStyle, kurbo::Affine>> =
            vec![DrawItem::Shape(path)];

        trim_shapes(&trim, &mut draws, 0.);
        let DrawItem::Shape(path) = &draws[0] else { panic!() };
        let segments = path.segments().collect::<Vec<_>>();
        assert_eq!(segments.len(), 2);
        use kurbo::ParamCurve;
        assert_eq!((segments[0].start().x, segments[0].end().x), (75., 100.));
        assert_eq!((segments[1].start().x, segments[1].end().x), (0., 25.));
    }

    #[test] fn sequential_trim_follows_reverse_render_order() {
        let trim: TrimPath = serde_json::from_str(r#"{
            "nm":"","ln":"","cl":"",
            "s":{"a":0,"k":0},"e":{"a":0,"k":25},"o":{"a":0,"k":0},"m":2
        }"#).unwrap();
        let line = |length, y| {
            let mut path = BezPath::new();
            path.move_to((0., y)); path.line_to((length, y)); path
        };
        let mut draws: Vec<DrawItem<BezPath, TestStyle, kurbo::Affine>> = vec![
            DrawItem::Shape(line(100., 0.)),
            DrawItem::Shape(line( 50., 1.)),
        ];

        trim_shapes(&trim, &mut draws, 0.);
        let [DrawItem::Shape(first), DrawItem::Shape(second)] = &draws[..] else { panic!() };
        assert!(first.is_empty());
        use kurbo::ParamCurve;
        let segment = second.segments().next().unwrap();
        assert_eq!((segment.start().x, segment.end().x), (0., 37.5));
    }

    #[test] fn sequential_trim_follows_nested_group_render_order() {
        let trim: TrimPath = serde_json::from_str(r#"{
            "nm":"","ln":"","cl":"",
            "s":{"a":0,"k":0},"e":{"a":0,"k":25},"o":{"a":0,"k":0},"m":2
        }"#).unwrap();
        let line = |length, y| {
            let mut path = BezPath::new();
            path.move_to((0., y)); path.line_to((length, y)); path
        };
        let group = vec![
            DrawItem::Shape(line(30., 1.)),
            DrawItem::Shape(line(20., 2.)),
        ];
        let mut draws: Vec<DrawItem<BezPath, TestStyle, kurbo::Affine>> = vec![
            DrawItem::Shape(line(100., 0.)),
            DrawItem::Group(group, vec![TM2DwO::default()]),
        ];

        trim_shapes(&trim, &mut draws, 0.);
        let [DrawItem::Shape(first), DrawItem::Group(group, _)] = &draws[..] else { panic!() };
        let [DrawItem::Shape(middle), DrawItem::Shape(last)] = &group[..] else { panic!() };
        assert!(first.is_empty());
        assert_eq!(last.segments().next().unwrap().arclen(0.1), 20.);
        assert_eq!(middle.segments().next().unwrap().arclen(0.1), 17.5);
    }

    #[test] fn simultaneous_trim_applies_the_range_to_each_path() {
        let trim: TrimPath = serde_json::from_str(r#"{
            "nm":"","ln":"","cl":"",
            "s":{"a":0,"k":25},"e":{"a":0,"k":75},"o":{"a":0,"k":0},"m":1
        }"#).unwrap();
        let line = |length, y| {
            let mut path = BezPath::new();
            path.move_to((0., y)); path.line_to((length, y)); path
        };
        let mut draws: Vec<DrawItem<BezPath, TestStyle, kurbo::Affine>> = vec![
            DrawItem::Shape(line(100., 0.)),
            DrawItem::Shape(line(40., 1.)),
        ];

        trim_shapes(&trim, &mut draws, 0.);
        let [DrawItem::Shape(first), DrawItem::Shape(second)] = &draws[..] else { panic!() };
        assert_eq!(first.segments().next().unwrap().arclen(0.1), 50.);
        assert_eq!(second.segments().next().unwrap().arclen(0.1), 20.);
    }
}
