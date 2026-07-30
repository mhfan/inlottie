//! Blend2D adapter for backend-neutral Rive display lists.

use std::collections::HashMap;
use intvg::blend2d::{BLCompOp, BLContext, BLEllipse, BLErr, BLFillRule, BLPath,
    BLGeometryDirection, BLGradient, BLImage, BLLinearGradientValues, BLMatrix2D,
    BLRadialGradientValues, BLRoundRect, BLRgba32, BLStrokeCap, BLStrokeJoin
};
use super::{RenderContext, RenderPath, apply_effects, shape_paths,
    display_list::{Brush, Clip, DisplayList, DrawItem, FillRule, Geometry, Shape,
        Paint as RivePaint, StrokeCap, StrokeJoin, GradientStop},
};

impl RenderContext for BLContext {
    fn render_animation(&mut self, list: &DisplayList,
        cache: &mut Self::Cache) -> Result<(), Self::Error> {
        render(self, list, cache)
    }
    type Cache = ImageCache;
    type Error = BLErr;
}

#[derive(Default)] pub struct ImageCache(HashMap<u32, BLImage>);

fn render(blctx: &mut BLContext, list: &DisplayList,
    assets: &mut ImageCache) -> Result<(), BLErr> {
    // Isolate caller state while retaining its viewport transform around Rive world space.
    blctx.save()?;
    blctx.set_global_alpha(1.0);
    blctx.set_stroke_alpha(1.0); blctx.set_fill_alpha(1.0);
    blctx.set_comp_op(BLCompOp::BL_COMP_OP_SRC_OVER);
    // The cache retains decoded assets across frames.
    let result = render_range(blctx, list, 0, &mut assets.0);
    let restore = blctx.restore();
    let flush   = blctx.flush();
    result.and(restore).and(flush)
}

fn render_range(blctx: &mut BLContext, items: &[DrawItem],
    depth: usize, assets: &mut HashMap<u32, BLImage>) -> Result<(), BLErr> {
    let mut start = 0;
    while   start < items.len() {
        let Some(clip) = items[start].clips.get(depth) else {
            draw_item(blctx, &items[start], assets)?;
            start += 1;     continue
        };
        let mut end = start + 1;
        // A shared clip-prefix is rendered once for the whole contiguous run.
        while   end < items.len() && items[end].clips.get(depth).is_some_and(|next|
                (next.scope, next.obj_idx) == (clip.scope, clip.obj_idx)) &&
            items[start].clips.iter().zip(items[end].clips.iter()).take(depth)
                .all(|(left, right)| (left.scope,  left.obj_idx) ==
                                    (right.scope, right.obj_idx)) { end += 1; }
        render_clip(blctx, &items[start..end], clip, depth, assets)?; start = end;
    }   Ok(())
}

fn render_clip(blctx: &mut BLContext, items: &[DrawItem], clip: &Clip, depth: usize,
    assets: &mut HashMap<u32, BLImage>) -> Result<(), BLErr> {
    let path = b2d_shapes(&clip.shapes, clip.rule)?;
    blctx.set_global_alpha(1.0);
    blctx.set_fill_rule(b2d_rule(clip.rule));
    // clip_to_path uses tight offscreen bounds and preserves its layer offset as meta transform.
    blctx.clip_to_path(&path, |content| render_range(content, items, depth + 1, assets))
}

fn draw_item(blctx: &mut BLContext, item: &DrawItem,
    assets: &mut HashMap<u32, BLImage>) -> Result<(), BLErr> {
    if let Some(image) = &item.image {
        return draw_image(blctx, image, item.opacity, assets)
    }
    let Some(style) = &item.paint else { return Ok(()) };
    let (brush, effects, rule) = match style {
        RivePaint::Fill   { brush, rule, effects } => (brush, effects.as_ref(), *rule),
        RivePaint::Stroke { brush,       effects, .. } =>
            (brush, effects.as_ref(), FillRule::NonZero),
    };
    // Native geometry avoids the intermediate BezPath unless effects require kurbo.
    let path = if effects.is_empty() { b2d_shapes(&item.shapes, rule)? } else {
        let paths = apply_effects(shape_paths(&item.shapes), effects);
        if  paths.is_empty() { return Ok(()) }
        b2d_path(&paths, rule)
    };

    let (paint, brush_opacity) = b2d_paint(brush)?;
    blctx.set_global_alpha((item.opacity * brush_opacity).clamp(0.0, 1.0) as _);

    match style {
        RivePaint::Fill { rule, .. } => {
            blctx.set_fill_rule(b2d_rule(*rule));
            paint.fill(blctx, &path)
        }
        RivePaint::Stroke { width, cap, join, trfm_scale, .. } => {
            blctx.set_stroke_width(if *trfm_scale {
                let shapes = &item.shapes;
                *width * if shapes.is_empty() { 1.0 } else {
                    shapes.iter().map(|shape|
                        (shape.trfm.xx.hypot(shape.trfm.yx) +
                         shape.trfm.xy.hypot(shape.trfm.yy)) * 0.5).sum::<f32>() /
                         shapes.len() as f32
                }
            } else { *width } as _);
            // Rive hard-codes the miter limit to 4 in every runtime.
            blctx.set_stroke_miter_limit(4.0);
            blctx.set_stroke_caps(match cap {
                StrokeCap::Butt   => BLStrokeCap::BL_STROKE_CAP_BUTT,
                StrokeCap::Round  => BLStrokeCap::BL_STROKE_CAP_ROUND,
                StrokeCap::Square => BLStrokeCap::BL_STROKE_CAP_SQUARE,
            });
            blctx.set_stroke_join(match join {
                StrokeJoin::Miter => BLStrokeJoin::BL_STROKE_JOIN_MITER_CLIP,
                StrokeJoin::Round => BLStrokeJoin::BL_STROKE_JOIN_ROUND,
                StrokeJoin::Bevel => BLStrokeJoin::BL_STROKE_JOIN_BEVEL,
            });
            blctx.set_stroke_dash(0.0, &[])?;
            paint.stroke(blctx, &path)
        }
    }
}

fn draw_image(blctx: &mut BLContext, image: &super::display_list::Image,
    opacity: f32, assets: &mut HashMap<u32, BLImage>) -> Result<(), BLErr> {
    if let std::collections::hash_map::Entry::Vacant(entry) =
        assets.entry(image.asset_id) {
        entry.insert(BLImage::read_from_data(&image.data)?);
    }
    let image_data = &assets[&image.asset_id];
    let (width, height) = (image_data.width(), image_data.height());
    let (ox, oy) = (width as f32 * image.origin.x, height as f32 * image.origin.y);
    let trfm = BLMatrix2D::new([
        image.trfm.xx as _, image.trfm.yx as _, image.trfm.xy as _, image.trfm.yy as _,
        (image.trfm.tx - image.trfm.xx * ox - image.trfm.xy * oy) as _,
        (image.trfm.ty - image.trfm.yx * ox - image.trfm.yy * oy) as _,
    ]);
    let previous = blctx.user_transform();
    blctx.apply_transform(&trfm);
    blctx.set_global_alpha(opacity.clamp(0.0, 1.0) as _);
    let result = blctx.blit_image_d((0.0, 0.0).into(),
        image_data, &(0, 0, width, height).into());
    blctx.reset_transform(Some(&previous)); result
}

enum B2DPaint { Solid(BLRgba32), Gradient(BLGradient) }
impl B2DPaint {
    fn fill(&self, blctx: &mut BLContext, path: &BLPath) -> Result<(), BLErr> {
        match self {
            Self::Solid(color)   => blctx.fill_geometry_rgba32(path, *color),
            Self::Gradient(grad) => blctx.fill_geometry_ext(path, grad),
        }
    }

    fn stroke(&self, blctx: &mut BLContext, path: &BLPath) -> Result<(), BLErr> {
        match self {
            Self::Solid(color)   => blctx.stroke_geometry_rgba32(path, *color),
            Self::Gradient(grad) => blctx.stroke_geometry_ext(path, grad),
        }
    }
}

fn b2d_paint(brush: &Brush) -> Result<(B2DPaint, f32), BLErr> {
    let color = |value|    BLRgba32::new((value >> 16) as u8,
        (value >> 8) as u8, value as u8, (value >> 24) as u8);
    let gradient = |mut gradient: BLGradient, stops: &[GradientStop]| {
        for stop in stops { gradient.add_stop(stop.pos, color(stop.color))? }
        Ok(gradient)
    };

    Ok(match brush {
        Brush::Solid(value) => (B2DPaint::Solid(color(*value)), 1.0),
        Brush::LinearGradient { start, end, trfm, opacity, stops } => {
            let (start, end) = (trfm.transform_point(*start), trfm.transform_point(*end));
            let values = BLLinearGradientValues::new(
                (start.x, start.y).into(), (end.x, end.y).into());
            (B2DPaint::Gradient(gradient(BLGradient::new(&values)?, stops)?), *opacity)
        }
        Brush::RadialGradient { center, radius, trfm, opacity, stops } => {
            let center = trfm.transform_point(*center);
            let scale = (trfm.xx.hypot(trfm.yx) + trfm.xy.hypot(trfm.yy)) * 0.5;
            let (center, radius) = ((center.x, center.y).into(), (radius * scale) as _);
            let values = BLRadialGradientValues::new(center, center, (radius, radius));
            (B2DPaint::Gradient(gradient(BLGradient::new(&values)?, stops)?), *opacity)
        }
    })
}

fn b2d_shapes(shapes: &[Shape], rule: FillRule) -> Result<BLPath, BLErr> {
    let mut output = BLPath::new();     use BLGeometryDirection::*;
    for shape in shapes {
        let trfm = BLMatrix2D::new([
            shape.trfm.xx as _, shape.trfm.yx as _, shape.trfm.xy as _,
            shape.trfm.yy as _, shape.trfm.tx as _, shape.trfm.ty as _]);
        // Compensate reflection so separate solid contours never cancel under NonZero.
        let reflected = shape.trfm.xx * shape.trfm.yy < shape.trfm.xy * shape.trfm.yx;
        let hole = shape.is_hole || rule == FillRule::Clockwise && reflected;
        let direction = Some(if hole ^ reflected { BL_GEOMETRY_DIRECTION_CCW
                                          } else { BL_GEOMETRY_DIRECTION_CW });

        match &shape.geom {
            Geometry::Ellipse(rect) => {
                let center = ((rect.x + rect.w * 0.5), (rect.y + rect.h * 0.5)).into();
                let ellipse = BLEllipse::new(center,
                    (rect.w.abs() as f64 * 0.5, rect.h.abs() as f64 * 0.5));
                output.add_geometry(&ellipse, &trfm, direction)?;
            }
            Geometry::RoundedRect { rect, radii } => {
                let limit = rect.w.abs().min(rect.h.abs()) * 0.5;
                let values = [radii.tl, radii.tr, radii.br, radii.bl]
                    .map(|radius| radius.clamp(0.0, limit));
                if values.iter().all(|&radius| radius == values[0]) {
                    let rect = (rect.x.min(rect.x + rect.w),
                                rect.y.min(rect.y + rect.h),
                                rect.w.abs(), rect.h.abs()).into();
                    output.add_geometry(
                        &BLRoundRect::new(&rect, values[0] as _), &trfm, direction)?;
                } else {
                    // Blend2D's native round-rect supports only one radius.
                    output.add_path(
                        &b2d_path(&shape_paths(core::slice::from_ref(shape)), rule))?;
                }
            }
            Geometry::Path(_) => output.add_path(
                        &b2d_path(&shape_paths(core::slice::from_ref(shape)), rule))?,
        }
    }   Ok(output)
}

fn b2d_path(paths: &[RenderPath], rule: FillRule) -> BLPath {
    let mut output = BLPath::new();
    for entry in paths {
        let area = entry.path.area();
        let hole = entry.hole || rule == FillRule::Clockwise && area < 0.0;
        // Blend2D has one fill rule for the whole path, so encode holes by winding.
        let reversed = (area != 0.0 && hole == (0.0 < area))
            .then(|| entry.path.reverse_subpaths());
        let path = reversed.as_ref().unwrap_or(&entry.path);

        for element in path.iter() {
            match element {
                MoveTo(p) => output.move_to((p.x, p.y).into()),
                LineTo(p) => output.line_to((p.x, p.y).into()),
                QuadTo(c, p) => output.quad_to((c.x, c.y).into(), (p.x, p.y).into()),
                CurveTo(a, b, p) => output.cubic_to(
                    (a.x, a.y).into(), (b.x, b.y).into(), (p.x, p.y).into()),
                ClosePath => output.close(),
            }
        }   use kurbo::{Shape, PathEl::*};
    }   output
}

fn b2d_rule(rule: FillRule) -> BLFillRule { match rule {
    FillRule::NonZero | FillRule::Clockwise => BLFillRule::BL_FILL_RULE_NON_ZERO,
    FillRule::EvenOdd => BLFillRule::BL_FILL_RULE_EVEN_ODD,
} }
