
//! Shape content assembly: geometry, brushes, paints, and path effects.

use std::mem;

use super::{Result, Runtime, RuntimeError, StrokeCap, StrokeJoin, TrimMode,
    Brush, Component, DashSegment, FillRule, Geometry, GradientStop, Paint, PathEffect, Point,
    boolean, core_color_default, float, object_ids, property_ids, uint,
};
use crate::rive::path::{build_path, vertex};

impl Runtime {
    pub(super) fn build_shape_content(&mut self) -> Result<()> {
        let mut vertices = vec![Vec::new(); self.components.len()];
        let mut stops = vec![Vec::new(); self.components.len()];
        let mut brushes = vec![None; self.components.len()];
        let mut dash_segments = vec![Vec::new(); self.components.len()];
        let mut effects = vec![Vec::new(); self.components.len()];

        // Pass 1 gathers leaf data under its owning path, paint, gradient, or dash component.
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
                        pos: float(object, property_ids::POSITION)?.clamp(0.0, 1.0),
                        color: object.color(property_ids::GRADIENTSTOP_COLORVALUE)?
                            .unwrap_or_else(||
                                core_color_default(property_ids::GRADIENTSTOP_COLORVALUE)),
                    });
                }
            } else if object.type_id.0 == object_ids::DASH {
                if let Some(parent) = component.parent {
                    dash_segments[parent as usize].push(DashSegment {
                        len: float(object, property_ids::DASH_LENGTH)?,
                        relative: boolean(object, property_ids::LENGTHISPERCENTAGE)?,
                    });
                }
            }
        }

        // Pass 2 resolves brushes and effects after all of their child objects are available.
        for index in 0..self.components.len() {
            let object = &self.file.ocoll[self.components[index].obj_idx as usize];
            match object.type_id.0 {
                object_ids::LINEAR_GRADIENT | object_ids::RADIAL_GRADIENT => {
                    stops[index].sort_by(|left, right|
                        left.pos.total_cmp(&right.pos));
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
                            trfm: transform, opacity, stops: gradient_stops }
                    } else { Brush::LinearGradient {
                            start, end, trfm: transform, opacity, stops: gradient_stops
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
                            relative:
                                boolean(object, property_ids::OFFSETISPERCENTAGE)?,
                            segments: mem::take(&mut dash_segments[index]).into(),
                        }));
                    }
                }
                _ => {}
            }
        }

        // Pass 3 materializes geometry and paints. Keeping these passes together avoids
        // independent full-scene traversals for paths, gradients, and strokes.
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
                        // Retain the source component -> effect slot mapping for animation.
                        let paint_effects: Vec<_> = paint_effects.into_iter().enumerate()
                            .map(|(effect, (source, value))| {
                                self.effect_targets[source as usize] =
                                    Some((index as u32, effect as u32));    value
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
                                    Some((index as u32, effect as u32));    value
                            }).collect();
                        self.components[index].paint = Some(Paint::Stroke {
                            width: float(object, property_ids::THICKNESS)?,
                            brush: brushes[index].take().unwrap_or_else(|| Brush::Solid(
                                core_color_default(property_ids::SOLIDCOLOR_COLORVALUE))),
                             cap: stroke_cap (uint(object, property_ids::CAP)?),
                            join: stroke_join(uint(object, property_ids::JOIN)?),
                            trfm_scale:
                                boolean(object, property_ids::TRANSFORMAFFECTSSTROKE)?,
                            effects: paint_effects.into(),
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
    effects: &[Option<(u32, u32)>], component: u32, prop_id: u32, value: f32) {
    if prop_id == property_ids::THICKNESS {
        if let Some(Paint::Stroke { width, .. }) = &mut components[component as usize].paint {
            *width = value;
        }   return
    }
    let Some((paint, effect)) = effects[component as usize] else { return };
    let Some(Paint::Fill { effects, .. } | Paint::Stroke { effects, .. }) =
        &mut components[paint as usize].paint else { return };
    let Some(PathEffect::Trim { start, end, offset, .. }) =
        std::sync::Arc::make_mut(effects).get_mut(effect as usize) else { return };
    match prop_id {
        property_ids::TRIMPATH_START => *start = value,
        property_ids::TRIMPATH_END => *end = value,
        property_ids::TRIMPATH_OFFSET => *offset = value,
        _ => {}
    }
}

fn stroke_cap(value: u32) -> StrokeCap { match value {
    1 => StrokeCap::Round, 2 => StrokeCap::Square, _ => StrokeCap::Butt,
} }

fn stroke_join(value: u32) -> StrokeJoin { match value {
    1 => StrokeJoin::Round, 2 => StrokeJoin::Bevel, _ => StrokeJoin::Miter,
} }
