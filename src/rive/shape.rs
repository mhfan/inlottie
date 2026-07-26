
//! Shape content assembly: geometry, brushes, paints, and path effects.

use std::{mem, sync::Arc};

use super::{ColorTarget, ComponentGeom, ComponentPaint, EffectTarget, Result, Runtime,
    RuntimeError, StrokeCap, StrokeJoin, TrimMode, Brush, Component, DashSegment, FillRule,
    Geometry, GradientStop, Paint, PathEffect,
    boolean, core_color_default, float, object_ids, property_ids, uint,
};
use crate::rive::path::build_path;

impl Runtime {
    pub(super) fn build_shape_content(&mut self) -> Result<()> {
        let len = self.components.len();
        let mut stops = vec![Vec::new(); len];
        let mut brushes = vec![None; len];
        let mut effects = vec![Vec::new(); len];
        let mut vertices = vec![Vec::new(); len];
        let mut dash_segments = vec![Vec::new(); len];

        // Pass 1 gathers leaf data under its owning path, paint, gradient, or dash component.
        for (index, component) in self.components.iter().enumerate() {
            let object = &self.file.ocoll[component.obj_idx as usize];
            if let Some(vertex) = component.vertex.as_ref().map(|params| params.vertex()) {
                if let Some(parent) = component.parent {
                    self.vertex_targets[index] =
                        Some((parent, vertices[parent as usize].len() as u32));
                    vertices[parent as usize].push(vertex);
                }
            } else if object.type_id.0 == object_ids::SOLID_COLOR {
                if let Some(parent) = component.parent {
                    let color = object.color(property_ids::SOLIDCOLOR_COLORVALUE)?
                        .unwrap_or_else(||
                            core_color_default(property_ids::SOLIDCOLOR_COLORVALUE));
                    brushes[parent as usize] = Some(Brush::Solid(color));
                    self.color_targets[index] = Some(ColorTarget::Solid(parent));
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
                        self.color_targets[source as usize] = Some(ColorTarget::Stop {
                            gradient: index as u32, stop: stop as u32,
                        });
                    }
                    let gradient = self.components[index].gradient.as_mut().unwrap();
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
                    self.gradient_targets[index] = Some(parent);
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
                    self.components[index].geom = Some(ComponentGeom::Points {
                        cached: Geometry::Path(build_path(&vertices, closed)),
                        vertices, closed, dirty: false,
                    });
                }
                object_ids::FILL => {
                    let mut paint_effects = mem::take(&mut effects[index]);
                    paint_effects.retain(|(_, effect, _)|
                        matches!(effect, PathEffect::Trim { .. }));
                    // Retain the source component -> effect slot mapping for animation.
                    let paint_effects: Vec<_> = paint_effects.into_iter().enumerate()
                        .map(|(effect, (source, value, segments))| {
                            self.effect_targets[source as usize] =
                                Some(EffectTarget::Effect {
                                    paint: index as u32, effect: effect as u32 });
                            for (segment, source) in segments.into_iter().enumerate() {
                                self.effect_targets[source as usize] =
                                    Some(EffectTarget::DashSegment {
                                        paint: index as u32, effect: effect as u32,
                                        segment: segment as u32 });
                            }   value
                        }).collect();
                    self.components[index].paint = Some(ComponentPaint {
                        visible: boolean(object, property_ids::SHAPEPAINT_ISVISIBLE)?,
                        value: Paint::Fill {
                            brush: brushes[index].take().unwrap_or_else(|| Brush::Solid(
                                core_color_default(property_ids::SOLIDCOLOR_COLORVALUE))),
                            rule: fill_rule(uint(object, property_ids::FILL_FILLRULE)?),
                            effects: paint_effects.into(),
                        },
                    });
                }
                object_ids::STROKE => {
                        let paint_effects: Vec<_> = mem::take(&mut effects[index])
                            .into_iter().enumerate()
                            .map(|(effect, (source, value, segments))| {
                                self.effect_targets[source as usize] =
                                    Some(EffectTarget::Effect {
                                        paint: index as u32, effect: effect as u32 });
                                for (segment, source) in segments.into_iter().enumerate() {
                                    self.effect_targets[source as usize] =
                                        Some(EffectTarget::DashSegment {
                                            paint: index as u32, effect: effect as u32,
                                            segment: segment as u32 });
                                }   value
                            }).collect();
                        self.components[index].paint = Some(ComponentPaint {
                            visible: boolean(object, property_ids::SHAPEPAINT_ISVISIBLE)?,
                            value: Paint::Stroke {
                                width: float(object, property_ids::THICKNESS)?,
                                brush: brushes[index].take().unwrap_or_else(|| Brush::Solid(
                                    core_color_default(property_ids::SOLIDCOLOR_COLORVALUE))),
                                cap: stroke_cap (uint(object, property_ids::CAP)?),
                                join: stroke_join(uint(object, property_ids::JOIN)?),
                                trfm_scale:
                                    boolean(object, property_ids::TRANSFORMAFFECTSSTROKE)?,
                                effects: paint_effects.into(),
                            },
                        });
                    }
                _ => {}
            }
        }   Ok(())
    }
}

fn fill_rule(value: u32) -> FillRule { match value {
    1 => FillRule::EvenOdd, 2 => FillRule::Clockwise, _ => FillRule::NonZero,
} }

pub(super) fn set_paint_value(components: &mut [Component],
    targets: &[Option<EffectTarget>], component: u32, prop_id: u32, value: f32) {
    if prop_id == property_ids::THICKNESS {
        if let Some(ComponentPaint {
            value: Paint::Stroke { width, .. }, ..
        }) = &mut components[component as usize].paint {
            *width = value;
        }   return
    }
    let Some(target) = targets[component as usize] else { return };
    let (paint, effect) = match target {
        EffectTarget::Effect { paint, effect } |
        EffectTarget::DashSegment { paint, effect, .. } => (paint, effect),
    };
    let Some(Paint::Fill { effects, .. } | Paint::Stroke { effects, .. }) =
        components[paint as usize].paint.as_mut()
            .map(|paint| &mut paint.value) else { return };
    let effect = effect as usize;
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
            EffectTarget::DashSegment { segment, .. },
            property_ids::DASH_LENGTH) =>
            segments.get(segment as usize).is_some_and(|segment| segment.len != value),
        _ => false,
    };
    if !changed { return }
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
    }
}

pub(super) fn set_effect_bool(components: &mut [Component],
    targets: &[Option<EffectTarget>], component: u32, prop_id: u32, value: bool) -> bool {
    let Some(target) = targets[component as usize] else { return false };
    let (paint, effect) = match target {
        EffectTarget::Effect { paint, effect } |
        EffectTarget::DashSegment { paint, effect, .. } => (paint, effect),
    };
    let Some(Paint::Fill { effects, .. } | Paint::Stroke { effects, .. }) =
        components[paint as usize].paint.as_mut().map(|paint| &mut paint.value)
            else { return false };
    let effect = effect as usize;
    let changed = match (effects.get(effect), target, prop_id) {
        (Some(PathEffect::Dash { relative, .. }), EffectTarget::Effect { .. },
            property_ids::OFFSETISPERCENTAGE) => *relative != value,
        (Some(PathEffect::Dash { segments, .. }),
            EffectTarget::DashSegment { segment, .. },
            property_ids::LENGTHISPERCENTAGE) =>
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
        &mut components[component as usize].paint else { return false };
    *trfm_scale = value;   true
}

pub(super) fn set_paint_uint(components: &mut [Component],
    targets: &[Option<EffectTarget>], component: u32,
    prop_id: u32, value: u32) -> bool {
    if let Some(paint) = &mut components[component as usize].paint {
        match (&mut paint.value, prop_id) {
            (Paint::Fill { rule, .. }, property_ids::FILL_FILLRULE) =>
                *rule = fill_rule(value),
            (Paint::Stroke { cap, .. }, property_ids::CAP) =>
                *cap = stroke_cap(value),
            (Paint::Stroke { join, .. }, property_ids::JOIN) =>
                *join = stroke_join(value),
            _ => return false,
        }   return true
    }
    if prop_id != property_ids::TRIMPATH_MODEVALUE { return false }
    let Some(EffectTarget::Effect { paint, effect }) = targets[component as usize]
        else { return false };
    let Some(Paint::Fill { effects, .. } | Paint::Stroke { effects, .. }) =
        components[paint as usize].paint.as_mut().map(|paint| &mut paint.value)
            else { return false };
    let mode = if value == 1 { TrimMode::Sequential } else { TrimMode::Synchronized };
    let effect = effect as usize;
    let Some(PathEffect::Trim { mode: current, .. }) =
        effects.get(effect) else { return false };
    if *current != mode {
        let Some(PathEffect::Trim { mode: current, .. }) =
            std::sync::Arc::make_mut(effects).get_mut(effect) else { return false };
        *current = mode;
    }   true
}

fn stroke_cap(value: u32) -> StrokeCap { match value {
    1 => StrokeCap::Round, 2 => StrokeCap::Square, _ => StrokeCap::Butt,
} }

fn stroke_join(value: u32) -> StrokeJoin { match value {
    1 => StrokeJoin::Round, 2 => StrokeJoin::Bevel, _ => StrokeJoin::Miter,
} }
