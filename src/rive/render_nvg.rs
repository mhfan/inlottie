//! femtovg adapter for backend-neutral Rive display lists.

use femtovg::{Canvas, Color, CompositeOperation, ErrorKind, FillRule as VgFillRule,
    ImageFlags, LineCap, LineJoin, Paint, Path, PixelFormat, RenderTarget, Solidity,
    renderer::SurfacelessRenderer
};
use super::{RenderContext, RenderPath, apply_effects, shape_paths,
    display_list::{Brush, Clip, DisplayList, DrawItem, FillRule,
        Paint as RivePaint, StrokeCap, StrokeJoin},
};

impl<T: SurfacelessRenderer> RenderContext for Canvas<T> {
    fn render_animation(&mut self, list: &DisplayList) -> Result<(), Self::Error> {
        FemtovgRenderer::new(self).render(list)
    }   type Error = ErrorKind;
}

struct FemtovgRenderer<'a, T: SurfacelessRenderer> {
    // femtovg executes commands at flush, so temporary images must outlive recursion.
    canvas: &'a mut Canvas<T>, images: Vec<femtovg::ImageId>,
}

impl<T: SurfacelessRenderer> FemtovgRenderer<'_, T> {
    fn new(canvas: &mut Canvas<T>) -> FemtovgRenderer<'_, T> {
        FemtovgRenderer { canvas, images: Vec::new() }
    }

    fn render(mut self, list: &DisplayList) -> Result<(), ErrorKind> {
        // Isolate caller state while retaining its viewport transform around Rive world space.
        let canvas = &mut *self.canvas; canvas.save();
        canvas.set_global_alpha(1.0);
        canvas.global_composite_operation(CompositeOperation::SourceOver);
        let result = self.render_range(list, 0, RenderTarget::Screen);

        let canvas = &mut *self.canvas;
        canvas.set_render_target(RenderTarget::Screen);
        canvas.restore();   canvas.flush();
        // Deletion is safe only after queued commands have consumed the images.
        for image in self.images { canvas.delete_image(image) }     result
    }

    fn render_range(&mut self, items: &[DrawItem],
        depth: usize, target: RenderTarget) -> Result<(), ErrorKind> {
        let mut start = 0;
        while   start < items.len() {
            let Some(clip) = items[start].clips.get(depth) else {
                self.canvas.set_render_target(target);
                self.draw_item(&items[start]); start += 1; continue
            };
            let mut end = start + 1;
            // Render one contiguous run sharing the same clip-prefix only once.
            while   end < items.len() && items[end].clips.get(depth)
                    .is_some_and(|next| next.obj_idx == clip.obj_idx) &&
                items[start].clips.iter().zip(items[end].clips.iter()).take(depth)
                    .all(|(left, right)| left.obj_idx == right.obj_idx) {       end += 1;
            }
            self.render_clip(&items[start..end], clip, depth, target)?; start = end;
        }   Ok(())
    }

    fn render_clip(&mut self, items: &[DrawItem], clip: &Clip,
        depth: usize, target: RenderTarget) -> Result<(), ErrorKind> {
        // Draw content and mask separately, then intersect alpha with DestinationIn.
        let content = self.new_target()?;
        self.render_range(items, depth + 1, RenderTarget::Image(content))?;
        let    mask = self.new_target()?;

        let canvas = &mut *self.canvas;
        let path = vg_path(&shape_paths(&clip.shapes), clip.rule);
        let mut paint = Paint::color(Color::white());
        paint.set_fill_rule(vg_rule(clip.rule));
        canvas.fill_path(&path, &paint);

        let (width, height) = (canvas.width(), canvas.height());
        let image_paint = |image| Paint::image(image, 0.0, 0.0,
             width as _, height as _, 0.0, 1.0);
        let mut viewport = Path::new();
        viewport.rect(0.0, 0.0, width as _, height as _);

        // Offscreen images cover the physical viewport; restore artboard fitting afterwards.
        let trfm = canvas.transform();  canvas.reset_transform();
        canvas.set_render_target(RenderTarget::Image(content));
        canvas.global_composite_operation(CompositeOperation::DestinationIn);
        canvas.fill_path(&viewport, &image_paint(mask));

        canvas.set_render_target(target);
        canvas.global_composite_operation(CompositeOperation::SourceOver);
        canvas.fill_path(&viewport, &image_paint(content));
        canvas.set_transform(&trfm);     Ok(())
    }

    fn draw_item(&mut self, item: &DrawItem) {
        let Some(style) = &item.paint else { return };
        let (brush, effects, rule) = match style {
            RivePaint::Fill   { brush, rule, effects } => (brush, effects.as_ref(), *rule),
            RivePaint::Stroke { brush,       effects, .. } =>
                (brush, effects.as_ref(), FillRule::NonZero),
        };

        let paths = apply_effects(shape_paths(&item.shapes), effects);
        if  paths.is_empty() { return }
        let path = vg_path(&paths, rule);
        let canvas = &mut *self.canvas;
        let (mut paint, brush_opacity) = vg_paint(brush);
        canvas.set_global_alpha((item.opacity * brush_opacity).clamp(0.0, 1.0));

        match style {
            RivePaint::Fill { rule, .. } => {
                paint.set_fill_rule(vg_rule(*rule));
                canvas.fill_path(&path, &paint);
            }
            RivePaint::Stroke { width, cap, join, trfm_scale, .. } => {
                paint.set_line_width(if *trfm_scale {
                    let shapes = &item.shapes;
                    *width * if shapes.is_empty() { 1.0 } else {
                        shapes.iter().map(|shape|
                            (shape.trfm.xx.hypot(shape.trfm.yx) +
                             shape.trfm.xy.hypot(shape.trfm.yy)) * 0.5).sum::<f32>() /
                             shapes.len() as f32
                    }
                } else { *width });
                // Rive hard-codes the miter limit to 4 in every runtime.
                paint.set_miter_limit(4.0);
                paint.set_line_cap(match cap {
                    StrokeCap::Butt => LineCap::Butt, StrokeCap::Round => LineCap::Round,
                    StrokeCap::Square => LineCap::Square,
                });
                paint.set_line_join(match join {
                    StrokeJoin::Miter => LineJoin::Miter, StrokeJoin::Round => LineJoin::Round,
                    StrokeJoin::Bevel => LineJoin::Bevel,
                });
                canvas.stroke_path(&path, &paint);
            }
        }
    }

    fn new_target(&mut self) -> Result<femtovg::ImageId, ErrorKind> {
        let (width, height) = (self.canvas.width(), self.canvas.height());
        let image = self.canvas.create_image_empty(width as _, height as _,
            PixelFormat::Rgba8, ImageFlags::FLIP_Y)?;
        self.images.push(image); // to delete after flush

        let canvas = &mut *self.canvas;
        canvas.set_render_target(RenderTarget::Image(image));
        // Transparent SourceOver cannot clear existing pixels; Copy replaces them.
        canvas.global_composite_operation(CompositeOperation::Copy);
        canvas.clear_rect(0, 0, width, height, Color::rgbaf(0.0, 0.0, 0.0, 0.0));
        canvas.set_global_alpha(1.0);
        canvas.global_composite_operation(CompositeOperation::SourceOver);
        Ok(image)
    }
}

fn vg_paint(brush: &Brush) -> (Paint, f32) {
    let to_argb = |value| Color::rgba((value >> 16) as u8,
        (value >> 8) as u8, value as u8, (value >> 24) as u8);

    match brush {
        Brush::Solid(color) => (Paint::color(to_argb(*color)), 1.0),
        Brush::LinearGradient { start, end, trfm, opacity, stops } => {
            let (start, end) = (trfm.transform_point(*start), trfm.transform_point(*end));
            (Paint::linear_gradient_stops(start.x, start.y, end.x, end.y,
                stops.iter().map(|stop| (stop.pos, to_argb(stop.color)))), *opacity)
        }
        Brush::RadialGradient { center, radius, trfm, opacity, stops } => {
            let center = trfm.transform_point(*center);
            let scale = (trfm.xx.hypot(trfm.yx) + trfm.xy.hypot(trfm.yy)) * 0.5;
            (Paint::radial_gradient_stops(center.x, center.y, 0.0, radius * scale,
                stops.iter().map(|stop| (stop.pos, to_argb(stop.color)))), *opacity)
        }
    }
}

fn vg_rule(rule: FillRule) -> VgFillRule { match rule {
    FillRule::NonZero | FillRule::Clockwise => VgFillRule::NonZero,
    FillRule::EvenOdd => VgFillRule::EvenOdd,
} }

fn vg_path(paths: &[RenderPath], rule: FillRule) -> Path {
    let mut output = Path::new();
    for entry in paths {    use kurbo::{Shape, PathEl::*};
        for element in entry.path.iter() { match element {
            MoveTo(p) => output.move_to(p.x as _, p.y as _),
            LineTo(p) => output.line_to(p.x as _, p.y as _),
            QuadTo(c, p) => output.quad_to(c.x as _, c.y as _, p.x as _, p.y as _),
            CurveTo(a, b, p) => output.bezier_to(
                a.x as _, a.y as _, b.x as _, b.y as _, p.x as _, p.y as _),
            ClosePath => output.close(),
        }}
        // femtovg stores solidity per contour, unlike its global fill rule.
        let hole = entry.hole || rule == FillRule::Clockwise && entry.path.area() < 0.0;
        output.solidity(if hole { Solidity::Hole } else { Solidity::Solid });
    }   output
}
