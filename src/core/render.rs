/****************************************************************
 * $ID: render.rs  	Fri 03 May 2024 22:07:36+0800               *
 *                                                              *
 * Maintainer: 范美辉 (MeiHui FAN) <mhfan@ustc.edu>              *
 * Copyright (c) 2024 M.H.Fan, All rights reserved.             *
 ****************************************************************/

use core::cell::RefCell;
use std::{collections::HashMap, rc::Rc};
use super::{helpers::{RGBA, IntBool}, style::{StyleConv, MatrixConv, TM2DwO, FSOpts},
    path_ops::MeasuredPath,
    schema::{Animation, AssetItem, LayerItem, ShapeItem, VisualLayer,
        TrimPath, TrimMultiple, MatteMode, FillRule},
    pathm::{PathBuilder, PathFactory}
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)] enum Parent { Root, Layer(u32), Invalid }
enum WorldState<MC: MatrixConv> { Pending, Invalid, Ready(TM2DwO<MC>) }

struct CompositionState {
     parents: Vec<Parent>, stack: Vec<usize>,
    precomps: Vec<Option<PrecompState>>,
}

struct PrecompState { asset: usize, composition: Box<CompositionState> }

impl CompositionState {
    fn new(layers: &[LayerItem]) -> Self {
        let _: u32 = layers.len().try_into().expect("too many composition layers");
        let mut indices = HashMap::<u32, Option<u32>>::with_capacity(layers.len());
        for (index, layer) in layers.iter().enumerate() {
            if let Some(id) = layer.visual_layer().and_then(|vl| vl.base.ind) {
                indices.entry(id).and_modify(|index| *index = None)
                    .or_insert(Some(index as u32));
            }
        }
        let parents: Vec<_> = layers.iter().map(|layer| layer.visual_layer().and_then(|vl|
            vl.base.parent.and_then(|id| indices.get(&id).copied().flatten()))).collect();
        let (mut states, mut stack) = (vec![0u8; layers.len()], Vec::new());

        for root in 0..layers.len() {
            if 1 < states[root] { continue }
            let mut index = root; stack.clear();
            let resolved = loop { match states[index] {
                0 => {
                    states[index] = 1;  stack.push(index);
                    if layers[index].visual_layer().is_none() { break false }
                    let Some(parent) = parents[index] else { break true };
                    index = parent as usize;
                }
                2 => break true,
                1 | 3 => break false,
                _ => unreachable!(),
            } };
            while let Some(index) = stack.pop() {
                states[index] = if resolved { 2 } else { 3 };
            }
        }
        let mut precomps = Vec::with_capacity(layers.len());
        precomps.resize_with(layers.len(), || None);

        let parents = parents.into_iter().zip(states).map(|(parent, state)| {
            if state != 2 { Parent::Invalid }
            else { parent.map_or(Parent::Root, Parent::Layer) }
        }).collect();
        Self { parents, stack, precomps }
    }

    fn with_precomps<'a>(layers: &[LayerItem], animation: &'a Animation,
        assets: &HashMap<&'a str, usize>, ancestors: &mut Vec<&'a str>) -> Self {
        let mut runtime = Self::new(layers);
        for (index, layer) in layers.iter().enumerate() {
            let LayerItem::PrecompLayer(layer) = layer else { continue };
            let Some(&asset) = assets.get(layer.rid.as_str()) else { continue };
            let AssetItem::Precomp(precomp) = &animation.assets[asset] else { unreachable!() };
            if ancestors.contains(&precomp.base.id.as_str()) { continue }

            ancestors.push(&precomp.base.id);
            let composition =
                Self::with_precomps(&precomp.layers, animation, assets, ancestors);
            ancestors.pop();
            runtime.precomps[index] =
                Some(PrecompState { asset, composition: Box::new(composition) });
        }
        runtime
    }

    fn evaluate<MC: MatrixConv>(&mut self, layers: &[LayerItem], global: f32,
        mut required: impl FnMut(&LayerItem) -> bool) -> Vec<WorldState<MC>> {
        debug_assert_eq!(layers.len(), self.parents.len());
        let mut worlds = Vec::with_capacity(layers.len());
        worlds.resize_with(layers.len(), || WorldState::Pending);
        for (index, layer) in layers.iter().enumerate() {
            if  self.parents[index] != Parent::Invalid && required(layer) {
                Self::resolve(&self.parents, index, layers, global,
                    &mut worlds, &mut self.stack);
            }
        }   worlds
    }

    fn resolve<MC: MatrixConv>(parents: &[Parent], root: usize,
        layers: &[LayerItem], global: f32,
        worlds: &mut [WorldState<MC>], stack: &mut Vec<usize>) {
        if !matches!(worlds[root], WorldState::Pending) { return }

        stack.clear();
        let mut index = root;
        while matches!(worlds[index], WorldState::Pending) {
            stack.push(index);
            match parents[index] {
                Parent::Layer(parent) => index = parent as usize,
                Parent::Invalid => unreachable!(),
                Parent::Root => break,
            }
        }

        while let Some(index) = stack.pop() {
            let Some(vl) = layers[index].visual_layer() else { unreachable!() };
            let Some(local) = vl.base.local_frame(global) else {
                worlds[index] = WorldState::Invalid; continue
            };
            let mut world = vl.ks.to_matrix(local, vl.ao);
            if let Parent::Layer(parent) = parents[index] {
                let WorldState::Ready(parent) = &worlds[parent as usize] else {
                    worlds[index] = WorldState::Invalid; continue
                };
                world = world.compose_matrix(&parent.0);
            }
            worlds[index] = WorldState::Ready(world);
        }
    }
}

pub struct LottieRuntime {
    elapsed: f32, fnth: f32,
    animation: Animation,
    root: CompositionState,
}

impl LottieRuntime {
    pub fn from_reader<R: std::io::Read>(reader: R) -> Result<Self, serde_json::Error> {
        let animation = Animation::from_reader(reader)?;
        let root = {
            let mut assets = HashMap::with_capacity(animation.assets.len());
            for (index, asset) in animation.assets.iter().enumerate() {
                if let AssetItem::Precomp(precomp) = asset {
                    assets.entry(precomp.base.id.as_str()).or_insert(index);
                }
            }
            CompositionState::with_precomps(
                &animation.layers, &animation, &assets, &mut Vec::new())
        };
        let fnth = animation.ip;
        Ok(Self { animation, elapsed: 0., fnth, root })
    }

    pub fn animation(&self) -> &Animation { &self.animation }
    pub fn frame(&self) -> f32 { self.fnth }

    /// `clear` selects a frame background; `None` preserves the current render target.
    pub fn render_next_frame<RC: RenderContext>(&mut self,
        rctx: &mut RC, elapsed: f32, clear: Option<RGBA>) -> bool {
        //debug_assert!(0. < self.fr && 0. <= self.ip && 1. < self.op - self.ip);
        let animation = &self.animation;

        if  self.fnth < animation.ip || animation.op <= self.fnth {
            self.fnth = animation.ip;
        }   self.elapsed += elapsed * animation.fr;
        if  self.elapsed < 1. && animation.ip < self.fnth { return false }

        if  2. <= self.elapsed {    // advance/skip elapsed frames
            let elapsed = (self.elapsed - 1.).floor();
            let duration =  animation.op - animation.ip;
            if 0. < duration {
                self.fnth = animation.ip +
                    (self.fnth -  animation.ip + elapsed).rem_euclid(duration);
            }   self.elapsed -= elapsed;
        }

        if let Some(color) = clear {
            let (width, height) = rctx.get_size();
            rctx.clear_rect_with(0, 0, width, height, color);
        }
        Self::render_layers(animation, rctx, &TM2DwO::default(),
            &animation.layers, self.fnth, &mut self.root);

        self.elapsed -= 1.;       self.fnth += 1.;
        if animation.op <= self.fnth { self.fnth = animation.ip; }    true
    }

    /// The render order goes from the last element to the first,
    /// items in list coming first will be rendered on top.
    fn render_layers<RC: RenderContext>(animation: &Animation, rctx: &mut RC,
        ptm: &TM2DwO<RC::TM2D>, layers: &[LayerItem], fnth: f32,
        runtime: &mut CompositionState) {
        let mut matte = None;
        let worlds = runtime.evaluate(layers, fnth, |layer| match layer {
            LayerItem::Shape(layer) => !layer.vl.should_hide(fnth),
            LayerItem::PrecompLayer(layer) => !layer.vl.should_hide(fnth),
            LayerItem::SolidColor(layer) => !layer.vl.should_hide(fnth),
            _ => false,
        });

        for (index, layer) in layers.iter().enumerate().rev() { match layer {
            LayerItem::Shape(shpl) =>
            if let WorldState::Ready(ltm) = &worlds[index] {
                let Some(local) = shpl.vl.base.local_frame(fnth) else { continue };
                let (draws, ctm) = convert_shapes(&shpl.shapes, local, shpl.vl.ao);
                let ltm = ltm.clone().compose(ptm);

                rctx.prepare_matte(&shpl.vl, &mut matte);
                rctx.render_shapes(&ctm.compose(&ltm), &draws);
                rctx.compose_matte(&shpl.vl, &mut matte, &ltm, fnth);
            }
            LayerItem::PrecompLayer(pcl) =>
            if let WorldState::Ready(ltm) = &worlds[index] {
                if let Some(child) = &mut runtime.precomps[index] {
                    let AssetItem::Precomp(pcomp) =
                        &animation.assets[child.asset] else { unreachable!() };
                    let Some(local) = pcl.vl.base.local_frame(fnth) else { continue };
                    let child_fnth = pcl.tm.as_ref().map_or(local,
                        |tm| tm.get_value(local) * animation.fr);
                    let ltm = ltm.clone().compose(ptm);

                    rctx.prepare_matte(&pcl.vl, &mut matte);
                    Self::render_layers(animation, rctx, &ltm, &pcomp.layers, child_fnth,
                        &mut child.composition);
                    rctx.compose_matte(&pcl.vl, &mut matte, &ltm, fnth);
                }   // XXX: clipping(pcl.w, pcl.h)?
            }
            LayerItem::SolidColor(scl) =>
            if let WorldState::Ready(ltm) = &worlds[index] {
                let ltm = ltm.clone().compose(ptm);
                let mut path = RC::VGPath::new(5);
                path.rect(0., 0., scl.sw, scl.sh);

                rctx.prepare_matte(&scl.vl, &mut matte);
                rctx.render_shapes(&ltm, &[DrawItem::Shape(path),
                    DrawItem::Style(Rc::new(RefCell::new((RC::VGStyle::solid_color(scl.sc),
                        FSOpts::Fill(FillRule::NonZero)))))]);
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
    fn fill_stroke(&mut self, path: &Self::VGPath,
        relative: Option<&Self::TM2D>, style: &RefCell<(Self::VGStyle, FSOpts)>);

    fn traverse_shapes(&mut self, stm: &TM2DwO<Self::TM2D>,
        relative: Option<&TM2DwO<Self::TM2D>>,
        draws: &[DrawItem<Self::VGPath, Self::VGStyle, Self::TM2D>],
        style: &RefCell<(Self::VGStyle, FSOpts)>) {
        self.apply_transform(&stm.0, Some(stm.1 * relative.map_or(1., |tm| tm.1)));
        draws.iter().rev().for_each(|draw| match draw {
            DrawItem::Shape(path) => self.fill_stroke(path, relative.map(|tm| &tm.0), style),
            DrawItem::Group(grp, rep) => rep.iter().rev().for_each(|gtm| {
                let child = Some(match relative {
                    Some(relative) => gtm.clone().compose(relative),
                    None => gtm.clone(),
                });
                self.traverse_shapes(stm, child.as_ref(), grp, style);
                self.apply_transform(&stm.0, Some(stm.1 * relative.map_or(1., |tm| tm.1)));
            }),
            DrawItem::Copies(copies) => copies.iter().rev().for_each(|(grp, gtm)| {
                let child = Some(match relative {
                    Some(relative) => gtm.clone().compose(relative),
                    None => gtm.clone(),
                });
                self.traverse_shapes(stm, child.as_ref(), grp, style);
                self.apply_transform(&stm.0, Some(stm.1 * relative.map_or(1., |tm| tm.1)));
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
                self.traverse_shapes(ptm, None, &draws[0..idx], style),
            DrawItem::Group(grp, rep) => rep.iter().rev().for_each(|gtm|
                    self.render_shapes_inner(&gtm.clone().compose(ptm), grp)),
            DrawItem::Copies(copies) => copies.iter().rev().for_each(|(grp, gtm)|
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
pub fn convert_shapes<VGPath: PathBuilder, VGPaint: StyleConv, TM2D: MatrixConv + Clone>(
    shapes: &[ShapeItem], fnth: f32, ao: IntBool) ->
    (Vec<DrawItem<VGPath, VGPaint, TM2D>>, TM2DwO<TM2D>) {
    let mut draws: Vec<DrawItem<VGPath, VGPaint, TM2D>> = Vec::with_capacity(shapes.len());
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
            draws.push(DrawItem::Style(Rc::new(fill.to_style(fnth).into()))),
        ShapeItem::Stroke(line) if !line.elem.hd =>
            draws.push(DrawItem::Style(Rc::new(line.to_style(fnth).into()))),
        ShapeItem::GradientFill(grad)   if !grad.elem.hd =>
            draws.push(DrawItem::Style(Rc::new(grad.to_style(fnth).into()))),
        ShapeItem::GradientStroke(grad) if !grad.elem.hd =>
            draws.push(DrawItem::Style(Rc::new(grad.to_style(fnth).into()))),
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
        ShapeItem::RoundedCorners(mdfr) if !mdfr.elem.hd => {
            let radius = mdfr.radius.get_value(fnth);
            for_each_path_mut(&mut draws, &mut |path| path.round_corners(radius));
        }
        ShapeItem::OffsetPath(mdfr) if !mdfr.elem.hd => {
            let amount = mdfr.amount.as_ref().map_or(0., |amount| amount.get_value(fnth));
            let limit = mdfr.ml.as_ref().map_or(4., |limit| limit.get_value(fnth));
            for_each_path_mut(&mut draws,
                &mut |path| path.offset_path(amount, mdfr.lj, limit));
        }

        ShapeItem::Merge (_) | ShapeItem::PuckerBloat(_) |
        ShapeItem::Twist (_) | ShapeItem::ZigZag(_) => dbg!(),  // TODO:

        ShapeItem::Transform(ts) if !ts.elem.hd => ctm = ts.trfm.to_matrix(fnth, ao),

        _ => (),
    } }     (draws, ctm)
}

//  https://lottie.github.io/lottie-spec/latest/specs/shapes/#graphic-element
pub enum DrawItem<VGPath: PathBuilder, VGPaint: StyleConv, TM2D: MatrixConv> {
    Shape(VGPath),                          // DrawItem is a.k.a Graphic Element
    Style(Rc<RefCell<(VGPaint, FSOpts)>>),  // shared only by expanded repeater copies
    Group(Vec<Self>, Vec<TM2DwO<TM2D>>),    // support batch Groups for Repeater
    Copies(Vec<(Vec<Self>, TM2DwO<TM2D>)>), // per-copy paths after sequential trim
}

fn for_each_path_mut<VGPath: PathBuilder, VGPaint: StyleConv, TM2D: MatrixConv>(
    draws: &mut [DrawItem<VGPath, VGPaint, TM2D>],
    closure: &mut impl FnMut(&mut VGPath)) {
    draws.iter_mut().rev().for_each(|draw| match draw {
        DrawItem::Group(group, _) => for_each_path_mut(group, closure),
        DrawItem::Copies(copies) => copies.iter_mut().rev()
            .for_each(|(group, _)| for_each_path_mut(group, closure)),
        DrawItem::Shape(path) => closure(path),
        DrawItem::Style(_) => (), // skip/ignore Style
    });
}

fn duplicate_draws<VGPath: PathBuilder, VGPaint: StyleConv, TM2D: MatrixConv + Clone>(
    draws: &[DrawItem<VGPath, VGPaint, TM2D>],
    paths: &mut HashMap<usize, super::pathm::BezPath>) ->
    Vec<DrawItem<VGPath, VGPaint, TM2D>> {
    draws.iter().map(|draw| match draw {
        DrawItem::Shape(path) => {
            let key = path as *const VGPath as usize;
            let path = paths.entry(key).or_insert_with(|| path.to_kurbo()).clone();
            DrawItem::Shape(VGPath::from_kurbo(path))
        }
        DrawItem::Style(style) => DrawItem::Style(Rc::clone(style)),
        DrawItem::Group(group, transforms) =>
            DrawItem::Group(duplicate_draws(group, paths), transforms.clone()),
        DrawItem::Copies(copies) => DrawItem::Copies(copies.iter().map(|(group, transform)|
            (duplicate_draws(group, paths), transform.clone())).collect()),
    }).collect()
}

fn expand_repeats<VGPath: PathBuilder, VGPaint: StyleConv, TM2D: MatrixConv + Clone>(
    draws: &mut [DrawItem<VGPath, VGPaint, TM2D>]) {
    for draw in draws { match draw {
        DrawItem::Group(group, transforms) => {
            expand_repeats(group);
            if transforms.len() == 1 { continue }

            let mut source = core::mem::take(group);
            let transforms = core::mem::take(transforms);
            let last = transforms.len().saturating_sub(1);
            let mut paths = HashMap::new();
            let copies = transforms.into_iter().enumerate().map(|(index, transform)| {
                let group = if index == last {
                    core::mem::take(&mut source)
                } else { duplicate_draws(&source, &mut paths) };
                (group, transform)
            }).collect();
            *draw = DrawItem::Copies(copies);
        }
        DrawItem::Copies(copies) => copies.iter_mut()
            .for_each(|(group, _)| expand_repeats(group)),
        DrawItem::Shape(_) | DrawItem::Style(_) => (),
    }}
}

fn trim_shapes<VGPath: PathBuilder, VGPaint: StyleConv, TM2D: MatrixConv + Clone>(
    mdfr: &TrimPath, draws: &mut [DrawItem<VGPath, VGPaint, TM2D>], fnth: f32) {
    let (start, trim) = normalize_trim(mdfr.start .get_value(fnth) / 100.,
                                       mdfr.end   .get_value(fnth) / 100.,
                                       mdfr.offset.get_value(fnth) / 360.);
    if  trim <= 0. {
        for_each_path_mut(draws, &mut |path| *path = VGPath::new(0)); return
    } else if 1. <= trim { return }

    if mdfr.multiple.is_some_and(|ml| matches!(ml, TrimMultiple::Simultaneously)) {
        for_each_path_mut(draws, &mut |path| path.trim_path(start, trim));
    } else {
        expand_repeats(draws);
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
            [(f64::from(start) * total, f64::from(end) * total), (0., 0.)]
        } else {
            [(f64::from(start) * total, total),
             (0., f64::from(end - 1.) * total)]
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

fn normalize_trim(start: f32, end: f32, offset: f32) -> (f32, f32) {
    let (start, end) = (start.clamp(0., 1.), end.clamp(0., 1.));
    ((start.min(end) + offset).rem_euclid(1.), (end - start).abs())
}

#[cfg(test)] mod tests { use super::*;
    use crate::core::{helpers::Vec2D, pathm::BezPath};
    use kurbo::ParamCurveArclen;

    fn layer_world_matrices<MC: MatrixConv>(
        layers: &[LayerItem], global: f32,
        required: impl FnMut(&LayerItem) -> bool) -> Vec<Option<TM2DwO<MC>>> {
        CompositionState::new(layers).evaluate(layers, global, required)
            .into_iter().map(|world| match world {
            WorldState::Ready(world) => Some(world),
            WorldState::Pending | WorldState::Invalid => None,
        }).collect()
    }

    struct TestStyle;
    impl StyleConv for TestStyle {
        fn solid_color(_: RGBA) -> Self { Self }
        fn linear_gradient(_: Vec2D, _: Vec2D, _: &[(f32, RGBA)]) -> Self { Self }
        fn radial_gradient(_: Vec2D, _: Vec2D, _: (f32, f32), _: &[(f32, RGBA)]) -> Self { Self }
    }

    #[derive(Default)] struct TestContext {
        clear: Option<RGBA>, clear_count: u32, draw_count: u32,
        current: kurbo::Affine, transforms: Vec<kurbo::Affine>,
        fills: Vec<(kurbo::Affine, Option<kurbo::Affine>)>,
        opacity: f32, stack: Vec<f32>, drawn: Vec<f32>,
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
        fn save_state(&mut self) { self.stack.push(self.opacity) }
        fn restore_state(&mut self) { self.opacity = self.stack.pop().unwrap() }
        fn apply_transform(&mut self, transform: &Self::TM2D, opacity: Option<f32>) {
            self.current = *transform; self.transforms.push(*transform);
            if let Some(opacity) = opacity { self.opacity = opacity }
        }
        fn fill_stroke(&mut self, _: &Self::VGPath, relative: Option<&Self::TM2D>,
            _: &RefCell<(Self::VGStyle, FSOpts)>) {
            self.draw_count += 1; self.drawn.push(self.opacity);
            self.fills.push((self.current, relative.copied()));
        }
    }

    fn line(length: f32, y: f32) -> BezPath {
        let mut path = BezPath::new();
        path.move_to((0., y)); path.line_to((length, y)); path
    }

    fn trim(start: f32, end: f32, offset: f32, multiple: u8) -> TrimPath {
        serde_json::from_str(&format!(r#"{{
            "s":{{"k":{start}}},"e":{{"k":{end}}},
            "o":{{"k":{offset}}},"m":{multiple}
        }}"#)).unwrap()
    }

    fn path_length(path: &BezPath) -> f32 {
        path.segments().map(|segment| segment.arclen(0.1)).sum::<f64>() as _
    }

    fn fill_style() -> DrawItem<BezPath, TestStyle, kurbo::Affine> {
        DrawItem::Style(Rc::new(RefCell::new(
            (TestStyle, FSOpts::Fill(FillRule::NonZero)))))
    }

    #[test] fn layer_world_matrix_composes_parents_without_inheriting_opacity() {
        let animation: Animation = serde_json::from_str(r#"{ "layers": [
            {"ty":3,"ind":1,"hd":true,"st":0,"ip":0,"op":10,
                "ks":{"p":{"k":[10,0]},"o":{"k":25}}},
            {"ty":3,"ind":2,"parent":1,"st":0,"ip":0,"op":10,
                "ks":{"p":{"k":[0,20]},"o":{"k":50}}},
            {"ty":3,"ind":3,"parent":2,"st":0,"ip":0,"op":10,
                "ks":{"p":{"k":[3,4]},"o":{"k":80}}},
            {"ty":3,"ind":4,"parent":99,"st":0,"ip":0,"op":10,"ks":{"p":{"k":[5,6]}}}
        ] }"#).unwrap();

        let matrices = layer_world_matrices::<kurbo::Affine>(
            &animation.layers, 0., |_| true);
        let child = matrices[2].as_ref().unwrap();
        assert_eq!(child.0.as_coeffs(), [1., 0., 0., 1., 13., 24.]);
        assert_eq!(child.1, 0.8);
        assert_eq!(matrices[3].as_ref().unwrap().0.as_coeffs(),
            [1., 0., 0., 1., 5., 6.]);
    }

    #[test] fn layer_world_matrix_skips_parent_cycles_and_their_descendants() {
        let animation: Animation = serde_json::from_str(r#"{ "layers": [
            {"ty":3,"ind":1,"parent":2,"st":0,"ip":0,"op":10,"ks":{}},
            {"ty":3,"ind":2,"parent":1,"st":0,"ip":0,"op":10,"ks":{}},
            {"ty":3,"ind":3,"parent":1,"st":0,"ip":0,"op":10,"ks":{}},
            {"ty":3,"ind":4,"st":0,"ip":0,"op":10,"ks":{"p":{"k":[5,6]}}}
        ] }"#).unwrap();

        let mut runtime = CompositionState::new(&animation.layers);
        assert_eq!(runtime.parents,
            [Parent::Invalid, Parent::Invalid, Parent::Invalid, Parent::Root]);
        let worlds: Vec<WorldState<kurbo::Affine>> =
            runtime.evaluate(&animation.layers, 0., |_| true);
        assert!(worlds[..3].iter()
            .all(|world| matches!(world, WorldState::Pending)));
        let WorldState::Ready(world) = &worlds[3] else { panic!() };
        assert_eq!(world.0.as_coeffs(), [1., 0., 0., 1., 5., 6.]);
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
        let mut runtime = LottieRuntime::from_reader(
            &br#"{"ip":10,"op":12,"fr":1,"layers":[]}"#[..]).unwrap();
        let mut context = TestContext::default();

        assert!(runtime.render_next_frame(
            &mut context, 1., Some(RGBA::new_u8(0, 0, 0, 0))));
        assert_eq!(runtime.frame(), 11.);
        assert!(runtime.render_next_frame(
            &mut context, 1., Some(RGBA::new_u8(0, 0, 0, 0))));
        assert_eq!(runtime.frame(), 10.);

        assert!(runtime.render_next_frame(
            &mut context, 2., Some(RGBA::new_u8(0, 0, 0, 0))));
        assert_eq!(runtime.frame(), 10.);
    }

    #[test] fn lottie_runtime_reuses_layer_graph_and_precomp_state() {
        let mut runtime = LottieRuntime::from_reader(&br##"{
            "ip":0,"op":10,"fr":1,
            "assets":[{"id":"nested","layers":[
                {"ty":1,"st":0,"ip":0,"op":10,
                    "sw":1,"sh":1,"sc":"#000000","ks":{}}
            ]}],
            "layers":[{"ty":0,"refId":"nested","w":1,"h":1,
                "st":0,"ip":0,"op":10,"ks":{}}]
        }"##[..]).unwrap();
        let graph = runtime.root.parents.as_ptr();
        let child_graph = runtime.root.precomps[0].as_ref().unwrap()
            .composition.parents.as_ptr();
        let mut context = TestContext::default();

        assert!(runtime.render_next_frame(&mut context, 1., None));
        assert!(runtime.render_next_frame(&mut context, 1., None));
        assert_eq!(graph, runtime.root.parents.as_ptr());
        assert_eq!(child_graph, runtime.root.precomps[0].as_ref().unwrap()
            .composition.parents.as_ptr());
    }

    #[test] fn precomp_time_remap_uses_root_fps_after_layer_time_mapping() {
        let json = br##"{
            "fr":24,"ip":20,"op":40,
            "assets":[{"id":"nested","fr":99,"layers":[{
                "ty":4,"st":0,"ip":0,"op":100,
                "ks":{"p":{"k":[{"t":0,"s":[0,0]},{"t":24,"s":[100,0]}]}},
                "shapes":[
                    {"ty":"rc","s":{"k":[1,1]},"p":{"k":[0,0]},"r":{"k":0}},
                    {"ty":"fl","c":{"k":[1,0,0]},"o":{"k":100}}
                ]
            }]}],
            "layers":[{"ty":0,"refId":"nested","w":1,"h":1,
                "st":4,"sr":2,"ip":0,"op":40,"ks":{},
                "tm":{"k":[{"t":0,"s":[0]},{"t":12,"s":[1]}]}}]
        }"##;
        let mut runtime = LottieRuntime::from_reader(&json[..]).unwrap();
        let mut context = TestContext::default();

        assert!(runtime.render_next_frame(&mut context, 1. / 24., None));
        assert!(context.transforms.iter().any(|transform|
            (transform.as_coeffs()[4] - 50.).abs() < 1e-4),
            "{:?}", context.transforms.iter().map(|tm| tm.as_coeffs()).collect::<Vec<_>>());
        let AssetItem::Precomp(precomp) = &runtime.animation.assets[0] else { panic!() };
        assert!(!serde_json::to_value(precomp).unwrap().as_object().unwrap().contains_key("fr"));
    }

    #[test] fn lottie_runtime_skips_recursive_precomp_references() {
        let runtime = LottieRuntime::from_reader(&br#"{
            "assets":[
                {"id":"a","layers":[{"ty":0,"refId":"b","w":1,"h":1,
                    "ip":0,"op":1,"ks":{}}]},
                {"id":"b","layers":[{"ty":0,"refId":"a","w":1,"h":1,
                    "ip":0,"op":1,"ks":{}}]}
            ],
            "layers":[{"ty":0,"refId":"a","w":1,"h":1,"ip":0,"op":1,"ks":{}}]
        }"#[..]).unwrap();

        let a = runtime.root.precomps[0].as_ref().unwrap();
        let b = a.composition.precomps[0].as_ref().unwrap();
        assert!(b.composition.precomps[0].is_none());
    }

    #[test] fn frame_clear_supports_transparent_color_and_preserve_modes() {
        let mut runtime = LottieRuntime::from_reader(
            &br#"{"ip":0,"op":10,"fr":1,"layers":[]}"#[..]).unwrap();
        let mut context = TestContext::default();

        assert!(runtime.render_next_frame(
            &mut context, 1., Some(RGBA::new_u8(0, 0, 0, 0))));
        let clear = context.clear.unwrap();
        assert_eq!((clear.r, clear.g, clear.b, clear.a), (0, 0, 0, 0));

        let red = RGBA::new_u8(255, 0, 0, 128);
        assert!(runtime.render_next_frame(&mut context, 1., Some(red)));
        let clear = context.clear.unwrap();
        assert_eq!((clear.r, clear.g, clear.b, clear.a), (255, 0, 0, 128));

        assert!(runtime.render_next_frame(&mut context, 1., None));
        assert_eq!(context.clear_count, 2);
    }

    #[test] fn recursive_render_restores_opacity_between_siblings() {
        let path = || DrawItem::Shape(BezPath::new());
        let draws = vec![path(),
            DrawItem::Group(vec![path()],
                vec![TM2DwO(kurbo::Affine::IDENTITY, 0.4)]),
            fill_style(),
        ];
        let mut context = TestContext { opacity: 1., ..Default::default() };

        context.render_shapes(&TM2DwO(kurbo::Affine::IDENTITY, 0.5), &draws);
        assert_eq!(context.drawn, [0.2, 0.5]);
        assert_eq!(context.opacity, 1.);
        assert!(context.stack.is_empty());
    }

    #[test] fn outer_styles_keep_their_scope_transform_across_nested_groups() {
        let path = || DrawItem::Shape(BezPath::new());
        let group_matrix = kurbo::Affine::translate((20., 0.));
        let group = TM2DwO(group_matrix, 1.);
        let draws = [path(), DrawItem::Group(vec![path()], vec![group]),
            fill_style()];
        let scope = TM2DwO(kurbo::Affine::translate((10., 0.)), 1.);
        let mut context = TestContext::default();

        context.render_shapes(&scope, &draws);
        assert_eq!(context.fills.len(), 2);
        assert_eq!(context.fills[0].0, scope.0);
        assert_eq!(context.fills[0].1, Some(group_matrix));
        assert_eq!(context.fills[1], (scope.0, None));
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
        let trim = trim(0., 50., 270., 2);
        let mut draws: Vec<DrawItem<BezPath, TestStyle, kurbo::Affine>> =
            vec![DrawItem::Shape(line(100., 0.))];

        trim_shapes(&trim, &mut draws, 0.);
        let DrawItem::Shape(path) = &draws[0] else { panic!() };
        let segments = path.segments().collect::<Vec<_>>();
        assert_eq!(segments.len(), 2);
        use kurbo::ParamCurve;
        assert_eq!((segments[0].start().x, segments[0].end().x), (75., 100.));
        assert_eq!((segments[1].start().x, segments[1].end().x), (0., 25.));
    }

    #[test] fn sequential_trim_follows_reverse_render_order() {
        let trim = trim(0., 25., 0., 2);
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
        let trim = trim(0., 25., 0., 2);
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
        let trim = trim(25., 75., 0., 1);
        let mut draws: Vec<DrawItem<BezPath, TestStyle, kurbo::Affine>> = vec![
            DrawItem::Shape(line(100., 0.)),
            DrawItem::Shape(line(40., 1.)),
        ];

        trim_shapes(&trim, &mut draws, 0.);
        let [DrawItem::Shape(first), DrawItem::Shape(second)] = &draws[..] else { panic!() };
        assert_eq!(first.segments().next().unwrap().arclen(0.1), 50.);
        assert_eq!(second.segments().next().unwrap().arclen(0.1), 20.);
    }

    #[test] fn sequential_trim_treats_repeater_copies_as_rendered_paths() {
        let trim = trim(0., 25., 0., 2);

        let mut after: Vec<DrawItem<BezPath, TestStyle, kurbo::Affine>> = vec![
            DrawItem::Group(vec![DrawItem::Shape(line(100., 0.))],
                vec![TM2DwO::default(); 4]),
        ];
        trim_shapes(&trim, &mut after, 0.);
        let DrawItem::Copies(copies) = &after[0] else { panic!() };
        assert_eq!(copies.len(), 4);
        for (index, (group, _)) in copies.iter().enumerate() {
            let [DrawItem::Shape(path)] = &group[..] else { panic!() };
            assert_eq!(path_length(path), if index == 3 { 100. } else { 0. });
        }
        after.push(fill_style());
        let mut context = TestContext::default();
        context.render_shapes(&TM2DwO::default(), &after);
        assert_eq!(context.draw_count, 4);

        let mut before: Vec<DrawItem<BezPath, TestStyle, kurbo::Affine>> =
            vec![DrawItem::Shape(line(100., 0.))];
        trim_shapes(&trim, &mut before, 0.);
        let DrawItem::Shape(path) = before.remove(0) else { panic!() };
        let batch: DrawItem<BezPath, TestStyle, kurbo::Affine> =
            DrawItem::Group(vec![DrawItem::Shape(path)], vec![TM2DwO::default(); 4]);
        let DrawItem::Group(group, transforms) = batch else { panic!() };
        let [DrawItem::Shape(path)] = &group[..] else { panic!() };
        assert_eq!(transforms.len(), 4);
        assert_eq!(path_length(path), 25.);
    }

    #[test] fn sequential_trim_measures_group_paths_before_group_transform() {
        let trim = trim(0., 25., 0., 2);
        let mut draws: Vec<DrawItem<BezPath, TestStyle, kurbo::Affine>> = vec![
            DrawItem::Shape(line(100., 0.)),
            DrawItem::Group(vec![DrawItem::Shape(line(100., 0.))],
                vec![TM2DwO(kurbo::Affine::scale(10.), 1.)]),
        ];

        trim_shapes(&trim, &mut draws, 0.);
        let DrawItem::Group(group, _) = &draws[1] else { panic!() };
        let [DrawItem::Shape(path)] = &group[..] else { panic!() };
        assert_eq!(path.segments().next().unwrap().arclen(0.1), 50.);
    }
}
