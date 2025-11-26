/****************************************************************
 * $ID: render.rs  	Fri 03 May 2024 22:07:36+0800               *
 *                                                              *
 * Maintainer: 范美辉 (MeiHui FAN) <mhfan@ustc.edu>              *
 * Copyright (c) 2024 M.H.Fan, All rights reserved.             *
 ****************************************************************/

use core::cell::RefCell;
use crate::{schema::*, helpers::*, pathm::*, style::*};
type DrawItem = crate::style::DrawItem<VGPath, VGPaint, TM2D>;
type TM2DwO   = crate::style::TM2DwO<TM2D>;

impl PathBuilder for VGPath {    // TODO: reserve capacity, current_pos
    #[inline] fn current_pos(&self) -> Option<Vec2D> { todo!() }
    #[inline] fn new(_capacity: u32) -> Self { Self::new() }
    #[inline] fn close(&mut self) { self.close() }

    #[inline] fn move_to(&mut self, end: Vec2D) { self.move_to(end.x, end.y) }
    #[inline] fn line_to(&mut self, end: Vec2D) { self.line_to(end.x, end.y) }
    #[inline] fn cubic_to(&mut self, ocp: Vec2D, icp: Vec2D, end: Vec2D) {
        self.bezier_to(ocp.x, ocp.y, icp.x, icp.y, end.x, end.y)
    }
    #[inline] fn quad_to(&mut self, cp: Vec2D, end: Vec2D) {
        self.quad_to(cp.x, cp.y, end.x, end.y)
    }
    #[inline] fn add_arc(&mut self, center: Vec2D, radii: Vec2D, start: f32, sweep: f32) {
        self.arc(center.x, center.y, (radii.x + radii.y) / 2.,
            start as _, sweep as _, femtovg::Solidity::Solid)   // XXX:
        //self.arc_to(x1, y1, x2, y2, (radii.x + radii.y) / 2.);
    }

    fn to_kurbo(&self) -> BezPath {   use femtovg::Verb::*;
        let mut pb = BezPath::with_capacity(self.verbs().count());
        self.verbs().for_each(|verb|  match verb {
            MoveTo(x, y) => pb.move_to((x, y)),
            LineTo(x, y) => pb.line_to((x, y)),
            BezierTo(ox, oy, ix, iy, x, y) =>
                pb.curve_to((ox, oy), (ix, iy), (x, y)),
            Solid | Hole => unreachable!(),
            Close => pb.close(),
        }); pb
    }
}

impl MatrixConv for TM2D {
    /*  |a c e|     TM2D::multiply (A' = B * A)
        |b d f|
        |0 0 1| */
    #[inline] fn identity() -> Self { Self::identity() }
    #[inline] fn rotate(&mut self, angle: f32) {
        let mut tm = Self::identity();
        tm.rotate(angle);   self.multiply(&tm)
    }
    #[inline] fn translate(&mut self, pos: Vec2D) {
        let mut tm = Self::identity();
        tm.translate(pos.x, pos.y);   self.multiply(&tm);
    }
    #[inline] fn skew_x(&mut self, sk: f32) {
        let mut tm = Self::identity();
        tm.skew_x(sk);  self.multiply(&tm);
    }

    #[inline] fn scale(&mut self, sl: Vec2D) {
        let mut tm = Self::identity();
        tm.scale(sl.x, sl.y);   self.multiply(&tm);
    }
    #[inline] fn premul(&mut self, tm: &Self) { self.premultiply(tm) }
}

impl From<RGBA> for VGColor {
    #[inline] fn from(color: RGBA) -> Self { Self::rgba(color.r, color.g, color.b, color.a) }
}
impl StyleConv for VGPaint {
    #[inline] fn solid_color(color: RGBA) -> Self { Self::color(color.into()) }
    #[inline] fn linear_gradient(sp: Vec2D, ep: Vec2D, stops: &[(f32, RGBA)]) -> Self {
        Self::linear_gradient_stops(sp.x, sp.y, ep.x, ep.y,
            stops.iter().map(|&(offset, color)| (offset, color.into())))
    }

    #[inline] fn radial_gradient(cp: Vec2D, _fp: Vec2D, radii: (f32, f32),
            stops: &[(f32, RGBA)]) -> Self {
        Self::radial_gradient_stops(cp.x, cp.y, radii.0, radii.1,
            stops.iter().map(|&(offset, color)| (offset, color.into())))
    }
}

impl<T: Renderer> RenderContext for Canvas<T> {
    type VGStyle = VGPaint;
    type VGPath  = VGPath;
    type TM2D = TM2D;

    fn clear_rect_with(&mut self, x: u32, y: u32, w: u32, h: u32, color: RGBA) {
        self.clear_rect(x, y, w, h, color.into());
    }
    fn reset_transform(&mut self, trfm: Option<&Self::TM2D>) {
        self.reset_transform();     //self.set_global_alpha(1.);
        if let Some(trfm) = trfm { self.set_transform(trfm) }
    }
    fn apply_transform(&mut self, trfm: &Self::TM2D, opacity: Option<f32>) -> Self::TM2D {
        let last_trfm = self.transform();
        if let Some(opacity) = opacity { self.set_global_alpha(opacity) }
        self.set_transform(trfm);   last_trfm
    }

    fn fill_stroke(&mut self, path: &Self::VGPath, style: &RefCell<(Self::VGStyle, FSOpts)>) {
        use femtovg::{FillRule as FFR, LineCap as FLC, LineJoin as FLJ};

        match &style.borrow().1 {
            FSOpts::Fill(rule) => {
                let paint = &mut style.borrow_mut().0;
                paint.set_fill_rule(match rule {
                    FillRule::NonZero => FFR::NonZero,
                    FillRule::EvenOdd => FFR::EvenOdd,
                }); self.fill_path(path, paint);
            }

            FSOpts::Stroke { width, limit,
                join, cap, dash } => {
                let paint = &mut style.borrow_mut().0;
                paint.set_line_width (*width);
                paint.set_miter_limit(*limit);

                paint.set_line_join(match join {
                    LineJoin::Miter => FLJ::Miter, LineJoin::Round => FLJ::Round,
                    LineJoin::Bevel => FLJ::Bevel,
                });
                paint.set_line_cap(match cap {
                    LineCap::Butt   => FLC::Butt,   LineCap::Round => FLC::Round,
                    LineCap::Square => FLC::Square,
                });

                if dash.len() < 3 { self.stroke_path(path, paint); } else {
                    self.stroke_path(&path.make_dash(dash[0], &dash[1..]), paint);
                }
            }
        }
    }
}

use femtovg::{Canvas, Renderer, CompositeOperation as CompOp,
    Transform2D as TM2D, Path as VGPath, Paint as VGPaint, Color as VGColor};

impl Animation {    /// https://lottiefiles.github.io/lottie-docs/rendering/
    //fn get_duration(&self) -> f32 { (self.op - self.ip) / self.fr }
    pub fn render_next_frame<T: Renderer>(&mut self,
        canvas: &mut Canvas<T>, elapsed: f32) -> bool {
        //debug_assert!(0. < self.fr && 0. <= self.ip && 1. < self.op - self.ip);

            self.elapsed += elapsed * self.fr;
        if  self.elapsed < 1. && self.ip < self.fnth { return false }

        if  2. <= self.elapsed {    // advance/skip elapsed frames
            let elapsed = (self.elapsed - 1.).floor();
            self.fnth = (self.fnth + elapsed) % self.op;
            self.elapsed -= elapsed;
        }

        /* let trfm = canvas.transform();
        let pt = trfm.transform_point(self.w as _, self.h as _);
        let ltrb = (trfm[4] as u32, trfm[5] as u32,
                pt.0.ceil() as u32,   pt.1.ceil() as u32); // viewport/viewbox */

        canvas.clear_rect(0, 0, canvas.width(), canvas.height(),
            //ltrb.0, ltrb.1, ltrb.2 - ltrb.0, ltrb.3 - ltrb.1,
            VGColor::rgbf(0.4, 0.4, 0.4));
        self.render_layers(canvas, &TM2DwO::default(), &self.layers, self.fnth);

        self.elapsed -= 1.;       self.fnth += 1.;
        if self.op <= self.fnth { self.fnth  = 0.; }    true
    }

    /// The render order goes from the last element to the first,
    /// items in list coming first will be rendered on top.
    fn render_layers<T: Renderer>(&self, canvas: &mut Canvas<T>,
        ptm: &TM2DwO, layers: &[LayerItem], fnth: f32) {
        let mut matte: Option<TrackMatte> = None;

        for layer in layers.iter().rev() { match layer {
            LayerItem::Shape(shpl) => if !shpl.vl.should_hide(fnth) {
                let ltm = shpl.vl.get_matrix(layers, fnth).compose(ptm);
                let (draws, ctm) =
                    convert_shapes(&shpl.shapes, fnth, shpl.vl.ao);

                prepare_matte(canvas, &shpl.vl, &mut matte);
                canvas.render_shapes(&ctm.compose(&ltm), &draws);
                compose_matte(canvas, &shpl.vl, &mut matte, &ltm, fnth);
            }
            LayerItem::PrecompLayer(pcl) => if !pcl.vl.should_hide(fnth) {
                if let Some(pcomp) = self.assets.iter().find_map(|asset|
                    match asset { AssetItem::Precomp(pcomp)
                        if pcomp.base.id == pcl.rid => Some(pcomp), _ => None }) {
                    let fnth = (fnth - pcl.vl.base.st) / pcl.vl.base.sr;

                    let fnth = pcl.tm.as_ref().map_or(fnth, // handle time remapping
                        |tm| tm.get_value(fnth) * pcomp.fr);
                    let ltm = pcl.vl.get_matrix(layers, fnth).compose(ptm);

                    prepare_matte(canvas, &pcl.vl, &mut matte);
                    self.render_layers(canvas, &ltm, &pcomp.layers, fnth);
                    compose_matte(canvas, &pcl.vl, &mut matte, &ltm, fnth);
                }   // XXX: clipping(pcl.w, pcl.h)?
            }
            LayerItem::SolidColor(scl) => if !scl.vl.should_hide(fnth) {
                let ltm = scl.vl.get_matrix(layers, fnth).compose(ptm);

                let mut path = VGPath::new();
                path.rect((self.w as f32 - scl.sw) / 2., // 0., 0.,
                          (self.h as f32 - scl.sh) / 2., scl.sw, scl.sh);

                prepare_matte(canvas, &scl.vl, &mut matte);
                canvas.render_shapes(&ltm, &[DrawItem::Shape(path.into()),
                    DrawItem::Style(RefCell::new((VGPaint::color(scl.sc.into()),
                        FSOpts::Fill(FillRule::NonZero))).into())]);
                compose_matte(canvas, &scl.vl, &mut matte, &ltm, fnth);
            }
            LayerItem::Image(_) | LayerItem::Text(_)  | LayerItem::Data(_)  |
            LayerItem::Audio(_) | LayerItem::Camera(_) => dbg!(),     // TODO:

            //LayerItem::Null(_) => (),    // used as a parent, nothing to do
            _ => (),
        } }
    }
}

use femtovg::{PixelFormat, ImageFlags, RenderTarget};
const CLEAR_COLOR: VGColor = VGColor::rgbaf(0., 0., 0., 0.);

fn prepare_matte<T: Renderer>(canvas: &mut Canvas<T>,
    vl: &VisualLayer, matte: &mut Option<TrackMatte>) {
	if vl.tt.is_none() && matte.is_none() { return }

	// XXX: limit image to viewport/viewbox
	let (w, h) = (canvas.width(), canvas.height());
	let (lx, ty) = canvas.transform().transform_point(0., 0.);
	let (lx, ty) = (lx as u32, ty as u32);

    if vl.tt.is_some() || vl.has_mask {
        let imgid = canvas.create_image_empty(w as _, h as _,
            PixelFormat::Rgba8, ImageFlags::FLIP_Y).unwrap();
        canvas.set_render_target(RenderTarget::Image(imgid));
        canvas.clear_rect(lx, ty, w - lx * 2, h - ty * 2, CLEAR_COLOR);

        *matte = Some(TrackMatte { mode: vl.tt.unwrap_or(MatteMode::Normal),
			mlid: vl.tp, imgid, mskid: None }); 	return
    } else if vl.td.is_some_and(|td| !td.as_bool()) { return }

    let matte = matte.as_mut().unwrap();
    if vl.base.ind.is_some_and(|ind|
        matte.mlid.is_some_and(|mlid| ind != mlid)) { return }

    let mskid = canvas.create_image_empty(w as _, h as _,
        PixelFormat::Rgba8, ImageFlags::FLIP_Y).unwrap();
    canvas.set_render_target(RenderTarget::Image(mskid));
    canvas.clear_rect(lx, ty, w - lx * 2, h - ty * 2, CLEAR_COLOR);
    matte.mskid = Some(mskid);
}

fn compose_matte<T: Renderer>(canvas: &mut Canvas<T>, vl: &VisualLayer,
    matte: &mut Option<TrackMatte>, ltm: &TM2DwO, fnth: f32) {
	if (vl.tt.is_some() || matte.is_none() ||
		vl.td.is_some_and(|td| !td.as_bool())) && !vl.has_mask { return }

    let track = matte.as_mut().unwrap();
    if  vl.base.ind.is_some_and(|ind|
         track.mlid.is_some_and(|mlid| ind != mlid)) { return }
	let (imgid, mut path) = (track.imgid, VGPath::new());

	// XXX: limit image to viewport/viewbox
	//let (w, h) = canvas.image_size(imgid).unwrap();
	let (w, h) = (canvas.width(), canvas.height());
	let (lx, ty) = canvas.transform().transform_point(0., 0.);
	path.rect(lx, ty, w as f32 - lx * 2., h as f32 - ty * 2.);

	if  vl.has_mask {
		let mskid = canvas.create_image_empty(w as _, h as _,
			PixelFormat::Rgba8, ImageFlags::FLIP_Y).unwrap();
		canvas.set_render_target(RenderTarget::Image(mskid));
		let paint = VGPaint::image(mskid, 0., 0., w as _, h as _, 0., 1.);
		let mut mpaint = VGPaint::color(CLEAR_COLOR);

		vl.masks.iter().for_each(|mask| {
			let mut path: VGPath = mask.shape.to_path(fnth);
			if mask.inv { path.solidity(femtovg::Solidity::Hole); }
			if let Some(_expand) = &mask.expand { todo!() }

			let  opacity = mask.opacity.as_ref().map_or(1.,
				|opacity| opacity.get_value(fnth) / 100.);
			mpaint.set_color(VGColor::rgbaf(0., 0., 0., opacity));

			canvas.clear_rect(lx as _, ty as _, w - lx as u32 * 2,
												h - ty as u32 * 2, CLEAR_COLOR);

            let last_trfm = canvas.apply_transform(&ltm.0, Some(ltm.1));
			canvas.fill_path(&path, &mpaint);
            canvas.reset_transform();    canvas.set_transform(&last_trfm);  // XXX:

			let cop = match mask.mode {
				MaskMode::Add       => Some(CompOp::DestinationIn),
				MaskMode::Subtract  => Some(CompOp::DestinationOut),
				MaskMode::Intersect => Some(CompOp::DestinationAtop),
				MaskMode::Lighten   => Some(CompOp::Lighter),
				MaskMode::Darken | MaskMode::Difference => unimplemented!(),
				MaskMode::None => None,
			};

			if let Some(cop) = cop {
				canvas.global_composite_operation (cop);
				canvas.set_render_target(RenderTarget::Image(imgid));
				canvas.fill_path(&path, &paint); 	canvas.flush();
			} 	canvas.set_render_target(RenderTarget::Image(mskid));
		}); 	canvas.delete_image(mskid);
	}

	if let Some(mskid) = track.mskid {
		// https://developer.mozilla.org/en-US/docs/Web/API/CanvasRenderingContext2D/
		let cop = match track.mode {
			MatteMode::Alpha =>         Some(CompOp::DestinationIn),
			MatteMode::InvertedAlpha => Some(CompOp::DestinationOut),
			MatteMode::Luma | MatteMode::InvertedLuma => unimplemented!(),
			MatteMode::Normal => None,
		};

		if let Some(cop) = cop {
			canvas.global_composite_operation (cop);
			canvas.set_render_target(RenderTarget::Image(imgid));

			let paint = VGPaint::image(mskid, 0., 0., w as _, h as _, 0., 1.);
			canvas.fill_path(&path, &paint); 	canvas.flush();
		} 	canvas.delete_image(mskid);
	}

	canvas.set_render_target(RenderTarget::Screen);
    canvas.global_composite_operation(CompOp::SourceOver);
	canvas.fill_path(&path, &VGPaint::image(imgid, 0., 0., w as _, h as _, 0., 1.));
	canvas.flush(); 	canvas.delete_image(imgid); 	*matte = None;
}

struct TrackMatte { mode: MatteMode, mlid: Option<u32>,
    imgid: femtovg::ImageId, mskid: Option<femtovg::ImageId> }

