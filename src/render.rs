/****************************************************************
 * $ID: render.rs  	Fri 03 May 2024 22:07:36+0800               *
 *                                                              *
 * Maintainer: 范美辉 (MeiHui FAN) <mhfan@ustc.edu>              *
 * Copyright (c) 2024 M.H.Fan, All rights reserved.             *
 ****************************************************************/

use core::cell::RefCell;
use crate::{schema::*, helpers::*, pathm::*, style::*};

impl PathBuilder for femtovg::Path {    // TODO: reserve capacity, current_pos
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

impl MatrixConv for femtovg::Transform2D {
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
    #[inline] fn multiply(&mut self, tm: &Self) { self.multiply(tm) }
}

impl From<RGBA> for femtovg::Color {
    #[inline] fn from(color: RGBA) -> Self { Self::rgba(color.r, color.g, color.b, color.a) }
}
impl StyleConv for femtovg::Paint {
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

use femtovg::{Canvas, Renderer, Transform2D as TM2D, CompositeOperation as CompOp,
    Path as VGPath, Paint as VGPaint, Color as VGColor};

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
        self.render_layers(canvas, None, &self.layers, self.fnth);

        self.elapsed -= 1.;       self.fnth += 1.;
        if self.op <= self.fnth { self.fnth  = 0.; }    true
    }

    /// The render order goes from the last element to the first,
    /// items in list coming first will be rendered on top.
    fn render_layers<T: Renderer>(&self, canvas: &mut Canvas<T>,
        ptm: Option<&TM2DwO>, layers: &[LayerItem], fnth: f32) {
        let mut matte: Option<TrackMatte> = None;

        let get_matrix = |vl: &VisualLayer, fnth: f32| {
            let mut trfm  = vl.ks.to_matrix(fnth, vl.ao);
            if let Some(pid) = vl.base.parent {
                let ptm = layers.iter().find_map(|layer|
                    layer.visual_layer().and_then(|vl|
                        vl.base.ind.and_then(|ind| if ind == pid {
                            Some(vl.ks.to_matrix(fnth, vl.ao)) } else { None })));

                if let Some(ptm) = &ptm { trfm.multiply(ptm) } else {
                    unreachable!()  //Default::default()
                }
            };  trfm
        };

        let last_trfm = canvas.transform();
        for layer in layers.iter().rev() { match layer {
            LayerItem::Shape(shpl) => if !shpl.vl.should_hide(fnth) {
                let mut trfm = get_matrix(&shpl.vl, fnth);
                canvas.set_transform(&trfm.0);
                //canvas.set_global_alpha(trfm.1);

                // XXX: transform are set correctly already, this is merely for
                // computing alpha/opacity correctly along rendering stack
                if let Some(ptm) = ptm { trfm.multiply(ptm) }

                prepare_matte(canvas, &last_trfm, &shpl.vl, &mut matte);
                let (draws, mut ts) =
                    convert_shapes(&shpl.shapes, fnth, shpl.vl.ao);
                canvas.set_transform(&ts.0);    ts.multiply(&trfm);
                canvas.set_global_alpha(ts.1);

                render_shapes(canvas, &trfm, &draws);
                render_matte (canvas, &last_trfm, &shpl.vl, &mut matte, fnth);

                canvas.reset_transform();    canvas.set_transform(&last_trfm);
            }
            LayerItem::PrecompLayer(pcl) => if !pcl.vl.should_hide(fnth) {
                if let Some(pcomp) = self.assets.iter().find_map(|asset|
                    match asset { AssetItem::Precomp(pcomp)
                        if pcomp.base.id == pcl.rid => Some(pcomp), _ => None }) {
                    let fnth = (fnth - pcl.vl.base.st) / pcl.vl.base.sr;

                    let mut trfm = get_matrix(&pcl.vl, fnth);
                    let fnth = pcl.tm.as_ref().map_or(fnth, // handle time remapping
                        |tm| tm.get_value(fnth) * pcomp.fr);

                    canvas.set_transform(&trfm.0);
                    if let Some(ptm) = ptm { trfm.multiply(ptm) }
                    canvas.set_global_alpha(trfm.1);

                    prepare_matte(canvas, &last_trfm, &pcl.vl, &mut matte);
                    self.render_layers(canvas, Some(&trfm), &pcomp.layers, fnth);
                     render_matte(canvas, &last_trfm, &pcl.vl, &mut matte, fnth);

                    canvas.reset_transform();    canvas.set_transform(&last_trfm);
                }   // clipping(pcl.w, pcl.h)?
            }
            LayerItem::SolidColor(scl) => if !scl.vl.should_hide(fnth) {
                let trfm = get_matrix(&scl.vl, fnth);
                canvas.set_transform(&trfm.0);

                //if let Some(ptm) = ptm { trfm.multiply(ptm) }
                //canvas.set_global_alpha(trfm.1); // should be fixed as 1.0?

                let mut path = VGPath::new();
                path.rect((self.w as f32 - scl.sw) / 2., // 0., 0.,
                          (self.h as f32 - scl.sh) / 2., scl.sw, scl.sh);
                let paint = VGPaint::color(VGColor::rgb(scl.sc.r, scl.sc.g, scl.sc.b));

                prepare_matte(canvas, &last_trfm, &scl.vl, &mut matte);
                canvas.fill_path(&path, &paint);
                 render_matte(canvas, &last_trfm, &scl.vl, &mut matte, fnth);

                canvas.reset_transform();   canvas.set_transform(&last_trfm);
            }
            LayerItem::Image(_) | LayerItem::Text(_)  | LayerItem::Data(_)  |
            LayerItem::Audio(_) | LayerItem::Camera(_) => dbg!(),     // TODO:

            //LayerItem::Null(_) => (),    // used as a parent, nothing to do
            _ => (),
        } }
    }
}

fn render_shapes<T: Renderer>(canvas: &mut Canvas<T>, trfm: &TM2DwO, draws: &[DrawItem]) {
    fn traverse_shapes<T: Renderer>(canvas: &mut Canvas<T>,
        draws: &[DrawItem], style: &RefCell<(VGPaint, FSOpts)>) {

        let last_trfm = canvas.transform();
        draws.iter().rev().for_each(|draw| match draw {
            DrawItem::Shape(path) => fill_stroke(canvas, path, style),

            DrawItem::Group(grp, ts) => {
                    canvas.set_transform(&ts.0);    traverse_shapes(canvas, grp, style);
                    canvas.reset_transform();       canvas.set_transform(&last_trfm);
            }   // apply transform in group-wise rather path-wise
            DrawItem::Repli(grp, rep) =>
                rep.iter().rev().for_each(|ts| {
                    canvas.set_transform(&ts.0);    traverse_shapes(canvas, grp, style);
                    canvas.reset_transform();       canvas.set_transform(&last_trfm);
                }),
            _ => (), // skip/ignore Style
        });
    }

    let last_trfm = canvas.transform();
    draws.iter().enumerate().rev().for_each(|(idx, draw)| match draw {
        DrawItem::Style(style) =>
            traverse_shapes(canvas, &draws[0..idx], style),
        DrawItem::Group(grp, ts) => {
                canvas.set_transform(&ts.0);
                let mut ts = ts.clone();    ts.multiply(trfm);

                canvas.set_global_alpha(ts.1);
                render_shapes(canvas, &ts, grp);

                canvas.reset_transform();   canvas.set_transform(&last_trfm);
                canvas.set_global_alpha(trfm.1);
        }   // apply transform in group-wise rather than path-wise
        DrawItem::Repli(grp, rep) => {
            rep.iter().rev().for_each(|ts| {
                canvas.set_transform(&ts.0);
                let mut ts = ts.clone();    ts.multiply(trfm);

                canvas.set_global_alpha(ts.1);
                render_shapes(canvas, &ts, grp);

                canvas.reset_transform();   canvas.set_transform(&last_trfm);
            }); canvas.set_global_alpha(trfm.1);
        }
        _ => (), // skip/ignore Shape
    });
}

fn fill_stroke<T: Renderer>(canvas: &mut Canvas<T>,
    path: &VGPath, style: &RefCell<(VGPaint, FSOpts)>) {
    use femtovg::{FillRule as FFR, LineCap as FLC, LineJoin as FLJ};

    match &style.borrow().1 {
        FSOpts::Fill(rule) => {
            let paint = &mut style.borrow_mut().0;
            paint.set_fill_rule(match rule {
                FillRule::EvenOdd => FFR::EvenOdd,
                FillRule::NonZero => FFR::NonZero,
            }); canvas.fill_path(path, paint);
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

            if dash.len() < 3 { canvas.stroke_path(path, paint); } else {
                canvas.stroke_path(&path.make_dash(dash[0], &dash[1..]), paint);
            }
        }
    }
}

use femtovg::{PixelFormat, ImageFlags, RenderTarget};
const CLEAR_COLOR: VGColor = VGColor::rgbaf(0., 0., 0., 0.);

fn prepare_matte<T: Renderer>(canvas: &mut Canvas<T>, last_trfm: &TM2D,
    vl: &VisualLayer, matte: &mut Option<TrackMatte>) {
	if vl.tt.is_none() && matte.is_none() { return }

	// XXX: limit image to viewport/viewbox
	let (w, h) = (canvas.width(), canvas.height());
	let (lx, ty) = last_trfm.transform_point(0., 0.);
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

fn  render_matte<T: Renderer>(canvas: &mut Canvas<T>, last_trfm: &TM2D,
    vl: &VisualLayer, matte: &mut Option<TrackMatte>, fnth: f32) {
	if (vl.tt.is_some() || matte.is_none() ||
		vl.td.is_some_and(|td| !td.as_bool())) && !vl.has_mask { return }

    let track = matte.as_mut().unwrap();
    if  vl.base.ind.is_some_and(|ind|
         track.mlid.is_some_and(|mlid| ind != mlid)) { return }
	let (imgid, mut path) = (track.imgid, VGPath::new());

	// XXX: limit image to viewport/viewbox
	//let (w, h) = canvas.image_size(imgid).unwrap();
	let (w, h) = (canvas.width(), canvas.height());
	let (lx, ty) = last_trfm.transform_point(0., 0.);
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
			canvas.fill_path(&path, &mpaint);

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
				let last_trfm = canvas.transform(); 	canvas.reset_transform();
				canvas.fill_path(&path, &paint); 	canvas.flush();
				canvas.set_transform(&last_trfm);
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
			let last_trfm = canvas.transform(); 	canvas.reset_transform();
			canvas.fill_path(&path, &paint); 	canvas.flush();
			canvas.set_transform(&last_trfm);
		} 	canvas.delete_image(mskid);
	}

	canvas.set_render_target(RenderTarget::Screen);
    canvas.global_composite_operation(CompOp::SourceOver);
	let last_trfm = canvas.transform(); 	canvas.reset_transform();
	canvas.fill_path(&path, &VGPaint::image(imgid, 0., 0., w as _, h as _, 0., 1.));
	canvas.flush(); 	canvas.delete_image(imgid); 	*matte = None;
    canvas.set_transform(&last_trfm);
}

struct TrackMatte { mode: MatteMode, mlid: Option<u32>,
    imgid: femtovg::ImageId, mskid: Option<femtovg::ImageId> }

type DrawItem = crate::style::DrawItem<femtovg::Path, femtovg::Paint, femtovg::Transform2D>;
type TM2DwO = crate::style::TM2DwO<femtovg::Transform2D>;

