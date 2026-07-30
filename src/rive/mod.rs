//! Rive binary decoding and the backend-neutral retained 2D runtime.

mod animation;
pub mod decode;
pub mod display_list;
#[cfg(feature = "b2d")]
pub mod render_b2d;
pub mod render_nvg;
#[cfg(feature = "rive-rs")]
pub mod rscpp_nvg;
mod path;
pub mod runtime;

use kurbo::{Affine, BezPath, Shape as _};
use display_list::{DisplayList, Geometry, PathCommand, PathEffect, Shape, TrimMode};
use crate::core::pathm::MeasuredPath;

const PATH_TOLERANCE: f64 = 1e-3;

/// Backend contract for consuming immutable Rive frame snapshots.
pub trait RenderContext { type Error; type Cache: Default;
    fn render_animation(&mut self, list: &DisplayList,
        cache: &mut Self::Cache) -> Result<(), Self::Error>;
}

/// Canonical work path used only when a backend needs geometry operations.
#[derive(Debug)] struct RenderPath { path: BezPath, hole: bool }

/// Normalize all geometry and bake each shape transform before Trim/Dash processing.
fn shape_paths(shapes: &[Shape]) -> Vec<RenderPath> {
    shapes.iter().map(|shape| {
        let mut path = BezPath::new();
        match &shape.geom {
            Geometry::Path(source) => for command in source.cmd.iter() { match command {
                PathCommand::MoveTo(to) => path.move_to((to.x as f64, to.y as f64)),
                PathCommand::LineTo(to) => path.line_to((to.x as f64, to.y as f64)),
                PathCommand::CubicTo { ctrl1, ctrl2, to } => path.curve_to(
                    (ctrl1.x as f64, ctrl1.y as f64),
                    (ctrl2.x as f64, ctrl2.y as f64),   (to.x as f64, to.y as f64)),
                PathCommand::Close => path.close_path(),
            }},
            Geometry::Ellipse(rect) => {
                let ellipse = kurbo::Ellipse::new(
                    ((rect.x + rect.w * 0.5) as f64, (rect.y + rect.h * 0.5) as f64),
                    (rect.w.abs() as f64 * 0.5, rect.h.abs() as f64 * 0.5), 0.0);
                path.extend(ellipse.to_path(0.1));
            }
            Geometry::RoundedRect { rect, radii } => {
                let limit = rect.w.abs().min(rect.h.abs()) * 0.5;
                let radii = kurbo::RoundedRectRadii::new(
                    radii.tl.clamp(0.0, limit) as _, radii.tr.clamp(0.0, limit) as _,
                    radii.br.clamp(0.0, limit) as _, radii.bl.clamp(0.0, limit) as _);
                path.extend(kurbo::RoundedRect::new(rect.x as _, rect.y as _,
                    (rect.x + rect.w) as _, (rect.y + rect.h) as _, radii).to_path(0.1));
            }
        }
        path.apply_affine(Affine::new([shape.trfm.xx as _, shape.trfm.yx as _,
            shape.trfm.xy as _, shape.trfm.yy as _, shape.trfm.tx as _, shape.trfm.ty as _]));
        RenderPath { path, hole: shape.is_hole }
    }).collect()
}

/// Apply ordered Rive path effects in a backend-independent representation.
fn apply_effects(mut paths: Vec<RenderPath>, effects: &[PathEffect]) -> Vec<RenderPath> {
    for effect in effects { match effect {
        PathEffect::Trim { start, end, offset, mode } => {
            if 1.0 <= (*end - *start).abs() { continue }
            let trim = (*end - *start).rem_euclid(1.0);
            if matches!(mode, TrimMode::Sequential) {
                // Sequential trim measures the painted contour stream as one path.
                let mut path = BezPath::new();
                for entry in paths { path.extend(entry.path) }
                let measured = MeasuredPath::new(path, PATH_TOLERANCE);
                path = trim_measured(&measured, *start + *offset, trim);
                paths = vec![RenderPath { path, hole: false }];
            } else {
                for entry in &mut paths {
                    let measured = MeasuredPath::new(
                        core::mem::take(&mut entry.path), PATH_TOLERANCE);
                    entry.path = trim_measured(&measured, *start + *offset, trim);
                }
            }
        }
        PathEffect::Dash { offset, relative, segments } => for entry in &mut paths {
            let measured = MeasuredPath::new(
                core::mem::take(&mut entry.path), PATH_TOLERANCE);
            // Relative dash values are fractions of the transformed contour length.
            let length = if *relative || segments.iter().any(|segment| segment.relative) {
                measured.length
            } else { 0.0 };
            let mut pattern: Vec<_> = segments.iter().map(|segment|
                if segment.relative { segment.len * length as f32
                } else { segment.len }.max(0.0) as f64).collect();
            if pattern.len() % 2 == 1 { pattern.extend_from_within(..) }
            let offset = if *relative { *offset as f64 * length } else { *offset as f64 };
            entry.path = if pattern.iter().any(|&value| 0.0 < value) {
                measured.dash(offset, &pattern)
            } else { BezPath::new() };
        },
    }}  paths
}

fn trim_measured(measured: &MeasuredPath, start: f32, trim: f32) -> BezPath {
    let start = start.rem_euclid(1.) as f64;
    let end = start + trim as f64;
    if  end <= 1. { measured.trim_ranges(&[(start, end)]) } else {
                    measured.trim_ranges(&[(start, 1.), (0., end - 1.)])
    }
}

#[cfg(test)] mod tests { use super::*;
    use kurbo::ParamCurveArclen as _;
    use display_list::DashSegment;
    use std::sync::Arc;

    #[test] fn trim_then_relative_dash_uses_the_trimmed_path_metrics() {
        let mut path = BezPath::new();
        path.move_to((0., 0.)); path.line_to((100., 0.));
        let effects = [
            PathEffect::Trim {
                start: 0., end: 0.5, offset: 0., mode: TrimMode::Synchronized,
            },
            PathEffect::Dash {
                segments: Arc::from([
                    DashSegment { len: 0.1, relative: true },
                    DashSegment { len: 0.1, relative: true },
                ]), offset: 0., relative: false,
            },
        ];
        let result = apply_effects(vec![RenderPath { path, hole: false }], &effects);
        let length = result[0].path.segments()
            .map(|segment| segment.arclen(PATH_TOLERANCE)).sum::<f64>();
        assert!((length - 25.).abs() < PATH_TOLERANCE);
    }
}
