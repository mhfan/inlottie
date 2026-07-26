
//! Shape content assembly: geometry, brushes, paints, and path effects.

use std::{mem, sync::Arc};

use super::{ColorTarget, ComponentGeom, ComponentPaint, ComponentTarget, EffectTarget,
    Result, Runtime,
    RuntimeError, StrokeCap, StrokeJoin, TrimMode, Brush, Component, DashSegment, FillRule,
    Geometry, GradientStop, Paint, PathEffect,
    boolean, core_color_default, float, object_ids, property_ids, uint,
};
use crate::rive::path::build_path;

type EffectEntry = (u32, PathEffect, Vec<u32>);

impl Runtime {
    pub(super) fn build_shape_content(&mut self) -> Result<Vec<ComponentTarget>> {
        let len = self.components.len();
        let mut targets = vec![ComponentTarget::None; len];
        let mut stops = vec![Vec::new(); len];
        let mut brushes = vec![None; len];
        let mut effects = vec![Vec::new(); len];
        let mut vertices = vec![Vec::new(); len];
        let mut dash_segments = vec![Vec::new(); len];

        // Pass 1 gathers leaf data under its owning path, paint, gradient, or dash component.
        for (index, component) in self.components.iter().enumerate() {
            let object = &self.file.ocoll[component.obj_idx as usize];
            if let Some(vertex) = component.vertex().map(|params| params.vertex()) {
                if let Some(parent) = component.parent {
                    targets[index] = ComponentTarget::Vertex {
                        path: parent, slot: vertices[parent as usize].len() as u32 };
                    vertices[parent as usize].push(vertex);
                }
            } else if object.type_id.0 == object_ids::SOLID_COLOR {
                if let Some(parent) = component.parent {
                    let color = object.color(property_ids::SOLIDCOLOR_COLORVALUE)?
                        .unwrap_or_else(||
                            core_color_default(property_ids::SOLIDCOLOR_COLORVALUE));
                    brushes[parent as usize] = Some(Brush::Solid(color));
                    targets[index] = ComponentTarget::Color(ColorTarget::Solid(parent));
                }
            } else if object.type_id.0 == object_ids::GRADIENT_STOP {
                if let Some(parent) = component.parent {
                    stops[parent as usize].push((index as u32, GradientStop {
                        pos: float(object, property_ids::POSITION)?.clamp(0.0, 1.0),
                        color: object.color(property_ids::GRADIENTSTOP_COLORVALUE)?
                            .unwrap_or_else(||
                                core_color_default(property_ids::GRADIENTSTOP_COLORVALUE)),
                    }));
                }
            } else if object.type_id.0 == object_ids::DASH {
                if let Some(parent) = component.parent {
                    dash_segments[parent as usize].push((index as u32, DashSegment {
                        len: float(object, property_ids::DASH_LENGTH)?,
                        relative: boolean(object, property_ids::LENGTHISPERCENTAGE)?,
                    }));
                }
            }
        }

        // Pass 2 resolves brushes and effects after all of their child objects are available.
        for index in 0..self.components.len() {
            let object = &self.file.ocoll[self.components[index].obj_idx as usize];
            match object.type_id.0 {
                object_ids::LINEAR_GRADIENT | object_ids::RADIAL_GRADIENT => {
                    let Some(parent) = self.components[index].parent else { continue };
                    let transform = self.components[index].world;
                    let entries = mem::take(&mut stops[index]);
                    for (stop, &(source, _)) in entries.iter().enumerate() {
                        targets[source as usize] =
                            ComponentTarget::Color(ColorTarget::Stop {
                                gradient: index as u32, stop: stop as u32 });
                    }
                    let gradient = self.components[index].gradient_mut().unwrap();
                    gradient.stops = entries.into_iter().map(|(_, stop)| stop).collect();
                    let (start, end, opacity, radial, gradient_stops) = (gradient.start,
                        gradient.end, gradient.opacity, gradient.radial,
                        gradient.sorted_stops());
                    let brush = if radial {
                        Brush::RadialGradient { center: start,
                            radius: (end.x - start.x).hypot(end.y - start.y),
                            trfm: transform, opacity, stops: gradient_stops }
                    } else { Brush::LinearGradient {
                            start, end, trfm: transform, opacity, stops: gradient_stops
                    } };
                    gradient.paint = Some(parent);
                    brushes[parent as usize] = Some(brush);
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
                        }, Vec::new()));
                    }
                }
                object_ids::DASH_PATH => {
                    if let Some(parent) = self.components[index].parent {
                        let segments = mem::take(&mut dash_segments[index]);
                        let sources = segments.iter().map(|&(source, _)| source).collect();
                        effects[parent as usize].push((index as u32, PathEffect::Dash {
                            offset: float(object, property_ids::DASHPATH_OFFSET)?,
                            relative:
                                boolean(object, property_ids::OFFSETISPERCENTAGE)?,
                            segments:
                                segments.into_iter().map(|(_, segment)| segment).collect(),
                        }, sources));
                    }
                }   _ => {}
            }
        }

        // Pass 3 materializes geometry and paints. Keeping these passes together avoids
        // independent full-scene traversals for paths, gradients, and strokes.
        for index in 0..self.components.len() {
            let object = &self.file.ocoll[self.components[index].obj_idx as usize];
            match object.type_id.0 {
                object_ids::POINTS_PATH => {
                    let closed = boolean(object, property_ids::POINTSCOMMONPATH_ISCLOSED)?;
                    let vertices = mem::take(&mut vertices[index]);
                    self.components[index].data =
                        super::ComponentData::Geometry(ComponentGeom::Points {
                        cached: Geometry::Path(build_path(&vertices, closed)),
                        vertices, closed, dirty: false,
                    });
                }
                object_ids::FILL => {
                    let paint_effects = materialize_effects(
                        mem::take(&mut effects[index]), &mut targets, index as u32, true);
                    self.components[index].data =
                        super::ComponentData::Paint(ComponentPaint {
                        visible: boolean(object, property_ids::SHAPEPAINT_ISVISIBLE)?,
                        value: Paint::Fill {
                            brush: brushes[index].take().unwrap_or_else(|| Brush::Solid(
                                core_color_default(property_ids::SOLIDCOLOR_COLORVALUE))),
                            rule: fill_rule(uint(object, property_ids::FILL_FILLRULE)?),
                            effects: paint_effects,
                        },
                    });
                }
                object_ids::STROKE => {
                        let paint_effects = materialize_effects(
                            mem::take(&mut effects[index]), &mut targets, index as u32, false);
                        self.components[index].data =
                            super::ComponentData::Paint(ComponentPaint {
                            visible: boolean(object, property_ids::SHAPEPAINT_ISVISIBLE)?,
                            value: Paint::Stroke {
                                width: float(object, property_ids::THICKNESS)?,
                                brush: brushes[index].take().unwrap_or_else(|| Brush::Solid(
                                    core_color_default(property_ids::SOLIDCOLOR_COLORVALUE))),
                                cap: stroke_cap (uint(object, property_ids::CAP)?),
                                join: stroke_join(uint(object, property_ids::JOIN)?),
                                trfm_scale:
                                    boolean(object, property_ids::TRANSFORMAFFECTSSTROKE)?,
                                effects: paint_effects,
                            },
                        });
                    }
                _ => {}
            }
        }   Ok(targets)
    }
}

fn materialize_effects(mut entries: Vec<EffectEntry>, targets: &mut [ComponentTarget],
    paint: u32, trim_only: bool) -> Arc<[PathEffect]> {
    if trim_only {
        entries.retain(|(_, effect, _)| matches!(effect, PathEffect::Trim { .. }));
    }
    entries.into_iter().enumerate().map(|(effect, (source, value, segments))| {
        let effect = effect as u32;
        targets[source as usize] =
            ComponentTarget::Effect(EffectTarget::Effect { paint, effect });
        for (segment, source) in segments.into_iter().enumerate() {
            targets[source as usize] =
                ComponentTarget::Effect(EffectTarget::DashSegment {
                    paint, effect, segment: segment as u32 });
        }   value
    }).collect()
}

fn fill_rule(value: u32) -> FillRule { match value {
    1 => FillRule::EvenOdd, 2 => FillRule::Clockwise, _ => FillRule::NonZero,
} }

pub(super) fn set_paint_value(components: &mut [Component],
    component: u32, prop_id: u32, value: f32) -> bool {
    if prop_id != property_ids::THICKNESS { return false }
    let Some(ComponentPaint { value: Paint::Stroke { width, .. }, .. }) =
        components[component as usize].paint_mut() else { return false };
    *width = value;   true
}

pub(super) fn set_effect_value(components: &mut [Component],
    target: EffectTarget, prop_id: u32, value: f32) -> bool {
    let Some((effects, effect)) = effect_slot(components, target) else { return false };
    let changed = match (effects.get(effect), target, prop_id) {
        (Some(PathEffect::Trim { start, .. }), EffectTarget::Effect { .. },
            property_ids::TRIMPATH_START) => *start != value,
        (Some(PathEffect::Trim { end, .. }), EffectTarget::Effect { .. },
            property_ids::TRIMPATH_END) => *end != value,
        (Some(PathEffect::Trim { offset, .. }), EffectTarget::Effect { .. },
            property_ids::TRIMPATH_OFFSET) => *offset != value,
        (Some(PathEffect::Dash { offset, .. }), EffectTarget::Effect { .. },
            property_ids::DASHPATH_OFFSET) => *offset != value,
        (Some(PathEffect::Dash { segments, .. }),
            EffectTarget::DashSegment { segment, .. }, property_ids::DASH_LENGTH) =>
            segments.get(segment as usize).is_some_and(|segment| segment.len != value),
        _ => false,
    };
    if !changed { return true }
    match (Arc::make_mut(effects).get_mut(effect), target, prop_id) {
        (Some(PathEffect::Trim { start, .. }), EffectTarget::Effect { .. },
            property_ids::TRIMPATH_START) => *start = value,
        (Some(PathEffect::Trim { end, .. }), EffectTarget::Effect { .. },
            property_ids::TRIMPATH_END) => *end = value,
        (Some(PathEffect::Trim { offset, .. }), EffectTarget::Effect { .. },
            property_ids::TRIMPATH_OFFSET) => *offset = value,
        (Some(PathEffect::Dash { offset, .. }), EffectTarget::Effect { .. },
            property_ids::DASHPATH_OFFSET) => *offset = value,
        (Some(PathEffect::Dash { segments, .. }),
            EffectTarget::DashSegment { segment, .. },
            property_ids::DASH_LENGTH) => {
            if let Some(segment) = Arc::make_mut(segments).get_mut(segment as usize) {
                segment.len = value;
            }
        }   _ => {}
    }   true
}

pub(super) fn set_effect_bool(components: &mut [Component],
    target: EffectTarget, prop_id: u32, value: bool) -> bool {
    let Some((effects, effect)) = effect_slot(components, target) else { return false };
    let changed = match (effects.get(effect), target, prop_id) {
        (Some(PathEffect::Dash { relative, .. }), EffectTarget::Effect { .. },
            property_ids::OFFSETISPERCENTAGE) => *relative != value,
        (Some(PathEffect::Dash { segments, .. }),
            EffectTarget::DashSegment { segment, .. }, property_ids::LENGTHISPERCENTAGE) =>
            segments.get(segment as usize).is_some_and(|segment| segment.relative != value),
        _ => return false,
    };
    if !changed { return true }
    match (Arc::make_mut(effects).get_mut(effect), target, prop_id) {
        (Some(PathEffect::Dash { relative, .. }), EffectTarget::Effect { .. },
            property_ids::OFFSETISPERCENTAGE) => *relative = value,
        (Some(PathEffect::Dash { segments, .. }),
            EffectTarget::DashSegment { segment, .. }, property_ids::LENGTHISPERCENTAGE) => {
            if let Some(segment) = Arc::make_mut(segments).get_mut(segment as usize) {
                segment.relative = value;
            }
        }   _ => {}
    }   true
}

pub(super) fn set_paint_bool(components: &mut [Component],
    component: u32, prop_id: u32, value: bool) -> bool {
    if prop_id != property_ids::TRANSFORMAFFECTSSTROKE { return false }
    let Some(ComponentPaint { value: Paint::Stroke { trfm_scale, .. }, .. }) =
        components[component as usize].paint_mut() else { return false };
    *trfm_scale = value;   true
}

pub(super) fn set_paint_uint(components: &mut [Component],
    component: u32, prop_id: u32, value: u32) -> bool {
    if let Some(paint) = components[component as usize].paint_mut() {
        match (&mut paint.value, prop_id) {
            (Paint::Fill { rule, .. }, property_ids::FILL_FILLRULE) =>
                *rule = fill_rule(value),
            (Paint::Stroke { cap, .. }, property_ids::CAP) =>
                *cap = stroke_cap(value),
            (Paint::Stroke { join, .. }, property_ids::JOIN) =>
                *join = stroke_join(value),
            _ => return false,
        }   return true
    }   false
}

pub(super) fn set_effect_uint(components: &mut [Component],
    target: EffectTarget, prop_id: u32, value: u32) -> bool {
    if prop_id != property_ids::TRIMPATH_MODEVALUE { return false }
    let EffectTarget::Effect { .. } = target else { return false };
    let Some((effects, effect)) = effect_slot(components, target) else { return false };
    let mode = if value == 1 { TrimMode::Sequential } else { TrimMode::Synchronized };
    let Some(PathEffect::Trim { mode: current, .. }) =
        effects.get(effect) else { return false };
    if *current != mode {
        let Some(PathEffect::Trim { mode: current, .. }) =
            std::sync::Arc::make_mut(effects).get_mut(effect) else { return false };
        *current = mode;
    }   true
}

fn effect_slot(components: &mut [Component], target: EffectTarget) ->
    Option<(&mut Arc<[PathEffect]>, usize)> {
    let (paint, effect) = match target {
        EffectTarget::Effect { paint, effect } |
        EffectTarget::DashSegment { paint, effect, .. } => (paint, effect),
    };
    let paint = components[paint as usize].paint_mut()?;
    let effects = match &mut paint.value {
        Paint::Fill { effects, .. } | Paint::Stroke { effects, .. } => effects,
    };  Some((effects, effect as usize))
}

fn stroke_cap(value: u32) -> StrokeCap { match value {
    1 => StrokeCap::Round, 2 => StrokeCap::Square, _ => StrokeCap::Butt,
} }

fn stroke_join(value: u32) -> StrokeJoin { match value {
    1 => StrokeJoin::Round, 2 => StrokeJoin::Bevel, _ => StrokeJoin::Miter,
} }
