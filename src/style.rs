/****************************************************************
 * $ID: style.rs  	Tue 18 Nov 2025 15:30:11+0800               *
 *                                                              *
 * Maintainer: 范美辉 (MeiHui FAN) <mhfan@ustc.edu>              *
 * Copyright (c) 2025 M.H.Fan, All rights reserved.             *
 ****************************************************************/

use crate::{schema::*, helpers::*};

impl MatrixConv for kurbo::Affine {
    #[inline] fn identity() -> Self { Self::IDENTITY }
    #[inline] fn rotate(&mut self, angle: f32) { *self *= Self::rotate(angle as _) }
    #[inline] fn translate(&mut self, pos: Vec2D) { *self *= Self::translate(pos) }
    #[inline] fn skew_x(&mut self, sk: f32) { *self *= Self::skew(sk as _, 0.) }
    #[inline] fn scale(&mut self, sl: Vec2D) {
        *self *= Self::scale_non_uniform(sl.x as _, sl.y as _)
    }
    #[inline] fn multiply(&mut self, tm: &Self) { *self *= *tm }
}

impl From<RGBA> for peniko::Color {
    #[inline] fn from(color: RGBA) -> Self {
        Self::from_rgba8(color.r, color.g, color.b, color.a)
    }
}
impl StyleConv for peniko::Brush {
    #[inline] fn solid_color(&mut self, color: RGBA) -> Self { Self::Solid(color.into()) }
    #[inline] fn linear_gradient(&mut self, sp: Vec2D, ep: Vec2D,
            stops: &[(f32, RGBA)]) -> Self {
        use peniko::{Gradient, ColorStop, color::DynamicColor};
        let stops = stops.iter().map(|&(offset, color)|
            (offset, DynamicColor::from_alpha_color(color.into())).into())
            .collect::<Vec<ColorStop>>();
        Self::Gradient(Gradient::new_linear(sp, ep).with_stops(stops.as_slice()))
    }
    #[inline] fn radial_gradient(&mut self, cp: Vec2D, fp: Vec2D, radii: (f32, f32),
            stops: &[(f32, RGBA)]) -> Self {
        use peniko::{Gradient, ColorStop, color::DynamicColor};
        let stops = stops.iter().map(|&(offset, color)|
            (offset, DynamicColor::from_alpha_color(color.into())).into())
            .collect::<Vec<ColorStop>>();
        Self::Gradient(Gradient::new_two_point_radial(cp, radii.0, fp, radii.1)
            .with_stops(stops.as_slice()))
    }

    /* fn set_fill_stroke(&mut self, fso: FSOpts) {
        use {peniko::Fill, kurbo::{Stroke, Cap, Join}};
        match fso {
            FSOpts::Fill { rule } => scene.fill(match rule {
                FillRule::NonZero => Fill::NonZero, FillRule::EvenOdd => Fill::EvenOdd,
            }),
            FSOpts::Stroke { width, limit, join, cap } =>
                scene.stroke(Stroke::new(width as _).with_miter_limit(limit as _)
                    .with_caps(match cap {
                        LineCap::Butt   => Cap::Butt,
                        LineCap::Round  => Cap::Round,
                        LineCap::Square => Cap::Square,
                    }).with_join(match join {
                        LineJoin::Miter => Join::Miter,
                        LineJoin::Round => Join::Round,
                        LineJoin::Bevel => Join::Bevel,
                    }).with_dashes(dash[0] as _, dash[1].iter().map(|&x| x as _))),
        }
    } */
}

pub trait RenderContext {
    type VGPath: crate::pathm::PathBuilder;
    type VGStyle: StyleConv;    // (VGBrush/VGPaint, FSOpts)
    type TM2D: MatrixConv;

    fn get_transform(&self) -> Self::TM2D;
    fn reset_transform(&mut self, trfm: Option<&Self::TM2D>);
    fn apply_transform(&mut self, trfm: &Self::TM2D, opacity: Option<f32>); // alpha

    fn   fill_path(&mut self, path: &Self::VGPath, style: &Self::VGStyle, fso: &FSOpts);
    fn stroke_path(&mut self, path: &Self::VGPath, style: &Self::VGStyle, fso: &FSOpts);
}

pub trait MatrixConv {
    fn identity() -> Self;
    //fn reset(&mut self, tm: Option<&Self>);

    fn rotate(&mut self, angle: f32);
    fn translate(&mut self, pos: Vec2D);
    fn skew_x(&mut self, sk: f32);
    fn scale(&mut self, sl: Vec2D);

    /// Multiplications are right multiplications (Next = Previous * StepOperation).
    /// If your transform is transposed (tx, ty are on the last column),
    /// perform left multiplication instead.
    fn multiply(&mut self, tm: &Self);
}

pub trait StyleConv {
    fn solid_color(&mut self, color: RGBA) -> Self;
    fn linear_gradient(&mut self, sp: Vec2D, ep: Vec2D, stops: &[(f32, RGBA)]) -> Self;
    fn radial_gradient(&mut self, cp: Vec2D, fp: Vec2D, radii: (f32, f32),
        stops: &[(f32, RGBA)]) -> Self;
}

pub enum FSOpts {   Fill(FillRule),     /// dash\[0\] is offset indeed; use SmallVec for dash?
    Stroke { width: f32, limit: f32, join: LineJoin, cap: LineCap, dash: Vec<f32>, }
}

