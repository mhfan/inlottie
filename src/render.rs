/****************************************************************
 * $ID: render.rs  	Fri 03 May 2024 22:07:36+0800               *
 *                                                              *
 * Maintainer: 范美辉 (MeiHui FAN) <mhfan@ustc.edu>              *
 * Copyright (c) 2024 M.H.Fan, All rights reserved.             *
 ****************************************************************/

use std::f32::consts::PI;
use crate::{schema::*, helpers::*};

trait PathFactory { fn to_path(&self, fnth: f32) -> VGPath; }

impl PathFactory for Rectangle {
    fn to_path(&self, fnth: f32) -> VGPath {
        let center = self. pos.get_value(fnth);
        let halves = self.size.get_value(fnth) / 2.;
        let radius = self.rcr.get_value(fnth).min(halves.x).min(halves.y);
        let (elt, erb) = (center - halves, center + halves);

        // Note that unlike other shapes, on lottie web when the `d` attribute is missing,
        // the rectangle defaults as being reversed.
        //let is_ccw = self.base.dir.map_or(true, |d| matches!(d, ShapeDirection::Reversed));

        let mut path = VGPath::new();
        if radius < ACCURACY_TOLERANCE {
            //path.rect(elt.x, elt.y, size.x, size.y); 	return path;
            path.move_to(erb.x, elt.y);
            if self.base.is_ccw() {
                path.line_to(elt.x, elt.y); path.line_to(elt.x, erb.y);
				path.line_to(erb.x, erb.y);
            } else {
                path.line_to(erb.x, erb.y); path.line_to(elt.x, erb.y);
				path.line_to(elt.x, elt.y);
            }   path.close();   	 return path;
        }

        //path.rounded_rect(elt.x, elt.y, size.x, size.y, radius); 	return path;
        let (clt, crb) = (elt + radius, erb - radius);
            path.move_to(erb.x, clt.y);

        /* let tangent = radius * 0.5519;   // approximate with cubic Bezier curve
		let (tlt, trb) = (clt - tangent, crb + tangent);

        if self.base.is_ccw() {
            path.bezier_to(erb.x, tlt.y, trb.x, elt.y, crb.x, elt.y); path.line_to(clt.x, elt.y);
            path.bezier_to(tlt.x, elt.y, elt.x, tlt.y, elt.x, clt.y); path.line_to(elt.x, crb.y);
            path.bezier_to(elt.x, trb.y, tlt.x, erb.y, clt.x, erb.y); path.line_to(crb.x, erb.y);
            path.bezier_to(trb.x, erb.y, erb.x, trb.y, erb.x, crb.y); //path.line_to(erb.x, clt.y);
        } else {
            path.line_to(erb.x, crb.y); path.bezier_to(erb.x, trb.y, trb.x, erb.y, crb.x, erb.y);
            path.line_to(clt.x, erb.y); path.bezier_to(tlt.x, erb.y, elt.x, trb.y, elt.x, crb.y);
            path.line_to(elt.x, clt.y); path.bezier_to(elt.x, tlt.y, tlt.x, elt.y, clt.x, elt.y);
            path.line_to(crb.x, elt.y); path.bezier_to(trb.x, elt.y, erb.x, tlt.y, erb.x, clt.y);
        }   path.close(); 	return path; */

        if self.base.is_ccw() {     let dir = femtovg::Solidity::Solid;
            //path.arc_to(erb.x, elt.y, crb.x, elt.y, radius);
            path.arc(crb.x, clt.y, radius,  0.,  PI / 2., dir); path.line_to(clt.x, elt.y);
            //path.arc_to(elt.x, elt.y, elt.x, clt.y, radius);
            path.arc(clt.x, clt.y, radius,  PI / 2.,  PI, dir); path.line_to(elt.x, crb.y);
            //path.arc_to(elt.x, erb.y, clt.x, erb.y, radius);
            path.arc(clt.x, crb.y, radius,  PI, -PI / 2., dir); path.line_to(crb.x, erb.y);
            //path.arc_to(erb.x, erb.y, erb.x, crb.y, radius);
            path.arc(crb.x, crb.y, radius, -PI / 2.,  0., dir); //path.line_to(erb.x, clt.y);
        } else {                    let dir = femtovg::Solidity::Hole; // XXX:
            path.line_to(erb.x, crb.y); path.arc(crb.x, crb.y, radius,  0., -PI / 2., dir);
                                        //path.arc_to(erb.x, erb.y, crb.x, erb.y, radius);
            path.line_to(clt.x, erb.y); path.arc(clt.x, crb.y, radius, -PI / 2.,  PI, dir);
                                        //path.arc_to(elt.x, erb.y, elt.x, crb.y, radius);
            path.line_to(elt.x, clt.y); path.arc(clt.x, clt.y, radius,  PI,  PI / 2., dir);
                                        //path.arc_to(elt.x, elt.y, clt.x, elt.y, radius);
            path.line_to(crb.x, elt.y); path.arc(crb.x, clt.y, radius,  PI / 2.,  0., dir);
                                        //path.arc_to(erb.x, elt.y, erb.x, clt.y, radius);
        }   path.close();   path
    }
}

impl PathFactory for Polystar {
    fn to_path(&self, fnth: f32) -> VGPath {
        let center = self.pos.get_value(fnth);
        let (or, nvp) = (self.or.get_value(fnth), self.pt.get_value(fnth));
        let orr = self.os.get_value(fnth) * PI / 2. / 100. / nvp;  // XXX:

        let ir  = self.ir.as_ref().map_or(0.,   // self.sy == StarType::Star
            |ir| ir.get_value(fnth));
        let irr = self.is.as_ref().map_or(0.,
            |is| is.get_value(fnth) * PI / 2. / 100. / nvp);

        let mut angle = -PI / 2. + self.rotation.get_value(fnth).to_radians();
        let angle_step = if matches!(self.sy, StarType::Star) { PI } else { PI * 2. } /
            if self.base.is_ccw() { -nvp } else { nvp };

		let rp = Vec2D::from_polar(angle) * or;
		let pt = center + rp; 	//let rp = rp * orr;
        let mut path = VGPath::new();   path.move_to(pt.x, pt.y);

        let (mut lotx, mut loty) = (pt.x - rp.y * orr, pt.y + rp.x * orr);
        let  mut add_bezier_to = |radius, rr| {
            angle += angle_step;

			let rp = Vec2D::from_polar(angle) * radius;
			let pt = center + rp; 	//let rp = rp * rr;
            let (rdx, rdy) = (rp.y * rr, -rp.x * rr);

            path.bezier_to(lotx, loty,  pt.x + rdx, pt.y + rdy, pt.x, pt.y); // pt + rd
            (lotx, loty) = (pt.x - rdx, pt.y - rdy); // pt - rd
        };

        for _ in 0..nvp as u32 {
            if matches!(self.sy, StarType::Star) { add_bezier_to(ir, irr); }
            add_bezier_to(or, orr);
        }   path.close();   path
    }
}

impl PathFactory for Ellipse {
    fn to_path(&self, fnth: f32) -> VGPath {
        let mut path = VGPath::new();
        let center = self. pos.get_value(fnth);
        let radii  = self.size.get_value(fnth) / 2.;
        //path.ellipse(center.x, center.y, radii.x, radii.y);   return path;

        //  Approximate a circle with cubic Bézier curves
        //  https://spencermortensen.com/articles/bezier-circle/
        let tangent = radii * 0.5519;   // a magic number
        let (elt, tlt) = (center - radii, center - tangent);
        let (erb, trb) = (center + radii, center + tangent);
        path.move_to(center.x, elt.y);

        if self.base.is_ccw() {
            path.bezier_to(tlt.x, elt.y, elt.x, tlt.y, elt.x, center.y);
            path.bezier_to(elt.x, trb.y, tlt.x, erb.y, center.x, erb.y);
            path.bezier_to(trb.x, erb.y, erb.x, trb.y, erb.x, center.y);
            path.bezier_to(erb.x, tlt.y, trb.x, elt.y, center.x, elt.y);
        } else {
            path.bezier_to(trb.x, elt.y, erb.x, tlt.y, erb.x, center.y);
            path.bezier_to(erb.x, trb.y, trb.x, erb.y, center.x, erb.y);
            path.bezier_to(tlt.x, erb.y, elt.x, trb.y, elt.x, center.y);
            path.bezier_to(elt.x, tlt.y, tlt.x, elt.y, center.x, elt.y);
        }   path.close();   path
    }
}

impl PathFactory for FreePath {
    fn to_path(&self, fnth: f32) -> VGPath {
        if !self.base.is_ccw() { return self.shape.to_path(fnth); }
        let curv = self.shape.get_value(fnth);
        debug_assert!(curv.vp.len() == curv.it.len() &&
                      curv.it.len() == curv.ot.len() && !curv.vp.is_empty());

        let pt = curv.vp.last().unwrap();
        let mut path = VGPath::new();   path.move_to(pt.x, pt.y);

        for ((cvp, cit), (lvp, lot)) in
            curv.vp.iter().zip(curv.it.iter()).rev().skip(1).zip(
            curv.vp.iter().zip(curv.ot.iter()).rev()) {
            path.bezier_to( lvp.x + lot.x, lvp.y + lot.y,
                            cvp.x + cit.x, cvp.y + cit.y, cvp.x, cvp.y);
        }
        /* let mut i = curv.vp.len() - 1;
        while 0 < i { let (j, pt) = (i - 1, &curv.vp[i]);
            path.bezier_to(curv.vp[j].x + curv.ot[j].x, curv.vp[j].y + curv.ot[j].y,
                    pt.x + curv.it[i].x, pt.y + curv.it[i].y, pt.x, pt.y);  i -= 1; } */

        if  curv.closed {  let j = curv.it.len() - 1;
            path.bezier_to(curv.vp[0].x + curv.ot[0].x, curv.vp[0].y + curv.ot[0].y,
                pt.x + curv.it[j].x, pt.y + curv.it[j].y, pt.x, pt.y);
            path.close();
        }   path
    }
}

impl PathFactory for ShapeProperty {    // for mask
    fn to_path(&self, fnth: f32) -> VGPath {
        let curv = self.get_value(fnth);
        debug_assert!(curv.vp.len() == curv.it.len() &&
                      curv.it.len() == curv.ot.len() && !curv.vp.is_empty());

        let pt = curv.vp.first().unwrap(); //&curv.vp[0];
        let mut path = VGPath::new();   path.move_to(pt.x, pt.y);

        /* let _ = curv.vp.iter().zip(curv.it.iter()).cycle().skip(1).take( //.rev()
                curv.vp.len() - if curv.closed { 0 } else { 1 }).zip(
                curv.vp.iter().zip(curv.ot.iter())); */

        for ((cvp, cit), (lvp, lot)) in
            curv.vp.iter().zip(curv.it.iter()).skip(1).zip(
            curv.vp.iter().zip(curv.ot.iter())) {
            path.bezier_to( lvp.x + lot.x, lvp.y + lot.y,
                            cvp.x + cit.x, cvp.y + cit.y, cvp.x, cvp.y);
        }
        /* for i in 1..curv.vp.len() { let (j, pt) = (i - 1, &curv.vp[i]);
            path.bezier_to(curv.vp[j].x + curv.ot[j].x, curv.vp[j].y + curv.ot[j].y,
                    pt.x + curv.it[i].x, pt.y + curv.it[i].y, pt.x, pt.y); } */

        if  curv.closed {  let j = curv.ot.len() - 1;
            path.bezier_to(curv.vp[j].x + curv.ot[j].x, curv.vp[j].y + curv.ot[j].y,
                pt.x + curv.it[0].x, pt.y + curv.it[0].y, pt.x, pt.y);
            path.close();
        }   path
    }
}

impl FillStrokeGrad {
    fn to_paint(&self, fnth: f32) -> VGPaint {
        fn convert_stops(stops: &[(f32, RGBA)], opacity: f32) -> Vec<(f32, VGColor)> {
            stops.iter().map(|(offset, rgba)| {
                let mut color = VGColor::rgba(rgba.r, rgba.g, rgba.b, rgba.a);
                color.set_alphaf(opacity * color.a);  (*offset, color)
            }).collect::<Vec<_>>()
        }

        let opacity = self.opacity.get_value(fnth) / 100.;
        let mut paint = match &self.grad {
            ColorGrad::Color { color } => {
                let color = color.get_value(fnth); // RGB indeed
                VGPaint::color(VGColor::rgba(color.r, color.g, color.b, (opacity * 255.) as _))
            }
            ColorGrad::Gradient(grad) => {
                let (sp, ep) = (grad.sp.get_value(fnth), grad.ep.get_value(fnth));
                let stops = grad.stops.cl.get_value(fnth).0;
                debug_assert!(stops.len() as u32 == grad.stops.cnt);

                if matches!(grad.r#type, GradientType::Radial) {
                    let (dx, dy) = (ep.x - sp.x, ep.y - sp.y);
                    let radius = dx.hypot(dy);

                    let _hl = grad.hl.as_ref().map_or(0., |hl|
                        hl.get_value(fnth).clamp(f32::EPSILON - 100.,
                            100. - f32::EPSILON) * radius / 100.);
                    let _ha = grad.ha.as_ref().map_or(0., |ha|
                        ha.get_value(fnth).to_radians()) + math::fast_atan2(dy, dx);

                    //ctx.createRadialGradient(sp.x, sp.y, 0.,  // XXX:
                    //  sp.x + ha.cos() * hl, sp.y + ha.sin() * hl, radius);

                    // Lottie doesn't have any focal radius concept
                         VGPaint::radial_gradient_stops(sp.x, sp.y, 0., radius,
                            convert_stops(&stops, opacity))
                } else { VGPaint::linear_gradient_stops(sp.x, sp.y, ep.x, ep.y,
                            convert_stops(&stops, opacity))
                }
            }
        };

        use femtovg::{FillRule as FRule, LineJoin as LJoin, LineCap as LCap};
        match &self.base {
            FillStroke::FillRule { rule } =>
                paint.set_fill_rule(match rule {
                    FillRule::NonZero => FRule::NonZero,
                    FillRule::EvenOdd => FRule::EvenOdd,
                }),
            FillStroke::Stroke(stroke) => {
                paint.set_line_width (stroke.width.get_value(fnth));
                paint.set_miter_limit(stroke.ml2.as_ref().map_or(stroke.ml,
                    |ml| ml.get_value(fnth)));

                // stroke.dash is handled separately
                paint.set_line_join(match stroke.lj {
                    LineJoin::Miter => LJoin::Miter,
                    LineJoin::Round => LJoin::Round,
                    LineJoin::Bevel => LJoin::Bevel,
                });
                paint.set_line_cap (match stroke.lc {
                    LineCap::Butt   => LCap::Butt,
                    LineCap::Round  => LCap::Round,
                    LineCap::Square => LCap::Square,
                });
            }
        }       paint
    }
}

impl Transform {
    fn to_matrix(&self, fnth: f32, ao: IntBool) -> TM2DwO {
        let opacity = self.opacity.as_ref().map_or(1.,
            |o| o.get_value(fnth) / 100.); // FIXME: for canvas global?

        // Multiplications are RIGHT multiplications (Next = Previous * StepOperation).
        // If your transform is transposed (`tx`, `ty` are on the last column),
        // perform LEFT multiplication instead. Perform the following operations on a
        // matrix starting from the identity matrix (or the parent object's transform matrix):
        let (mut trfm, mut ts) = (TM2D::identity(), TM2D::identity());
        if  let Some(anchor) = &self.anchor {
            let anchor = anchor.get_value(fnth);
            trfm.multiply(&TM2D::new_translation(-anchor.x, -anchor.y));
        }

        if  let Some(scale) = &self.scale {
            let scale = scale.get_value(fnth) / 100.;
            //if scale.x == 0. { scale.x = f32::EPSILON; } // workaround for some lottie file?
            //if scale.y == 0. { scale.y = f32::EPSILON; }
            ts.scale(scale.x, scale.y);     trfm.multiply(&ts);
        }

        if  let Some(skew) = &self.skew {
            let axis = self.skew_axis.as_ref()
                .map(|axis| axis.get_value(fnth).to_radians());
            if let Some(axis) = axis { ts.rotate(-axis);   trfm.multiply(&ts); }

            let skew = -skew.get_value(fnth); //.clamp(-85., 85.); // SKEW_LIMIT
            ts.skew_x(skew.to_radians().tan());     trfm.multiply(&ts);

            if let Some(axis) = axis { ts.rotate( axis);   trfm.multiply(&ts); }
        }

        match &self.extra {
            TransRotation::Normal2D { rotation: Some(rdegree) } => {
                ts.rotate(rdegree.get_value(fnth).to_radians()); trfm.multiply(&ts);
            }

            TransRotation::Split3D(_) => unimplemented!(), //debug_assert!(ddd);
            _ => (),
        }

        match &self.position {
            Some(Translation::Normal(apos)) => {
                let pos  = apos.get_value(fnth);
                if  ao.as_bool() &&  apos.animated.as_bool() {
                    let orient = pos - apos.get_value(fnth - 1.);
                    ts.rotate(math::fast_atan2(orient.y, orient.x));  trfm.multiply(&ts);
                }   trfm.multiply(&TM2D::new_translation(pos.x, pos.y));
            }

            Some(Translation::Split(sv)) => {   debug_assert!(sv.split);
                let pos = Vec2D { x: sv.x.get_value(fnth), y: sv.y.get_value(fnth) };
                if  ao.as_bool() {
                    let orient = Vec2D { x: pos.x - sv.x.get_value(fnth - 1.),
                                                      y: pos.y - sv.y.get_value(fnth - 1.) };
                    ts.rotate(math::fast_atan2(orient.y, orient.x));  trfm.multiply(&ts);
                }   trfm.multiply(&TM2D::new_translation(pos.x, pos.y));
                if sv.z.is_some() { unimplemented!(); }
            }
            _ => (),
        }   TM2DwO(trfm, opacity)
    }

    fn to_repeat_trfm(&self, fnth: f32, offset: f32) -> TM2D {
        let (mut trfm, mut ts) = (TM2D::identity(), TM2D::identity());
        let anchor = if let Some(anchor) = &self.anchor {
            let anchor = anchor.get_value(fnth);
            trfm.multiply(&TM2D::new_translation(-anchor.x, -anchor.y));    anchor
        } else { Vec2D { x: 0., y: 0. } };

        if  let Some(scale) = &self.scale {
            let scale = scale.get_value(fnth) / 100.;
            ts.scale(scale.x.powf(offset), scale.y.powf(offset));
            trfm.multiply(&ts);
        }

        if  let Some(skew) = &self.skew {
            let axis = self.skew_axis.as_ref()
                .map(|axis| axis.get_value(fnth).to_radians());
            if let Some(axis) = axis { ts.rotate(-axis); trfm.multiply(&ts); }

            let skew = -skew.get_value(fnth); //.clamp(-85., 85.); // SKEW_LIMIT
            ts.skew_x(skew.to_radians().tan());     trfm.multiply(&ts); // XXX:

            if let Some(axis) = axis { ts.rotate( axis); trfm.multiply(&ts); }
        }

        match &self.extra {
            TransRotation::Normal2D { rotation: Some(rdegree) } => {
                ts.rotate(rdegree.get_value(fnth).to_radians() * offset);
                trfm.multiply(&ts);
            }

            TransRotation::Split3D(_) => unimplemented!(), //debug_assert!(ddd);
            _ => (),
        }

        let pos = match &self.position {
            Some(Translation::Normal(apos)) => apos.get_value(fnth),
            Some(Translation::Split(sv)) => {   debug_assert!(sv.split);
                Vec2D { x: sv.x.get_value(fnth), y: sv.y.get_value(fnth) }
            }   _ => Vec2D { x: 0., y: 0. },
        };  // XXX: shouldn't need to deal with auto orient?

        trfm.multiply(&TM2D::new_translation(pos.x * offset + anchor.x,
                                             pos.y * offset + anchor.y));   trfm
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
    fn traverse_shapes<T: Renderer>(canvas:  &mut Canvas<T>,
        draws: &[DrawItem], render: &impl Fn(&mut Canvas<T>, &VGPath)) {

        let last_trfm = canvas.transform();
        draws.iter().rev().for_each(|draw| match draw {
            DrawItem::Shape(path) => render(canvas, path),

            DrawItem::Group(grp, ts) => {
                    canvas.set_transform(&ts.0);    traverse_shapes(canvas, grp, render);
                    canvas.reset_transform();       canvas.set_transform(&last_trfm);
            }   // apply transform in group-wise rather path-wise
            DrawItem::Repli(grp, rep) =>
                rep.iter().rev().for_each(|ts| {
                    canvas.set_transform(&ts.0);    traverse_shapes(canvas, grp, render);
                    canvas.reset_transform();       canvas.set_transform(&last_trfm);
                }),
            _ => (), // skip/ignore Style
        });
    }

    let last_trfm = canvas.transform();
    draws.iter().enumerate().rev().for_each(|(idx, draw)| match draw {
        DrawItem::Style(style, dash) =>
            traverse_shapes(canvas, &draws[0..idx], &|canvas, path| {
                if let Some(dash) = dash {
                    if dash.len() < 3 { canvas.stroke_path(path, style);
                    } else { canvas.stroke_path(&path.make_dash(dash[0], &dash[1..]), style); }
                } else { canvas.fill_path(path, style); }
            }),
        DrawItem::Group(grp, ts) => {
                canvas.set_transform(&ts.0);
                let mut ts = ts.clone();    ts.multiply(trfm);

                canvas.set_global_alpha(ts.1);
                render_shapes(canvas, &ts, grp);

                canvas.reset_transform();   canvas.set_transform(&last_trfm);
                canvas.set_global_alpha(trfm.1);
        }   // apply transform in group-wise rather path-wise
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
			let mut path = mask.shape.to_path(fnth);
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

/// calculate transform matrix, convert shapes to paths, modify/change the paths,
/// and convert style(fill/stroke/gradient) to draw items, recursively
fn convert_shapes(shapes: &[ShapeItem], fnth: f32, ao: IntBool) -> (Vec<DrawItem>, TM2DwO) {
    let (mut draws, mut trfm) = (vec![], Default::default());
    for shape in shapes.iter() { match shape {
        ShapeItem::Rectangle(rect)    if !rect.base.elem.hd =>
            draws.push(DrawItem::Shape(Box::new(rect.to_path(fnth)))),
        ShapeItem::Polystar(star) if !star.base.elem.hd =>
            draws.push(DrawItem::Shape(Box::new(star.to_path(fnth)))),
        ShapeItem::Ellipse(elps)        if !elps.base.elem.hd =>
            draws.push(DrawItem::Shape(Box::new(elps.to_path(fnth)))),
        ShapeItem::Path(curv)          if !curv.base.elem.hd =>
            draws.push(DrawItem::Shape(Box::new(curv.to_path(fnth)))),

        // styles affect on all preceding paths ever before
        ShapeItem::Fill(fill)   if !fill.elem.hd =>
            draws.push(DrawItem::Style(Box::new(fill.to_paint(fnth)), None)),
        ShapeItem::Stroke(line) if !line.elem.hd =>
            draws.push(DrawItem::Style(Box::new(line.to_paint(fnth)),
                Some(line.get_dash(fnth)))),
        ShapeItem::GradientFill(grad)   if !grad.elem.hd =>
            draws.push(DrawItem::Style(Box::new(grad.to_paint(fnth)), None)),
        ShapeItem::GradientStroke(grad) if !grad.elem.hd =>
            draws.push(DrawItem::Style(Box::new(grad.to_paint(fnth)),
                Some(grad.get_dash(fnth)))),
        ShapeItem::NoStyle(_) => eprintln!("Nothing to do here?"),

        ShapeItem::Group(group) if !group.elem.hd => {
            let (grp, trfm) = convert_shapes(&group.shapes, fnth, ao);
            draws.push(DrawItem::Group(grp, trfm));
        }

        ShapeItem::Repeater(mdfr) if !mdfr.elem.hd => {
            let grp = draws;    draws = vec![];
            // repeat preceding (Shape/Style) items into new Groups?
            //get_repeater(mdfr, fnth).into_iter().for_each(|ts|
            //    draws.push(DrawItem::Group(grp.clone(), ts)));
            draws.push(DrawItem::Repli(grp, get_repeater(mdfr, fnth)));
        }

        // other modifiers usually just affect on all preceding paths ever before
        ShapeItem::Trim(mdfr) if !mdfr.elem.hd =>
            trim_shapes(mdfr, &mut draws, fnth),

        ShapeItem::Merge (_) | ShapeItem::OffsetPath (_) |
        ShapeItem::Twist (_) | ShapeItem::PuckerBloat(_) |
        ShapeItem::ZigZag(_) | ShapeItem::RoundedCorners(_) => dbg!(),  // TODO:

        ShapeItem::Transform(ts) if !ts.elem.hd =>
            trfm = ts.trfm.to_matrix(fnth, ao),

        _ => (),
    } }     (draws, trfm)
}

#[derive(Clone)] enum DrawItem { // Graphic Element
    Shape(Box<VGPath>),
    Style(Box<VGPaint>, Option<Vec<f32>>),  // optional stroke dash (offset and pattern)
    Repli(Vec<DrawItem>, Vec<TM2DwO>), // something like batch Groups
    Group(Vec<DrawItem>, TM2DwO),
}

#[derive(Clone)] struct TM2DwO(TM2D, f32);
impl Default for TM2DwO { fn default() -> Self { Self(TM2D::identity(), 1.) } }
impl TM2DwO {
    #[inline] fn multiply(&mut self, other: &TM2DwO) {
        self.0  .multiply(&other.0);  self.1 *= other.1;
    }
}

fn get_repeater(mdfr: &Repeater, fnth: f32) -> Vec<TM2DwO> {
    let mut opacity = mdfr.tr.so.as_ref().map_or(1.,
        |so| so.get_value(fnth) / 100.);
    let offset = mdfr.offset.as_ref().map_or(0.,
        |offset| offset.get_value(fnth));   // range: [-1, 2]?

    let cnt = mdfr.cnt.get_value(fnth) as u32;
    let delta = if 1 < cnt { (mdfr.tr.eo.as_ref().map_or(1., |eo|
        eo.get_value(fnth) / 100.) - opacity) / (cnt - 1) as f32 } else { 0. };
    let mut coll = Vec::with_capacity(cnt as usize);

    for i in 0..cnt {
        let i = if matches!(mdfr.order, Composite::Below) { i } else { cnt - 1 - i };
        coll.push(TM2DwO(mdfr.tr.trfm.to_repeat_trfm(fnth, offset + i as f32), opacity));
        opacity += delta;
    }   coll
}

fn trim_shapes(mdfr: &TrimPath, draws: &mut [DrawItem], fnth: f32) {
    fn modify_shapes(draws: &mut [DrawItem], closure: &mut impl FnMut(&mut VGPath)) {
        draws.iter_mut().for_each(|draw| match draw {
            DrawItem::Group(grp, _) => modify_shapes(grp, closure),
            DrawItem::Repli(grp, _) => modify_shapes(grp, closure),
            DrawItem::Shape(path) => closure(path),
            _ => (), // skip/ignore Style
        });
    }       // XXX: how to treat repeated shapes?

    fn traverse_shapes(draws: &mut [DrawItem], closure: &mut impl FnMut(&mut VGPath)) {
        draws.iter_mut().for_each(|draw| match draw {
            DrawItem::Shape(path) => closure(path),
            DrawItem::Group(grp, _) => traverse_shapes(grp, closure),
            DrawItem::Repli(grp, _) => traverse_shapes(grp, closure),
            _ => (), // skip/ignore Style
        });
    }

    let offset   = mdfr.offset.get_value(fnth) as f64 / 360.;
    let start    = mdfr. start.get_value(fnth) as f64 / 100.;
    let mut trim = mdfr.   end.get_value(fnth) as f64 / 100. - start;
    if 1. < trim { trim = 1.; } //debug_assert!((0.0..=1.).contains(&trim));
    let start = (start + offset) % 1.;

    if mdfr.multiple.is_some_and(|ml| matches!(ml, TrimMultiple::Simultaneously)) {
        modify_shapes(draws, &mut |path| *path = trim_path(path.verbs(), start, trim));
    } else {
        let (mut idx, mut suml) = (0u32, 0.);
        let (mut lens, mut tri0) = (vec![], 0.);

        traverse_shapes(draws, &mut |path| {
            let len = kurbo::segments(path.verbs().map(convert_path_f2k)).fold(0.,
                |acc, seg| acc + seg.arclen(ACCURACY_TOLERANCE));
            lens.push(len);     suml += len;
        });

        if 1. < start + trim { tri0 = start + trim - 1.; trim = 1. - start; }
        let (start, mut trim) = (suml * start, suml * trim);
        tri0 *= suml;   suml = 0.;

        traverse_shapes(draws, &mut |path: &mut VGPath| { // same logic as in trim_path
            let len = lens[idx as usize];   idx += 1;

            if suml <= start &&  start < suml + len {   let start = start - suml;
                if  start + trim < len {
                    *path = trim_path(path.verbs(), start / len, trim  / len);  trim = 0.;
                } else { trim -= len - start;   let start = start / len;
                    *path = trim_path(path.verbs(), start, 1. - start);
                }
            } else if start < suml && 0. < trim { if trim < len {
                    *path = trim_path(path.verbs(), 0., (trim / len) as _);     trim = 0.;
                } else { trim -= len; }
            } else if 0. < tri0 { if tri0 < len {
                    *path = trim_path(path.verbs(), 0., (tri0 / len) as _);     tri0 = 0.;
                } else { tri0 -= len; }
            } else { *path = VGPath::new(); }   suml += len;
        });
    }
}

use kurbo::{BezPath, ParamCurve, ParamCurveArclen};
fn trim_path<I: Iterator<Item = Verb>>(path: I, start: f64, mut trim: f64) -> VGPath {
    // https://lottiefiles.github.io/lottie-docs/scripts/lottie_bezier.js
    // or use curve_length(curve, merr) and subdivide(t, seg) of flo_curves
    //let segments = kurbo::segments(path.map(convert_path_f2k));
    let path = path.map(convert_path_f2k).collect::<BezPath>();

    let (mut tri0, mut suml) = (0., path.segments().fold(0.,
        |acc, seg| acc + seg.arclen(ACCURACY_TOLERANCE as _)));
    if 1. < start + trim { tri0 = start + trim - 1.; trim = 1. - start; }
    let (start, mut trim) = (suml * start, suml * trim);
    let mut fpath = VGPath::new();  tri0 *= suml;  suml = 0.;

    BezPath::from_path_segments(path.segments().filter_map(|seg| {
        let len = seg.arclen(ACCURACY_TOLERANCE as _);

        let range = if suml <= start && start < suml + len {
            let start = start - suml;   let end = start + trim;
            if  end < len { trim = 0.;      start / len .. end / len
            } else { trim -= len - start;   start / len .. 1. }
        } else if start < suml && 0. < trim {
            if trim < len { let end = trim / len;   trim = 0.;  0.0 .. end
            } else { trim -= len;   0.0 .. 1. }
        } else if 0. < tri0 {   // rewound part
            if tri0 < len { let end = tri0 / len;   tri0 = 0.;  0.0 .. end
            } else { tri0 -= len;   0.0 .. 1. }
        } else {     suml += len;   return None };
        suml += len;    Some(seg.subsegment(range))
    })).iter().for_each(|el| convert_path_k2f(el, &mut fpath));  fpath
}

// https://docs.rs/kurbo/latest/kurbo/offset/index.html
// https://github.com/nical/lyon/blob/main/crates/algorithms/src/walk.rs
// https://www.reddit.com/r/rust/comments/12do1dq/rendering_text_along_a_curve/
// https://developer.mozilla.org/en-US/docs/Web/SVG/Reference/Element/textPath
#[allow(unused)] fn walk_along_path() { }

fn path_to_dash(path: &VGPath, dash: &(f32, Vec<f32>)) -> VGPath {
    let mut npath = VGPath::new();  debug_assert!(dash.1.len() < 5);
    kurbo::dash(path.verbs().map(convert_path_f2k), dash.0 as _,
        &dash.1.iter().map(|v| *v as f64).collect::<Vec<_>>())
        .for_each(|el| convert_path_k2f(el, &mut npath));   npath
}

use {kurbo::PathEl, femtovg::Verb};
fn convert_path_f2k(verb: Verb)  -> PathEl { match verb {
    Verb::MoveTo(x, y) => PathEl::MoveTo((x, y).into()),
    Verb::LineTo(x, y) => PathEl::LineTo((x, y).into()),
    Verb::BezierTo(ox, oy, ix, iy, x, y) =>
        PathEl::CurveTo((ox, oy).into(), (ix, iy).into(), (x, y).into()),
    Verb::Solid | Verb::Hole => unreachable!(),
    Verb::Close => PathEl::ClosePath,
} }

fn convert_path_k2f(elem: PathEl, path: &mut VGPath) { match elem {
    PathEl::MoveTo(pt) => path.move_to(pt.x as _, pt.y as _),
    PathEl::LineTo(pt) => path.line_to(pt.x as _, pt.y as _),
    PathEl::CurveTo(ot, it, pt) =>
        path.bezier_to(ot.x as _, ot.y as _, it.x as _, it.y as _, pt.x as _, pt.y as _),
    PathEl::QuadTo(ct, pt) =>
        path.quad_to(ct.x as _, ct.y as _, pt.x as _, pt.y as _),
    //    let (ot, it) = (ct + (lp - ct) / 3, ct + (pt - ct) / 3);  // elevating curve order
    //    path.bezier_to(ot.x as _, ot.y as _, it.x as _, it.y as _, pt.x as _, pt.y as _),
    PathEl::ClosePath => path.close(),
} }

