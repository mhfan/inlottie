/****************************************************************
 * $ID: winit_b2d.rs  	    Sun 02 Jun 2024 20:31:38+0800       *
 *                                                              *
 * Maintainer: 范美辉 (MeiHui FAN) <mhfan@ustc.edu>              *
 * Copyright (c) 2024 M.H.Fan, All rights reserved.             *
 ****************************************************************/

use winit::{event_loop::EventLoop, window::WindowBuilder, event::{Event, WindowEvent}};
use std::{fs, time::Instant, collections::VecDeque};
use intvg::blend2d::*;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    eprintln!(r"{} v{}-g{}, {}, {} 🦀", env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"),
        env!("BUILD_GIT_HASH"), env!("BUILD_TIMESTAMP"), env!("CARGO_PKG_AUTHORS"));
        //build_time::build_time_local!("%H:%M:%S%:z %Y-%m-%d"), //option_env!("ENV_VAR_NAME");
    println!("Usage: {} [<path-to-file>]", std::env::args().next().unwrap());

    /* #[cfg(feature = "rive-rs")] let mut viewport = rive_rs::Viewport::default();
    #[cfg(feature = "rive-rs")] let mut scene = None;
    #[cfg(feature = "rive-rs")] use inlottie::rive::NanoVG;

    let mut lottie = None;
    use inlottie::schema::Animation; */

    let mut usvg_opts = usvg::Options::default();
    usvg_opts.fontdb_mut().load_system_fonts();     let mut tree = None;
    let path = std::env::args().nth(1).unwrap_or("data/tiger.svg".to_owned());

    //if fs::metadata(&path).is_ok() {} //if std::path::Path(&path).exists() {}
    match path.rfind('.').map_or("", |i| &path[1 + i..]) {
        //"json" => lottie = Animation::from_reader(fs::File::open(&path)?).ok(),
        #[cfg(feature = "rive-rs")]
        "riv"  => scene = NanoVG::new_scene(&fs::read(&path)?),
        "svg"  => tree  = usvg::Tree::from_data(&fs::read(&path)?, &usvg_opts).ok(),
        _ => { eprintln!("File format is not supported: {path}"); }
    }

    let event_loop = EventLoop::new().unwrap();
    let mut wsize = event_loop.primary_monitor().unwrap().size();
    wsize.width  /= 2;  wsize.height /= 2;

    let window = WindowBuilder::new().with_inner_size(wsize)
        .with_resizable(true).with_title("Blend2D - demo").build(&event_loop).unwrap();

    //let mut pixels = pixels::PixelsBuilder::new(wsize.width, wsize.height,
    //    pixels::SurfaceTexture::new(wsize.width, wsize.height, &window))
    //    .surface_texture_format(pixels::wgpu::TextureFormat::Rgba8UnormSrgb).build()?;
    let mut pixels = pixels::Pixels::new(wsize.width, wsize.height,
        pixels::SurfaceTexture::new(wsize.width, wsize.height, &window)).unwrap();
    //pixels.clear_color(pixels::wgpu::Color { r: 0.1, g: 0.1, b: 0.1, a: 1.0 });
    //pixels.clear_color(pixels::wgpu::Color::TRANSPARENT);
    let mut blctx = None;

    fn resize_blend2d(pixels: &mut pixels::Pixels, wsize: (f32, f32),
        csize: (f32, f32)) -> (BLImage, BLContext) {
        let scale = (wsize.0 / csize.0).min(wsize.1 / csize.1) * 0.95;
        let csize = (csize.0 * scale, csize.1 * scale);
        let  orig = ((wsize.0 - csize.0) / 2., (wsize.1 - csize.1) / 2.);

        pixels.frame_mut().chunks_exact_mut(4).for_each(|pix|
            pix.copy_from_slice(&[99, 99, 99, 255]));
        let frame = &mut pixels.frame_mut()[
            (orig.1 as usize * wsize.0 as usize + orig.0 as usize) * 4 ..];

        let mut blimg = BLImage::from_buffer(csize.0 as _, csize.1 as _,
            BLFormat::BL_FORMAT_PRGB32, frame, wsize.0 as u32 * 4);
        //BLImage::new(csize.0 as _, csize.1 as _, BLFormat::BL_FORMAT_PRGB32);
        let mut blctx = BLContext::new(&mut blimg);

        // blctx.translate(orig.0, orig.1);
        blctx.scale(scale, scale);  (blimg, blctx)
    }

    let mut focused = true;
    let (mut perf, mut prevt) = (PerfGraph::new(), Instant::now());
    //event_loop.set_control_flow(ControlFlow::Poll);

    event_loop.run(move |event, elwt| match event {
        Event::WindowEvent { event, window_id }
            if window_id == window.id() => match event {
            WindowEvent::CloseRequested | WindowEvent::Destroyed => elwt.exit(),
            WindowEvent::Focused(bl) => focused = bl,

            #[cfg(not(target_arch = "wasm32"))]     // first occur on window creation
            WindowEvent::Resized(size) => {
                let _ = pixels.resize_surface(size.width, size.height);
                let _ = pixels.resize_buffer (size.width, size.height);
                let csize = if let Some(tree) = &tree {     // blend2d logo
                    (tree.size().width(), tree.size().height()) } else { (480., 480.) };
                blctx = Some(resize_blend2d(&mut pixels,
                    (size.width as _, size.height as _), csize));   wsize = size;
            }

            WindowEvent::DroppedFile(path) => {     tree = None;
                let file = fs::read(&path).unwrap_or(vec![]);
                match path.extension().and_then(|ext| ext.to_str()) {
                    Some("svg") => tree  = usvg::Tree::from_data(&file,
                        &usvg_opts).ok().map(|tree| {   //window.inner_size(),
                            resize_blend2d(&mut pixels, (wsize.width as _, wsize.height as _),
                                (tree.size().width(), tree.size().height())); tree }),

                    /* #[cfg(feature = "rive-rs")]
                    Some("riv") => scene = NanoVG::new_scene(&file),
                    Some("json") => lottie = Animation::from_reader(
                        fs::File::open(&path).unwrap()).ok().map(|lottie| {
                            resize_canvas(window.inner_size(),
                                lottie.w as _, lottie.h as _); lottie }), */
                    _ => eprintln!("File format is not supported: {}", path.display()),
                }   window.request_redraw();
            }
            WindowEvent::RedrawRequested => {
                let _ = prevt.elapsed();    prevt = Instant::now();
                let Some((blimg, blctx)) =
                    &mut blctx else { return };

                if let Some(tree) = &tree {     //blctx.clearAll();
                    blctx.fillAllRgba32((99, 99, 99, 255).into());
                    render_nodes(blctx, tree.root(), &usvg::Transform::identity());
                } else { blend2d_logo(blctx); }

                perf.update(prevt.elapsed().as_secs_f32());
                perf.render(blctx, 3., 3.);
                blimg.to_rgba_inplace();

                /* let imgd = img.getData();    use std::slice;
                for (src, dst) in imgd.pixels().chunks_exact(imgd.stride() as
                    usize).zip(pixels.frame_mut().chunks_exact_mut(wsize.width as usize * 4)) {
                    let (src, dst) = unsafe { (
                        slice::from_raw_parts(src.as_ptr() as *const u32, src.len() / 4),
                        slice::from_raw_parts_mut(dst.as_mut_ptr() as *mut u32, dst.len() / 4),
                    ) };

                    src.iter().zip(dst.iter_mut()).for_each(|(src, dst)|
                        *dst = src.swap_bytes().rotate_right(8)) // 0xAARRGGGBB -> 0xAABBGGRR
                } */    let _ = pixels.render();
                //if focused { window.request_redraw(); }
            }
            _ => (),
        },

        Event::AboutToWait => if focused { window.request_redraw() },
        Event::LoopExiting => elwt.exit(),
        _ => (),
    })?; Ok(())
}

fn blend2d_logo(ctx: &mut BLContext) {  // Pixel color format: 0xAARRGGBB
    //let mut img = BLImage::new(480, 480, BLFormat::BL_FORMAT_PRGB32);
    ctx.clearAll();     //let mut ctx = BLContext::new(&mut img);
    let mut radial = BLGradient::new(&BLRadialGradientValues::new(
        &(180, 180).into(), &(180, 180).into(), 180.0, 0.));
    radial.addStop(0.0, 0xFFFFFFFF.into());
    radial.addStop(1.0, 0xFFFF6F3F.into());

    ctx.fillGeometryExt(&BLCircle::new(&(180, 180).into(), 160.0), &radial);

    let mut linear = BLGradient::new(&BLLinearGradientValues::new(
        &(195, 195).into(), &(470, 470).into()));
    linear.addStop(0.0, 0xFFFFFFFF.into());
    linear.addStop(1.0, 0xFF3F9FFF.into());

    ctx.setCompOp(BLCompOp::BL_COMP_OP_DIFFERENCE);
    ctx.fillGeometryExt(&BLRoundRect::new(&(195, 195, 270, 270).into(), 25.0), &linear);
    ctx.setCompOp(BLCompOp::BL_COMP_OP_SRC_OVER);   // restore to default

    //let _ = img.writeToFile("target/logo_b2d.png");
}

#[allow(dead_code)] pub struct PerfGraph { que: VecDeque<f32>, max: f32, sum: f32
    /*, time: Instant*/, font: Option<BLFont> }

impl PerfGraph {
    #[allow(clippy::new_without_default)] pub fn new() -> Self {
        let face = BLFontFace::from_file("data/Roboto-Regular.ttf").ok();
        Self { que: VecDeque::with_capacity(100), max: 0., sum: 0. /*, time: Instant::now()*/,
            font: face.as_ref().map(|face| BLFont::new(face, 14.)) }
    }

    pub fn update(&mut self, ft: f32) { //debug_assert!(f32::EPSILON < ft);
        //let ft = self.time.elapsed().as_secs_f32();   self.time = Instant::now();
        let fps = 1. / ft;  if self.max <  fps { self.max = fps } // (ft + f32::EPSILON)
        if self.que.len() == 100 {  self.sum -= self.que.pop_front().unwrap_or(0.); }
        self.que.push_back(fps);    self.sum += fps;
    }

    pub fn render(&self, blctx: &mut BLContext, x: f32, y: f32) {
        let (rw, rh, mut path) = (100., 20., BLPath::new());
        path.addRect(&(0., 0., rw, rh).into());

        let last_trfm = blctx.reset_transform(None);    blctx.translate(x, y);
        blctx.fillGeometryRgba32(&path, (0, 0, 0, 99).into());  // to clear the exact area?
        path.reset();   path.moveTo(&(0., rh).into());
        for i in 0..self.que.len() {  // self.que[i].min(100.) / 100.
            path.lineTo(&(rw * i as f32 / self.que.len() as f32,
                rh - rh * self.que[i] / self.max).into());
        }   path.lineTo(&(rw, rh).into());
        blctx.fillGeometryRgba32(&path, (255, 192, 0, 128).into());

        //paint.set_color(Color::rgba(240, 240, 240, 255));
        //paint.set_text_baseline(femtovg::Baseline::Top);
        //paint.set_text_align(femtovg::Align::Right);
        //paint.set_font_size(14.0); // some fixed values can be moved into the structure

        let fps = self.sum / self.que.len() as f32; // self.que.iter().sum::<f32>()
        if let Some(font) = &self.font {
            blctx.fillUtf8TextDRgba32(&(10., 15.).into(), font,   // XXX:
                &format!("{fps:.2} FPS"), (240, 240, 240, 255).into());
        }   blctx.reset_transform(Some(&last_trfm));
    }
}

fn render_nodes(blctx: &mut BLContext, parent: &usvg::Group, trfm: &usvg::Transform) {
    fn convert_paint(paint: &usvg::Paint, opacity: usvg::Opacity,
        _trfm: &usvg::Transform) -> Option<Box<dyn B2DStyle>> {
        fn convert_stops(grad: &mut BLGradient, stops: &[usvg::Stop], opacity: usvg::Opacity) {
            stops.iter().for_each(|stop| {   let color = stop.color();
                let color = (color.red, color.green, color.blue,
                    (stop.opacity() * opacity).to_u8()).into();
                grad.addStop(stop.offset().get(), color);
            });
        }

        Some(match paint { usvg::Paint::Pattern(_) => { // trfm should be applied here
                eprintln!("Not support pattern painting"); return None }
            // https://github.com/RazrFalcon/resvg/blob/master/crates/resvg/src/path.rs#L179
            usvg::Paint::Color(color) => Box::new(BLSolidColor::initRgba32(
                    (color.red, color.green, color.blue, opacity.to_u8()).into())),

            usvg::Paint::LinearGradient(grad) => {
                let mut linear = BLGradient::new(&BLLinearGradientValues::new(
                    &(grad.x1(), grad.y1()).into(), &(grad.x2(), grad.y2()).into()));
                convert_stops(&mut linear, grad.stops(), opacity);     Box::new(linear)
            }
            usvg::Paint::RadialGradient(grad) => {
                let mut radial = BLGradient::new(&BLRadialGradientValues::new(
                    &(grad.cx(), grad.cy()).into(), &(grad.fx(), grad.fy()).into(),
                    grad.r().get(), 1.));   // XXX: 1./0.
                    //(grad.cx() - grad.fx()).hypot(grad.cy() - grad.fy())
                convert_stops(&mut radial, grad.stops(), opacity);     Box::new(radial)
            }
        })
    }

    for child in parent.children() { match child {
        usvg::Node::Group(group) =>     // trfm is needed on rendering only
            render_nodes(blctx, group, &trfm.pre_concat(group.transform())),

        usvg::Node::Path(path) => if path.is_visible() {
            let tpath = if trfm.is_identity() { None
            } else { path.data().clone().transform(*trfm) };    // XXX:
            let mut fpath = BLPath::new();

            for seg in tpath.as_ref().unwrap_or(path.data()).segments() {
                use usvg::tiny_skia_path::PathSegment;
                match seg {     PathSegment::Close => fpath.close(),
                    PathSegment::MoveTo(pt) => fpath.moveTo(&(pt.x, pt.y).into()),
                    PathSegment::LineTo(pt) => fpath.lineTo(&(pt.x, pt.y).into()),

                    PathSegment::QuadTo(ctrl, end) =>
                        fpath.quadTo (&(ctrl.x, ctrl.y).into(), &(end.x, end.y).into()),
                    PathSegment::CubicTo(ctrl0, ctrl1, end) =>
                        fpath.cubicTo(&(ctrl0.x, ctrl0.y).into(), &(ctrl1.x, ctrl1.y).into(),
                            &(end.x, end.y).into()),
                }
            }

            let fpaint = path.fill().and_then(|fill| {
                blctx.setFillRule(match fill.rule() {
                    usvg::FillRule::NonZero => BLFillRule::BL_FILL_RULE_NON_ZERO,
                    usvg::FillRule::EvenOdd => BLFillRule::BL_FILL_RULE_EVEN_ODD,
                }); convert_paint(fill.paint(), fill.opacity(), trfm)
            });

            let lpaint = path.stroke().and_then(|stroke| {
                blctx.setStrokeMiterLimit(stroke.miterlimit().get());
                blctx.setStrokeWidth(stroke.width().get());

                blctx.setStrokeJoin(match stroke.linejoin() {
                    usvg::LineJoin::MiterClip => BLStrokeJoin::BL_STROKE_JOIN_MITER_CLIP,
                    usvg::LineJoin::Miter => BLStrokeJoin::BL_STROKE_JOIN_MITER_BEVEL,
                    usvg::LineJoin::Round => BLStrokeJoin::BL_STROKE_JOIN_ROUND,
                    usvg::LineJoin::Bevel => BLStrokeJoin::BL_STROKE_JOIN_BEVEL,
                });
                blctx.setStrokeCaps(match stroke.linecap () {
                    usvg::LineCap::Butt   => BLStrokeCap::BL_STROKE_CAP_BUTT,
                    usvg::LineCap::Round  => BLStrokeCap::BL_STROKE_CAP_ROUND,
                    usvg::LineCap::Square => BLStrokeCap::BL_STROKE_CAP_SQUARE,
                }); convert_paint(stroke.paint(), stroke.opacity(), trfm)
            });

            match path.paint_order() {
                usvg::PaintOrder::FillAndStroke => {
                    if let Some(paint) = fpaint {
                        blctx.fillGeometryExt(&fpath, paint.as_ref());
                    }
                    if let Some(paint) = lpaint {
                        blctx.strokeGeometryExt(&fpath, paint.as_ref());
                    }
                }
                usvg::PaintOrder::StrokeAndFill => {
                    if let Some(paint) = lpaint {
                        blctx.strokeGeometryExt(&fpath, paint.as_ref());
                    }
                    if let Some(paint) = fpaint {
                        blctx.fillGeometryExt(&fpath, paint.as_ref());
                    }
                }
            }

            /* let mat = blctx.getUserTransform(); // XXX: must be in screen viewport
            if  matches!(fpath.hitTest(&mat.invert().mapPonitD(&(mouse.0, mouse.1).into()),
                BLFillRule::BL_FILL_RULE_NON_ZERO), BLHitTest::BL_HIT_TEST_IN) {
                blctx.setStrokeWidth(1. / mat.getScaling().0);
                blctx.strokeGeometryRgba32(&fpath, (32, 240, 32, 128).into());
            } */
        }

        usvg::Node::Image(img) => if img.is_visible() {
            match img.kind() {            usvg::ImageKind::JPEG(_) |
                usvg::ImageKind::PNG(_) | usvg::ImageKind::GIF(_) => todo!(),
                // https://github.com/linebender/vello_svg/blob/main/src/lib.rs#L212
                usvg::ImageKind::SVG(svg) => render_nodes(blctx, svg.root(), trfm),
            }
        }

        usvg::Node::Text(text) => { let group = text.flattened();
            render_nodes(blctx, group, &trfm.pre_concat(group.transform()));
        }
    } }
}

