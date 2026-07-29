/****************************************************************
 * $ID: render.rs  	Fri 03 May 2024 22:07:36+0800               *
 *                                                              *
 * Maintainer: 范美辉 (MeiHui FAN) <mhfan@ustc.edu>              *
 * Copyright (c) 2024 M.H.Fan, All rights reserved.             *
 ****************************************************************/

use core::mem;
use std::{collections::HashMap, rc::Rc};
use super::{composite::{self, CompositeContext}, path_ops::MeasuredPath,
    helpers::{Vec2D, RGBA, IntBool}, style::{StyleConv, MatrixConv, TM2DwO, FSOpts},
    pathm::{BezPath, PathBuilder, PathFactory, trim_kurbo, round_kurbo, offset_kurbo},
    schema::{Animation, AssetItem, LayerItem, ShapeItem, TrimPath, TrimMultiple, FillRule},
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)] enum Parent { Root, Layer(u32), Invalid }
enum WorldState<MC: MatrixConv> { Pending, Invalid, Ready(TM2DwO<MC>) }

struct CompositionState {
     parents: Vec<Parent>, stack: Vec<usize>,
    precomps: Vec<Option<PrecompState>>,
    path_mod: Vec<bool>,
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
        let path_mod = layers.iter().map(|layer| match layer {
            LayerItem::Shape(layer) => has_path_modifier(&layer.shapes),
            _ => false,
        }).collect();

        let parents = parents.into_iter().zip(states).map(|(parent, state)| {
            if state != 2 { Parent::Invalid }
            else { parent.map_or(Parent::Root, Parent::Layer) }
        }).collect();
        Self { parents, stack, precomps, path_mod }
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
    pub fn render_next_frame<RC: CompositeContext>(&mut self, rctx: &mut RC,
        elapsed: f32, clear: Option<RGBA>) -> Result<bool, RC::Error> {
        //debug_assert!(0. < self.fr && 0. <= self.ip && 1. < self.op - self.ip);
        let animation = &self.animation;

        if  self.fnth < animation.ip || animation.op <= self.fnth {
            self.fnth = animation.ip;
        }   self.elapsed += elapsed * animation.fr;
        if  self.elapsed < 1. && animation.ip < self.fnth { return Ok(false) }

        if  2. <= self.elapsed {    // advance/skip elapsed frames
            let elapsed = (self.elapsed - 1.).floor();
            let duration =  animation.op - animation.ip;
            if 0. < duration {
                self.fnth = animation.ip +
                    (self.fnth -  animation.ip + elapsed).rem_euclid(duration);
            }   self.elapsed -= elapsed;
        }

        // Preserve the caller's complete backend state once per rendered frame. Shape traversal
        // explicitly installs every transform and opacity it uses, so per-layer saves are redundant.
        let state = rctx.save_state()?;
        // Capture `?` errors instead of returning early, so backend state is always restored.
        let rendered = (|| {
            if let Some(color) = clear {
                let (width, height) = rctx.get_size();
                rctx.clear_rect_with(0, 0, width, height, color)?;
            }
            Self::render_layers(animation, rctx, &TM2DwO::default(),
                &animation.layers, self.fnth, &mut self.root)
        })();
        let restored = rctx.restore_state(state);
        rendered.and(restored)?;

        self.elapsed -= 1.;       self.fnth += 1.;
        if animation.op <= self.fnth { self.fnth = animation.ip; }    Ok(true)
    }

    /// The render order goes from the last element to the first,
    /// items in list coming first will be rendered on top.
    fn render_layers<RC: CompositeContext>(animation: &Animation, rctx: &mut RC,
        ptm: &TM2DwO<RC::TM2D>, layers: &[LayerItem], fnth: f32,
        runtime: &mut CompositionState) -> Result<(), RC::Error> {
        let mut composite = composite::Compositor::default();
        let worlds = runtime.evaluate(layers, fnth, |layer| match layer {
            LayerItem::Shape(layer) => !layer.vl.should_hide(fnth),
            LayerItem::PrecompLayer(layer) => !layer.vl.should_hide(fnth),
            LayerItem::SolidColor(layer) => !layer.vl.should_hide(fnth),
            _ => false,
        });

        // Capture `?` errors so pending matte images are discarded before returning.
        let rendered = (|| { for (index, layer) in layers.iter().enumerate().rev() {
        let mut handled = false; match layer {
            LayerItem::Shape(shpl) =>
            if let WorldState::Ready(ltm) = &worlds[index] {
                let Some(local) = shpl.vl.base.local_frame(fnth) else {
                    composite.skip(rctx, &shpl.vl);     continue
                };  handled = true;
                let (draws, ctm) = convert_shapes_known(&shpl.shapes, local,
                    shpl.vl.ao, runtime.path_mod[index]);
                let ltm = ltm.clone().compose(ptm);

                composite.render(rctx, &shpl.vl, &ltm, fnth, |rctx|
                    rctx.render_shapes(&ctm.compose(&ltm), &draws))?;
            }
            LayerItem::PrecompLayer(pcl) =>
            if let WorldState::Ready(ltm) = &worlds[index] {
                if let Some(child) = &mut runtime.precomps[index] {
                    let AssetItem::Precomp(pcomp) =
                        &animation.assets[child.asset] else { unreachable!() };
                    let Some(local) = pcl.vl.base.local_frame(fnth) else {
                        composite.skip(rctx, &pcl.vl);  continue
                    };  handled = true;
                    let child_fnth = pcl.tm.as_ref().map_or(local,
                        |tm| tm.get_value(local) * animation.fr);
                    let ltm = ltm.clone().compose(ptm);

                    composite.render(rctx, &pcl.vl, &ltm, fnth, |rctx|
                        Self::render_layers(animation, rctx, &ltm,
                            &pcomp.layers, child_fnth, &mut child.composition))?;
                }   // XXX: clipping(pcl.w, pcl.h)?
            }
            LayerItem::SolidColor(scl) =>
            if let WorldState::Ready(ltm) = &worlds[index] {
                let ltm = ltm.clone().compose(ptm);
                let mut path = RC::VGPath::new(5);
                path.rect(0., 0., scl.sw, scl.sh);
                let opts = FSOpts::Fill(FillRule::NonZero);
                let mut style = RC::VGStyle::solid_color(scl.sc);
                style.configure(&opts);     handled = true;

                composite.render(rctx, &scl.vl, &ltm, fnth, |rctx|
                    rctx.render_shapes(&ltm, &[DrawItem::Shape(path),
                        DrawItem::Style(Rc::new((style, opts)))]))?;
            }
            LayerItem::Image(_) | LayerItem::Text(_)  | LayerItem::Data(_)  |
            LayerItem::Audio(_) | LayerItem::Camera(_) => dbg!(),     // TODO:

            //LayerItem::Null(_) => (),    // used as a parent, nothing to do
            _ => (),
        }
        if !handled {
            if let Some(layer) = layer.visual_layer() { composite.skip(rctx, layer); }
        }
        }   Ok(()) })();
        composite.finish(rctx);     rendered
    }
}

pub trait RenderContext {
    type VGPath: PathBuilder;
    type VGStyle: StyleConv;    // (VGBrush/VGPaint, FSOpts)
    type TM2D: MatrixConv + Clone;
    type State;
    type Error;

    //fn set_comp_op(&mut self, op: CompOp);

    fn get_size(&self) -> (u32, u32);
    fn clear_rect_with(&mut self, x: u32, y: u32, w: u32, h: u32,
        color: RGBA) -> Result<(), Self::Error>;

    fn save_state(&mut self) -> Result<Self::State, Self::Error>;
    fn restore_state(&mut self, state: Self::State) -> Result<(), Self::Error>;
    fn apply_transform(&mut self, trfm: &Self::TM2D,
        opacity: Option<f32>) -> Result<(), Self::Error>;
    fn fill_stroke(&mut self, path: &Self::VGPath, relative: Option<&Self::TM2D>,
        style: &(Self::VGStyle, FSOpts)) -> Result<(), Self::Error>;

    fn traverse_shapes(&mut self, stm: &TM2DwO<Self::TM2D>,
        relative: Option<&TM2DwO<Self::TM2D>>,
        draws: &[DrawItem<Self::VGPath, Self::VGStyle, Self::TM2D>],
        style: &(Self::VGStyle, FSOpts)) -> Result<(), Self::Error> {
        self.apply_transform(&stm.0, Some(stm.1 * relative.map_or(1., |tm| tm.1)))?;

        for draw in draws.iter().rev() { match draw {
            DrawItem::Shape(path) =>
                self.fill_stroke(path, relative.map(|tm| &tm.0), style)?,
            DrawItem::Group(grp, rep) => for gtm in rep.iter().rev() {
                let child = Some(match relative {
                    Some(relative) => gtm.clone().compose(relative),
                    None => gtm.clone(),
                });
                self.traverse_shapes(stm, child.as_ref(), grp, style)?;
                self.apply_transform(&stm.0,
                    Some(stm.1 * relative.map_or(1., |tm| tm.1)))?;
            },
            DrawItem::Copies(copies) => for (grp, gtm) in copies.iter().rev() {
                let child = Some(match relative {
                    Some(relative) => gtm.clone().compose(relative),
                    None => gtm.clone(),
                });
                self.traverse_shapes(stm, child.as_ref(), grp, style)?;
                self.apply_transform(&stm.0,
                    Some(stm.1 * relative.map_or(1., |tm| tm.1)))?;
            },  _ => (), // skip/ignore Style
        } } Ok(())
    }

    fn render_shapes(&mut self, ptm: &TM2DwO<Self::TM2D>,
        draws: &[DrawItem<Self::VGPath, Self::VGStyle, Self::TM2D>]) ->
        Result<(), Self::Error> {
        for (idx, item) in draws.iter().enumerate().rev() { match item {
            DrawItem::Style(style) =>
                self.traverse_shapes(ptm, None, &draws[0..idx], style)?,
            DrawItem::Group(grp, rep) => for gtm in rep.iter().rev() {
                self.render_shapes(&gtm.clone().compose(ptm), grp)?;
            },
            DrawItem::Copies(copies) => for (grp, gtm) in copies.iter().rev() {
                self.render_shapes(&gtm.clone().compose(ptm), grp)?;
            },  _ => (), // skip/ignore Shape
        } } Ok(())
    }
}

enum PendingPath<P: PathBuilder> { Native(P), Kurbo(BezPath) }

impl<P: PathBuilder> PendingPath<P> {
    // Shape commands are complete before a modifier can switch the path to Kurbo.
    fn native(&mut self) -> &mut P {
        let Self::Native(path) = self else { unreachable!() }; path
    }

    fn kurbo_mut(&mut self) -> &mut BezPath {
        if let Self::Native(path) = self {
            *self = Self::Kurbo(mem::replace(path, P::new(0)).into_kurbo());
        }
        let Self::Kurbo(path) = self else { unreachable!() }; path
    }

    fn into_native(self) -> P { match self {
        Self::Kurbo(path) => P::from_kurbo(path),
        Self::Native(path) => path,
    }}
}

impl<P: PathBuilder> PathBuilder for PendingPath<P> {
    fn new(capacity: u32) -> Self { Self::Native(P::new(capacity)) }
    fn close(&mut self) { self.native().close() }
    fn current_pos(&self) -> Option<Vec2D> {
        let Self::Native(path) = self else { unreachable!() }; path.current_pos()
    }
    fn move_to(&mut self, end: Vec2D) { self.native().move_to(end) }
    fn line_to(&mut self, end: Vec2D) { self.native().line_to(end) }
    fn quad_to(&mut self, cp: Vec2D, end: Vec2D) {
        self.native().quad_to(cp, end)
    }
    fn cubic_to(&mut self, ocp: Vec2D, icp: Vec2D, end: Vec2D) {
        self.native().cubic_to(ocp, icp, end)
    }
    fn from_kurbo(path: BezPath) -> Self { Self::Kurbo(path) }
    fn to_kurbo(&self) -> BezPath { match self {
        Self::Native(path) => path.to_kurbo(),
        Self::Kurbo (path) => path.clone(),
    }}
    fn into_kurbo(self) -> BezPath { match self {
        Self::Native(path) => path.into_kurbo(),
        Self::Kurbo (path) => path,
    }}
    fn trim_path(&mut self, start: f32, trim: f32) {
        if trim <= 0. { *self = Self::new(0); return }
        if 1. <= trim { return }
        trim_kurbo(self.kurbo_mut(), start, trim)
    }
    fn round_corners(&mut self, radius: f32) {
        if radius <= 0. { return }
        round_kurbo(self.kurbo_mut(), radius)
    }
    fn offset_path(&mut self, amount: f32,
        join: super::schema::LineJoin, miter_limit: f32) {
        if amount != 0. { offset_kurbo(self.kurbo_mut(), amount, join, miter_limit) }
    }
}

/// calculate transform matrix, convert shapes to paths, modify/change the paths,
/// and convert style(fill/stroke/gradient) to draw items, recursively
pub fn convert_shapes<VGPath: PathBuilder, VGPaint: StyleConv, TM2D: MatrixConv + Clone>(
    shapes: &[ShapeItem], fnth: f32, ao: IntBool) ->
    (Vec<DrawItem<VGPath, VGPaint, TM2D>>, TM2DwO<TM2D>) {
    convert_shapes_known(shapes, fnth, ao, has_path_modifier(shapes))
}

fn convert_shapes_known<VGPath: PathBuilder, VGPaint: StyleConv,
    TM2D: MatrixConv + Clone>(shapes: &[ShapeItem], fnth: f32, ao: IntBool,
    has_modifier: bool) -> (Vec<DrawItem<VGPath, VGPaint, TM2D>>, TM2DwO<TM2D>) {
    if has_modifier {
        let (draws, ctm) =
            convert_shapes_inner::<PendingPath<VGPath>, VGPaint, TM2D>(shapes, fnth, ao);
        (materialize_draws(draws), ctm)
    } else {
        convert_shapes_inner::<VGPath, VGPaint, TM2D>(shapes, fnth, ao)
    }
}

fn has_path_modifier(shapes: &[ShapeItem]) -> bool {
    shapes.iter().any(|shape| match shape {
        ShapeItem::Trim(modifier) => !modifier.elem.hd,
        ShapeItem::RoundedCorners(modifier) => !modifier.elem.hd,
        ShapeItem::OffsetPath(modifier) => !modifier.elem.hd,
        ShapeItem::Group(group) => !group.elem.hd && has_path_modifier(&group.shapes),
        _ => false,
    })
}

fn convert_shapes_inner<Path: PathBuilder, VGPaint: StyleConv,
    TM2D: MatrixConv + Clone>(shapes: &[ShapeItem], fnth: f32, ao: IntBool) ->
    (Vec<DrawItem<Path, VGPaint, TM2D>>, TM2DwO<TM2D>) {
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
            draws.push(DrawItem::Style(Rc::new(fill.to_style(fnth)))),
        ShapeItem::Stroke(line) if !line.elem.hd =>
            draws.push(DrawItem::Style(Rc::new(line.to_style(fnth)))),
        ShapeItem::GradientFill(grad)   if !grad.elem.hd =>
            draws.push(DrawItem::Style(Rc::new(grad.to_style(fnth)))),
        ShapeItem::GradientStroke(grad) if !grad.elem.hd =>
            draws.push(DrawItem::Style(Rc::new(grad.to_style(fnth)))),
        ShapeItem::NoStyle(_) => eprintln!("Nothing to do here?"),

        ShapeItem::Group(group) if !group.elem.hd => {
            let (grp, ctm) =
                convert_shapes_inner::<Path, VGPaint, TM2D>(&group.shapes, fnth, ao);
            draws.push(DrawItem::Group(grp, vec![ctm]));
        }

        ShapeItem::Repeater(mdfr) if !mdfr.elem.hd => {
            let grp = mem::take(&mut draws);
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

fn materialize_draws<VGPath: PathBuilder, VGPaint: StyleConv, TM2D: MatrixConv>(
    draws: Vec<DrawItem<PendingPath<VGPath>, VGPaint, TM2D>>) ->
    Vec<DrawItem<VGPath, VGPaint, TM2D>> {
    draws.into_iter().map(|draw| match draw {
        DrawItem::Shape(path) => DrawItem::Shape(path.into_native()),
        DrawItem::Style(style) => DrawItem::Style(style),
        DrawItem::Group(group, transforms) =>
            DrawItem::Group(materialize_draws(group), transforms),
        DrawItem::Copies(copies) => DrawItem::Copies(copies.into_iter()
            .map(|(group, transform)| (materialize_draws(group), transform)).collect()),
    }).collect()
}

// TODO: Compile Shape/Style scopes into explicit paint batches
// so rendering no longer rescans each style's preceding DrawItems.
//  https://lottie.github.io/lottie-spec/latest/specs/shapes/#graphic-element
pub enum DrawItem<VGPath: PathBuilder, VGPaint: StyleConv, TM2D: MatrixConv> {
    Shape(VGPath),                          // DrawItem is a.k.a Graphic Element
    Style(Rc<(VGPaint, FSOpts)>),           // shared only by expanded repeater copies
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

            let mut source = mem::take(group);
            let transforms = mem::take(transforms);
            let last = transforms.len().saturating_sub(1);
            let mut paths = HashMap::new();
            let copies = transforms.into_iter().enumerate().map(|(index, transform)| {
                let group = if index == last { mem::take(&mut source)
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
            let path = MeasuredPath::new(mem::replace(path, VGPath::new(0)).into_kurbo());
            total += path.length;
            paths.push(path);
        });
        if total == 0. {
            for_each_path_mut(draws, &mut |path| *path = VGPath::new(0));
            return
        }

        let end = start + trim;
        let ranges = if end <= 1. {
            [(start as f64 * total, end as f64 * total), (0., 0.)]
        } else {
            [(start as f64 * total, total), (0., (end - 1.) as f64 * total)]
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

#[cfg(test)] #[path = "render_tests.rs"] mod tests;
