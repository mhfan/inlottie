/****************************************************************
 * $ID: femtovg.rs      Sat 04 Nov 2023 15:13:31+0800           *
 *                                                              *
 * Maintainer: 范美辉 (MeiHui FAN) <mhfan@ustc.edu>              *
 * Copyright (c) 2024 M.H.Fan, All rights reserved.             *
 ****************************************************************/

#![cfg_attr(coverage_nightly, feature(coverage_attribute))]

use std::{collections::VecDeque, time::Instant, error::Error};
use femtovg::{renderer::OpenGl, Canvas, Path, Paint, Color};

/* fn render_offs() -> Result<(), Box<dyn Error>> {   // FIXME: offscreen not work
    let mut canvas = Canvas::new(get_renderer()?)?;

    let (width, height) = (640, 480);
    canvas.set_size(width, height, 1.);
    canvas.clear_rect(0, 0, width * 4, height * 4, Color::rgbf(0.9, 0.0, 0.9));

    let mut path = Path::new();
    path.rect(0., 0., width as _, height as _);
    canvas.fill_path(&path, &Paint::linear_gradient(0., 0., width as _, 0.,
        Color::rgba(255, 0, 0, 255), Color::rgba(0, 0, 255, 255)));
dbg!();
    canvas.flush();
dbg!();

    //let buf = canvas.screenshot()?.pixels().flat_map(|pixel|
    //    pixel.iter()).collect::<Vec<_>>();
    let buf = canvas.screenshot()?.into_contiguous_buf();
    let buf = unsafe { std::slice::from_raw_parts(buf.0.as_ptr() as *const u8,
        (width * height * 4) as _) };

    let mut encoder = png::Encoder::new(
        std::io::BufWriter::new(std::fs::File::create("target/foo.png")?), width, height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);

    encoder.set_source_gamma(png::ScaledFloat::new(1. / 2.2));
    //    png::ScaledFloat::from_scaled(45455)  // 1. / 2.2 scaled by 100000
    //let source_chromaticities = png::SourceChromaticities::new( // unscaled instant
    //    (0.3127, 0.3290), (0.6400, 0.3300), (0.3000, 0.6000), (0.1500, 0.0600));
    //encoder.set_source_chromaticities(source_chromaticities);
    encoder.write_header()?.write_image_data(buf)?;

    Ok(())
}

// https://github.com/Ionizing/femtovg-offscreen/blob/master/src/main.rs
// https://github.com/servo/surfman/blob/master/surfman/examples/offscreen.rs
// https://github.com/nobuyuki83/del-gl/blob/master/src/glutin/off_screen_render.rs
#[cfg(target_os = "macos")] fn get_renderer() -> Result<OpenGl, Box<dyn Error>> {
    //use glutin::{Context, ContextCurrentState, CreationError};
    use glutin::{context::GlProfile, ContextBuilder, GlRequest};

    let ctx = ContextBuilder::new()
        .with_gl_profile(GlProfile::Core).with_gl(GlRequest::Latest)
        .build_headless(&EventLoop::new(), PhysicalSize::new(1, 1))?;
    let ctx = unsafe { ctx.make_current() }.unwrap();

    Ok(unsafe { OpenGl::new_from_function(|s| ctx.get_proc_address(s) as *const _) }?)
}

#[cfg(not(target_arch = "wasm32"))] #[cfg(not(target_os = "macos"))]
fn get_renderer() -> Result<OpenGl, Box<dyn Error>> {
    use glutin::config::{ConfigSurfaceTypes, ConfigTemplateBuilder};
    use glutin::context::{ContextApi, ContextAttributesBuilder};
    use glutin::api::egl::{device::Device, display::Display};
    use glutin::{display::GetGlDisplay, prelude::*};

    let devices = Device::query_devices()
        .expect("Failed to query devices").collect::<Vec<_>>();
    devices.iter().enumerate().for_each(|(index, device)|
        println!("Device {}: Name: {} Vendor: {}", index,
            device.name().unwrap_or("UNKNOWN"), device.vendor().unwrap_or("UNKNOWN")));

    let display = unsafe { Display::with_device(devices.first()
        .expect("No available devices"), None) }?;

    let config = unsafe { display.find_configs(
        ConfigTemplateBuilder::default().with_alpha_size(8)
            .with_surface_type(ConfigSurfaceTypes::empty()).build()) }.unwrap()
        .reduce(|config, accum| {
            if (config.supports_transparency().unwrap_or(false) &
                !accum.supports_transparency().unwrap_or(false)) ||
                config.num_samples() < accum.num_samples() { config } else { accum }
        })?;    println!("Picked a config with {} samples", config.num_samples());

    let _context = unsafe { display.create_context(&config,
        &ContextAttributesBuilder::new().build(None))
            .unwrap_or_else(|_| display.create_context(&config,
                &ContextAttributesBuilder::new()
                    .with_context_api(ContextApi::Gles(None)).build(None))
                .expect("failed to create context"))
    }.make_current_surfaceless()?;

    Ok(unsafe { OpenGl::new_from_function_cstr(|s| display.get_proc_address(s) as *const _) }?)
} */

use winit::{window::Window, event_loop::EventLoop, dpi::PhysicalSize};

#[cfg_attr(coverage_nightly, coverage(off))] //#[cfg(not(tarpaulin_include))]
fn main() -> Result<(), Box<dyn Error>> {
    eprintln!(r"{} v{}-g{}, {}, {} 🦀", env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"),
        env!("BUILD_GIT_HASH"), env!("BUILD_TIMESTAMP"), env!("CARGO_PKG_AUTHORS"));
        //build_time::build_time_local!("%H:%M:%S%:z %Y-%m-%d"), //option_env!("ENV_VAR_NAME");
    println!("Usage: {} [<path-to-svg>]", std::env::args().next().unwrap());

    let mut fontdb = usvg::fontdb::Database::new(); fontdb.load_system_fonts();
    let path = std::env::args().nth(1).unwrap_or("data/tiger.svg".to_owned());
    let mut tree = usvg::Tree::from_data(&std::fs::read(path)?,
        &usvg::Options::default(), &fontdb)?;

    let event_loop = EventLoop::new()?;
    use winit::event::{Event, WindowEvent, MouseButton, ElementState};

    #[cfg(not(target_arch = "wasm32"))]
    let (window, surface, glctx,
        mut canvas) = create_window(&event_loop, "SVG Renderer - Femtovg")?;

    #[cfg(target_arch = "wasm32")] let (window, mut canvas) = {
        use winit::platform::web::WindowBuilderExtWebSys;
        use wasm_bindgen::JsCast;

        let canvas = web_sys::window().unwrap()     //  XXX: HTML5/canvas API
            .document().unwrap().get_element_by_id("canvas").unwrap()
            .dyn_into::<web_sys::HtmlCanvasElement>().unwrap();

        let window = winit::window::WindowBuilder::new()
            .with_canvas(Some(canvas)).build(&event_loop).unwrap();
        let canvas = Canvas::new(OpenGl::new_from_html_canvas(&canvas)
            .expect("Cannot create renderer")).expect("Cannot create canvas");

        (window, canvas)    // need to resize canvas
    };

    let (mut dragging, mut focused, mut mouse) = (false, true, (0., 0.));
    let (mut perf, mut prevt) = (PerfGraph::new(), Instant::now());

    event_loop.run(|event, target| {
        let mut resize_canvas =
            |size: PhysicalSize<u32>, orig: usvg::Size| {
            canvas.reset();     mouse = (0., 0.);
            let scale = (size.width  as f32 / orig.width())
                         .min(size.height as f32 / orig.height()) * 0.95;
            canvas.translate((size.width  as f32 - scale * orig.width())  / 2.,
                             (size.height as f32 - scale * orig.height()) / 2.);
            canvas.set_size  (size.width, size.height, 1.); // window.scale_factor() as _
            canvas.scale(scale, scale);
        };

        match event {
            Event::WindowEvent { window_id: _, event } => match event {
                WindowEvent::CloseRequested | WindowEvent::Destroyed => target.exit(),

                #[cfg(not(target_arch = "wasm32"))]     // first occur on window creation
                WindowEvent::Resized(size) => {
                    surface.resize(&glctx,  size.width .try_into().unwrap(),
                                            size.height.try_into().unwrap());
                    resize_canvas(size, tree.size());
                }

                WindowEvent::MouseInput { button: MouseButton::Left,
                    state, .. } => match state {
                    ElementState::Pressed  => dragging = true,
                    ElementState::Released => dragging = false,
                },

                WindowEvent::Focused(bl) => focused = bl,
                WindowEvent::MouseWheel { device_id: _, delta:
                    winit::event::MouseScrollDelta::LineDelta(_, y), .. } => {
                    let pt = canvas.transform().inversed()
                        .transform_point(mouse.0, mouse.1);
                    canvas.translate( pt.0,  pt.1);
                    canvas.scale(1. + (y / 10.), 1. + (y / 10.));
                    canvas.translate(-pt.0, -pt.1);
                }

                WindowEvent::CursorMoved { device_id: _,
                    position, .. } => {
                    if dragging {
                        let p0 = canvas.transform().inversed()
                            .transform_point(mouse.0, mouse.1);
                        let p1 = canvas.transform().inversed()
                            .transform_point(position.x as _, position.y as _);
                        canvas.translate(p1.0 - p0.0, p1.1 - p0.1);
                    }   mouse = (position.x as _, position.y as _);
                }

                WindowEvent::DroppedFile(path) => {
                    tree = usvg::Tree::from_data(&std::fs::read(path).unwrap(),
                        &usvg::Options::default(), &fontdb).unwrap();
                    //let mut size =  window.inner_size();  size.width += 1; size.height += 1;
                    //let _ = window.request_inner_size(size);
                    resize_canvas(window.inner_size(), tree.size());
                    window.request_redraw();
                }

                WindowEvent::RedrawRequested => {
                    let now = Instant::now();
                    perf.update((now - prevt).as_secs_f32());   prevt = now;

                    canvas.clear_rect(0, 0, canvas.width(),
                                            canvas.height(), Color::rgbf(0.3, 0.3, 0.32));
                    render_nodes(&mut canvas, &mouse, tree.root(), &usvg::Transform::default());

                    perf.render(&mut canvas, 3., 3.);
                    canvas.flush(); // Tell renderer to execute all drawing commands
                    #[cfg(not(target_arch = "wasm32"))]
                    surface.swap_buffers(&glctx).expect("Could not swap buffers");
                }   // Display what just rendered

                _ => ()
            },

            Event::AboutToWait => if focused { window.request_redraw() },
            Event::LoopExiting => target.exit(),
            _ => () //println!("{:?}", event)
    }})?;   Ok(())  //loop {}
}

//  https://github.com/rust-windowing/glutin/blob/master/glutin_examples/examples/egl_device.rs

#[cfg(not(target_arch = "wasm32"))]
use glutin::{surface::{Surface, WindowSurface}, context::PossiblyCurrentContext, prelude::*};

#[allow(clippy::type_complexity)] #[cfg(not(target_arch = "wasm32"))]
fn create_window(event_loop: &EventLoop<()>, title: &str) -> Result<(Window,
    Surface<WindowSurface>, PossiblyCurrentContext, Canvas<OpenGl>), Box<dyn Error>> {
    use glutin::{config::ConfigTemplateBuilder, surface::SurfaceAttributesBuilder,
        context::{ContextApi, ContextAttributesBuilder}, display::GetGlDisplay};
    use {raw_window_handle::HasRawWindowHandle, glutin_winit::DisplayBuilder};

    let mut size = event_loop.primary_monitor().unwrap().size();
    size.width  /= 2;   size.height /= 2;   use std::num::NonZeroU32;

    let (window, gl_config) = DisplayBuilder::new()
        .with_window_builder(Some(winit::window::WindowBuilder::new()
            .with_inner_size(size).with_resizable(true).with_title(title)))
        .build(event_loop, ConfigTemplateBuilder::new().with_alpha_size(8),
            |configs|
                // Find the config with the maximum number of samples,
                // so our triangle will be smooth.
                configs.reduce(|config, accum| {
                    if (config.supports_transparency().unwrap_or(false) &
                        !accum.supports_transparency().unwrap_or(false)) ||
                        config.num_samples() < accum.num_samples() { config } else { accum }
                }).unwrap())?;

    let window = window.unwrap(); //let size = window.inner_size();
    let raw_window_handle = window.raw_window_handle();
    let gl_display = gl_config.display();

    let surf_attr =
        SurfaceAttributesBuilder::<WindowSurface>::new()
            .build(raw_window_handle, NonZeroU32::new(size. width).unwrap(),
                                      NonZeroU32::new(size.height).unwrap());
    let surface = unsafe {
        gl_display.create_window_surface(&gl_config, &surf_attr)? };

    let glctx = Some(unsafe {
        gl_display.create_context(&gl_config,
            &ContextAttributesBuilder::new()
                .build(Some(raw_window_handle)))
            .unwrap_or_else(|_| gl_display.create_context(&gl_config,
                &ContextAttributesBuilder::new()
                    .with_context_api(ContextApi::Gles(None))
                    .build(Some(raw_window_handle))).expect("Failed to create context"))
    }).take().unwrap().make_current(&surface)?;

    let mut canvas = Canvas::new(unsafe {
        OpenGl::new_from_function_cstr(|s|
            gl_display.get_proc_address(s) as *const _) }?)?;
    canvas.add_font_dir("data/fonts").expect("Cannot add font dir/files");

    Ok((window, surface, glctx, canvas))
}

pub struct PerfGraph(VecDeque<f32>, /*max: */f32, /*sum: */f32/*, Instant*/);
impl PerfGraph { #[allow(clippy::new_without_default)]
    pub fn new() -> Self { Self(VecDeque::with_capacity(100), 0., 0./*, Instant::now()*/) }

    pub fn update(&mut self, ft: f32) {  //let now = Instant::now();
        //let ft = (now - self.1).as_secs_f32();   self.1 = now;
        let fps = 1. / ft;  if self.1 < fps { self.1 = fps } // (ft + f32::EPSILON)
        if self.0.len() == 100 { self.2 -= self.0.pop_front().unwrap_or(0.); }
        self.0.push_back(fps);   self.2 += fps;
    }

    pub fn render<T: femtovg::Renderer>(&self, canvas: &mut Canvas<T>, x: f32, y: f32) {
        let (rw, rh, mut path) = (100., 20., Path::new());
        path.rect(0., 0., rw, rh);

        let mut paint   = Paint::color(Color::rgba(240, 240, 240, 255));
        paint.set_text_baseline(femtovg::Baseline::Top);
        paint.set_text_align(femtovg::Align::Right);
        paint.set_font_size(14.0);

        canvas.save();  canvas.reset_transform();   canvas.translate(x, y);
        canvas.fill_path(&path, &Paint::color(Color::rgba(0, 0, 0, 128)));

        path = Path::new();     path.move_to(0., rh);
        for i in 0..self.0.len() {  // self.0[i].min(100.) / 100.
            path.line_to(rw * i as f32 / self.0.len() as f32, rh - rh * self.0[i] / self.1);
        }   path.line_to(rw, rh);
        canvas.fill_path(&path, &Paint::color(Color::rgba(255, 192, 0, 128)));

        let _ = canvas.fill_text(rw - 10., 0.,  // self.0.iter().sum::<f32>()
            &format!("{:.2} FPS", self.2 / self.0.len() as f32), &paint);
        canvas.restore();
    }
}

fn render_nodes(canvas: &mut Canvas<OpenGl>, mouse: &(f32, f32),
    parent: &usvg::Group, trfm: &usvg::Transform) {
    fn convert_paint(paint: &usvg::Paint, opacity: usvg::Opacity,
        _trfm: &usvg::Transform) -> Option<Paint> {
        fn convert_stops(stops: &[usvg::Stop], opacity: usvg::Opacity) -> Vec<(f32, Color)> {
            stops.iter().map(|stop| {   let color = stop.color();
                let mut fc = Color::rgb(color.red, color.green, color.blue);
                fc.set_alphaf((stop.opacity() * opacity).get());    (stop.offset().get(), fc)
            }).collect::<Vec<_>>()
        }

        Some(match paint { usvg::Paint::Pattern(_) => { // trfm should be applied here
                eprintln!("Not support pattern painting"); return None }
            usvg::Paint::Color(color) => {
                let mut fc = Color::rgb(color.red, color.green, color.blue);
                fc.set_alphaf(opacity.get());   Paint::color(fc)
            }

            usvg::Paint::LinearGradient(grad) =>
                Paint::linear_gradient_stops(grad.x1(), grad.y1(), grad.x2(), grad.y2(),
                    convert_stops(grad.stops(), opacity)),
            usvg::Paint::RadialGradient(grad) => {
                let (dx, dy) = (grad.cx() - grad.fx(), grad.cy() - grad.fy());
                let radius = (dx * dx + dy * dy).sqrt();    // XXX: 1.
                Paint::radial_gradient_stops(grad.fx(), grad.fy(), radius, grad.r().get(),
                    convert_stops(grad.stops(), opacity))
            }
        })
    }

    for child in parent.children() { match child {
        usvg::Node::Group(group) =>     // trfm is needed on rendering only
            render_nodes(canvas, mouse, group, &trfm.pre_concat(group.transform())),

        usvg::Node::Path(path) => {
            let tpath = if trfm.is_identity() { None
            } else { path.data().clone().transform(*trfm) };
            let mut fpath = Path::new();

            for seg in tpath.as_ref().unwrap_or(path.data()).segments() {
                use usvg::tiny_skia_path::PathSegment;
                match seg {     PathSegment::Close => fpath.close(),
                    PathSegment::MoveTo(pt) => fpath.move_to(pt.x, pt.y),
                    PathSegment::LineTo(pt) => fpath.line_to(pt.x, pt.y),

                    PathSegment::QuadTo(ctrl, end) =>
                        fpath.quad_to  (ctrl.x, ctrl.y, end.x, end.y),
                    PathSegment::CubicTo(ctrl0, ctrl1, end) =>
                        fpath.bezier_to (ctrl0.x, ctrl0.y, ctrl1.x, ctrl1.y, end.x, end.y),
                }
            }

            use femtovg::{FillRule, LineCap, LineJoin};
            if let Some(fill) = path.fill() {
                if let Some(mut paint) = convert_paint(fill.paint(), fill.opacity(), trfm) {
                    paint.set_fill_rule(match fill.rule() {
                        usvg::FillRule::NonZero => FillRule::NonZero,
                        usvg::FillRule::EvenOdd => FillRule::EvenOdd,
                    }); canvas.fill_path(&fpath, &paint);
                }
            }

            if let Some(stroke) = path.stroke() {
                if let Some(mut paint) = convert_paint(stroke.paint(),
                    stroke.opacity(), trfm) {
                    paint.set_miter_limit(stroke.miterlimit().get());
                    paint.set_line_width (stroke.width().get());

                    paint.set_line_join(match stroke.linejoin() { usvg::LineJoin::MiterClip |
                        usvg::LineJoin::Miter => LineJoin::Miter,
                        usvg::LineJoin::Round => LineJoin::Round,
                        usvg::LineJoin::Bevel => LineJoin::Bevel,
                    });
                    paint.set_line_cap (match stroke.linecap () {
                        usvg::LineCap::Butt   => LineCap::Butt,
                        usvg::LineCap::Round  => LineCap::Round,
                        usvg::LineCap::Square => LineCap::Square,
                    }); canvas.stroke_path(&fpath, &paint);
                }
            }

            if  canvas.contains_point(&fpath, mouse.0, mouse.1, FillRule::NonZero) {
                canvas.stroke_path(&fpath,
                    &Paint::color(Color::rgb(32, 240, 32)).with_line_width(1.));
            }
        }

        usvg::Node::Image(_) => eprintln!("Not support image node"),
        usvg::Node::Text(text) => { let group = text.flattened();
            render_nodes(canvas, mouse, group, &trfm.pre_concat(group.transform()));
        }
    } }
}

