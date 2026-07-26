
//! Retained Rive scene state, artboard selection, hierarchy updates, and playback control.

use std::{error::Error as StdError, fmt, f32};

use super::{animation::{LinearAnimation, build_animations, evaluate_track},
    display_list::{Affine2, Brush, DashSegment, DisplayList, FillRule,
        Geometry, GradientStop, Paint, PathEffect, Point, Shape,
        DrawItem, StrokeCap, StrokeJoin, TrimMode
    },
    decode::{self, DecodeError, Object, RiveFile, object_ids, property_ids,
        core_boolean_default, core_color_default, core_float_default, core_varuint_default,
        core_is_component, core_is_transform_component,
    }, path::{bounds, parametric_path, rectangle_radii},
};

#[path = "draw.rs"] mod draw;
#[path = "shape.rs"] mod shape;
use shape::set_paint_value;

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

#[derive(Debug)] struct Component {
    // Runtime indices are dense and stable; object indices continue to refer into RiveFile.
    obj_idx: u32, parent: Option<u32>,
    geometry: Option<Geometry>,
    paint: Option<Paint>,
    local_opacity: f32,
    world_opacity: f32,
    local: Affine2,
    world: Affine2,
    is_hole: bool,
}

#[derive(Debug)] struct DrawGroup {
    obj_idx: u32,
    opacity_component: u32,
    components: Vec<u32>,
    paints: Vec<u32>,
}

#[derive(Debug, Clone, Copy, PartialEq)] pub struct AnimationInfo<'a> {
    pub name: &'a [u8], pub duration: u32, pub fps: u32,
    pub speed: f32, pub loop_mode: u32,
}

#[derive(Clone, Copy)] struct TransformValues {
    x: f32, y: f32, rotation: f32, scale_x: f32, scale_y: f32, opacity: f32,
}

impl TransformValues {
    fn from_object(object: &Object) -> Self { Self {
        x: float(object, property_ids::NODE_X).unwrap_or(0.0),
        y: float(object, property_ids::NODE_Y).unwrap_or(0.0),
        rotation: float(object, property_ids::TRANSFORMCOMPONENT_ROTATION).unwrap_or(0.0),
         scale_x: float(object, property_ids::TRANSFORMCOMPONENT_SCALEX).unwrap_or(1.0),
         scale_y: float(object, property_ids::TRANSFORMCOMPONENT_SCALEY).unwrap_or(1.0),
         opacity: float(object, property_ids::WORLDTRANSFORMCOMPONENT_OPACITY).unwrap_or(1.0),
    } }

    fn set(&mut self, prop_id: u32, value: f32) -> bool { match prop_id {
        property_ids::NODE_X => self.x = value,
        property_ids::NODE_Y => self.y = value,
        property_ids::TRANSFORMCOMPONENT_ROTATION => self.rotation = value,
        property_ids::TRANSFORMCOMPONENT_SCALEX => self.scale_x = value,
        property_ids::TRANSFORMCOMPONENT_SCALEY => self.scale_y = value,
        property_ids::WORLDTRANSFORMCOMPONENT_OPACITY => self.opacity = value,
        _ => return false,
    }   true }

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
    draw_groups: Vec<DrawGroup>,
    effect_targets: Vec<Option<(u32, u32)>>,
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
            let geometry = match object.type_id.0 {
                object_ids::ELLIPSE   => Some(Geometry::Ellipse(bounds(object)?)),
                object_ids::RECTANGLE => Some(Geometry::RoundedRect {
                    rect: bounds(object)?, radii: rectangle_radii(object)?,
                }),
                object_ids::TRIANGLE | object_ids::POLYGON | object_ids::STAR =>
                    Some(Geometry::Path(parametric_path(object)?)),
                _ => None,
            };
            obj_comps[obj_idx as usize] = Some(components.len() as u32);
            components.push(Component {
                is_hole: boolean(object, property_ids::ISHOLE)?,
                obj_idx, parent: None, paint: None, world_opacity: 1.0, geometry,
                local_opacity: float(object, property_ids::WORLDTRANSFORMCOMPONENT_OPACITY)?,
                local: local_transform(object)?, world: Affine2::default(),
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

        let effect_targets = vec![None; components.len()];
        let animations = build_animations(&file, context_start, context_end, &obj_comps)?;
        let mut runtime = Self { file, artboard_obj: context_start as u32, components,
            update_order: Vec::new(), draw_groups: Vec::new(), effect_targets,
            animations, active_animation: None, elapsed: 0.0
        };
        // Construction order matters: world transforms feed gradients, then shape content feeds
        // draw grouping and finally draw rules reorder those completed groups.
        runtime.validate_hierarchy()?;
        runtime.update_world_state();
        runtime.build_shape_content()?;
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

        // Reset only this animation's targets to file defaults before applying evaluated tracks.
        // This keeps seeks and loop boundaries deterministic without scanning every component.
        let mut values: Vec<_> = animation.components.iter().map(|&index| {
            let component = &self.components[index as usize];
            TransformValues::from_object(&self.file.ocoll[component.obj_idx as usize])
        }).collect();
        for &component in &animation.components {
            let state  = &self.components[component as usize];
            let object = &self.file.ocoll[state.obj_idx as usize];
            match object.type_id.0 {
                object_ids::STROKE => set_paint_value(&mut self.components,
                    &self.effect_targets, component, property_ids::THICKNESS,
                    float(object, property_ids::THICKNESS).unwrap_or(0.0)),
                object_ids::TRIM_PATH => {
                    for prop_id in [property_ids::TRIMPATH_START,
                        property_ids::TRIMPATH_END, property_ids::TRIMPATH_OFFSET] {
                        set_paint_value(&mut self.components, &self.effect_targets,
                            component, prop_id, float(object, prop_id).unwrap_or(0.0));
                    }
                }   _ => {}
            }
        }
        for track in &animation.tracks {
            let Some(value) = evaluate_track(track, frame) else { continue };
            let target = &mut values[track.slot as usize];
            if !target.set(track.prop_id, value) {
                set_paint_value(&mut self.components, &self.effect_targets,
                    track.component, track.prop_id, value);
            }
        }
        for (&component, value) in animation.components.iter().zip(values) {
             self.components[component as usize].local = value.affine();
             self.components[component as usize].local_opacity = value.opacity;
        }   self.update_world_state();
    }

    fn reset_animation(&mut self, animation: u32) {
        for &index in &self.animations[animation as usize].components {
            let component = &mut self.components[index as usize];
            let object = &self.file.ocoll[component.obj_idx as usize];
            component.local = local_transform(object).unwrap_or_default();
            component.local_opacity =
                float(object, property_ids::WORLDTRANSFORMCOMPONENT_OPACITY).unwrap_or(1.0);
            match object.type_id.0 {
                object_ids::STROKE => set_paint_value(&mut self.components,
                    &self.effect_targets, index, property_ids::THICKNESS,
                    float(object, property_ids::THICKNESS).unwrap_or(0.0)),
                object_ids::TRIM_PATH => for prop_id in [property_ids::TRIMPATH_START,
                    property_ids::TRIMPATH_END, property_ids::TRIMPATH_OFFSET] {
                    set_paint_value(&mut self.components, &self.effect_targets,
                        index, prop_id, float(object, prop_id).unwrap_or(0.0));
                }   _ => {}
            }
        }   self.update_world_state();
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
        // update_order guarantees every parent world value is ready before its children.
        for &index in &self.update_order {
            let index = index as usize;
            let component = &self.components[index];
            let world = component.parent.map_or(component.local, |parent|
                    self.components[parent as usize].world.then(component.local));
            let world_opacity = component.parent.map_or(component.local_opacity, |parent|
                self.components[parent as usize].world_opacity * component.local_opacity);
            self.components[index].world_opacity = world_opacity;
            self.components[index].world = world;
        }
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

fn local_transform(object: &Object) -> decode::Result<Affine2> {
    if !core_is_transform_component(object.type_id.0) { return Ok(Affine2::default()) }
    Ok(Affine2::from_transform(
        float(object, property_ids::NODE_X)?,
        float(object, property_ids::NODE_Y)?,
        float(object, property_ids::TRANSFORMCOMPONENT_ROTATION)?,
        float(object, property_ids::TRANSFORMCOMPONENT_SCALEX)?,
        float(object, property_ids::TRANSFORMCOMPONENT_SCALEY)?,
    ))
}

#[cfg(test)] #[path = "rt_tests.rs"] mod tests;
