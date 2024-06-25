/****************************************************************
 * $ID: rive.rs  	Mon 13 May 2024 15:34:52+0800               *
 *                                                              *
 * Maintainer: 范美辉 (MeiHui FAN) <mhfan@ustc.edu>              *
 * Copyright (c) 2024 M.H.Fan, All rights reserved.             *
 ****************************************************************/

use crate::helpers::Vec2D;
use rive_rs::{path as rpath, Scene, Instantiate, File, Artboard, Handle,
    renderer::{self, PaintStyle, BlendMode, BufferType, BufferFlags}};
use femtovg::{Renderer, FillRule, CompositeOperation as CompOp,
    Transform2D as TM2D, Path as VGPath, Paint as VGPaint};

pub struct NanoVG<T: Renderer + 'static>(&'static mut femtovg::Canvas<T>);

impl<T: Renderer> NanoVG<T> {
    #[inline] pub fn new(canvas: &mut femtovg::Canvas<T>) -> Self {
        Self(unsafe { std::mem::transmute(canvas) }) // force pretend to be 'static
    }

    pub fn new_scene(file: &[u8]) -> Option<Box<dyn Scene<Self>>> {
        File::new(file).ok().and_then(|riv|
            Artboard::instantiate(&riv, Handle::Default).and_then(|abd|
            Box::<_>::instantiate(&abd, Handle::Default)))
            //println!("Load scene: {}, {}x{}, {}s", scene.name(), scene.width(),
            //    scene.height(), scene.duration().map_or(0., |dur| dur.as_secs_f32()));
    }
}

impl<T: Renderer + 'static> renderer::Renderer for NanoVG<T> { // aka Femtovg
    type Path  = Path;
    type Paint = Paint;
    type Gradient = Gradient;
    type Buffer = Buffer;
    type Image  = Image;

    #[inline] fn state_push(&mut self) { self.0.save(); }
    #[inline] fn state_pop (&mut self) { self.0.restore(); }
    #[inline] fn set_clip  (&mut self, _path: &Self::Path) { }  // XXX: not capable

    #[inline] fn transform(&mut self, trfm: &[f32; 6]) {
        let trfm = unsafe { &*(trfm.as_ptr() as *const TM2D) };
        self.0.set_transform(trfm);
    }

    #[inline] fn draw_path(&mut self, path: &Self::Path, paint: &Self::Paint) {
        let inner = if path.1 != FillRule::NonZero {
            Some(paint.inner.clone().with_fill_rule(path.1)) } else { None };
        //if paint.bm != BlendMode::SrcOver { }     // XXX: not capable

        match paint.style {
            PaintStyle::Fill   => self.0.  fill_path(&path.0,
                                    inner.as_ref(). unwrap_or(&paint.inner)),
            PaintStyle::Stroke => self.0.stroke_path(&path.0, &paint.inner),
        }
    }

    fn draw_image(&mut self, img: &Self::Image, _bm: BlendMode, opacity: f32) {
        //if bm != BlendMode::SrcOver { }   // XXX: not capable
        let canvas = &mut self.0;

        let data = unsafe { std::slice::from_raw_parts(img.0, img.1 as _) };
        let imgid = canvas.load_image_mem(data, femtovg::ImageFlags::FLIP_Y).unwrap();
        let (w, h) = canvas.image_size(imgid).unwrap();
        let (w, h) = (w as _, h as _);  // XXX: need to test and check

        let paint = VGPaint::image(imgid, 0., 0., w, h, 0., opacity);
        let mut path = VGPath::new();    path.rect(w / -2., h / -2., w, h);
        canvas.fill_path(&path, &paint);    canvas.flush();     canvas.delete_image(imgid);
    }

    fn draw_image_mesh(&mut self, img: &Self::Image, vertices: &Self::Buffer,
        uvs: &Self::Buffer, indices: &Self::Buffer, _bm: BlendMode, opacity: f32) {
        //debug_assert!(vertices.0.len() % 8 == 0 && uvs.0.len() % 8 == 0 &&
        //              vertices.0.len() == uvs.0.len() && indices.0.len() % 6 == 0);

        let vtx = unsafe { std::slice::from_raw_parts(
            vertices.0.as_ptr() as *const (f32, f32), vertices.0.len() / 8) };
        let uvs = unsafe { std::slice::from_raw_parts(
            uvs.0.as_ptr() as *const (f32, f32), uvs.0.len() / 8) };
        let indices = unsafe { std::slice::from_raw_parts(
            indices.0.as_ptr() as *const u16, indices.0.len() / 2) };

        let canvas = &mut self.0;
        let data = unsafe { std::slice::from_raw_parts(img.0, img.1 as _) };
        let imgid = canvas.load_image_mem(data, femtovg::ImageFlags::FLIP_Y).unwrap();
        let (w, h) = canvas.image_size(imgid).unwrap();
        let (w, h) = (w as _, h as _);

        let paint = VGPaint::image(imgid, 0., 0., w, h, 0., opacity);
        let last_trfm = canvas.transform();     //canvas.save();
        //if bm != BlendMode::SrcOver { }   // XXX: not capable

        for idx in indices.chunks_exact(3) {
            //let mut ltrb = (0f32, 0f32, 0f32, 0f32);
            let mut path = VGPath::new();
            //let mut center = (0f32, 0f32);

            let pt = vtx[2];    path.move_to(pt.0, pt.1); // start from last point
            let mesh = idx.iter().map(|idx| {
                let idx = *idx as usize;
                let (pt, tp) = (vtx[idx], uvs[idx]);
                let tp = (tp.0 * w, tp.1 * h);

                //ltrb.0 = ltrb.0.min(tp.0); ltrb.1 = ltrb.1.min(tp.1);
                //ltrb.2 = ltrb.2.max(tp.0); ltrb.3 = ltrb.3.max(tp.1);
                //center.0 += pt.0; center.1 += pt.1;
                path.line_to(pt.0, pt.1);

                (Vec2D { x: pt.0, y: pt.1 }, Vec2D { x: tp.0, y: tp.1 })
            }).collect::<Vec<_>>(); //path.close();

            //center.0 /= 3.; center.1 /= 3.;
            // canvas.translate(center)/scale(1.03)/translate(-center)?
            canvas.set_transform(&simplex_affine_mapping(&mesh));  // XXX:
            canvas.fill_path(&path, &paint);    //canvas.path_bbox(&path);
            canvas.reset_transform();   canvas.set_transform(&last_trfm);
            canvas.flush();     canvas.delete_image(imgid);
        }   //canvas.restore();
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
                    points[1].x, points[1].y,  points[2].x, points[2].y),
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
            let trfm = unsafe { &*(trfm.as_ptr() as *const TM2D) };
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

    #[inline] fn reset(&mut self) { self.0 = VGPath::new(); }
    #[inline] fn set_fill_rule(&mut self, rule: rpath::FillRule) {
        self.1 = match rule {
            rpath::FillRule::NonZero => FillRule::NonZero,
            rpath::FillRule::EvenOdd => FillRule::EvenOdd,
        };
    }

    #[inline] fn    close(&mut self) { self.0.close(); }
    #[inline] fn  move_to(&mut self,  x: f32,  y: f32) {  self.0.move_to(x, y); }
    #[inline] fn  line_to(&mut self,  x: f32,  y: f32) {  self.0.line_to(x, y); }
    #[inline] fn cubic_to(&mut self, ox: f32, oy: f32, ix: f32, iy: f32, x: f32, y: f32) {
        self.0. bezier_to(ox, oy, ix, iy, x, y);
    }
}

pub struct Paint { bm: BlendMode, style: PaintStyle, inner: VGPaint, }

impl Default for Paint {
    #[inline] fn default() -> Self { Self {
        bm: BlendMode::SrcOver, style: PaintStyle::Fill, inner: Default::default()
    } }
}

#[inline] fn to_femtovg_color(color: renderer::Color) -> femtovg::Color {
    femtovg::Color::rgba(color.r, color.g, color.b, color.a)
}

#[inline] fn _to_femtovg_composite_op(bm: BlendMode) -> CompOp {
    match bm {  // TODO:
        BlendMode::SrcOver => CompOp::SourceOver,
        BlendMode::Screen  | BlendMode::Darken  | BlendMode::Overlay | BlendMode::Lighten |
        BlendMode::Difference | BlendMode::Saturation |
        BlendMode::Luminosity | BlendMode::ColorDodge |
        BlendMode::ColorBurn | BlendMode::HardLight |
        BlendMode::SoftLight | BlendMode::Exclusion |
        BlendMode::Multiply  | BlendMode::Color |
        BlendMode::Hue => unimplemented!(),
    }
}

impl renderer::Paint for Paint {    type Gradient = Gradient;
    #[inline] fn set_style(&mut self, style: PaintStyle) { self.style = style; }
    #[inline] fn set_thickness(&mut self, thick: f32) { self.inner.set_line_width(thick); }

    #[inline] fn set_join(&mut self, join: renderer::StrokeJoin) {  use femtovg::LineJoin;
        self.inner.set_line_join(match join {
            renderer::StrokeJoin::Miter => LineJoin::Miter,
            renderer::StrokeJoin::Round => LineJoin::Round,
            renderer::StrokeJoin::Bevel => LineJoin::Bevel,
        });
    }

    #[inline] fn set_cap (&mut self,  cap: renderer::StrokeCap) {   use femtovg::LineCap;
        self.inner.set_line_cap(match cap {
            renderer::StrokeCap::Butt   => LineCap::Butt,
            renderer::StrokeCap::Round  => LineCap::Round,
            renderer::StrokeCap::Square => LineCap::Square,
        });
    }

    #[inline] fn set_color(&mut self, color: renderer::Color) {
        self.inner.set_color(to_femtovg_color(color));
    }
    fn set_gradient(&mut self, grad: &Self::Gradient) {
        let stops = unsafe {
            std::slice::from_raw_parts(grad.stops.0, grad.stops.2 as _)
        }.iter().zip(unsafe { std::slice::from_raw_parts(grad.stops.1, grad.stops.2 as _)
        }).map(|(offset, color)| (*offset, to_femtovg_color(*color)));

        let mut paint = match grad.base {
            GradientBase::Linear { sx, sy, ex, ey } =>
                VGPaint::linear_gradient_stops(sx, sy, ex, ey, stops),
            GradientBase::Radial { cx, cy, radius } =>
                VGPaint::radial_gradient_stops(cx, cy, 1., radius, stops),
        };

        // XXX: in case set_gradient was not called at first?
        paint.set_line_width(self.inner.line_width());
        paint.set_line_join (self.inner.line_join());
        //paint.set_fill_rule (self.inner.fill_rule());     // never called
        paint.set_line_cap  (self.inner.line_cap_start());  self.inner = paint;
    }

    #[inline] fn set_blend_mode(&mut self, bm: BlendMode) { self.bm = bm; }
    #[inline] fn invalidate_stroke(&mut self) { } // not needed in femtovg?
}

enum GradientBase {
    Linear { sx: f32, sy: f32, ex: f32, ey: f32 },
    Radial { /* fx: f32, fy: f32, */cx: f32, cy: f32, radius: f32 },
}

pub struct Gradient {   base: GradientBase,
    stops: (*const f32, *const renderer::Color, u32),
}

impl renderer::Gradient for Gradient {
    #[inline] fn new_linear(sx: f32, sy: f32, ex: f32, ey: f32,
        colors: &[renderer::Color], stops: &[f32]) -> Self { Self {
            base: GradientBase::Linear { sx, sy, ex, ey },
            stops: (stops.as_ptr(), colors.as_ptr(), stops.len() as u32),
    } } //debug_assert!(stops.len() == colors.len());

    #[inline] fn new_radial(cx: f32, cy: f32, radius: f32,
        colors: &[renderer::Color], stops: &[f32]) -> Self { Self {
            base: GradientBase::Radial { cx, cy, radius },
            stops: (stops.as_ptr(), colors.as_ptr(), stops.len() as u32),
    } }
}

pub struct Buffer(Vec<u8>);
pub struct Image(*const u8, u32);

impl renderer::Buffer for Buffer {
    #[inline] fn new(_: BufferType, _: BufferFlags, len: usize) -> Self { Self(vec![0; len]) }
    #[inline] fn map(&mut self) -> &mut [u8] { &mut self.0 }
    #[inline] fn unmap(&mut self) {}
}

impl renderer::Image for Image {
    fn decode(data: &[u8]) -> Option<Self> { Some(Self(data.as_ptr(), data.len() as _)) }
            //image::io::Reader::new(std::io::Cursor::new(data))
            //.with_guessed_format().ok()?.decode().ok()?.into_rgba8()
}

/// Finds the affine transform that maps triangle `from` to triangle `to`. The algorithm
/// is based on the [Simplex Affine Mapping] method which has a [Swift implementation].
///
/// [Simplex Affine Mapping]: https://www.researchgate.net/publication/332410209_Beginner%27s_guide_to_mapping_simplexes_affinely
/// [Swift implementation]: https://rethunk.medium.com/finding-an-affine-transform-using-three-2d-point-correspondences-using-simplex-affine-mapping-255aeb4e8055
fn simplex_affine_mapping(mesh: &[(Vec2D, Vec2D)]) -> TM2D {
    //debug_assert!(mesh.len() == 3);
    let ((a, d), (b, e), (c, f)) = (mesh[0], mesh[1], mesh[2]);

    let det_recip = (a.x * b.y + b.x * c.y + c.x * a.y -
                     a.x * c.y - b.x * a.y - c.x * b.y).recip();

    let p = (d * (b.y - c.y) - e * (a.y - c.y) + f * (a.y - b.y)) * det_recip;
    let q = (e * (a.x - c.x) - d * (b.x - c.x) - f * (a.x - b.x)) * det_recip;

    let t = (d * (b.x * c.y - b.y * c.x) - e * (a.x * c.y - a.y * c.x) +
             f * (a.x * b.y - a.y * b.x)) * det_recip;

    TM2D::identity().new(p.x, p.y, q.x, q.y, t.x, t.y)
}


/// ## Rive runtime format:
/// Binary representation of Artboards, Shapes, Animations, State Machines, etc.
/// The format was designed to provide a balance of quick load times, small file sizes,
/// and flexibility with regards to future changes/addition of features.
/// https://rive.app/community/doc/format/docxcTF9lJxR

/// ### Binary Types:
/// A binary reader for Rive runtime files needs to be able to read these data types
/// from the stream. **Byte order is little endian.**
///
/// - varuint ([LEB128](https://en.wikipedia.org/wiki/LEB128) variable encoded unsigned integer)
/// - string (u32 followed by utf-8 encoded byte array of provided length)
/// - u32, f32
///
/// https://github.com/rive-app/rive-cpp/blob/master/src/core/binary_reader.cpp
#[allow(unused)] pub struct VarUInt(u32); // u128?

/// ### Header:
/// A ToC (table of contents/field definition) is provided which allows the runtime to
/// understand how it can skip over properties and objects it may not understand. This is
/// part of what makes the format resilient to future changes/feature additions to the editor.
/// An older runtime can at least attempt to load an older file and display it without
/// the objects and properties it doesn't understand.
#[allow(unused)] pub struct Header {
    //magic: [u8; 4], // Fingerprint: 0x52 0x49 0x56 0x45 / "RIVE"
    /// Major versions are not cross-compatible.
    majorv: VarUInt,
    /// Minor version changes are compatible with each other provided the major version is
    /// the same. However, certain newer features may not be available if the runtime is of
    /// a different minor version.
    minorv: VarUInt,
    /// a unique identifier for the file that in the future will be able to
    /// be used to distinguish the file
    fileid: VarUInt,

    /// The Table of Contents section of the header is a list of the properties in the file
    /// along with their backing type. This allows the runtime to read past properties it
    /// wishes to skip or doesn't understand. It does this by providing the backing type for
    /// each property ID.
    ///
    /// The list of known properties is serialized as a sequence of variable unsigned integers
    /// with a 0 terminator. A valid property key is distinguished by a non-zero unsigned
    /// integer id/key. Following the properties is a bit array which is composed of the read
    /// property count / 4 bytes. Every property gets 2 bits to define which backing type
    /// deserializer can be used to read past it.
    ///
    /// Backing Type | 2 bit value
    /// -------------|------------
    /// Uint/Bool    | 0
    /// String       | 1
    /// Float        | 2
    /// Color        | 3
    toc: Vec<u8>, // byte aligned bit array
}

/*/ ## Content:
/// The rest of the file is simply a list of objects, each containing a list of their
/// properties and values. An object is represented as a varuint type key. It is immediately
/// followed by the list of properties. Properties are terminated with a 0 varuint. If a non 0
/// value is read, it is expected to the the type key for the property. If the runtime knows
/// the type key, it will know the backing type and how to decode it. The bytes following the
/// type key will be one of the binary types specified earlier. If it is unknown,
/// it can determine from the ToC what the backing type is and read past it.
///
/// ## Core:
/// All objects and properties are defined in a set of files we call core defs for
/// [Core Definitions](https://github.com/rive-app/rive-cpp/tree/master/dev/defs). These are
/// defined in a series of JSON objects and help Rive generate serialization, deserialization,
/// and animation property code. The C++ and Flutter runtimes both have helpers to read and
/// generate a lot of the boilerplate code for these types.
///
/// ### Object:
/// A core object is represented by its Core type key. For example, a Shape has core type key 3.
/// Similarly you can see the generated code for the C++ runtime also identifies a Shape with
/// the same key.
///
/// ### Properties:
/// Properties are similarly represented by a Core type key. These are unique across all objects,
/// so property key 13 will always be the X value of a Node object, and it matches in the
/// runtime. A Node's X value is known to be a floating point value so when it is encountered it
/// will be decoded as such. Property key 0 is reserved as a null terminator (meaning we are
/// done reading properties for the current object).
//
/// ## Context:
/// Objects are always provided in context of each other. A Shape will always be provided after
/// an Artboard. The Node's artboard can always be determined by finding the latest read
/// Artboard. This concept is used extensively to provide the context for objects that require
/// it. Another example, a KeyFrame will always be provided after a LinearAnimation, meaning
/// you can always determine which LinearAnimation a KeyFrame belongs to by simply tracking
/// that last read LinearAnimation.
///
/// ## Hierarchy:
/// Objects inside the Artboard can be parented to other objects in the Artboard. This mapping
/// is more complex and requires identifiers to find the parent. The identifiers are provided
/// as a core def property. The value is always an unsigned integer representing the index
/// within the Artboard of the ContainerComponent derived object that makes a valid parent.
///
/// https://github.com/rive-app/rive-cpp/src */

