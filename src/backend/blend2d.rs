/****************************************************************
 * $ID: blend2d.rs  	Thu 20 Nov 2025 16:50:16+0800           *
 *                                                              *
 * Maintainer: 范美辉 (MeiHui FAN) <mhfan@ustc.edu>              *
 * Copyright (c) 2025 M.H.Fan, All rights reserved.             *
 ****************************************************************/

use crate::core::{CompositeContext, helpers::{Vec2D, RGBA}, render::RenderContext,
    schema::{FillRule, LineJoin, LineCap, MatteMode, MaskMode, VisualLayer},
    style::{StyleConv, MatrixConv, TM2DwO, FSOpts},
    pathm::{PathBuilder, PathFactory, BezPath},
};
use intvg::blend2d::{BLPoint, BLPath, BLMatrix2D, BLContext, BLRgba32, BLErr, BLFormat,
    BLRadialGradientValues, B2DStyle, BLFillRule::*, BLStrokeJoin::*, BLStrokeCap::*,
    BLRectI, BLCompOp::*, BLSolidColor, BLGradient, BLLinearGradientValues, BLImage,
};

pub struct B2DTarget(Option<BLContext>);

impl CompositeContext for BLContext {
    type Offscreen = B2DTarget;
    type Image = BLImage;

    fn begin_offscreen(&mut self) -> Result<Self::Offscreen, Self::Error> {
        let size = self.get_target_size();
        let mut layer = BLContext::new(size.width() as _, size.height() as _,
            BLFormat::BL_FORMAT_PRGB32)?;   layer.clear_all()?;
        Ok(B2DTarget(Some(core::mem::replace(self, layer))))
    }

    fn abort_offscreen(&mut self, mut target: Self::Offscreen) {
        if let Some(parent) = target.0.take() { *self = parent; }
    }

    fn end_offscreen(&mut self, mut target: Self::Offscreen) ->
        Result<Self::Image, Self::Error> {
        let parent = target.0.take().expect("offscreen target is consumed once");
        core::mem::replace(self, parent).end()
    }

    fn apply_masks(&mut self, mut content: Self::Image, layer: &VisualLayer,
        transform: &TM2DwO<Self::TM2D>, frame: f32) ->
        Result<Self::Image, Self::Error> {
        let size = self.get_target_size();
        let area: BLRectI = (0, 0, size.width(), size.height()).into();

        let mut mask = BLContext::new(size.width() as _, size.height() as _,
            BLFormat::BL_FORMAT_A8)?;   mask.clear_all()?;
        let mut initialized = false;
        for item in &layer.masks {
            if matches!(item.mode, MaskMode::None) { continue }
            let mut part = BLContext::new(size.width() as _, size.height() as _,
                BLFormat::BL_FORMAT_A8)?;   part.clear_all()?;
            part.reset_transform(Some(&transform.0));
            let mut path: Self::VGPath = item.shape.to_path(frame);
            if let Some(expand) = &item.expand {
                path.offset_path(expand.get_value(frame), LineJoin::Round, 4.);
            }
            part.fill_geometry_rgba32(&path, BLRgba32::new(255, 255, 255, 255))?;
            let mut image = part.end()?;

            if item.inv {
                let mut inverse = BLContext::new(size.width() as _,
                    size.height() as _, BLFormat::BL_FORMAT_A8)?;
                inverse.fill_all_rgba32(BLRgba32::new(255, 255, 255, 255))?;
                inverse.set_comp_op(BL_COMP_OP_DST_OUT);
                inverse.blit_image_d(BLPoint::new(), &image, &area)?;
                image = inverse.end()?;
            }
            if !initialized && matches!(item.mode,
                MaskMode::Subtract | MaskMode::Intersect | MaskMode::Darken) {
                mask.fill_all_rgba32(BLRgba32::new(255, 255, 255, 255))?;
            }
            mask.set_global_alpha(item.opacity.as_ref().map_or(1.,
                |opacity| opacity.get_value(frame) / 100.) as _);
            mask.set_comp_op(match item.mode {
                MaskMode::Add        => BL_COMP_OP_SRC_OVER,
                MaskMode::Subtract   => BL_COMP_OP_DST_OUT,
                MaskMode::Intersect  => BL_COMP_OP_DST_IN,
                MaskMode::Lighten    => BL_COMP_OP_LIGHTEN,
                MaskMode::Darken     => BL_COMP_OP_DARKEN,
                MaskMode::Difference => BL_COMP_OP_XOR,
                MaskMode::None => unreachable!(),
            });
            mask.blit_image_d(BLPoint::new(), &image, &area)?;
            initialized = true;
        }
        if  initialized {
            let mask = mask.end()?;
            let mut masked = BLContext::from_image(content)?;
            masked.set_comp_op(BL_COMP_OP_DST_IN);
            masked.blit_image_d(BLPoint::new(), &mask, &area)?;
            content = masked.end()?;
        }   Ok(content)
    }

    fn apply_matte(&mut self, content: Self::Image, matte: Self::Image,
        mode: MatteMode) -> Result<Self::Image, Self::Error> {
        // Rec.709/sRGB luminance weights (Y = 0.2126 R + 0.7152 G + 0.0722 B)
        // in Blend2D PRGB32's little-endian BGRA memory order.
        const LUMA_BGR: [f32; 3] = [0.0722, 0.7152, 0.2126];

        if matches!(mode, MatteMode::Normal) { return Ok(content) }
        let area: BLRectI = (0, 0, matte.width(), matte.height()).into();
        let mut masked = BLContext::from_image(content)?;
        masked.set_comp_op(match mode {
            MatteMode::Alpha | MatteMode::Luma => BL_COMP_OP_DST_IN,
            MatteMode::InvertedAlpha | MatteMode::InvertedLuma => BL_COMP_OP_DST_OUT,
            MatteMode::Normal => unreachable!(),
        });
        if matches!(mode, MatteMode::Luma | MatteMode::InvertedLuma) {
            let stride = matte.stride() as usize;
            let mut alpha = vec![0; matte.width() as usize * matte.height() as usize];
            if let Some(pixels) = matte.pixels() {
                for (src, dst) in pixels.chunks(stride)
                    .zip(alpha.chunks_mut(matte.width() as usize)) {
                    for (bgra, value) in src.chunks_exact(4).zip(dst) {
                        *value = (LUMA_BGR[0] * bgra[0] as f32 +
                                  LUMA_BGR[1] * bgra[1] as f32 +
                                  LUMA_BGR[2] * bgra[2] as f32).round() as u8;
                    }
                }
                // SAFETY: `alpha` outlives the temporary image and synchronous blit.
                let mask = unsafe { Self::Image::from_buffer(matte.width(), matte.height(),
                    BLFormat::BL_FORMAT_A8, &mut alpha, matte.width())?
                };
                     masked.blit_image_d(BLPoint::new(), &mask,  &area)?;
            } else { masked.blit_image_d(BLPoint::new(), &matte, &area)?; }
        }     else { masked.blit_image_d(BLPoint::new(), &matte, &area)?; }
                     masked.end()
    }

    fn present(&mut self, content: Self::Image) -> Result<(), Self::Error> {
        let area: BLRectI = (0, 0, content.width(), content.height()).into();
        self.save()?;   self.set_comp_op(BL_COMP_OP_SRC_OVER);
        self.reset_transform(None);     self.set_global_alpha(1.);
        let result = self.blit_image_d(BLPoint::new(), &content, &area);
        result.and(self.restore())
    }
    fn discard(&mut self, _: Self::Image) {}
}

impl RenderContext for BLContext {
    type State = (Self::TM2D, f64);
    type TM2D = BLMatrix2D;
    type VGStyle = BLStyle;
    type VGPath  = BLPath;
    type Error = BLErr;

    fn get_size(&self) -> (u32, u32) {
        let sz = self.get_target_size();
        (sz.width() as _, sz.height() as _)
    }

    fn clear_rect_with(&mut self, x: u32, y: u32, w: u32, h: u32,
        color: RGBA) -> Result<(), Self::Error> {
        if color.a < u8::MAX {
            self.clear_rect_d(&(x, y, w, h).into())?;
        }
        if color.a != 0 {
            self.fill_rect_i_rgba32(&(x, y, w, h).into(), color.into())?;
        }   Ok(())
    }
    fn save_state(&mut self) -> Result<Self::State, Self::Error> {
        //self.save().expect("failed to save Blend2D state");
        Ok((self.user_transform(), self.get_global_alpha()))
    }
    fn restore_state(&mut self, (transform, alpha): Self::State) -> Result<(), Self::Error> {
        //self.restore().expect("failed to restore Blend2D state");
        self.reset_transform(Some(&transform)); self.set_global_alpha(alpha); Ok(())
    }
    fn apply_transform(&mut self, trfm: &Self::TM2D,
        opacity: Option<f32>) -> Result<(), Self::Error> {
        if let Some(opacity) = opacity { self.set_global_alpha(opacity as _) }
        self.reset_transform(Some(trfm));   Ok(())
    }

    fn fill_stroke(&mut self, path: &Self::VGPath, relative: Option<&Self::TM2D>,
        style: &(Self::VGStyle, FSOpts)) -> Result<(), Self::Error> {
        let transformed = relative.map(|transform| {
            let mut result = Self::VGPath::new();
            result.add_transformed_path(path, transform).map(|()| result)
        }).transpose()?;

        let path = transformed.as_ref().unwrap_or(path);
        match &style.1 {
            FSOpts::Fill(rule) => {
                self.set_fill_rule(match rule {
                    FillRule::NonZero => BL_FILL_RULE_NON_ZERO,
                    FillRule::EvenOdd => BL_FILL_RULE_EVEN_ODD,
                });

                self.set_fill_style(style.0.as_b2d_style());
                self.fill_geometry(path)?;
            }

            FSOpts::Stroke { width, limit,
                join, cap, dash } => {
                self.set_stroke_width(*width as _);
                self.set_stroke_miter_limit(*limit as _);

                self.set_stroke_join(match join {
                    LineJoin::Miter => BL_STROKE_JOIN_MITER_CLIP,
                    LineJoin::Round => BL_STROKE_JOIN_ROUND,
                    LineJoin::Bevel => BL_STROKE_JOIN_BEVEL,
                });
                self.set_stroke_caps(match cap {
                    LineCap::Butt   => BL_STROKE_CAP_BUTT,
                    LineCap::Round  => BL_STROKE_CAP_ROUND,
                    LineCap::Square => BL_STROKE_CAP_SQUARE,
                });

                if dash.1.is_empty() { self.set_stroke_dash(0., &[])?; } else {
                    self.set_stroke_dash(dash.0 as _,
                        &dash.1.iter().map(|&x| x as _).collect::<Vec<_>>())?;
                }

                self.set_stroke_style(style.0.as_b2d_style());
                self.stroke_geometry(path)?;
            }
        }   Ok(())
    }

    fn draw_image(&mut self, image: &[u8],
        width: f32, height: f32) -> Result<(), Self::Error> {
        let image = BLImage::read_from_data(image)?;
        let width  = if 0. < width  { width  } else { image.width()  as _ };
        let height = if 0. < height { height } else { image.height() as _ };
        let area: BLRectI = (0, 0, image.width(), image.height()).into();
        if width == image.width() as f32 && height == image.height() as f32 {
            self.blit_image_d(BLPoint::new(), &image, &area)
        } else {
            let mut scaled = BLImage::new(width.max(0.) as _,
                height.max(0.) as _, BLFormat::BL_FORMAT_PRGB32)?;
            scaled.scale(&image, width.max(0.) as _, height.max(0.) as _,
                intvg::blend2d::BLImageScaleFilter::BL_IMAGE_SCALE_FILTER_BILINEAR)?;
            let area: BLRectI = (0, 0, scaled.width(), scaled.height()).into();
            self.blit_image_d(BLPoint::new(), &scaled, &area)
        }
    }
}

impl PathBuilder for BLPath {
    fn new(capacity: u32) -> Self {
        let mut path = Self::new();
        if capacity != 0 {
            path.reserve((2 * capacity) as _).expect("failed to reserve Blend2D path");
        }   path
    }   // different commands vary in size for BLPath
    fn close(&mut self) { self.close() }
    fn current_pos(&self) -> Option<Vec2D> {
        self.get_last_vertex().ok()
            .map(|pt| Vec2D { x: pt.x() as _, y: pt.y() as _ })
    }

    fn move_to (&mut self, end: Vec2D) { self.move_to(end.into()) }
    fn line_to (&mut self, end: Vec2D) { self.line_to(end.into()) }
    fn cubic_to(&mut self, ocp: Vec2D, icp: Vec2D, end: Vec2D) {
        self.cubic_to(ocp.into(), icp.into(), end.into())
    }
    fn quad_to (&mut self, cpt: Vec2D, end: Vec2D) {
        self.quad_to(cpt.into(), end.into())
    }
    fn add_arc(&mut self, center: Vec2D, radii: Vec2D, start: f32, sweep: f32) {
        self.arc_to(center.into(), (radii.x as _, radii.y as _), start as _, sweep as _)
            .expect("failed to append Blend2D arc")
    }
    fn elliptic_arc_to(&mut self, radii: Vec2D,
        x_rot: f32, large: bool, sweep: bool, end: Vec2D) {
        self.elliptic_arc_to((radii.x as _, radii.y as _),
                x_rot as _, large, sweep, end.into())
            .expect("failed to append Blend2D elliptic arc")
    }

    fn to_kurbo(&self) -> BezPath {   use intvg::blend2d::BLPathItem::*;
        let mut pb = BezPath::with_capacity(self.get_size() as _);
        self.iter().for_each(|item| match item {
            MoveTo(end) => pb.move_to((end.x(), end.y())),
            LineTo(end) => pb.line_to((end.x(), end.y())),
            QuadTo(cpt, end) =>      pb.quad_to((cpt.x(), cpt.y()), (end.x(), end.y())),
            CubicTo(ocp, icp, end) =>
                pb.curve_to((ocp.x(), ocp.y()), (icp.x(), icp.y()), (end.x(), end.y())),
            Close => pb.close(),
        }); pb
    }
}
impl From<Vec2D> for BLPoint { fn from(pt: Vec2D) -> Self { (pt.x, pt.y).into() } }

impl MatrixConv for BLMatrix2D {
    /*  | a b 0 |   BLMatrix2D::transform (A' = B * A)
        | c d 0 |
        | e f 1 | */
    fn identity() -> Self { Self::identity() }
    fn rotate(&mut self, angle: f32) { self.post_rotate(angle as _, None) }
    fn translate(&mut self, pos: Vec2D) { self.post_translate(pos.into()) }
    fn skew_x(&mut self, sk: f32) { self.post_skew((sk as _, 0.)) }
    fn scale(&mut self, sl: Vec2D) { self.post_scale((sl.x as _, sl.y as _)) }
    fn premul(&mut self, tm: &Self) { self.transform(tm) }
}

impl StyleConv for BLStyle {
    fn solid_color(color: RGBA) -> Self {
        Self::Solid(BLSolidColor::init_rgba32(color.into())
            .expect("failed to create Blend2D solid color"))
    }

    fn linear_gradient(sp: Vec2D, ep: Vec2D, stops: &[(f32, RGBA)]) -> Self {
        let stops = stops.iter().map(|&(offset, color)|
                (offset, color.into()).into()).collect::<Vec<_>>();
        Self::Gradient(BLGradient::new(&BLLinearGradientValues::new(sp.into(), ep.into()))
            .and_then(|gradient| gradient.with_stops(&stops))
            .expect("failed to create Blend2D linear gradient"))
    }

    fn radial_gradient(cp: Vec2D, fp: Vec2D, radii: (f32, f32),
            stops: &[(f32, RGBA)]) -> Self {
        let stops = stops.iter().map(|&(offset, color)|
                (offset, color.into()).into()).collect::<Vec<_>>();
        Self::Gradient(BLGradient::new(&BLRadialGradientValues::
            new(cp.into(), fp.into(), (radii.0 as _, radii.1 as _)))
            .and_then(|gradient| gradient.with_stops(&stops))
            .expect("failed to create Blend2D radial gradient"))
    }
}

pub enum BLStyle { Solid(BLSolidColor), Gradient(BLGradient), }
impl BLStyle {
    fn as_b2d_style(&self) -> &dyn B2DStyle {
        match self {
            Self::Solid(style) => style,
            Self::Gradient(style) => style,
        }
    }
}
impl From<RGBA> for BLRgba32 {
    fn from(color: RGBA) -> Self { (color.r, color.g, color.b, color.a).into() }
}
