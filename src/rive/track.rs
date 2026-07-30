
//! Runtime binding and application of decoded animation tracks.

use super::{Brush, ColorTarget, Component, ComponentPaint, ComponentTarget, EffectTarget,
    Paint, Runtime, TrackValue, core_boolean_default, core_color_default, core_float_default,
    core_is_transform_component, core_varuint_default, float, property_ids,
    apply_constraints, shape::{set_effect, set_paint}, update_world_state,
};
use crate::rive::animation::{
    Animation, LinearAnimation, RawAnimation, evaluate_track, mix_value
};

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
    ClipVisibility { component: u32 },
    Constraint { component: u32, prop_id: u32 },
    Image { component: u32, prop_id: u32 },
    NestedAnimation { component: u32, prop_id: u32 },
}

#[derive(Debug, Clone, Copy)] pub(in crate::rive) struct TrackBinding {
    target: TrackTarget, default: TrackValue,
}

impl Runtime {
    pub(super) fn bind_animations(&self, animations: Vec<RawAnimation>,
        bindings: &[ComponentTarget]) -> Vec<LinearAnimation> {
        animations.into_iter().map(|animation| {
            let Animation { name, duration, fps, speed, loop_mode, tracks, .. } = animation;
            let (mut geometries, mut gradients) = (Vec::new(), Vec::new());
            let tracks = tracks.into_iter().filter_map(|track| {
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
                let target = resolve_target(&self.components, bindings,
                    component, track.prop_id, track.value_type(),
                    core_is_transform_component(object.type_id.0))?;
                match target {
                    TrackTarget::Transform { .. } => {}
                    TrackTarget::Geometry { component, .. } =>
                        push_unique(&mut geometries, component),
                    TrackTarget::Vertex { path, .. } =>
                        push_unique(&mut geometries, path),
                    TrackTarget::Gradient { component, .. } |
                    TrackTarget::GradientStopPos { component, .. } |
                    TrackTarget::GradientStopColor { component, .. } =>
                        push_unique(&mut gradients, component),
                    _ => {}
                }
                Some(track.bind(TrackBinding { target, default }))
            }).collect();
            Animation { name, duration, fps, speed, loop_mode,
                tracks, geometries, gradients }
        }).collect()
    }

    pub(super) fn apply_animation(&mut self) {
        let Some(index) = self.active_animation else { return };
        let animation = &self.animations[index as usize];
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
        self.update_animation(index, Some(frame), 1.0);
    }

    pub(super) fn reset_animation(&mut self, animation: u32) {
        self.update_animation(animation, None, 1.0);
    }

    pub(super) fn apply_animation_sample(&mut self,
        index: u32, seconds: f32, mix: f32) -> bool {
        let Some(animation) = self.animations.get(index as usize) else { return false };
        let (duration, fps, speed, loop_mode) =
            (animation.duration as f32, animation.fps as f32,
             animation.speed, animation.loop_mode);
        let mut frame = seconds * fps * speed;
        if 0.0 < duration { match loop_mode {
            1 => frame = frame.rem_euclid(duration),
            2 => {
                frame = frame.rem_euclid(duration * 2.0);
                if duration < frame { frame = duration * 2.0 - frame }
            }
            _ => frame = frame.clamp(0.0, duration),
        }}
        self.update_animation(index, Some(frame), mix); true
    }

    pub(super) fn apply_animation_progress(&mut self,
        index: u32, progress: f32, mix: f32) -> bool {
        let Some(duration) = self.animations.get(index as usize)
            .map(|animation| animation.duration as f32) else { return false };
        self.update_animation(index, Some(duration * progress.clamp(0.0, 1.0)), mix); true
    }

    fn update_animation(&mut self, index: u32, frame: Option<f32>, mix: f32) {
        let animation = &self.animations[index as usize];
        let mut transform_dirty = false;
        for track in &animation.tracks {
            let value = frame.and_then(|frame| evaluate_track(track, frame))
                .map_or(track.binding.default,
                    |value| mix_value(track.binding.default, value, mix));
            transform_dirty |= apply_track(&mut self.components,
                track.binding.target, value);
        }
        refresh_geometry(&mut self.components, &animation.geometries);
        if transform_dirty {
            update_world_state(&mut self.components, &self.update_order);
            apply_constraints(&mut self.components, &self.update_order, &self.constraints,
                &mut self.constraint_dirty);
        }
        let gradients = if transform_dirty { &self.gradients } else { &animation.gradients };
        sync_gradients(&mut self.components, gradients);
    }
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

fn resolve_target(components: &[Component], bindings: &[ComponentTarget],
    component: u32, prop_id: u32, value: TrackValue,
    transformable: bool) -> Option<TrackTarget> {
    let state = &components[component as usize];
    if transformable && matches!(value, TrackValue::Scalar(_)) && transform_prop(prop_id) {
        return Some(TrackTarget::Transform { component, prop_id })
    }
    match (bindings[component as usize], value) {
        (ComponentTarget::Color(ColorTarget::Solid(component)), TrackValue::Color(_)) =>
            return Some(TrackTarget::SolidColor { component }),
        (ComponentTarget::Color(ColorTarget::Stop { gradient: component, stop }),
            TrackValue::Color(_)) =>
            return Some(TrackTarget::GradientStopColor { component, stop }),
        (ComponentTarget::Color(ColorTarget::Stop { gradient: component, stop }),
            TrackValue::Scalar(_)) if prop_id == property_ids::POSITION =>
            return Some(TrackTarget::GradientStopPos { component, stop }),
        (ComponentTarget::Effect(target), _) =>
            return Some(TrackTarget::Effect { target, prop_id }),
        (ComponentTarget::Vertex { path, slot }, TrackValue::Scalar(_)) =>
            return Some(TrackTarget::Vertex { component, path, slot, prop_id }),
        _ => {}
    }
    match value {
        TrackValue::Scalar(_) | TrackValue::Bool(_) | TrackValue::Uint(_)
            if state.constraint().is_some() =>
            Some(TrackTarget::Constraint { component, prop_id }),
        TrackValue::Scalar(_) if state.image().is_some() =>
            Some(TrackTarget::Image { component, prop_id }),
        TrackValue::Scalar(_) | TrackValue::Bool(_)
            if state.nested_animation().is_some() =>
            Some(TrackTarget::NestedAnimation { component, prop_id }),
        TrackValue::Scalar(_) if state.gradient().is_some() =>
            Some(TrackTarget::Gradient { component, prop_id }),
        TrackValue::Scalar(_) | TrackValue::Bool(_) | TrackValue::Uint(_)
            if state.geom().is_some() => Some(TrackTarget::Geometry { component, prop_id }),
        TrackValue::Bool(_) if prop_id == property_ids::SHAPEPAINT_ISVISIBLE &&
            state.paint().is_some() => Some(TrackTarget::Visibility { component }),
        TrackValue::Bool(_) if prop_id == property_ids::CLIPPINGSHAPE_ISVISIBLE &&
            state.clip().is_some() => Some(TrackTarget::ClipVisibility { component }),
        TrackValue::Scalar(_) | TrackValue::Bool(_) | TrackValue::Uint(_)
            if state.paint().is_some() => Some(TrackTarget::Paint { component, prop_id }),
        _ => None,
    }
}

fn apply_track(components: &mut [Component], target: TrackTarget, value: TrackValue) -> bool {
    match (target, value) {
        (TrackTarget::Transform { component, prop_id }, TrackValue::Scalar(value)) =>
            return components[component as usize].transform.set(prop_id, value),
        (TrackTarget::Geometry { component, prop_id }, value) => {
            if let Some(geom) = components[component as usize].geom_mut() {
                geom.set(prop_id, value);
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
        (TrackTarget::GradientStopPos { component, stop }, TrackValue::Scalar(value)) => {
            if let Some(gradient) = components[component as usize].gradient_mut() {
                gradient.set_stop_pos(stop, value);
            }
        }
        (TrackTarget::GradientStopColor { component, stop }, TrackValue::Color(value)) => {
            if let Some(gradient) = components[component as usize].gradient_mut() {
                gradient.set_stop_color(stop, value);
            }
        }
        (TrackTarget::SolidColor { component }, TrackValue::Color(value)) => {
            let Some(Paint::Fill { brush, .. } | Paint::Stroke { brush, .. }) =
                components[component as usize].paint_mut()
                    .map(|paint| &mut paint.value) else { return false };
            if let Brush::Solid(color) = brush { *color = value }
        }
        (TrackTarget::Paint { component, prop_id }, value) =>
            { set_paint(components, component, prop_id, value); }
        (TrackTarget::Effect { target, prop_id }, value) =>
            { set_effect(components, target, prop_id, value); }
        (TrackTarget::Visibility { component }, TrackValue::Bool(value)) => {
            if let Some(paint) = components[component as usize].paint_mut() {
                paint.visible = value;
            }
        }
        (TrackTarget::ClipVisibility { component }, TrackValue::Bool(value)) => {
            if let Some(clip) = components[component as usize].clip_mut() {
                clip.visible = value;
            }
        }
        (TrackTarget::Constraint { component, prop_id }, value) =>
            return components[component as usize].constraint_mut()
                .is_some_and(|constraint| constraint.set(prop_id, value)),
        (TrackTarget::Image { component, prop_id }, TrackValue::Scalar(value)) =>
            return components[component as usize].image_mut()
                .is_some_and(|image| image.set(prop_id, value)),
        (TrackTarget::NestedAnimation { component, prop_id }, value) =>
            return components[component as usize].nested_animation_mut()
                .is_some_and(|animation| animation.set(prop_id, value)),
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
        Paint::Fill   { brush: Brush::LinearGradient { start: brush_start, end: brush_end,
            trfm, opacity: brush_opacity, stops: brush_stops }, .. } |
        Paint::Stroke { brush: Brush::LinearGradient { start: brush_start, end: brush_end,
            trfm, opacity: brush_opacity, stops: brush_stops }, .. } if !radial => {
            if (*brush_start, *brush_end, *trfm, *brush_opacity) !=
                (start, end, transform, opacity) {
                (*brush_start, *brush_end, *trfm, *brush_opacity) =
                    (start, end, transform, opacity);
            }
            if let Some(stops) = stops { *brush_stops = stops }
        }
        Paint::Fill   { brush: Brush::RadialGradient { center, radius, trfm,
            opacity: brush_opacity, stops: brush_stops }, .. } |
        Paint::Stroke { brush: Brush::RadialGradient { center, radius, trfm,
            opacity: brush_opacity, stops: brush_stops }, .. } if radial => {
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
