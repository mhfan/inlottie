/****************************************************************
 * $ID: render.rs  	Fri 03 May 2024 22:07:36+0800               *
 *                                                              *
 * Maintainer: 范美辉 (MeiHui FAN) <mhfan@ustc.edu>              *
 * Copyright (c) 2024 M.H.Fan, All rights reserved.             *
 ****************************************************************/

use crate::{schema::*, helpers::{*, math::*}};
use std::f32::consts::PI;

/// https://lottiefiles.github.io/lottie-docs/rendering/
trait PathFactory { fn to_path(&self, fnth: f32) -> VGPath; }

impl PathFactory for Rectangle {
    fn to_path(&self, fnth: f32) -> VGPath {
        let mut path = VGPath::new();
        let center = self. pos.get_value(fnth);
        let half   = self.size.get_value(fnth) / 2.;    //let  half = size / 2.;
        let radius = self.rcr.get_value(fnth).min(half.x).min(half.y);

        let (elx, ety) = (center.x - half.x, center.y - half.y); // center - half
        let (erx, eby) = (center.x + half.x, center.y + half.y); // center + half

        // Note that unlike other shapes, on lottie web when the `d` attribute is missing,
        // the rectangle defaults as being reversed.
        //let is_ccw = self.base.dir.map_or(true, |d| matches!(d, ShapeDirection::Reversed));

        if radius < f32::EPSILON {      path.move_to(erx, ety);
            if self.base.is_ccw() {
                path.line_to(elx, ety); path.line_to(elx, eby); path.line_to(erx, eby);
            } else {
                path.line_to(erx, eby); path.line_to(elx, eby); path.line_to(elx, ety);
            }   path.close();   return  path
            //path.rect(elx, ety, size.x, size.y);    return;
        }

        //path.rounded_rect(elx, ety, size.x, size.y, radius);  return;
        let (clx, cty) = (elx + radius, ety + radius); // elt + radius
        let (crx, cby) = (erx - radius, eby - radius); // erb - radius
            path.move_to(erx, cty);

        /* let tangent = radius * 0.5519;   // approximate with cubic Bezier curve
        let (tlx, tty) = (clx - tangent, cty - tangent); // clt - tangent
        let (trx, tby) = (crx + tangent, cby + tangent); // crb + tangent

        if self.base.is_ccw() {
            path.bezier_to(erx, tty, trx, ety, crx, ety);     path.line_to(clx, ety);
            path.bezier_to(tlx, ety, elx, tty, elx, cty);     path.line_to(elx, cby);
            path.bezier_to(elx, tby, tlx, eby, clx, eby);     path.line_to(crx, eby);
            path.bezier_to(trx, eby, erx, tby, erx, cby);     //path.line_to(erx, cty);
        } else {
            path.line_to(erx, cby);     path.bezier_to(erx, tby, trx, eby, crx, eby);
            path.line_to(clx, eby);     path.bezier_to(tlx, eby, elx, tby, elx, cby);
            path.line_to(elx, cty);     path.bezier_to(elx, tty, tlx, ety, clx, ety);
            path.line_to(crx, ety);     path.bezier_to(trx, ety, erx, tty, erx, cty);
        }   path.close(); */

        if self.base.is_ccw() {     let dir = femtovg::Solidity::Solid;
            //path.arc_to(erx, ety, crx, ety, radius);
            path.arc(crx, cty, radius,  0.,  PI / 2., dir);     path.line_to(clx, ety);
            //path.arc_to(elx, ety, elx, cty, radius);
            path.arc(clx, cty, radius,  PI / 2.,  PI, dir);     path.line_to(elx, cby);
            //path.arc_to(elx, eby, clx, eby, radius);
            path.arc(clx, cby, radius,  PI, -PI / 2., dir);     path.line_to(crx, eby);
            //path.arc_to(erx, eby, erx, cby, radius);
            path.arc(crx, cby, radius, -PI / 2.,  0., dir);     //path.line_to(erx, cty);
        } else {                    let dir = femtovg::Solidity::Hole; // XXX:
            path.line_to(erx, cby);     path.arc(crx, cby, radius,  0., -PI / 2., dir);
                                        //path.arc_to(erx, cby, crx, eby, radius);
            path.line_to(clx, eby);     path.arc(clx, cby, radius, -PI / 2.,  PI, dir);
                                        //path.arc_to(elx, eby, elx, cby, radius);
            path.line_to(elx, cty);     path.arc(clx, cty, radius,  PI,  PI / 2., dir);
                                        //path.arc_to(elx, ety, clx, ety, radius);
            path.line_to(crx, ety);     path.arc(crx, cty, radius,  PI / 2.,  0., dir);
                                        //path.arc_to(erx, ety, erx, cty, radius);
        }   path.close();   path
    }
}

impl PathFactory for Polystar {
    fn to_path(&self, fnth: f32) -> VGPath {
        let center = self.pos.get_value(fnth);
        let nvp = self.pt.get_value(fnth) as u32;
        let or  = self.or.get_value(fnth);
        let orr = self.os.get_value(fnth) * PI / 2. / 100. / nvp as f32;  // XXX:

        let ir  = self.ir.as_ref().map_or(0.,   // self.sy == StarType::Star
            |ir| ir.get_value(fnth));
        let irr = self.is.as_ref().map_or(0.,
            |is| is.get_value(fnth) * PI / 2. / 100. / nvp as f32);

        let mut angle = -PI / 2. + self.rotation.get_value(fnth) * PI / 180.;
        let angle_step = if matches!(self.sy, StarType::Star) { PI } else { PI * 2. } /
            if self.base.is_ccw() { -(nvp as f32) } else { nvp as _ };

        let (rpx, rpy) = (angle.cos() * or, angle.sin() * or); // rpt * or
        let (ptx, pty) = (center.x + rpx, center.y + rpy); // center + rpt
        let mut path = VGPath::new();   path.move_to(ptx, pty);

        let (mut lotx, mut loty) = (ptx - rpy * orr, pty + rpx * orr);
        let  mut add_bezier_to = |radius, rr| {
            angle += angle_step;    // rpt * radius

            let (rpx, rpy): (f32, _) = (angle.cos() * radius, angle.sin() * radius);
            let (ptx, pty) = (center.x + rpx, center.y + rpy); // center + rpt
            let (rdx, rdy) = (rpy * rr, -rpx * rr);

            path.bezier_to(lotx, loty, ptx + rdx, pty + rdy, ptx, pty); // pt + rd
            (lotx, loty) = (ptx - rdx, pty - rdy); // pt - rd
        };

        for _ in 0..nvp {
            if matches!(self.sy, StarType::Star) { add_bezier_to(ir, irr); }
            add_bezier_to(or, orr);
        }   path.close();   path
    }
}

impl PathFactory for Ellipse {
    fn to_path(&self, fnth: f32) -> VGPath {
        let center = self. pos.get_value(fnth);
        let radius = self.size.get_value(fnth) / 2.;
        //path.ellipse(center.x, center.y, radius.x, radius.y);   return;

        //  Approximate a circle with cubic Bézier curves
        //  https://spencermortensen.com/articles/bezier-circle/
        let tangent = radius * 0.5519;  // a magic number
        let (elx, ety) = (center.x -  radius.x, center.y -  radius.y); // center -  radius
        let (erx, eby) = (center.x +  radius.x, center.y +  radius.y); // center +  radius
        let (tlx, tty) = (center.x - tangent.x, center.y - tangent.y); // center - tangent
        let (trx, tby) = (center.x + tangent.x, center.y + tangent.y); // center + tangent

        let mut path = VGPath::new();   path.move_to(center.x, ety);
        if self.base.is_ccw() {
            path.bezier_to(tlx, ety, elx, tty, elx, center.y);
            path.bezier_to(elx, tby, tlx, eby, center.x, eby);
            path.bezier_to(trx, eby, erx, tby, erx, center.y);
            path.bezier_to(erx, tty, trx, ety, center.x, ety);
        } else {
            path.bezier_to(trx, ety, erx, tty, erx, center.y);
            path.bezier_to(erx, tby, trx, eby, center.x, eby);
            path.bezier_to(tlx, eby, elx, tby, elx, center.y);
            path.bezier_to(elx, tty, tlx, ety, center.x, ety);
        }   path.close();   path
    }
}

impl PathFactory for FreePath {
    fn to_path(&self, fnth: f32) -> VGPath {
        if !self.base.is_ccw() { return self.shape.to_path(fnth); }
        let curv = self.shape.get_value(fnth);
        debug_assert!(curv.vp.len() == curv.it.len() &&
                      curv.it.len() == curv.ot.len() && !curv.vp.is_empty());

        let fpt = curv.vp.last().unwrap();
        let mut path = VGPath::new();   path.move_to(fpt.x, fpt.y);

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
                fpt.x + curv.it[j].x, fpt.y + curv.it[j].y, fpt.x, fpt.y);
            path.close();
        }   path
    }
}

impl PathFactory for ShapeProperty {    // for mask
    fn to_path(&self, fnth: f32) -> VGPath {
        let curv = self.get_value(fnth);
        debug_assert!(curv.vp.len() == curv.it.len() &&
                      curv.it.len() == curv.ot.len() && !curv.vp.is_empty());

        let fpt = curv.vp.first().unwrap(); //&curv.vp[0];
        let mut path = VGPath::new();   path.move_to(fpt.x, fpt.y);

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
                fpt.x + curv.it[0].x, fpt.y + curv.it[0].y, fpt.x, fpt.y);
            path.close();
        }   path
    }
}

impl FillStrokeGrad {
    fn to_paint(&self, fnth: f32) -> VGPaint {
        fn convert_stops(stops: &[(f32, Rgba)], opacity: f32) -> Vec<(f32, VGColor)> {
            stops.iter().map(|(offset, rgba)| {
                let mut color = VGColor::rgba(rgba.r, rgba.g, rgba.b, rgba.a);
                color.set_alphaf(opacity * color.a);  (*offset, color)
            }).collect::<Vec<_>>()
        }

        let opacity = self.opacity.get_value(fnth) / 100.;
        let mut paint = match &self.grad {
            ColorGradEnum::Color(ColorWrapper { color }) => {
                let color = color.get_value(fnth); // RGB indeed
                VGPaint::color(VGColor::rgba(color.r, color.g, color.b, (opacity * 255.) as _))
            }
            ColorGradEnum::Gradient(grad) => {
                let sp = grad.sp.get_value(fnth);
                let ep = grad.ep.get_value(fnth);

                let stops = grad.stops.cl.get_value(fnth).0;
                debug_assert!(stops.len() as u32 == grad.stops.cnt);
                if matches!(grad.r#type, GradientType::Radial) {
                    /* let hl = grad.hl.as_ref().map_or(0.,
                        |hl| hl.get_value(fnth) * radius / 100.);
                    let ha = grad.ha.as_ref().map_or(0., |ha|
                        ha.get_value(fnth) * PI / 180.) + fast_atan2(dy, dx);

                    ctx.createRadialGradient(sp.x + ha.cos() * hl, sp.y + ha.sin() * hl, 0,
                        sp.x, sp.y, (ep.x - sp.x).hypot(ep.y - sp.y));
                        // XXX: femtovg::Paint doesn't support focal? */

                         VGPaint::radial_gradient_stops(sp.x, sp.y, 1., // 0.
                            (ep.x - sp.x).hypot(ep.y - sp.y), convert_stops(&stops, opacity))
                } else { VGPaint::linear_gradient_stops(sp.x, sp.y, ep.x, ep.y,
                            convert_stops(&stops, opacity))
                }
            }
        };

        use femtovg::{FillRule as FRule, LineJoin as LJoin, LineCap as LCap};
        match &self.base {
            FillStrokeEnum::FillRule(FillRuleWrapper { rule }) =>
                paint.set_fill_rule(match rule {
                    FillRule::NonZero => FRule::NonZero,
                    FillRule::EvenOdd => FRule::EvenOdd,
                }),
            FillStrokeEnum::Stroke(stroke) => {
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
                .map(|axis| axis.get_value(fnth) * PI / 180.);
            if let Some(axis) = axis { ts.rotate(-axis);   trfm.multiply(&ts); }

            let skew = (skew.get_value(fnth) * PI / -180.).tan();
            ts.skew_x(skew);    trfm.multiply(&ts);

            if let Some(axis) = axis { ts.rotate( axis);   trfm.multiply(&ts); }
        }

        match &self.extra {
            TransRotation::Normal2D { rotation: Some(rdegree) } => {
                ts.rotate(rdegree.get_value(fnth) * PI / 180.); trfm.multiply(&ts);
            }

            TransRotation::Split3D(_) => unimplemented!(), //debug_assert!(ddd);
            _ => (),
        }

        match &self.position {
            Some(Translation::Normal(apos)) => {
                let pos  = apos.get_value(fnth);
                if  ao.as_bool() &&  apos.animated.as_bool() {
                    let orient = pos - apos.get_value(fnth - 1.);
                    ts.rotate(fast_atan2(orient.y, orient.x));  trfm.multiply(&ts);
                }   trfm.multiply(&TM2D::new_translation(pos.x, pos.y));
            }

            Some(Translation::Split(sv)) => {   debug_assert!(sv.split);
                let pos = Vector2D { x: sv.x.get_value(fnth), y: sv.y.get_value(fnth) };
                if  ao.as_bool() {
                    let orient = Vector2D { x: pos.x - sv.x.get_value(fnth - 1.),
                                                      y: pos.y - sv.y.get_value(fnth - 1.) };
                    ts.rotate(fast_atan2(orient.y, orient.x));  trfm.multiply(&ts);
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
        } else { Vector2D { x: 0., y: 0. } };

        if  let Some(scale) = &self.scale {
            let scale = scale.get_value(fnth) / 100.;
            ts.scale(scale.x.powf(offset), scale.y.powf(offset));
            trfm.multiply(&ts);
        }

        if  let Some(skew) = &self.skew {
            let axis = self.skew_axis.as_ref()
                .map(|axis| axis.get_value(fnth) * PI / 180.);
            if let Some(axis) = axis { ts.rotate(-axis); trfm.multiply(&ts); }

            let skew = (skew.get_value(fnth) * offset * PI / -180.).tan();
            ts.skew_x(skew);    trfm.multiply(&ts); // XXX:

            if let Some(axis) = axis { ts.rotate( axis); trfm.multiply(&ts); }
        }

        match &self.extra {
            TransRotation::Normal2D { rotation: Some(rdegree) } => {
                ts.rotate(rdegree.get_value(fnth) * offset * PI / 180.);
                trfm.multiply(&ts);
            }

            TransRotation::Split3D(_) => unimplemented!(), //debug_assert!(ddd);
            _ => (),
        }

        let pos = match &self.position {
            Some(Translation::Normal(apos)) => apos.get_value(fnth),
            Some(Translation::Split(sv)) => {   debug_assert!(sv.split);
                Vector2D { x: sv.x.get_value(fnth), y: sv.y.get_value(fnth) }
            }   _ => Vector2D { x: 0., y: 0. },
        };  // XXX: shouldn't need to deal with auto orient?

        trfm.multiply(&TM2D::new_translation(pos.x * offset + anchor.x,
                                             pos.y * offset + anchor.y));   trfm
    }
}

struct TM2DwO(TM2D, f32);
impl   TM2DwO {
    #[inline] fn multiply(&mut self, other: &TM2DwO) {
        self.0  .multiply(&other.0);  self.1 *= other.1;
    }
}

use femtovg::{Canvas, Renderer, Transform2D as TM2D, CompositeOperation as CompOp,
    Path as VGPath, Paint as VGPaint, Color as VGColor};

impl Animation {
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

        let trfm = canvas.transform();
        let pt = trfm.transform_point(self.w as _, self.h as _);
        let ltrb = (trfm[4] as u32, trfm[5] as u32,
                pt.0.ceil() as u32,   pt.1.ceil() as u32); // viewport/viewbox

        canvas.clear_rect(//0, 0, canvas.width(), canvas.height(),
            ltrb.0, ltrb.1, ltrb.2 - ltrb.0, ltrb.3 - ltrb.1, VGColor::rgbf(0.4, 0.4, 0.4));
        self.render_layers(canvas, None, &self.layers, self.fnth);

        self.elapsed -= 1.;       self.fnth += 1.;
        if self.op <= self.fnth { self.fnth  = 0.; }    true
    }

    /// The render order goes from the last element to the first,
    /// items in list coming first will be rendered on top.
    fn render_layers<T: Renderer>(&self, canvas: &mut Canvas<T>,
        ptm: Option<&TM2DwO>, layers: &[LayersItem], fnth: f32) {
        let mut matte: Option<TrackMatte> = None;

        let get_matrix = |vl: &VisualLayer, fnth: f32| {
            let mut trfm  = vl.ks.to_matrix(fnth, vl.ao);
            if let Some(pid) = vl.base.parent {
                let ptm = layers.iter().find_map(|layer|
                    get_visual_layer(layer).and_then(|vl|
                        vl.base.ind.and_then(|ind| if ind == pid {
                            Some(vl.ks.to_matrix(fnth, vl.ao)) } else { None })));

                if let Some(ptm) = &ptm { trfm.multiply(ptm) } else { unreachable!() }
            };  if let Some(ptm) =  ptm { trfm.multiply(ptm) }  trfm
        };

        #[inline] fn get_visual_layer(layer: &LayersItem) -> Option<&VisualLayer> {
            Some(match layer {
                LayersItem::PrecompLayer(layer) => &layer.vl,
                LayersItem::SolidColor(layer) => &layer.vl,
                LayersItem::Shape(layer) => &layer.vl,

                LayersItem::Image(layer) => &layer.vl,
                LayersItem::Text(layer) => &layer.vl,
                LayersItem::Data(layer) => &layer.vl,

                LayersItem::Null(null) => null,
                LayersItem::Audio(_) | LayersItem::Camera(_) => return None,
            })
        }

        fn need_to_hide(vl: &VisualLayer, fnth: f32) -> bool {
            vl.base.hd || fnth < vl.base.ip || vl.base.op <= fnth || fnth < vl.base.st
        }

        for layer in layers.iter().rev() { match layer {
                LayersItem::Shape(shpl) => if !need_to_hide(&shpl.vl, fnth) {
                    let trfm = get_matrix(&shpl.vl, fnth);

                    prepare_matte(canvas, &shpl.vl, &mut matte);
                    Self::render_shapes(canvas, &trfm, &shpl.shapes, fnth, shpl.vl.ao);
                     render_matte(canvas, &shpl.vl, &mut matte, fnth);
                }
                LayersItem::PrecompLayer(pcl) => if !need_to_hide(&pcl.vl, fnth) {
                    if let Some(pcomp) = self.assets.iter().find_map(|asset|
                        match asset { AssetsItem::Precomp(pcomp)
                            if pcomp.base.id == pcl.rid => Some(pcomp), _ => None }) {
                        let fnth = (fnth - pcl.vl.base.st) / pcl.vl.base.sr;

                        let trfm = get_matrix(&pcl.vl, fnth);
                        let fnth = pcl.tm.as_ref().map_or(fnth, // handle time remapping
                            |tm| tm.get_value(fnth) * pcomp.fr);

                        prepare_matte(canvas, &pcl.vl, &mut matte);
                        self.render_layers(canvas, Some(&trfm), &pcomp.layers, fnth);
                         render_matte(canvas, &pcl.vl, &mut matte, fnth);
                    }   // clipping(pcl.w, pcl.h)?
                }
                LayersItem::SolidColor(scl) => if !need_to_hide(&scl.vl, fnth) {
                    let trfm = get_matrix(&scl.vl, fnth);
                    prepare_matte(canvas, &scl.vl, &mut matte);

                    let mut path = VGPath::new();
                    path.rect((self.w as f32 - scl.sw) / 2., // 0., 0.,
                              (self.h as f32 - scl.sh) / 2., scl.sw, scl.sh);
                    let paint = VGPaint::color(VGColor::rgb(scl.sc.r, scl.sc.g, scl.sc.b));

                    let last_trfm =    canvas.transform();
                    canvas.set_transform(&trfm.0);  canvas.fill_path(&path, &paint);
                    canvas.reset_transform();       canvas.set_transform(&last_trfm);
                     render_matte(canvas, &scl.vl, &mut matte, fnth);
                }
                LayersItem::Image(_) | LayersItem::Text(_)  | LayersItem::Data(_)  |
                LayersItem::Audio(_) | LayersItem::Camera(_) => dbg!(),

                //LayersItem::Null(_) => (), // nothing to do except for get_parent_trfm
                _ => (),
        } }
    }

    fn render_shapes<T: Renderer>(canvas: &mut Canvas<T>,
        ptm: &TM2DwO, coll: &[ShapeListItem], fnth: f32, ao: IntBool) {
        let (mut path, mut trfm) = (vec![], None);
        let (mut fillp, mut linep) = (vec![], vec![]);
        let (mut dash, mut repeater) = ((0., vec![]), vec![]);
        let  mut trim = None;

        // convert shape to path, convert_style(fill/stroke/gradient)
        // render path of shape with trfm and style to screen/pixmap
        for shape in coll.iter().rev() { match shape {
                ShapeListItem::Rectangle(rect)
                    if !rect.base.elem.hd => path.push(Box::new(rect.to_path(fnth))),
                ShapeListItem::Polystar(star)
                    if !star.base.elem.hd => path.push(Box::new(star.to_path(fnth))),
                ShapeListItem::Ellipse(elps)
                    if !elps.base.elem.hd => path.push(Box::new(elps.to_path(fnth))),
                ShapeListItem::Path(curv)
                    if !curv.base.elem.hd => path.push(Box::new(curv.to_path(fnth))),

                ShapeListItem::Fill(fill)
                    if !fill.elem.hd => fillp.push(fill.to_paint(fnth)),
                ShapeListItem::Stroke(line) if !line.elem.hd => {
                    let paint = line.to_paint(fnth);    dash = line.get_dash(fnth);
                        linep.push(paint);       debug_assert!(dash.1.len() < 5);
                }
                ShapeListItem::NoStyle(_) => eprintln!("What need to do here?"),
                ShapeListItem::GradientFill(grad)
                    if !grad.elem.hd => fillp.push(grad.to_paint(fnth)),
                ShapeListItem::GradientStroke(grad)
                    if !grad.elem.hd => linep.push(grad.to_paint(fnth)),

                ShapeListItem::Repeater(mdfr)
                    if !mdfr.elem.hd => repeater = repeat_shape(mdfr, fnth),
                ShapeListItem::Trim(mdfr) if !mdfr.elem.hd => trim = Some(mdfr),

                ShapeListItem::RoundedCorners(_) |  //round_corners(&mdfr, &mut path, fnth);
                ShapeListItem::PuckerBloat(_) | ShapeListItem::Twist(_) |
                ShapeListItem::OffsetPath(_)  | ShapeListItem::Merge(_) |
                ShapeListItem::ZigZag(_) => dbg!(),

                ShapeListItem::Group(group) if !group.elem.hd => {
                    Self::render_shapes(canvas, trfm.as_ref().unwrap_or(ptm),
                        &group.shapes, fnth, ao);
                }
                ShapeListItem::GroupTransform(ts) if !ts.elem.hd => {
                    let mut tm = ts.trfm.to_matrix(fnth, ao);
                    tm.multiply(ptm);  trfm = Some(tm);
                }
                _ => (),
        } }

        if !dash.1.is_empty() { for path in path.iter_mut() {
            let mut npath = VGPath::new();
            kurbo::dash(path.verbs().map(convert_path_f2k), dash.0 as _, &dash.1.iter()
                .map(|v| *v as f64).collect::<Vec<_>>())
                .for_each(|el| convert_path_k2f(el, &mut npath));
            *path = Box::new(npath);
        } }

        if let Some(mdfr) = trim {
            if mdfr.multiple.is_some_and(|ml|
                matches!(ml, TrimMultiple::Simultaneously)) {
                path.iter_mut().for_each(|path|
                    *path =  Box::new(trim_path(mdfr, path.verbs(), fnth)));
            } else {
                path = vec![ Box::new(trim_path(mdfr,
                    path.iter().rev().flat_map(|path| path.verbs()), fnth)) ];
            }
        }

        let trfm = trfm.as_ref().unwrap_or(ptm);    canvas.save();
        if repeater.is_empty() {    // XXX: execute in order of fill/stroke layer?
            canvas.set_global_alpha(trfm.1);      canvas.set_transform(&trfm.0);
            for path in path.iter().rev() {
                fillp.iter().for_each(|paint| canvas.  fill_path(path, paint));
                linep.iter().for_each(|paint| canvas.stroke_path(path, paint));
            }
        }   let last_trfm = canvas.transform();

        repeater.into_iter().rev().for_each(|mut ts| {  ts.multiply(trfm);
            canvas.set_global_alpha(ts.1);        canvas.set_transform(&ts.0);
            for path in path.iter().rev() {
                fillp.iter().for_each(|paint| canvas.  fill_path(path, paint));
                linep.iter().for_each(|paint| canvas.stroke_path(path, paint));
            }   canvas.reset_transform();             canvas.set_transform(&last_trfm);
        });     canvas.restore();
    }
}

struct TrackMatte { mode: MatteMode, mlid: Option<u32>, imgid: femtovg::ImageId, }

fn prepare_matte<T: Renderer>(canvas: &mut Canvas<T>,
    vl: &VisualLayer, matte: &mut Option<TrackMatte>) {
    if vl.tt.is_some() || vl.has_mask {
        let (w, h) = (canvas.width(), canvas.height());
        let imgid = canvas.create_image_empty(w as _, h as _,
            femtovg::PixelFormat::Rgba8, femtovg::ImageFlags::FLIP_Y).unwrap();

        canvas.set_render_target(femtovg::RenderTarget::Image(imgid));
        let (lx, ty) = canvas.transform().transform_point(0., 0.);
        let (lx, ty) = (lx as u32, ty as u32);  // limit to viewport/viewbox
        canvas.clear_rect(lx, ty, w - lx * 2, h - ty * 2, VGColor::rgbaf(0., 0., 0., 0.));

        *matte = Some(TrackMatte {
            mode: vl.tt.unwrap_or(MatteMode::Normal), mlid: vl.tp, imgid });
    } else if let Some(matte) = matte {     //canvas.restore();
        match matte.mode {  MatteMode::Normal => (),
            MatteMode::Alpha => // XXX: femtovg seems not work correctly for DstIn
                canvas.global_composite_operation(CompOp::DestinationIn),
            MatteMode::InvertedAlpha =>
                canvas.global_composite_operation(CompOp::DestinationOut),
            MatteMode::Luma | MatteMode::InvertedLuma => unimplemented!(),
            // https://developer.mozilla.org/en-US/docs/Web/API/CanvasRenderingContext2D/globalCompositeOperation
        }

        if vl.base.ind.is_some_and(|ind|
            matte.mlid.is_some_and(|mlid| ind != mlid)) {
            eprintln!("Unexpected matte layer structure!");
        }   //debug_assert!(vl.td.is_some_and(|td| td.as_bool()));
    }
}

fn render_masks<T: Renderer>(canvas: &mut Canvas<T>, masks_prop: &[Mask], fnth: f32) {
    masks_prop.iter().for_each(|mask| {
        let cop = if mask.inv { match mask.mode {
            MaskMode::Subtract  => Some(CompOp::DestinationIn),
            MaskMode::Add       => Some(CompOp::DestinationOut),
            MaskMode::Intersect => Some(CompOp::DestinationAtop),
            MaskMode::Lighten   => Some(CompOp::Lighter),
            MaskMode::Darken | MaskMode::Difference => unimplemented!(),
            MaskMode::None => None,
        } } else { match mask.mode {
            MaskMode::Add       => Some(CompOp::DestinationIn),
            MaskMode::Subtract  => Some(CompOp::DestinationOut),
            MaskMode::Intersect => Some(CompOp::DestinationAtop),
            MaskMode::Lighten   => Some(CompOp::Lighter),
            MaskMode::Darken | MaskMode::Difference => unimplemented!(),
            MaskMode::None => None,
        } };

        if let Some(_expand) = &mask.expand { todo!() }
        let opacity = mask.opacity.as_ref().map_or(1., |opacity|
            opacity.get_value(fnth) / 100.);    //canvas.set_global_alpha(opacity);

        if let Some(cop) = cop { canvas.global_composite_operation(cop); }
        canvas.fill_path(&mask.shape.to_path(fnth),
            &VGPaint::color(VGColor::rgbaf(0., 0., 0., opacity)));
    }); //canvas.set_global_alpha(1.);  // restore global alpha
}

fn render_matte<T: Renderer>(canvas: &mut Canvas<T>,
    vl: &VisualLayer, matte: &mut Option<TrackMatte>, fnth: f32) {
    if !vl.has_mask && (vl.tt.is_some() || matte.is_none()) { return }
    let imgid = matte.as_ref().unwrap().imgid;  *matte = None;

    if  vl.has_mask { render_masks(canvas, &vl.masks_prop, fnth); }
    canvas.global_composite_operation(CompOp::SourceOver);
    canvas.set_render_target(femtovg::RenderTarget::Screen);    //canvas.restore();

    let last_trfm = canvas.transform();
    let (lx, ty) = last_trfm.transform_point(0., 0.);
    let (w, h) = canvas.image_size(imgid).unwrap();

    let mut path = VGPath::new();
    path.rect(lx, ty, w as f32 - lx * 2., h as f32 - ty * 2.);  // XXX:
    let paint = VGPaint::image(imgid, 0., 0., w as _, h as _, 0., 1.);

    canvas.reset_transform();           canvas.fill_path(&path, &paint);
    canvas.set_transform(&last_trfm);   canvas.flush();     canvas.delete_image(imgid);
}

fn repeat_shape(mdfr: &Repeater, fnth: f32) -> Vec<TM2DwO> {
    let mut opacity = mdfr.tr.so.as_ref().map_or(1.,
        |so| so.get_value(fnth) / 100.);
    let offset = mdfr.offset.as_ref().map_or(0.,
        |offset| offset.get_value(fnth));   // range [-1, 2]?

    let cnt = mdfr.cnt.get_value(fnth);
    let delta = (mdfr.tr.eo.as_ref().map_or(1.,
        |eo| eo.get_value(fnth) / 100.) - opacity) / cnt;

    let mut coll = vec![];
    for i in 0..cnt as u32 {    let i = i as f32;   opacity += delta;
        coll.push(TM2DwO(mdfr.tr.trfm.to_repeat_trfm(fnth, offset + i), opacity));
    }   if matches!(mdfr.order, Composite::Below) { coll.reverse(); }   coll
}

fn trim_path<I: Iterator<Item = Verb>>(mdfr: &TrimPath, path: I, fnth: f32) -> VGPath {
    // https://lottiefiles.github.io/lottie-docs/scripts/lottie_bezier.js
    // or use curve_length(curve, merr) and subdivide(t, seg) of flo_curves
    let offset = mdfr.offset.get_value(fnth) / 360.;
    let start  = mdfr. start.get_value(fnth) / 100.;
    let trim   = mdfr.   end.get_value(fnth) / 100. - start;
    debug_assert!(start + offset < 2. && (0.0..=1.).contains(&trim));

    let accuracy = 1e-2;
    use kurbo::{BezPath, ParamCurve, ParamCurveArclen};
    //let segments = kurbo::segments(path.map(convert_path_f2k));
    let path = path.map(convert_path_f2k).collect::<BezPath>();

    let total = path.segments().fold(0., |acc, seg| acc + seg.arclen(accuracy));
    let (start, mut slen) = (total * ((start + offset) % 1.)  as f64, 0.);
    let (mut fpath, mut trim) = (VGPath::new(), total * trim as f64);

    BezPath::from_path_segments(path.segments().chain(path.segments()).filter_map(|seg| {
        let len = seg.arclen(accuracy);

        let range = if slen <= start && start < slen + len {
            let start = start - slen;   let end = start + trim;
            if  end < len { trim = 0.;      start / len .. end / len
            } else { trim -= len - start;   start / len .. 1. }
        } else if start < slen && 0. < trim {
            if trim < len { let end = trim / len;
                     trim  = 0.;    0.0 .. end
            } else { trim -= len;   0.0 .. 1. }
        } else {     slen += len;   return None };

        slen += len;    Some(seg.subsegment(range))
    })).iter().for_each(|el| convert_path_k2f(el, &mut fpath));  fpath
}

use {kurbo::PathEl, femtovg::Verb};
#[inline] fn convert_path_f2k(verb: Verb) -> PathEl { match verb {
    Verb::MoveTo(x, y) => PathEl::MoveTo((x as f64, y as f64).into()),
    Verb::LineTo(x, y) => PathEl::LineTo((x as f64, y as f64).into()),
    Verb::BezierTo(ox, oy, ix, iy, x, y) =>
        PathEl::CurveTo((ox as f64, oy as f64).into(),
                        (ix as f64, iy as f64).into(), (x as f64, y as f64).into()),
    Verb::Solid | Verb::Hole => unreachable!(),
    Verb::Close => PathEl::ClosePath,
} }

#[inline] fn convert_path_k2f(elem: PathEl, path: &mut VGPath) { match elem {
    PathEl::MoveTo(pt) => path.move_to(pt.x as _, pt.y as _),
    PathEl::LineTo(pt) => path.line_to(pt.x as _, pt.y as _),
    PathEl::CurveTo(ot, it, pt) =>
        path.bezier_to(ot.x as _, ot.y as _, it.x as _, it.y as _, pt.x as _, pt.y as _),
    PathEl::QuadTo(_, _) => unreachable!(),
    PathEl::ClosePath => path.close(),
} }

