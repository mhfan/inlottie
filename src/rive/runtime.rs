
//! Retained Rive scene state, artboard selection, hierarchy updates, and playback control.

use std::{error::Error as StdError, fmt, f32};

use super::{animation::{LinearAnimation, TrackValue,
        build_animations, evaluate_track},
    display_list::{Affine2, Brush, DashSegment, DisplayList, FillRule,
        Geometry, GradientStop, Paint, PathEffect, Point, Shape,
        DrawItem, StrokeCap, StrokeJoin, TrimMode
    },
    decode::{self, DecodeError, Object, RiveFile, object_ids, property_ids,
        core_boolean_default, core_color_default, core_float_default, core_varuint_default,
        core_is_component, core_is_transform_component,
    }, path::{GeomParams, Vertex, VertexParams, build_path},
};

#[path = "draw.rs"] mod draw;
#[path = "shape.rs"] mod shape;
use shape::{set_effect_bool, set_effect_uint, set_effect_value,
    set_paint_bool, set_paint_uint, set_paint_value};

pub type Result<T> = std::result::Result<T, RuntimeError>;

#[derive(Debug)] pub enum RuntimeError {
    Decode(DecodeError), AnimationNameNotFound, AnimationNotFound(u32), ArtboardNotFound(u32),
    DrawOrderCycle(u32), InvalidInterpolation(u32), InvalidInterpolator(u32),
    InvalidTrimMode(u32), ParentCycle(u32), TooManyObjects, TooManyVertices(u32),
    InvalidParent { comp_id: u32, parent_id: u32 },
}

impl fmt::Display for RuntimeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { match self {
        Self::Decode(error) => error.fmt(f),
        Self::AnimationNameNotFound => f.write_str("Rive animation name does not exist"),
        Self::AnimationNotFound(index) => write!(f, "Rive animation {index} does not exist"),
        Self::ArtboardNotFound(index) => write!(f, "Rive artboard {index} does not exist"),
        Self::DrawOrderCycle(obj_idx) =>
            write!(f, "Rive draw-rule cycle at object {obj_idx}"),
        Self::InvalidInterpolation(value) =>
            write!(f, "invalid Rive keyframe interpolation {value}"),
        Self::InvalidInterpolator(index) =>
            write!(f, "invalid Rive cubic interpolator {index}"),
        Self::InvalidTrimMode(value) => write!(f, "invalid Rive trim-path mode {value}"),
        Self::TooManyObjects => f.write_str("Rive object count exceeds u32"),
        Self::TooManyVertices(count) =>
            write!(f, "Rive parametric path has too many vertices: {count}"),
        Self::InvalidParent { comp_id, parent_id } =>
            write!(f, "component {comp_id} references missing parent {parent_id}"),
        Self::ParentCycle(comp_id) => write!(f, "component parent cycle at {comp_id}"),
    } }
}

impl StdError for RuntimeError {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        match self { Self::Decode(error) => Some(error), _ => None }
    }
}

impl From<DecodeError> for RuntimeError {
    fn from(error: DecodeError) -> Self { Self::Decode(error) }
}

#[derive(Debug)] enum ComponentGeom {
    Parametric { cached: Geometry, params: GeomParams, dirty: bool },
    Points { cached: Geometry, vertices: Vec<Vertex>, closed: bool, dirty: bool },
}

impl ComponentGeom {
    fn parametric(params: GeomParams) -> Self {
        let cached = params.geometry();
        Self::Parametric { cached, params, dirty: false }
    }

    fn geometry(&self) -> &Geometry { match self {
        Self::Parametric { cached: geometry, .. } |
        Self::Points { cached: geometry, .. } => geometry,
    } }

    fn set_float(&mut self, prop_id: u32, value: f32) -> bool {
        let Self::Parametric { params, dirty, .. } = self else { return false };
        let Some(changed) = params.set_float(prop_id, value) else { return false };
        *dirty |= changed;   true
    }

    fn set_bool(&mut self, prop_id: u32, value: bool) -> bool {
        match self {
            Self::Parametric { params, dirty, .. } => {
                let Some(changed) = params.set_bool(prop_id, value) else { return false };
                *dirty |= changed;
            }
            Self::Points { closed, dirty, .. }
                if prop_id == property_ids::POINTSCOMMONPATH_ISCLOSED => {
                    *dirty |= *closed != value;
                    *closed = value;
                }
            _ => return false,
        }   true
    }

    fn set_uint(&mut self, prop_id: u32, value: u32) -> bool {
        let Self::Parametric { params, dirty, .. } = self else { return false };
        let Some(changed) = params.set_uint(prop_id, value) else { return false };
        *dirty |= changed;   true
    }

    fn set_vertex(&mut self, slot: u32, vertex: Vertex) {
        let Self::Points { vertices, dirty, .. } = self else { return };
        let Some(target) = vertices.get_mut(slot as usize) else { return };
        if target.position != vertex.position || target.incoming != vertex.incoming ||
            target.outgoing != vertex.outgoing || target.radius != vertex.radius {
            *target = vertex;
            *dirty = true;
        }
    }

    fn refresh(&mut self) {
        match self {
            Self::Parametric { cached, params, dirty } if *dirty => {
                *cached = params.geometry();
                *dirty = false;
            }
            Self::Points { cached, vertices, closed, dirty } if *dirty => {
                *cached = Geometry::Path(build_path(vertices, *closed));
                *dirty = false;
            }   _ => {}
        }
    }
}

#[derive(Debug)] struct ComponentPaint { value: Paint, visible: bool }

#[derive(Debug, Default)] enum ComponentData {
    #[default] None,
    Geometry(ComponentGeom),
    Vertex(VertexParams),
    Gradient(GradientState),
    Paint(ComponentPaint),
}

#[derive(Debug)] struct GradientState {
    start: Point, end: Point, opacity: f32, radial: bool,
    stops: Vec<GradientStop>, stops_dirty: bool, paint: Option<u32>,
}

impl GradientState {
    fn from_object(object: &Object) -> decode::Result<Option<Self>> {
        let radial = match object.type_id.0 {
            object_ids::LINEAR_GRADIENT => false,
            object_ids::RADIAL_GRADIENT => true,
            _ => return Ok(None),
        };
        Ok(Some(Self { radial, stops: Vec::new(), stops_dirty: false, paint: None,
            start: Point { x: float(object, property_ids::STARTX)?,
                y: float(object, property_ids::STARTY)? },
              end: Point { x: float(object, property_ids::ENDX)?,
                y: float(object, property_ids::ENDY)? },
            opacity: float(object, property_ids::LINEARGRADIENT_OPACITY)?,
        }))
    }

    fn set(&mut self, prop_id: u32, value: f32) -> bool {
        match prop_id {
            property_ids::STARTX => self.start.x = value,
            property_ids::STARTY => self.start.y = value,
            property_ids::ENDX => self.end.x = value,
            property_ids::ENDY => self.end.y = value,
            property_ids::LINEARGRADIENT_OPACITY => self.opacity = value,
            _ => return false,
        }   true
    }

    fn set_stop_pos(&mut self, stop: u32, value: f32) {
        let Some(target) = self.stops.get_mut(stop as usize) else { return };
        let value = value.clamp(0.0, 1.0);
        if  target.pos != value {
            target.pos  = value;
            self.stops_dirty = true;
        }
    }

    fn set_stop_color(&mut self, stop: u32, value: u32) {
        let Some(target) = self.stops.get_mut(stop as usize) else { return };
        if  target.color != value {
            target.color  = value;
            self.stops_dirty = true;
        }
    }

    fn sorted_stops(&self) -> std::sync::Arc<[GradientStop]> {
        let mut stops = self.stops.clone();
        stops.sort_by(|left, right| left.pos.total_cmp(&right.pos));
        stops.into()
    }
}

#[derive(Debug)] struct Component {
    // Runtime indices are dense and stable; object indices continue to refer into RiveFile.
    obj_idx: u32, parent: Option<u32>,
    data: ComponentData,
    transform: TransformValues,
    world_opacity: f32,
    world: Affine2,
    is_hole: bool,
}

impl Component {
    fn geom(&self) -> Option<&ComponentGeom> {
        if let ComponentData::Geometry(value) = &self.data { Some(value) } else { None }
    }
    fn geom_mut(&mut self) -> Option<&mut ComponentGeom> {
        if let ComponentData::Geometry(value) = &mut self.data { Some(value) } else { None }
    }
    fn vertex(&self) -> Option<&VertexParams> {
        if let ComponentData::Vertex(value) = &self.data { Some(value) } else { None }
    }
    fn vertex_mut(&mut self) -> Option<&mut VertexParams> {
        if let ComponentData::Vertex(value) = &mut self.data { Some(value) } else { None }
    }
    fn gradient(&self) -> Option<&GradientState> {
        if let ComponentData::Gradient(value) = &self.data { Some(value) } else { None }
    }
    fn gradient_mut(&mut self) -> Option<&mut GradientState> {
        if let ComponentData::Gradient(value) = &mut self.data { Some(value) } else { None }
    }
    fn paint(&self) -> Option<&ComponentPaint> {
        if let ComponentData::Paint(value) = &self.data { Some(value) } else { None }
    }
    fn paint_mut(&mut self) -> Option<&mut ComponentPaint> {
        if let ComponentData::Paint(value) = &mut self.data { Some(value) } else { None }
    }
}

#[derive(Debug)] struct DrawGroup {
    obj_idx: u32,
    opacity_component: u32,
    components: Vec<u32>,
    paints: Vec<u32>,
}

#[derive(Debug, Clone, Copy)] pub(super) enum ColorTarget {
    Solid(u32), Stop { gradient: u32, stop: u32 },
}

#[derive(Debug, Clone, Copy)] pub(super) enum EffectTarget {
    DashSegment { paint: u32, effect: u32, segment: u32 },
    Effect { paint: u32, effect: u32 },
}

#[derive(Debug, Clone, Copy, Default)] pub(super) enum ComponentTarget {
    #[default] None,
    Color(ColorTarget),
    Effect(EffectTarget),
    Vertex { path: u32, slot: u32 },
}

#[derive(Debug, Clone, Copy)] pub(super) enum TrackTarget {
    Transform { component: u32, prop_id: u32 },
    Geometry { component: u32, prop_id: u32 },
    Vertex { component: u32, path: u32, slot: u32, prop_id: u32 },
    Gradient { component: u32, prop_id: u32 },
    GradientStopPos { component: u32, stop: u32 },
    GradientStopColor { component: u32, stop: u32 },
    SolidColor { component: u32 },
    Paint { component: u32, prop_id: u32 },
    Effect { target: EffectTarget, prop_id: u32 },
    Visibility { component: u32 },
}

#[derive(Debug, Clone, Copy)] pub(super) struct TrackBinding {
    target: TrackTarget, default: TrackValue,
}

#[derive(Debug, Clone, Copy, PartialEq)] pub struct AnimationInfo<'a> {
    pub name: &'a [u8], pub duration: u32, pub fps: u32,
    pub speed: f32, pub loop_mode: u32,
}

#[derive(Debug, Clone, Copy)] pub(super) struct TransformValues {
    x: f32, y: f32, rotation: f32, scale_x: f32, scale_y: f32, opacity: f32,
}

impl TransformValues {
    fn from_object(object: &Object) -> decode::Result<Self> {
        if !core_is_transform_component(object.type_id.0) {
            return Ok(Self { x: 0.0, y: 0.0, rotation: 0.0,
                scale_x: 1.0, scale_y: 1.0, opacity: 1.0 })
        }
        Ok(Self {
            x: float(object, property_ids::NODE_X)?,
            y: float(object, property_ids::NODE_Y)?,
            rotation: float(object, property_ids::TRANSFORMCOMPONENT_ROTATION)?,
            scale_x: float(object, property_ids::TRANSFORMCOMPONENT_SCALEX)?,
            scale_y: float(object, property_ids::TRANSFORMCOMPONENT_SCALEY)?,
            opacity: float(object, property_ids::WORLDTRANSFORMCOMPONENT_OPACITY)?,
        })
    }

    fn set(&mut self, prop_id: u32, value: f32) -> bool { match prop_id {
        property_ids::NODE_X => replace_changed(&mut self.x, value),
        property_ids::NODE_Y => replace_changed(&mut self.y, value),
        property_ids::TRANSFORMCOMPONENT_ROTATION =>
            replace_changed(&mut self.rotation, value),
        property_ids::TRANSFORMCOMPONENT_SCALEX =>
            replace_changed(&mut self.scale_x, value),
        property_ids::TRANSFORMCOMPONENT_SCALEY =>
            replace_changed(&mut self.scale_y, value),
        property_ids::WORLDTRANSFORMCOMPONENT_OPACITY =>
            replace_changed(&mut self.opacity, value),
        _ => false,
    } }

    fn affine(self) -> Affine2 {
        Affine2::from_transform(self.x, self.y, self.rotation, self.scale_x, self.scale_y)
    }
}

/// Retained Rive scene state.
///
/// The first implementation resolves component transforms and emits static parametric and
/// points-path geometry with solid or gradient paint. Animation, constraints, clipping,
/// text and state machines can update this retained state without changing the display-list API.
///
/// TODO: Add constraints, clipping, text, state machines, skins/deformers, and nested artboards.
#[derive(Debug)] pub struct Runtime {
    file: RiveFile, artboard_obj: u32, elapsed: f32,
    components: Vec<Component>,
    update_order: Vec<u32>,
    gradients: Vec<u32>,
    draw_groups: Vec<DrawGroup>,
    animations: Vec<LinearAnimation>,
    active_animation: Option<u32>,
}

impl Runtime {
    pub fn from_file(file: RiveFile) -> Result<Self> { Self::from_artboard(file, 0) }

    pub fn from_artboard(file: RiveFile, artboard_index: u32) -> Result<Self> {
        // An artboard owns the contiguous object range up to the next artboard object.
        let context_start = file.ocoll.iter().enumerate()
            .filter(|(_, object)| object.type_id.0 == object_ids::ARTBOARD)
            .nth(artboard_index as usize).map(|(index, _)| index)
            .ok_or(RuntimeError::ArtboardNotFound(artboard_index))?;
        let context_end = file.ocoll[context_start + 1..].iter()
            .position(|object| object.type_id.0 == object_ids::ARTBOARD)
            .map_or(file.ocoll.len(), |offset| context_start + 1 + offset);
        let (mut components, mut parent_objs) = (Vec::new(), Vec::new());
        let mut obj_comps = vec![None; file.ocoll.len()];

        for (obj_idx, object) in file.ocoll.iter().enumerate()
            .take(context_end).skip(context_start) {
            if !core_is_component(object.type_id.0) { continue }
            let obj_idx =  u32::try_from(obj_idx).map_err(|_| RuntimeError::TooManyObjects)?;
            if  obj_idx == u32::MAX { return Err(RuntimeError::TooManyObjects) }

            let parent_id = uint(object, property_ids::COMPONENT_PARENTID)?;
            let data = if let Some(value) = GeomParams::from_object(object)? {
                ComponentData::Geometry(ComponentGeom::parametric(value))
            } else if let Some(value) = VertexParams::from_object(object)? {
                ComponentData::Vertex(value)
            } else if let Some(value) = GradientState::from_object(object)? {
                ComponentData::Gradient(value)
            } else { ComponentData::None };
            obj_comps[obj_idx as usize] = Some(components.len() as u32);
            components.push(Component {
                is_hole: boolean(object, property_ids::ISHOLE)?, obj_idx, parent: None,
                data, world_opacity: 1.0,
                transform: TransformValues::from_object(object)?, world: Affine2::default(),
            });
            parent_objs.push(if obj_idx as usize == context_start { None } else {
                Some((context_start.checked_add(parent_id as usize)
                    .unwrap_or(file.ocoll.len()), parent_id))
            });
        }

        // Parent IDs are artboard-relative object IDs; convert once to dense component indices.
        for (index, parent) in parent_objs.into_iter().enumerate() {
            let Some((parent_obj, parent_id)) = parent else { continue };
            let Some(parent) = obj_comps.get(parent_obj).copied().flatten() else {
                return Err(RuntimeError::InvalidParent {
                    comp_id: components[index].obj_idx + 1, parent_id })
            };
            components[index].parent = Some(parent);
        }

        let animations = build_animations(&file, context_start, context_end, &obj_comps)?;
        let mut runtime = Self { file, artboard_obj: context_start as u32, components,
            update_order: Vec::new(), gradients: Vec::new(),
            draw_groups: Vec::new(),
            animations, active_animation: None, elapsed: 0.0
        };
        // Construction order matters: world transforms feed gradients, then shape content feeds
        // draw grouping and finally draw rules reorder those completed groups.
        runtime.validate_hierarchy()?;
        runtime.update_world_state();
        let targets = runtime.build_shape_content()?;
        runtime.gradients = runtime.components.iter().enumerate()
            .filter_map(|(index, component)|
                component.gradient().is_some().then_some(index as u32)).collect();
        runtime.bind_animations(&targets);
        runtime.build_draw_groups();
        runtime.apply_draw_rules()?;
        Ok(runtime)
    }

    pub fn file(&self) -> &RiveFile { &self.file }
    pub fn elapsed(&self) -> f32 { self.elapsed }
    pub fn artboard_object_index(&self) -> u32 { self.artboard_obj }
    pub fn component_count(&self) -> usize { self.components.len() }

    pub fn animation_count(&self) -> u32 { self.animations.len() as u32 }
    pub fn animation(&self, index: u32) -> Option<AnimationInfo<'_>> {
        self.animations.get(index as usize).map(|animation| AnimationInfo {
            name: &animation.name, duration: animation.duration, fps: animation.fps,
            speed: animation.speed, loop_mode: animation.loop_mode,
        })
    }
    pub fn set_animation(&mut self, index: u32) -> Result<()> {
        if index as usize >= self.animations.len() {
            return Err(RuntimeError::AnimationNotFound(index))
        }
        if let Some(active) = self.active_animation { self.reset_animation(active); }
        self.active_animation = Some(index); self.elapsed = 0.0;
        self.apply_animation(); Ok(())
    }
    pub fn set_animation_by_name(&mut self, name: &[u8]) -> Result<()> {
        let index = self.animations.iter().position(|animation| animation.name == name)
            .ok_or(RuntimeError::AnimationNameNotFound)? as u32;
        self.set_animation(index)
    }

    pub fn advance(&mut self, delta_seconds: f32) -> bool {
        if delta_seconds <= 0.0 || self.active_animation.is_none() { return false }
        self.elapsed += delta_seconds.max(0.0);
        self.apply_animation();     true
    }

    fn bind_animations(&mut self, bindings: &[ComponentTarget]) {
        for animation in &mut self.animations {
            for track in &mut animation.tracks {
                let component = track.component;
                let object = &self.file.ocoll[
                    self.components[component as usize].obj_idx as usize];
                let default = match track.value_type() {
                    TrackValue::Scalar(_) => TrackValue::Scalar(
                        float(object, track.prop_id)
                            .unwrap_or_else(|_| core_float_default(track.prop_id))),
                    TrackValue::Color(_) => TrackValue::Color(
                        object.color(track.prop_id).ok().flatten()
                            .unwrap_or_else(|| core_color_default(track.prop_id))),
                    TrackValue::Bool(_) => TrackValue::Bool(
                        object.boolean(track.prop_id).ok().flatten()
                            .unwrap_or_else(|| core_boolean_default(track.prop_id))),
                    TrackValue::Uint(_) => TrackValue::Uint(
                        object.varuint(track.prop_id).ok().flatten()
                            .unwrap_or_else(|| core_varuint_default(track.prop_id))),
                };
                let target = resolve_track_target(&self.components, bindings,
                    component, track.prop_id, track.value_type(),
                    core_is_transform_component(object.type_id.0));
                let Some(target) = target else { continue };
                track.binding = Some(TrackBinding { target, default });
                match target {
                    TrackTarget::Transform { .. } => {}
                    TrackTarget::Geometry { component, .. } =>
                        push_unique(&mut animation.geometries, component),
                    TrackTarget::Vertex { path, .. } =>
                        push_unique(&mut animation.geometries, path),
                    TrackTarget::Gradient { component, .. } |
                    TrackTarget::GradientStopPos { component, .. } |
                    TrackTarget::GradientStopColor { component, .. } =>
                        push_unique(&mut animation.gradients, component),
                    _ => {}
                }
            }
            animation.tracks.retain(|track| track.binding.is_some());
        }
    }

    fn apply_animation(&mut self) {
        let Some(animation) = self.active_animation
            .and_then(|index| self.animations.get(index as usize)) else { return };
        let duration = animation.duration as f32;
        let mut frame = self.elapsed * animation.fps as f32 * animation.speed;
        if 0.0 < duration { match animation.loop_mode {
            1 => frame = frame.rem_euclid(duration),
            2 => {
                frame = frame.rem_euclid(duration * 2.0);
                if duration < frame { frame = duration * 2.0 - frame }
            }
            _ => frame = frame.min(duration),
        }}
        let mut transform_dirty = false;
        for track in &animation.tracks {
            if let (Some(binding), Some(value)) =
                (track.binding, evaluate_track(track, frame)) {
                transform_dirty |= apply_track(&mut self.components, binding.target, value);
            }
        }
        let (geometries, gradients) = (&animation.geometries, &animation.gradients);
        refresh_geometry(&mut self.components, geometries);
        if transform_dirty {
            update_world_state(&mut self.components, &self.update_order);
        }
        let gradients = if transform_dirty { &self.gradients } else { gradients };
        sync_gradients(&mut self.components, gradients);
    }

    fn reset_animation(&mut self, animation: u32) {
        let animation = &self.animations[animation as usize];
        let mut transform_dirty = false;
        for track in &animation.tracks {
            if let Some(binding) = track.binding {
                transform_dirty |= apply_track(
                    &mut self.components, binding.target, binding.default);
            }
        }
        let (geometries, gradients) = (&animation.geometries, &animation.gradients);
        refresh_geometry(&mut self.components, geometries);
        if transform_dirty {
            update_world_state(&mut self.components, &self.update_order);
        }
        let gradients = if transform_dirty { &self.gradients } else { gradients };
        sync_gradients(&mut self.components, gradients);
    }

    fn validate_hierarchy(&mut self) -> Result<()> {
        // The DFS simultaneously rejects parent cycles and builds parent-before-child order.
        let mut state = vec![0u8; self.components.len()];
        for index in 0..self.components.len() {
            self.visit_component(index, &mut state)?;
        }   Ok(())
    }

    fn visit_component(&mut self, index: usize, state: &mut [u8]) -> Result<()> {
        match state[index] {
            1 => return Err(RuntimeError::ParentCycle(self.components[index].obj_idx + 1)),
            2 => return Ok(()), _ => state[index] = 1,
        }
        if let Some(parent) = self.components[index].parent {
            self.visit_component(parent as usize, state)?;
        }
        self.update_order.push(index as u32);
        state[index] = 2;   Ok(())
    }

    fn update_world_state(&mut self) {
        update_world_state(&mut self.components, &self.update_order);
    }
}

fn update_world_state(components: &mut [Component], order: &[u32]) {
    // The validated order guarantees every parent world value is ready before its children.
    for &index in order {
        let index = index as usize;
        let component = &components[index];
        let local = component.transform.affine();
        let world = component.parent.map_or(local, |parent|
            components[parent as usize].world.then(local));
        let opacity = component.parent.map_or(component.transform.opacity, |parent|
            components[parent as usize].world_opacity * component.transform.opacity);
        components[index].world_opacity = opacity;
        components[index].world = world;
    }
}

fn replace_changed<T: PartialEq>(target: &mut T, value: T) -> bool {
    let changed = *target != value;
    *target = value;   changed
}

fn push_unique(values: &mut Vec<u32>, value: u32) {
    if !values.contains(&value) { values.push(value) }
}

fn transform_prop(prop_id: u32) -> bool {
    matches!(prop_id, property_ids::NODE_X | property_ids::NODE_Y |
        property_ids::TRANSFORMCOMPONENT_ROTATION |
        property_ids::TRANSFORMCOMPONENT_SCALEX |
        property_ids::TRANSFORMCOMPONENT_SCALEY |
        property_ids::WORLDTRANSFORMCOMPONENT_OPACITY)
}

fn resolve_track_target(components: &[Component], bindings: &[ComponentTarget],
    component: u32, prop_id: u32, value: TrackValue,
    transformable: bool) -> Option<TrackTarget> {
    let state = &components[component as usize];
    if transformable && matches!(value, TrackValue::Scalar(_)) && transform_prop(prop_id) {
        return Some(TrackTarget::Transform { component, prop_id })
    }
    match (bindings[component as usize], value) {
        (ComponentTarget::Color(ColorTarget::Solid(paint)), TrackValue::Color(_)) =>
            return Some(TrackTarget::SolidColor { component: paint }),
        (ComponentTarget::Color(ColorTarget::Stop { gradient, stop }),
            TrackValue::Color(_)) =>
            return Some(TrackTarget::GradientStopColor { component: gradient, stop }),
        (ComponentTarget::Color(ColorTarget::Stop { gradient, stop }),
            TrackValue::Scalar(_)) if prop_id == property_ids::POSITION =>
            return Some(TrackTarget::GradientStopPos { component: gradient, stop }),
        (ComponentTarget::Effect(target), _) =>
            return Some(TrackTarget::Effect { target, prop_id }),
        (ComponentTarget::Vertex { path, slot }, TrackValue::Scalar(_)) =>
            return Some(TrackTarget::Vertex { component, path, slot, prop_id }),
        _ => {}
    }
    match value {
        TrackValue::Scalar(_) if state.gradient().is_some() =>
            Some(TrackTarget::Gradient { component, prop_id }),
        TrackValue::Scalar(_) | TrackValue::Bool(_) | TrackValue::Uint(_)
            if state.geom().is_some() => Some(TrackTarget::Geometry { component, prop_id }),
        TrackValue::Bool(_) if prop_id == property_ids::SHAPEPAINT_ISVISIBLE &&
            state.paint().is_some() => Some(TrackTarget::Visibility { component }),
        TrackValue::Scalar(_) | TrackValue::Bool(_) | TrackValue::Uint(_)
            if state.paint().is_some() => Some(TrackTarget::Paint { component, prop_id }),
        _ => None,
    }
}

fn apply_track(components: &mut [Component], target: TrackTarget, value: TrackValue) -> bool {
    match (target, value) {
        (TrackTarget::Transform { component, prop_id }, TrackValue::Scalar(value)) => {
            return components[component as usize].transform.set(prop_id, value);
        }
        (TrackTarget::Geometry { component, prop_id }, TrackValue::Scalar(value)) => {
            if let Some(geom) = components[component as usize].geom_mut() {
                geom.set_float(prop_id, value);
            }
        }
        (TrackTarget::Geometry { component, prop_id }, TrackValue::Bool(value)) => {
            if let Some(geom) = components[component as usize].geom_mut() {
                geom.set_bool(prop_id, value);
            }
        }
        (TrackTarget::Geometry { component, prop_id }, TrackValue::Uint(value)) => {
            if let Some(geom) = components[component as usize].geom_mut() {
                geom.set_uint(prop_id, value);
            }
        }
        (TrackTarget::Vertex { component, path, slot, prop_id },
            TrackValue::Scalar(value)) => {
            let Some(params) = components[component as usize].vertex_mut()
                else { return false };
            if params.set(prop_id, value) == Some(true) {
                let vertex = params.vertex();
                if let Some(geom) = components[path as usize].geom_mut() {
                    geom.set_vertex(slot, vertex);
                }
            }
        }
        (TrackTarget::Gradient { component, prop_id }, TrackValue::Scalar(value)) => {
            if let Some(gradient) = components[component as usize].gradient_mut() {
                gradient.set(prop_id, value);
            }
        }
        (TrackTarget::GradientStopPos { component: gradient, stop },
            TrackValue::Scalar(value)) => {
            if let Some(gradient) = components[gradient as usize].gradient_mut() {
                gradient.set_stop_pos(stop, value);
            }
        }
        (TrackTarget::GradientStopColor { component: gradient, stop },
            TrackValue::Color(value)) => {
            if let Some(gradient) = components[gradient as usize].gradient_mut() {
                gradient.set_stop_color(stop, value);
            }
        }
        (TrackTarget::SolidColor { component: paint }, TrackValue::Color(value)) => {
            let Some(Paint::Fill { brush, .. } | Paint::Stroke { brush, .. }) =
                components[paint as usize].paint_mut()
                    .map(|paint| &mut paint.value) else { return false };
            if let Brush::Solid(color) = brush { *color = value }
        }
        (TrackTarget::Paint { component, prop_id }, TrackValue::Scalar(value)) => {
            set_paint_value(components, component, prop_id, value);
        }
        (TrackTarget::Paint { component, prop_id }, TrackValue::Bool(value)) => {
            set_paint_bool(components, component, prop_id, value);
        }
        (TrackTarget::Paint { component, prop_id }, TrackValue::Uint(value)) => {
            set_paint_uint(components, component, prop_id, value);
        }
        (TrackTarget::Effect { target, prop_id }, TrackValue::Scalar(value)) => {
            set_effect_value(components, target, prop_id, value);
        }
        (TrackTarget::Effect { target, prop_id }, TrackValue::Bool(value)) => {
            set_effect_bool(components, target, prop_id, value);
        }
        (TrackTarget::Effect { target, prop_id }, TrackValue::Uint(value)) => {
            set_effect_uint(components, target, prop_id, value);
        }
        (TrackTarget::Visibility { component }, TrackValue::Bool(value)) => {
            if let Some(paint) = components[component as usize].paint_mut() {
                paint.visible = value;
            }
        }
        _ => {}
    }   false
}

fn refresh_geometry(components: &mut [Component], targets: &[u32]) {
    for &index in targets {
        if let Some(geom) = components[index as usize].geom_mut() { geom.refresh() }
    }
}

fn sync_gradients(components: &mut [Component], targets: &[u32]) {
    for &index in targets { sync_gradient(components, index as usize); }
}

fn sync_gradient(components: &mut [Component], index: usize) {
        let Some(gradient) = components[index].gradient_mut() else { return };
        let Some(paint) = gradient.paint else { return };
        let stops = gradient.stops_dirty.then(|| {
            gradient.stops_dirty = false;
            gradient.sorted_stops()
        });
        let (start, end, opacity, radial) =
            (gradient.start, gradient.end, gradient.opacity, gradient.radial);
        let transform = components[index].world;
        let Some(ComponentPaint { value, .. }) =
            components[paint as usize].paint_mut() else { return };
        match value {
            Paint::Fill { brush: Brush::LinearGradient {
                start: brush_start, end: brush_end, trfm, opacity: brush_opacity,
                stops: brush_stops }, .. } |
            Paint::Stroke { brush: Brush::LinearGradient {
                start: brush_start, end: brush_end, trfm, opacity: brush_opacity,
                stops: brush_stops }, .. } if !radial => {
                if (*brush_start, *brush_end, *trfm, *brush_opacity) !=
                    (start, end, transform, opacity) {
                    (*brush_start, *brush_end, *trfm, *brush_opacity) =
                        (start, end, transform, opacity);
                }
                if let Some(stops) = stops { *brush_stops = stops }
            }
            Paint::Fill { brush: Brush::RadialGradient {
                center, radius, trfm, opacity: brush_opacity,
                stops: brush_stops }, .. } |
            Paint::Stroke { brush: Brush::RadialGradient {
                center, radius, trfm, opacity: brush_opacity,
                stops: brush_stops }, .. } if radial => {
                let next_radius = (end.x - start.x).hypot(end.y - start.y);
                if (*center, *radius, *trfm, *brush_opacity) !=
                    (start, next_radius, transform, opacity) {
                    (*center, *radius, *trfm, *brush_opacity) =
                        (start, next_radius, transform, opacity);
                }
                if let Some(stops) = stops { *brush_stops = stops }
            }   _ => {}
        }
}

pub(super) fn float(object: &Object, prop_id: u32) -> decode::Result<f32> {
    Ok(object.float(prop_id)?.unwrap_or_else(|| core_float_default(prop_id)))
}

pub(super) fn uint(object: &Object, prop_id: u32) -> decode::Result<u32> {
    Ok(object.varuint(prop_id)?.unwrap_or_else(|| core_varuint_default(prop_id)))
}

pub(super) fn boolean(object: &Object, prop_id: u32) -> decode::Result<bool> {
    Ok(object.boolean(prop_id)?.unwrap_or_else(|| core_boolean_default(prop_id)))
}

#[cfg(test)] #[path = "rt_tests.rs"] mod tests;
