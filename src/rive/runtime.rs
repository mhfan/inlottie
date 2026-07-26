
use std::{error::Error as StdError, fmt, f32, mem};

use super::{display_list::{Affine2, Brush, CornerRadii, DashSegment, DisplayList, FillRule,
        Geometry, GeometryInstance, GradientStop, Paint, Path, PathCommand, PathEffect, Point,
        Primitive, Rect, StrokeCap, StrokeJoin, TrimMode
    },
    decode::{self, DecodeError, Object, RiveFile, object_ids, property_ids,
        core_boolean_default, core_color_default, core_float_default, core_varuint_default,
        core_is_component, core_is_transform_component,
    },
};

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

#[derive(Debug, Clone, Copy)] enum Interpolation {
    Hold, Linear, Cubic { x1: f32, y1: f32, x2: f32, y2: f32 },
}

#[derive(Debug)] struct Keyframe {
    frame: u32, value: f32, interpolation: Interpolation,
}

#[derive(Debug)] struct PropertyTrack {
    component: u32, prop_id: u32, keyframes: Vec<Keyframe>,
}

#[derive(Debug)] struct LinearAnimation {
    name: Vec<u8>, duration: u32, fps: u32, speed: f32, loop_mode: u32,
    tracks: Vec<PropertyTrack>,
}

/// Retained Rive scene state.
///
/// The first implementation resolves component transforms and emits static parametric and
/// points-path geometry with solid or gradient paint. Animation, constraints, clipping,
/// text and state machines can update this retained state without changing the display-list API.
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
    pub fn from_file(file: RiveFile) -> Result<Self> {
        Self::from_artboard(file, 0)
    }

    pub fn from_artboard(file: RiveFile, artboard_index: u32) -> Result<Self> {
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
        runtime.validate_hierarchy()?;
        runtime.update_world_state();
        runtime.build_paths_and_paints()?;
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
        self.apply_animation(); true
    }

    pub fn display_list(&self) -> DisplayList {
        let mut list = DisplayList::default();
        self.write_display_list(&mut list);
        list
    }

    pub fn write_display_list(&self, list: &mut DisplayList) {
        list.clear();
        let primitive_count = self.draw_groups.iter().filter(|group|
                0.0 < self.components[group.opacity_component as usize].world_opacity)
            .map(|group| group.paints.iter().filter_map(|&index|
                self.components[index as usize].paint.as_ref())
                .filter(|paint| visible_paint(paint)).count().max(1)).sum();
        list.primitives.reserve(primitive_count);

        for group in &self.draw_groups {
            let opacity = self.components[group.opacity_component as usize].world_opacity;
            if  opacity <= 0.0 { continue }
            let geometries: std::sync::Arc<[_]> = group.components.iter().map(|&index| {
                let component = &self.components[index as usize];
                GeometryInstance { obj_idx: component.obj_idx, is_hole: component.is_hole,
                    transform: component.world,
                    geometry: component.geometry.as_ref().unwrap().clone() }
            }).collect();
            if group.paints.is_empty() {
                list.primitives.push(Primitive {
                    obj_idx: group.obj_idx, opacity, geometries, paint: None });
            } else {
                let start = list.primitives.len();
                list.primitives.extend(group.paints.iter().filter_map(|&index| {
                    let paint = self.components[index as usize].paint.as_ref()?;
                    visible_paint(paint).then(|| Primitive {
                        obj_idx: group.obj_idx, opacity,
                        geometries: geometries.clone(), paint: Some(paint.clone()),
                    })
                }));
                if  list.primitives.len() == start {
                    list.primitives.push(Primitive {
                        obj_idx: group.obj_idx, opacity, geometries, paint: None });
                }
            }
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

        #[derive(Clone, Copy)] struct TransformValues {
            x: f32, y: f32, rotation: f32, scale_x: f32, scale_y: f32, opacity: f32,
        }
        let mut values: Vec<_> = self.components.iter().map(|component| {
            let object = &self.file.ocoll[component.obj_idx as usize];
            TransformValues {
                x: float(object, property_ids::NODE_X).unwrap_or(0.0),
                y: float(object, property_ids::NODE_Y).unwrap_or(0.0),
                rotation: float(object, property_ids::TRANSFORMCOMPONENT_ROTATION)
                    .unwrap_or(0.0),
                scale_x: float(object, property_ids::TRANSFORMCOMPONENT_SCALEX)
                    .unwrap_or(1.0),
                scale_y: float(object, property_ids::TRANSFORMCOMPONENT_SCALEY)
                    .unwrap_or(1.0),
                opacity: float(object, property_ids::WORLDTRANSFORMCOMPONENT_OPACITY)
                    .unwrap_or(1.0),
            }
        }).collect();
        let mut paint_values = Vec::new();
        for (component, state) in self.components.iter().enumerate() {
            let object = &self.file.ocoll[state.obj_idx as usize];
            match object.type_id.0 {
                object_ids::STROKE => paint_values.push((component as u32,
                    property_ids::THICKNESS,
                    float(object, property_ids::THICKNESS).unwrap_or(0.0))),
                object_ids::TRIM_PATH => {
                    for prop_id in [property_ids::TRIMPATH_START,
                        property_ids::TRIMPATH_END, property_ids::TRIMPATH_OFFSET] {
                        paint_values.push((component as u32, prop_id,
                            float(object, prop_id).unwrap_or(0.0)));
                    }
                }
                _ => {}
            }
        }
        for track in &animation.tracks {
            let Some(value) = evaluate_track(track, frame) else { continue };
            let target = &mut values[track.component as usize];
            match track.prop_id {
                property_ids::NODE_X => target.x = value,
                property_ids::NODE_Y => target.y = value,
                property_ids::TRANSFORMCOMPONENT_SCALEX => target.scale_x = value,
                property_ids::TRANSFORMCOMPONENT_SCALEY => target.scale_y = value,
                property_ids::TRANSFORMCOMPONENT_ROTATION => target.rotation = value,
                property_ids::WORLDTRANSFORMCOMPONENT_OPACITY => target.opacity = value,
                property_ids::THICKNESS | property_ids::TRIMPATH_START
                    | property_ids::TRIMPATH_END | property_ids::TRIMPATH_OFFSET =>
                    paint_values.push((track.component, track.prop_id, value)),
                _ => {},
            }
        }
        for (component, value) in self.components.iter_mut().zip(values) {
             component.local = Affine2::from_transform(value.x, value.y,
                value.rotation, value.scale_x, value.scale_y);
             component.local_opacity = value.opacity;
        }
        for (component, prop_id, value) in paint_values {
            if prop_id == property_ids::THICKNESS {
                if let Some(Paint::Stroke { width, .. }) =
                    &mut self.components[component as usize].paint {
                    *width = value;
                }   continue
            }
            let Some((paint, effect)) = self.effect_targets[component as usize] else { continue };
            let Some(Paint::Fill { effects, .. } | Paint::Stroke { effects, .. }) =
                &mut self.components[paint as usize].paint else { continue };
            let Some(PathEffect::Trim { start, end, offset, .. }) =
                std::sync::Arc::make_mut(effects).get_mut(effect as usize) else { continue };
            match prop_id {
                property_ids::TRIMPATH_OFFSET => *offset = value,
                property_ids::TRIMPATH_START => *start = value,
                property_ids::TRIMPATH_END => *end = value,
                _ => {}
            }
        }
        self.update_world_state();
    }

    fn ancestor_of_type(&self, mut component: Option<u32>, type_id: u32) -> Option<u32> {
        while let Some(index) = component {
            let candidate = &self.components[index as usize];
            if self.file.ocoll[candidate.obj_idx as usize].type_id.0 == type_id {
                return Some(index)
            }   component = candidate.parent;
        }   None
    }

    fn build_draw_groups(&mut self) {
        let shapes: Vec<_> = self.components.iter().map(|component|
            self.ancestor_of_type(component.parent, object_ids::SHAPE)).collect();
        let mut shape_groups = vec![None; self.components.len()];
        for (index, component) in self.components.iter().enumerate() {
            let type_id = self.file.ocoll[component.obj_idx as usize].type_id.0;
            if  type_id == object_ids::SHAPE {
                shape_groups[index] = Some(self.draw_groups.len());
                self.draw_groups.push(DrawGroup {
                    obj_idx: component.obj_idx, opacity_component: index as u32,
                    components: Vec::new(), paints: Vec::new()
                });
            } else if component.geometry.is_some() && shapes[index].is_none() {
                self.draw_groups.push(DrawGroup {
                    obj_idx: component.obj_idx, opacity_component: index as u32,
                    components: vec![index as u32], paints: Vec::new(),
                });
            }
        }
        for (index, component) in self.components.iter().enumerate() {
            let Some(shape) = shapes[index] else { continue };
            let group = &mut self.draw_groups[shape_groups[shape as usize].unwrap()];
            if component.geometry.is_some() { group.components.push(index as u32) }
            if component.paint.is_some() { group.paints.push(index as u32) }
        }
        self.draw_groups.retain(|group| !group.components.is_empty());
    }

    fn apply_draw_rules(&mut self) -> Result<()> {
        let mut rules_by_owner = vec![None; self.components.len()];
        for (index, component) in self.components.iter().enumerate() {
            let object = &self.file.ocoll[component.obj_idx as usize];
            if object.type_id.0 == object_ids::DRAW_RULES {
                if let Some(owner) = component.parent {
                    rules_by_owner[owner as usize] = Some(index as u32);
                }
            }
        }
        let group_rules: Vec<_> = self.draw_groups.iter().map(|group| {
            let mut component = Some(group.opacity_component);
            while let Some(index) = component {
                if let Some(rules) = rules_by_owner[index as usize] { return Some(rules) }
                component = self.components[index as usize].parent;
            }   None
        }).collect();

        let mut before = vec![Vec::new(); self.draw_groups.len()];
        let mut after  = vec![Vec::new(); self.draw_groups.len()];
        let mut attached = vec![false; self.draw_groups.len()];
        for rule_index in rules_by_owner.into_iter().flatten() {
            let rule = &self.file.ocoll[self.components[rule_index as usize].obj_idx as usize];
            let target_id = uint(rule, property_ids::DRAWTARGETID)?;
            let Some(target_obj) = self.artboard_obj.checked_add(target_id) else { continue };
            let Some(target_component) = self.components.iter()
                .find(|component| component.obj_idx == target_obj) else { continue };
            let target = &self.file.ocoll[target_component.obj_idx as usize];
            if target.type_id.0 != object_ids::DRAW_TARGET { continue }

            let drawable_id = uint(target, property_ids::DRAWABLEID)?;
            let Some(drawable_obj) = self.artboard_obj
                .checked_add(drawable_id) else { continue };
            let Some(target_group) = self.draw_groups.iter()
                .position(|group| group.obj_idx == drawable_obj) else { continue };
            let moved: Vec<_> = group_rules.iter().enumerate()
                .filter(|(_, rules)| **rules == Some(rule_index))
                .map(|(index, _)| index).collect();
            if moved.is_empty() || moved.contains(&target_group) { continue }

            let placement = if uint(target, property_ids::PLACEMENTVALUE)? == 0 {
                &mut before[target_group]
            } else {
                &mut  after[target_group]
            };
            for &index in &moved {
                if !attached[index] {
                    attached[index] = true;
                    placement.push(index);
                }
            }
        }

        fn emit(index: usize, groups: &mut [Option<DrawGroup>],
            before: &[Vec<usize>], after: &[Vec<usize>], state: &mut [u8],
            output: &mut Vec<DrawGroup>) -> Result<()> {
            match state[index] {
                1 => return Err(RuntimeError::DrawOrderCycle(
                    groups[index].as_ref().map_or(0, |group| group.obj_idx))),
                2 => return Ok(()), _ => state[index] = 1,
            }
            for &child in &before[index] {
                emit(child, groups, before, after, state, output)?;
            }
            output.push(groups[index].take().unwrap());
            for &child in &after[index] {
                emit(child, groups, before, after, state, output)?;
            }   state[index] = 2;   Ok(())
        }

        let mut groups: Vec<_> = mem::take(&mut self.draw_groups)
            .into_iter().map(Some).collect();
        let mut state = vec![0; groups.len()];
        let mut output = Vec::with_capacity(groups.len());
        for index in 0..groups.len() {
            if !attached[index] {
                emit(index, &mut groups, &before, &after, &mut state, &mut output)?;
            }
        }
        if let Some(index) = state.iter().position(|&value| value == 0) {
            emit(index, &mut groups, &before, &after, &mut state, &mut output)?;
        }
        self.draw_groups = output;  Ok(())
    }

    fn build_paths_and_paints(&mut self) -> Result<()> {
        let mut vertices = vec![Vec::new(); self.components.len()];
        let mut stops = vec![Vec::new(); self.components.len()];
        let mut brushes = vec![None; self.components.len()];
        let mut dash_segments = vec![Vec::new(); self.components.len()];
        let mut effects = vec![Vec::new(); self.components.len()];

        for component in &self.components {
            let object = &self.file.ocoll[component.obj_idx as usize];
            if let Some(vertex) = vertex(object)? {
                if let Some(parent) = component.parent {
                    vertices[parent as usize].push(vertex);
                }
            } else if object.type_id.0 == object_ids::SOLID_COLOR {
                if let Some(parent) = component.parent {
                    let color = object.color(property_ids::SOLIDCOLOR_COLORVALUE)?
                        .unwrap_or_else(||
                            core_color_default(property_ids::SOLIDCOLOR_COLORVALUE));
                    brushes[parent as usize] = Some(Brush::Solid(color));
                }
            } else if object.type_id.0 == object_ids::GRADIENT_STOP {
                if let Some(parent) = component.parent {
                    stops[parent as usize].push(GradientStop {
                        position: float(object, property_ids::POSITION)?.clamp(0.0, 1.0),
                        color: object.color(property_ids::GRADIENTSTOP_COLORVALUE)?
                            .unwrap_or_else(||
                                core_color_default(property_ids::GRADIENTSTOP_COLORVALUE)),
                    });
                }
            } else if object.type_id.0 == object_ids::DASH {
                if let Some(parent) = component.parent {
                    dash_segments[parent as usize].push(DashSegment {
                        length: float(object, property_ids::DASH_LENGTH)?,
                        is_percentage: boolean(object, property_ids::LENGTHISPERCENTAGE)?,
                    });
                }
            }
        }

        for index in 0..self.components.len() {
            let object = &self.file.ocoll[self.components[index].obj_idx as usize];
            match object.type_id.0 {
                object_ids::LINEAR_GRADIENT | object_ids::RADIAL_GRADIENT => {
                    stops[index].sort_by(|left, right|
                        left.position.total_cmp(&right.position));
                    let start = Point { x: float(object, property_ids::STARTX)?,
                        y: float(object, property_ids::STARTY)? };
                    let end = Point { x: float(object, property_ids::ENDX)?,
                        y: float(object, property_ids::ENDY)? };
                    let transform = self.components[index].world;
                    let opacity = float(object, property_ids::LINEARGRADIENT_OPACITY)?;
                    let gradient_stops = mem::take(&mut stops[index]).into();
                    let brush = if object.type_id.0 == object_ids::RADIAL_GRADIENT {
                        Brush::RadialGradient { center: start,
                            radius: (end.x - start.x).hypot(end.y - start.y),
                            transform, opacity, stops: gradient_stops }
                    } else { Brush::LinearGradient {
                            start, end, transform, opacity, stops: gradient_stops
                    } };
                    if let Some(parent) = self.components[index].parent {
                        brushes[parent as usize] = Some(brush);
                    }
                }
                object_ids::TRIM_PATH => {
                    let mode = match uint(object, property_ids::TRIMPATH_MODEVALUE)? {
                        1 => TrimMode::Sequential, 2 => TrimMode::Synchronized,
                        value => return Err(RuntimeError::InvalidTrimMode(value)),
                    };
                    if let Some(parent) = self.components[index].parent {
                        effects[parent as usize].push((index as u32, PathEffect::Trim {
                             start: float(object, property_ids::TRIMPATH_START)?,
                               end: float(object, property_ids::TRIMPATH_END)?,
                            offset: float(object, property_ids::TRIMPATH_OFFSET)?, mode,
                        }));
                    }
                }
                object_ids::DASH_PATH => {
                    if let Some(parent) = self.components[index].parent {
                        effects[parent as usize].push((index as u32, PathEffect::Dash {
                            offset: float(object, property_ids::DASHPATH_OFFSET)?,
                            offset_is_percentage:
                                boolean(object, property_ids::OFFSETISPERCENTAGE)?,
                            segments: mem::take(&mut dash_segments[index]).into(),
                        }));
                    }
                }
                _ => {}
            }
        }

        for index in 0..self.components.len() {
            let object = &self.file.ocoll[self.components[index].obj_idx as usize];
            match object.type_id.0 {
                object_ids::POINTS_PATH => {
                    let closed = boolean(object, property_ids::POINTSCOMMONPATH_ISCLOSED)?;
                    self.components[index].geometry =
                        Some(Geometry::Path(build_path(&vertices[index], closed)));
                }
                object_ids::FILL => {
                    if boolean(object, property_ids::SHAPEPAINT_ISVISIBLE)? {
                        let mut paint_effects = mem::take(&mut effects[index]);
                        paint_effects.retain(|(_, effect)|
                            matches!(effect, PathEffect::Trim { .. }));
                        let paint_effects: Vec<_> = paint_effects.into_iter().enumerate()
                            .map(|(effect, (source, value))| {
                                self.effect_targets[source as usize] =
                                    Some((index as u32, effect as u32));
                                value
                            }).collect();
                        self.components[index].paint = Some(Paint::Fill {
                            brush: brushes[index].take().unwrap_or_else(|| Brush::Solid(
                                core_color_default(property_ids::SOLIDCOLOR_COLORVALUE))),
                            rule: fill_rule(uint(object, property_ids::FILL_FILLRULE)?),
                            effects: paint_effects.into(),
                        });
                    }
                }
                object_ids::STROKE
                    if boolean(object, property_ids::SHAPEPAINT_ISVISIBLE)? => {
                        let paint_effects: Vec<_> = mem::take(&mut effects[index])
                            .into_iter().enumerate().map(|(effect, (source, value))| {
                                self.effect_targets[source as usize] =
                                    Some((index as u32, effect as u32));
                                value
                            }).collect();
                        self.components[index].paint = Some(Paint::Stroke {
                            width: float(object, property_ids::THICKNESS)?,
                            brush: brushes[index].take().unwrap_or_else(|| Brush::Solid(
                                core_color_default(property_ids::SOLIDCOLOR_COLORVALUE))),
                             cap: stroke_cap (uint(object, property_ids::CAP)?),
                            join: stroke_join(uint(object, property_ids::JOIN)?),
                            transform_affects:
                                boolean(object, property_ids::TRANSFORMAFFECTSSTROKE)?,
                            effects: paint_effects.into(),
                        });
                    }
                _ => {}
            }
        }
        Ok(())
    }

    fn validate_hierarchy(&mut self) -> Result<()> {
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

fn build_animations(file: &RiveFile, context_start: usize, context_end: usize,
    obj_comps: &[Option<u32>]) -> Result<Vec<LinearAnimation>> {
    let (mut current_animation, mut animations) = (None, Vec::new());
    let (mut current_component, mut current_track) = (None, None);

    for object in &file.ocoll[context_start..context_end] { match object.type_id.0 {
        object_ids::LINEAR_ANIMATION => {
            animations.push(LinearAnimation {
                name: object.bytes(property_ids::ANIMATION_NAME)?
                    .unwrap_or_default().to_vec(),
                duration: uint(object, property_ids::LINEARANIMATION_DURATION)?,
                fps: uint(object, property_ids::FPS)?,
                speed: float(object, property_ids::LINEARANIMATION_SPEED)?,
                loop_mode: uint(object, property_ids::LOOPVALUE)?,
                tracks: Vec::new(),
            });
            current_animation = Some(animations.len() - 1);
            current_component = None; current_track = None;
        }
        object_ids::KEYED_OBJECT => {
            let target = context_start.checked_add(
                uint(object, property_ids::KEYEDOBJECT_OBJECTID)? as usize);
            current_component = target.and_then(|index|
                obj_comps.get(index).copied().flatten());
            current_track = None;
        }
        object_ids::KEYED_PROPERTY => {
            current_track = match (current_animation, current_component) {
                (Some(animation), Some(component)) => {
                    animations[animation].tracks.push(PropertyTrack {
                        component,
                        prop_id: uint(object, property_ids::KEYEDPROPERTY_PROPERTYKEY)?,
                        keyframes: Vec::new(),
                    });
                    Some((animation, animations[animation].tracks.len() - 1))
                }
                _ => None,
            };
        }
        object_ids::KEY_FRAME_DOUBLE => if let Some((animation, track)) = current_track {
            animations[animation].tracks[track].keyframes.push(Keyframe {
                frame: uint(object, property_ids::FRAME)?,
                value: float(object, property_ids::KEYFRAMEDOUBLE_VALUE)?,
                interpolation: keyframe_interpolation(file, context_start, object)?,
            });
        }
        _ => {}
    }}
    for animation in &mut animations {
        animation.tracks.retain(|track| !track.keyframes.is_empty());
        for track in &mut animation.tracks {
            track.keyframes.sort_by_key(|keyframe| keyframe.frame);
        }
    }
    Ok(animations)
}

fn keyframe_interpolation(file: &RiveFile, context_start: usize,
    keyframe: &Object) -> Result<Interpolation> {
    let kind = uint(keyframe, property_ids::INTERPOLATINGKEYFRAME_INTERPOLATIONTYPE)?;
    if  kind == 0 { return Ok(Interpolation::Hold) }
    if  kind == 1 { return Ok(Interpolation::Linear) }
    if  kind != 2 { return Err(RuntimeError::InvalidInterpolation(kind)) }

    let id = uint(keyframe, property_ids::INTERPOLATINGKEYFRAME_INTERPOLATORID)?;
    let interpolator = context_start.checked_add(id as usize)
        .and_then(|index| file.ocoll.get(index))
        .ok_or(RuntimeError::InvalidInterpolator(id))?;
    let props = if interpolator.type_id.0 == object_ids::CUBIC_INTERPOLATOR_COMPONENT {
        [property_ids::CUBICINTERPOLATORCOMPONENT_X1,
         property_ids::CUBICINTERPOLATORCOMPONENT_Y1,
         property_ids::CUBICINTERPOLATORCOMPONENT_X2,
         property_ids::CUBICINTERPOLATORCOMPONENT_Y2]
    } else if matches!(interpolator.type_id.0, object_ids::CUBIC_EASE_INTERPOLATOR |
        object_ids::CUBIC_VALUE_INTERPOLATOR | object_ids::CUBIC_INTERPOLATOR) {
        [property_ids::CUBICINTERPOLATOR_X1, property_ids::CUBICINTERPOLATOR_Y1,
         property_ids::CUBICINTERPOLATOR_X2, property_ids::CUBICINTERPOLATOR_Y2]
    } else { return Err(RuntimeError::InvalidInterpolator(id)) };
    let [x1, y1, x2, y2] = props;
    let (x1, y1, x2, y2) = (float(interpolator, x1)?, float(interpolator, y1)?,
        float(interpolator, x2)?, float(interpolator, y2)?);
    if [x1, y1, x2, y2].iter().any(|value| !value.is_finite()) {
        return Err(RuntimeError::InvalidInterpolator(id))
    }
    Ok(Interpolation::Cubic { x1, y1, x2, y2 })
}

fn evaluate_track(track: &PropertyTrack, frame: f32) -> Option<f32> {
    let first = track.keyframes.first()?;
    let upper = track.keyframes.partition_point(|keyframe| keyframe.frame as f32 <= frame);
    if upper == 0 { return Some(first.value) }
    let current = &track.keyframes[upper - 1];
    let Some(next) = track.keyframes.get(upper) else { return Some(current.value) };
    if matches!(current.interpolation, Interpolation::Hold) ||
        next.frame == current.frame {
        return Some(current.value)
    }
    let mut factor = ((frame - current.frame as f32) /
        (next.frame - current.frame) as f32).clamp(0.0, 1.0);
    if let Interpolation::Cubic { x1, y1, x2, y2 } = current.interpolation {
        let at = |t: f32, p1: f32, p2: f32|
            ((3.0 * p1 - 3.0 * p2 + 1.0) * t +
             (3.0 * p2 - 6.0 * p1)) * t * t + 3.0 * p1 * t;
        let slope = |t: f32, p1: f32, p2: f32|
            3.0 * (3.0 * p1 - 3.0 * p2 + 1.0) * t * t +
            2.0 * (3.0 * p2 - 6.0 * p1) * t + 3.0 * p1;
        let (x, mut parameter) = (factor, factor);
        for _ in 0..6 {
            let derivative = slope(parameter, x1, x2);
            if  derivative.abs() <= f32::EPSILON { break }
            parameter = (parameter - (at(parameter, x1, x2) - x) / derivative)
                .clamp(0.0, 1.0);
        }
        let (mut lower, mut upper) = (0.0, 1.0);
        for _ in 0..10 {
            if at(parameter, x1, x2) < x { lower = parameter } else { upper = parameter }
            parameter = (lower + upper) * 0.5;
        }   factor = at(parameter, y1, y2);
    }
    Some(current.value + (next.value - current.value) * factor)
}

#[derive(Clone, Copy)] struct Vertex {
    position: Point,
    incoming: Option<Point>,
    outgoing: Option<Point>,
    radius: f32,
}

fn vertex(object: &Object) -> decode::Result<Option<Vertex>> {
    let point = || -> decode::Result<Point> { Ok(Point {
        x: float(object, property_ids::VERTEX_X)?,
        y: float(object, property_ids::VERTEX_Y)?,
    }) };
    let control = |position: Point, rotation: f32, distance: f32| Point {
        x: position.x + rotation.cos() * distance,
        y: position.y + rotation.sin() * distance,
    };
    let (position, incoming, outgoing, radius) = match object.type_id.0 {
        object_ids::STRAIGHT_VERTEX =>
            (point()?, None, None, float(object, property_ids::RADIUS)?),
        object_ids::CUBIC_DETACHED_VERTEX => {
            let position = point()?;
            let incoming = control(position,
                float(object, property_ids::INROTATION)?,
                float(object, property_ids::CUBICDETACHEDVERTEX_INDISTANCE)?);
            let outgoing = control(position,
                float(object, property_ids::OUTROTATION)?,
                float(object, property_ids::CUBICDETACHEDVERTEX_OUTDISTANCE)?);
            (position, Some(incoming), Some(outgoing), 0.0)
        }
        object_ids::CUBIC_ASYMMETRIC_VERTEX => {
            let position = point()?;
            let rotation = float(object, property_ids::CUBICASYMMETRICVERTEX_ROTATION)?;
            let incoming = control(position, rotation,
                -float(object, property_ids::CUBICASYMMETRICVERTEX_INDISTANCE)?);
            let outgoing = control(position, rotation,
                 float(object, property_ids::CUBICASYMMETRICVERTEX_OUTDISTANCE)?);
            (position, Some(incoming), Some(outgoing), 0.0)
        }
        object_ids::CUBIC_MIRRORED_VERTEX => {
            let position = point()?;
            let rotation = float(object, property_ids::CUBICMIRROREDVERTEX_ROTATION)?;
            let distance = float(object, property_ids::CUBICMIRROREDVERTEX_DISTANCE)?;
            (position, Some(control(position, rotation, -distance)),
                       Some(control(position, rotation,  distance)), 0.0)
        }   _ => return Ok(None),
    };  Ok(Some(Vertex { position, incoming, outgoing, radius }))
}

fn build_path(vertices: &[Vertex], closed: bool) -> Path {
    if vertices.len() < 2 { return Path::default() }
    let rendered: Vec<_> = vertices.iter().enumerate()
        .map(|(index, &vertex)| rounded_vertex(vertices, index, closed, vertex)).collect();
    let first = rendered[0];
    let mut commands = Vec::with_capacity(vertices.len() * 2 + usize::from(closed));
    commands.push(PathCommand::MoveTo(first.entry));
    push_corner(&mut commands, first);
    for pair in rendered.windows(2) {
        push_segment(&mut commands, pair[0], pair[1]);
        push_corner (&mut commands, pair[1]);
    }
    if closed {
        let last = *rendered.last().unwrap();
        if last.outgoing.is_some() || first.incoming.is_some() {
            push_segment(&mut commands, last, first);
        }
        commands.push(PathCommand::Close);
    }   Path { commands: commands.into() }
}

#[derive(Clone, Copy)] struct RenderVertex {
    entry: Point, exit: Point,
    incoming: Option<Point>, outgoing: Option<Point>,
    corner: Option<(Point, Point)>,
}

fn rounded_vertex(vertices: &[Vertex], index: usize, closed: bool,
    vertex: Vertex) -> RenderVertex {
    let plain = || RenderVertex { entry: vertex.position, exit: vertex.position,
        incoming: vertex.incoming, outgoing: vertex.outgoing, corner: None };
    if vertex.radius == 0.0 || (!closed && (index == 0 || index + 1 == vertices.len())) {
        return plain()
    }

    let prev = vertices[(index + vertices.len() - 1) % vertices.len()];
    let next = vertices[(index + 1) % vertices.len()];
    let mut to_prev = offset(prev.outgoing.unwrap_or(prev.position), vertex.position);
    let mut to_next = offset(next.incoming.unwrap_or(next.position), vertex.position);
    let (prev_len, next_len) = (normalize(&mut to_prev), normalize(&mut to_next));
    if prev_len == 0.0 || next_len == 0.0 { return plain() }

    let radius = vertex.radius.abs().min(prev_len / 2.0).min(next_len / 2.0);
    let ideal = ideal_control_distance(to_prev, to_next, radius);
    let entry = add_scaled(vertex.position, to_prev, radius);
    let exit  = add_scaled(vertex.position, to_next, radius);
    let mut ctrl1 = add_scaled(vertex.position, to_prev, radius - ideal);
    let mut ctrl2 = add_scaled(vertex.position, to_next, radius - ideal);
    if vertex.radius < 0.0 {
        rotate_corner(exit, entry, vertex.position, &mut ctrl1, &mut ctrl2);
    }
    RenderVertex { entry, exit, incoming: None, outgoing: None, corner: Some((ctrl1, ctrl2)) }
}

fn push_segment(commands: &mut Vec<PathCommand>, from: RenderVertex, to: RenderVertex) {
    if from.outgoing.is_some() || to.incoming.is_some() {
        commands.push(PathCommand::CubicTo {
            ctrl1: from.outgoing.unwrap_or(from.exit),
            ctrl2:   to.incoming.unwrap_or(to.entry), to: to.entry,
        });
    } else {
        commands.push(PathCommand::LineTo(to.entry));
    }
}

fn push_corner(commands: &mut Vec<PathCommand>, vertex: RenderVertex) {
    if let Some((ctrl1, ctrl2)) = vertex.corner {
        commands.push(PathCommand::CubicTo { ctrl1, ctrl2, to: vertex.exit });
    }
}

fn offset(point: Point, origin: Point) -> Point {
    Point { x: point.x - origin.x, y: point.y - origin.y }
}

fn add_scaled(point: Point, vector: Point, scale: f32) -> Point {
    Point { x: point.x + vector.x * scale, y: point.y + vector.y * scale }
}

fn normalize(vector: &mut Point) -> f32 {
    let length = vector.x.hypot(vector.y);
    if  length != 0.0 {
        vector.x /= length;
        vector.y /= length;
    }   length
}

fn ideal_control_distance(prev: Point, next: Point, radius: f32) -> f32 {
    let angle = (prev.x * next.y - prev.y * next.x)
          .atan2(prev.x * next.x + prev.y * next.y).abs(); // XXX: fast_atan2
    4.0 / 3.0 * (angle / 4.0).tan() * radius *
    if angle < f32::consts::FRAC_PI_2 { 1.0 + angle.cos() } else { 2.0 - angle.sin() }
}

fn rotate_corner(next: Point, prev: Point, point: Point,
    outgoing: &mut Point, incoming: &mut Point) {
    let (v1, v2) = (offset(prev, next), offset(point, next));
    let angle = (v1.x * v2.y - v1.y * v2.x).atan2(v1.x * v2.x + v1.y * v2.y);
    *outgoing = rotate_around(*outgoing, prev,  angle * 2.0);
    *incoming = rotate_around(*incoming, next, -angle * 2.0);
}

fn rotate_around(point: Point, origin: Point, angle: f32) -> Point {
    let (sin, cos) = angle.sin_cos();
    let point = offset(point, origin);
    Point { x: point.x * cos - point.y * sin + origin.x,
            y: point.x * sin + point.y * cos + origin.y }
}

fn fill_rule(value: u32) -> FillRule { match value {
    1 => FillRule::EvenOdd, 2 => FillRule::Clockwise, _ => FillRule::NonZero,
} }

fn visible_paint(paint: &Paint) -> bool { match paint {
    Paint::Stroke { width, .. } => 0.0 < *width,
    _ => true,
} }

fn stroke_cap(value: u32) -> StrokeCap { match value {
    1 => StrokeCap::Round, 2 => StrokeCap::Square, _ => StrokeCap::Butt,
} }

fn stroke_join(value: u32) -> StrokeJoin { match value {
    1 => StrokeJoin::Round, 2 => StrokeJoin::Bevel, _ => StrokeJoin::Miter,
} }

fn float(object: &Object, prop_id: u32) -> decode::Result<f32> {
    Ok(object.float(prop_id)?.unwrap_or_else(|| core_float_default(prop_id)))
}

fn uint(object: &Object, prop_id: u32) -> decode::Result<u32> {
    Ok(object.varuint(prop_id)?.unwrap_or_else(|| core_varuint_default(prop_id)))
}

fn boolean(object: &Object, prop_id: u32) -> decode::Result<bool> {
    Ok(object.boolean(prop_id)?.unwrap_or_else(|| core_boolean_default(prop_id)))
}

fn local_transform(object: &Object) -> decode::Result<Affine2> {
    if !core_is_transform_component(object.type_id.0) {
        return Ok(Affine2::default())
    }
    Ok(Affine2::from_transform(
        float(object, property_ids::NODE_X)?,
        float(object, property_ids::NODE_Y)?,
        float(object, property_ids::TRANSFORMCOMPONENT_ROTATION)?,
        float(object, property_ids::TRANSFORMCOMPONENT_SCALEX)?,
        float(object, property_ids::TRANSFORMCOMPONENT_SCALEY)?,
    ))
}

fn bounds(object: &Object) -> decode::Result<Rect> {
    let width  = float(object, property_ids::PARAMETRICPATH_WIDTH)?;
    let height = float(object, property_ids::PARAMETRICPATH_HEIGHT)?;
    let origin_x = float(object, property_ids::PARAMETRICPATH_ORIGINX)?;
    let origin_y = float(object, property_ids::PARAMETRICPATH_ORIGINY)?;
    Ok(Rect { x: -width * origin_x, y: -height * origin_y, width, height, })
}

fn parametric_path(object: &Object) -> Result<Path> {
    let rect = bounds(object)?;
    if object.type_id.0 == object_ids::TRIANGLE {
        return Ok(build_path(&[
            straight_vertex(rect.x + rect.width / 2.0, rect.y, 0.0),
            straight_vertex(rect.x + rect.width, rect.y + rect.height, 0.0),
            straight_vertex(rect.x, rect.y + rect.height, 0.0),
        ], true))
    }

    let points = uint(object, property_ids::POINTS)?;
    let (vertex_count, inner_radius) = if object.type_id.0 == object_ids::STAR {
        (points.saturating_mul(2), float(object, property_ids::INNERRADIUS)?)
    } else { (points, 1.0) };
    if u32::from(u16::MAX) < vertex_count {
        return Err(RuntimeError::TooManyVertices(vertex_count))
    }
    let vertex_count = vertex_count as usize;
    let (half_width, half_height) = (rect.width / 2.0, rect.height / 2.0);
    let center = Point { x: rect.x + half_width, y: rect.y + half_height };
    let radius = float(object, property_ids::CORNERRADIUS)?;
    let mut vertices = Vec::with_capacity(vertex_count);
    for index in 0..vertex_count {
        let angle = -f32::consts::FRAC_PI_2 +
                     f32::consts::TAU * index as f32 / vertex_count as f32;
        let scale = if object.type_id.0 == object_ids::STAR && index % 2 == 1 {
            inner_radius
        } else { 1.0 };
        vertices.push(straight_vertex(
            center.x + angle.cos() * half_width  * scale,
            center.y + angle.sin() * half_height * scale, radius));
    }
    Ok(build_path(&vertices, true))
}

fn straight_vertex(x: f32, y: f32, radius: f32) -> Vertex {
    Vertex { position: Point { x, y }, incoming: None, outgoing: None, radius }
}

fn rectangle_radii(object: &Object) -> decode::Result<CornerRadii> {
    let top_left = float(object, property_ids::RECTANGLE_CORNERRADIUSTL)?;
    let linked = boolean(object, property_ids::RECTANGLE_LINKCORNERRADIUS)?;
    let radius = |prop_id| float(object, prop_id);
    Ok(if linked { CornerRadii { top_left, top_right: top_left,
            bottom_right: top_left, bottom_left: top_left,
    } } else { CornerRadii { top_left,
               top_right: radius(property_ids::RECTANGLE_CORNERRADIUSTR)?,
            bottom_right: radius(property_ids::RECTANGLE_CORNERRADIUSBR)?,
            bottom_left:  radius(property_ids::RECTANGLE_CORNERRADIUSBL)?,
    } })
}

#[cfg(test)] #[path = "runtime_tests.rs"] mod tests;
