/****************************************************************
 * $ID: rive.rs  	Mon 13 May 2024 15:34:52+0800               *
 *                                                              *
 * Maintainer: 范美辉 (MeiHui FAN) <mhfan@ustc.edu>              *
 * Copyright (c) 2024 M.H.Fan, All rights reserved.             *
 ****************************************************************/

use crate::core::helpers::Vec2D;
use rive_rs::{path as rpath, Scene, Instantiate, File, Artboard, Handle,
    renderer::{self, PaintStyle, BlendMode, BufferType, BufferFlags}
};
use femtovg::{renderer::SurfacelessRenderer as Renderer, FillRule,
    Transform2D as TM2D, Path as VGPath, Paint as VGPaint};

pub struct RiveNVG<T: Renderer + 'static> {
    canvas: &'static mut femtovg::Canvas<T>,
    opacity: Vec<f32>,
}

impl<T: Renderer> RiveNVG<T> {  /// # Safety
    /// The returned adapter must not outlive `canvas`, and no other code may access the
    /// canvas while the adapter is in use. `rive-rs::Renderer` currently requires `'static`,
    /// so this invariant cannot be expressed in the type.
    pub unsafe fn new(canvas: &mut femtovg::Canvas<T>) -> Self {
        #[allow(clippy::missing_transmute_annotations)] Self {
            canvas: unsafe { std::mem::transmute(canvas) }, // force pretend to be 'static
            opacity: vec![1.],
        }
    }

    pub fn new_scene(file: &[u8]) -> Option<Box<dyn Scene<Self>>> {
        File::new(file).ok().and_then(|riv|
            Artboard::instantiate(&riv, Handle::Default).and_then(|art|
            Box::<_>::instantiate(&art, Handle::Default)))
            //println!("Load scene: {}, {}x{}, {}s", scene.name(), scene.width(),
            //    scene.height(), scene.duration().map_or(0., |dur| dur.as_secs_f32()));
    }
}

impl<T: Renderer + 'static> renderer::Renderer for RiveNVG<T> { // aka Femtovg
    type Path  = Path;
    type Paint = Paint;
    type Gradient = Gradient;
    type Buffer = Buffer;
    type Image  = Image;

    fn state_push(&mut self) { self.canvas.save();
        self.opacity.push(*self.opacity.last().unwrap_or(&1.));
    }
    fn state_pop (&mut self) { self.canvas.restore();
        if 1 < self.opacity.len() { self.opacity.pop(); }
    }
    fn set_clip  (&mut self, _path: &Self::Path) { }  // XXX: not capable

    fn transform(&mut self, trfm: &[f32; 6]) {
        //let trfm = unsafe { &*(trfm.as_ptr() as *const TM2D) };
        self.canvas.set_transform(
            &TM2D::new(trfm[0], trfm[1], trfm[2], trfm[3], trfm[4], trfm[5]));
    }

    fn modulate_opacity(&mut self, opacity: f32) {
        let value = (self.opacity.last().copied().unwrap_or(1.) * opacity).max(0.);
        *self.opacity.last_mut().unwrap() = value;
        self.canvas.set_global_alpha(value);
    }

    fn draw_path(&mut self, path: &Self::Path, paint: &Self::Paint) {
        let inner = if path.1 != FillRule::NonZero {
            Some(paint.inner.clone().with_fill_rule(path.1)) } else { None };
        //if paint.bm != BlendMode::SrcOver { }     // XXX: not capable

        match paint.style {
            PaintStyle::Fill   => self.canvas.fill_path(&path.0,
                                          inner.as_ref().unwrap_or(&paint.inner)),
            PaintStyle::Stroke => self.canvas.stroke_path(&path.0, &paint.inner),
        }
    }

    fn draw_image(&mut self, img: &Self::Image, _bm: BlendMode, opacity: f32) {
        //if bm != BlendMode::SrcOver { }   // XXX: not capable
        let canvas = &mut self.canvas;

        let Ok(imgid) = canvas.load_image_mem(&img.0,
            femtovg::ImageFlags::FLIP_Y) else { return };
        let Ok((w, h)) = canvas.image_size(imgid) else {
            canvas.delete_image(imgid); return
        };  let (w, h) = (w as _, h as _);

        let paint = VGPaint::image(imgid, 0., 0., w, h, 0., opacity);
        let mut path = VGPath::new();    path.rect(w / -2., h / -2., w, h);
        canvas.fill_path(&path, &paint); canvas.flush();
        canvas.delete_image(imgid);
    }

    fn draw_image_mesh(&mut self, img: &Self::Image, vertices: &Self::Buffer,
        uvs: &Self::Buffer, indices: &Self::Buffer, _bm: BlendMode, opacity: f32) {
        if vertices.0.len() % 8 != 0 || vertices.0.len() != uvs.0.len() ||
            indices.0.len() % 6 != 0 { return }
        let decode_points = |data: &[u8]| data.chunks_exact(8).map(|chunk| (
            f32::from_ne_bytes(chunk[0..4].try_into().unwrap()),
            f32::from_ne_bytes(chunk[4..8].try_into().unwrap()),
        )).collect::<Vec<_>>();
        let (vtx, uvs) = (decode_points(&vertices.0), decode_points(&uvs.0));
        let indices = indices.0.chunks_exact(2).map(|chunk|
            u16::from_ne_bytes(chunk.try_into().unwrap())).collect::<Vec<_>>();
        if  indices.iter().any(|&idx| vtx.len() <= idx as usize ||
                                      uvs.len() <= idx as usize) { return; }

        let canvas = &mut self.canvas;
        let Ok(imgid) = canvas.load_image_mem(&img.0,
            femtovg::ImageFlags::FLIP_Y) else { return };
        let Ok((w, h)) = canvas.image_size(imgid) else {
            canvas.delete_image(imgid); return
        };
        let (w, h) = (w as _, h as _);

        let paint = VGPaint::image(imgid, 0., 0., w, h, 0., opacity);
        let last_trfm = canvas.transform();     //canvas.save();
        //if bm != BlendMode::SrcOver { }   // XXX: not capable

        for idx in indices.chunks_exact(3) {
            let mut path = VGPath::new();

            let pt = vtx[idx[2] as usize];    path.move_to(pt.0, pt.1); // start from last point
            let mesh = idx.iter().map(|idx| {
                let idx = *idx as usize;
                let (pt, tp) = (vtx[idx], uvs[idx]);
                let tp = (tp.0 * w, tp.1 * h);

                path.line_to(pt.0, pt.1);

                (Vec2D { x: pt.0, y: pt.1 }, Vec2D { x: tp.0, y: tp.1 })
            }).collect::<Vec<_>>();

            let Some(mapping) = simplex_affine_mapping(&mesh) else { continue };
            canvas.set_transform(&mapping);     canvas.fill_path(&path, &paint);
            canvas.reset_transform();           canvas.set_transform(&last_trfm);
        }
        canvas.flush();     canvas.delete_image(imgid);
    }
}

#[derive(Default)] pub struct Path(VGPath, FillRule);

impl renderer::Path for Path {
    fn new(cmds: &mut rpath::Commands, rule: rpath::FillRule) -> Self {
        let mut path = Self::default();
        for (verb, points) in cmds { match verb {
            rpath::Verb::Close => path.close(),
            rpath::Verb::Move  => path. move_to(points[0].x, points[0].y),
            rpath::Verb::Line  => path. line_to(points[0].x, points[0].y),
            rpath::Verb::Cubic => path.cubic_to(points[0].x, points[0].y,
                    points[1].x, points[1].y,   points[2].x, points[2].y),
        }}  path.set_fill_rule(rule);   path
    }

    fn extend(&mut self, from: &Self, trfm: &[f32; 6]) {    use femtovg::Verb;
        if  trfm == &[1., 0., 0., 1., 0., 0.] { // identity
            from.0.verbs().for_each(|verb| match verb {
                Verb::MoveTo(x, y) => self.move_to(x, y),
                Verb::LineTo(x, y) => self.line_to(x, y),
                Verb::BezierTo(ox, oy, ix, iy, x, y) =>
                    self.cubic_to(ox, oy, ix, iy, x, y),
                Verb::Solid | Verb::Hole => unreachable!(),
                Verb::Close => self.close(),
            });
        } else {
            //let trfm = unsafe { &*(trfm.as_ptr() as *const TM2D) };
            let trfm = TM2D::new(trfm[0], trfm[1], trfm[2], trfm[3], trfm[4], trfm[5]);
            from.0.verbs().for_each(|verb| match verb {
                Verb::MoveTo(x, y) => {
                    let pt = trfm.transform_point(x, y);
                    self.move_to(pt.0, pt.1);
                }
                Verb::LineTo(x, y) => {
                    let pt = trfm.transform_point(x, y);
                    self.line_to(pt.0, pt.1);
                }
                Verb::BezierTo(ox, oy, ix, iy, x, y) => {
                    let ot = trfm.transform_point(ox, oy);
                    let it = trfm.transform_point(ix, iy);
                    let pt = trfm.transform_point( x,  y);
                    self.cubic_to(ot.0, ot.1, it.0, it.1, pt.0, pt.1);
                }
                Verb::Solid | Verb::Hole => unreachable!(),
                Verb::Close => self.close(),
            });
        }
    }

    fn reset(&mut self) { self.0 = VGPath::new(); }
    fn set_fill_rule(&mut self, rule: rpath::FillRule) {
        self.1 = match rule {
            rpath::FillRule::NonZero => FillRule::NonZero,
            rpath::FillRule::EvenOdd => FillRule::EvenOdd,
        };
    }

    fn    close(&mut self) { self.0.close(); }
    fn  move_to(&mut self,  x: f32,  y: f32) {  self.0.move_to(x, y); }
    fn  line_to(&mut self,  x: f32,  y: f32) {  self.0.line_to(x, y); }
    fn cubic_to(&mut self, ox: f32, oy: f32, ix: f32, iy: f32, x: f32, y: f32) {
        self.0. bezier_to(ox, oy, ix, iy, x, y);
    }
}

pub struct Paint { bm: BlendMode, style: PaintStyle, inner: VGPaint, }

impl Default for Paint {
    fn default() -> Self { Self {
        bm: BlendMode::SrcOver, style: PaintStyle::Fill, inner: Default::default()
    } }
}

fn to_femtovg_color(color: renderer::Color) -> femtovg::Color {
    femtovg::Color::rgba(color.r, color.g, color.b, color.a)
}

impl renderer::Paint for Paint {    type Gradient = Gradient;
    fn set_style(&mut self, style: PaintStyle) { self.style = style; }
    fn set_thickness(&mut self, thick: f32) { self.inner.set_line_width(thick); }

    fn set_join(&mut self, join: renderer::StrokeJoin) {  use femtovg::LineJoin;
        self.inner.set_miter_limit(4.0);
        self.inner.set_line_join(match join {
            renderer::StrokeJoin::Miter => LineJoin::Miter,
            renderer::StrokeJoin::Round => LineJoin::Round,
            renderer::StrokeJoin::Bevel => LineJoin::Bevel,
        });
    }

    fn set_cap (&mut self,  cap: renderer::StrokeCap) {   use femtovg::LineCap;
        self.inner.set_line_cap(match cap {
            renderer::StrokeCap::Butt   => LineCap::Butt,
            renderer::StrokeCap::Round  => LineCap::Round,
            renderer::StrokeCap::Square => LineCap::Square,
        });
    }

    fn set_color(&mut self, color: renderer::Color) {
        self.inner.set_color(to_femtovg_color(color));
    }
    fn set_gradient(&mut self, grad: &Self::Gradient) {
        let stops = grad.stops.iter()
            .map(|&(offset, color)| (offset, to_femtovg_color(color)));

        let mut paint = match grad.base {
            GradientBase::Linear { sx, sy, ex, ey } =>
                VGPaint::linear_gradient_stops(sx, sy, ex, ey, stops),
            GradientBase::Radial { cx, cy, radius } =>
                VGPaint::radial_gradient_stops(cx, cy, 0., radius, stops),
        };

        // XXX: in case set_gradient was not called at first?
        paint.set_line_width(self.inner.line_width());
        paint.set_line_join (self.inner.line_join());
        paint.set_miter_limit(self.inner.miter_limit());
        //paint.set_fill_rule (self.inner.fill_rule());     // never called
        paint.set_line_cap  (self.inner.line_cap_start());  self.inner = paint;
    }

    fn set_blend_mode(&mut self, bm: BlendMode) { self.bm = bm; }
    fn invalidate_stroke(&mut self) { } // not needed in femtovg?
}

enum GradientBase {
    Linear { sx: f32, sy: f32, ex: f32, ey: f32 },
    Radial { cx: f32, cy: f32, radius: f32/*, fx: f32, fy: f32, r1: f32*/ },
}

pub struct Gradient {
    base: GradientBase,
    stops: Vec<(f32, renderer::Color)>,
}

impl renderer::Gradient for Gradient {
    fn new_linear(sx: f32, sy: f32, ex: f32, ey: f32,
        colors: &[renderer::Color], stops: &[f32]) -> Self { Self {
            base: GradientBase::Linear { sx, sy, ex, ey },
            stops: stops.iter().copied().zip(colors.iter().copied()).collect(),
    } } //debug_assert!(stops.len() == colors.len());

    fn new_radial(cx: f32, cy: f32, radius: f32,
        colors: &[renderer::Color], stops: &[f32]) -> Self { Self {
            base: GradientBase::Radial { cx, cy, radius },
            stops: stops.iter().copied().zip(colors.iter().copied()).collect(),
    } }
}

pub struct Buffer(Vec<u8>);
pub struct Image(Vec<u8>);

impl renderer::Buffer for Buffer {
    fn new(_: BufferType, _: BufferFlags, len: usize) -> Self { Self(vec![0; len]) }
    fn map(&mut self) -> &mut [u8] { &mut self.0 }
    fn unmap(&mut self) {}
}

impl renderer::Image for Image {
    fn decode(data: &[u8]) -> Option<Self> {
        image::load_from_memory(data).ok()?;
        Some(Self(data.to_vec()))
    }
}

/// Finds the affine transform that maps triangle `from` to triangle `to`, or `None` for
/// a degenerate source triangle. The algorithm is based on the [Simplex Affine Mapping]
/// method which has a [Swift implementation].
///
/// [Simplex Affine Mapping]: https://www.researchgate.net/publication/332410209_Beginner%27s_guide_to_mapping_simplexes_affinely
/// [Swift implementation]: https://rethunk.medium.com/finding-an-affine-transform-using-three-2d-point-correspondences-using-simplex-affine-mapping-255aeb4e8055
fn simplex_affine_mapping(mesh: &[(Vec2D, Vec2D)]) -> Option<TM2D> {
    let ((a, d), (b, e), (c, f)) = (mesh[0], mesh[1], mesh[2]);

    let det = a.x * b.y + b.x * c.y + c.x * a.y -
              a.x * c.y - b.x * a.y - c.x * b.y;
    if !det.is_finite() || det.abs() <= f32::EPSILON { return None }
    let det_recip = det.recip();

    let p = (d * (b.y - c.y) - e * (a.y - c.y) + f * (a.y - b.y)) * det_recip;
    let q = (e * (a.x - c.x) - d * (b.x - c.x) - f * (a.x - b.x)) * det_recip;

    let t = (d * (b.x * c.y - b.y * c.x) - e * (a.x * c.y - a.y * c.x) +
                    f * (a.x * b.y - a.y * b.x)) * det_recip;

    Some(TM2D::new(p.x, p.y, q.x, q.y, t.x, t.y))
}
