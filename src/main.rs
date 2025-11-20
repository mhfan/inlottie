/****************************************************************
 * $ID: femtovg.rs      Sat 04 Nov 2023 15:13:31+0800           *
 *                                                              *
 * Maintainer: 范美辉 (MeiHui FAN) <mhfan@ustc.edu>              *
 * Copyright (c) 2024 M.H.Fan, All rights reserved.             *
 ****************************************************************/

#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
#![allow(unreachable_code)]

use femtovg::{renderer::OpenGl, Renderer, Canvas};
#[cfg(feature = "b2d")] use {intvg::blend2d::*, std::rc::Rc};
use std::{collections::VecDeque, time::Instant, error::Error, fs, env};

use winit::{application::ApplicationHandler, window::{Window, WindowId},
    event_loop::{ActiveEventLoop, EventLoop}, event::WindowEvent};
#[cfg(not(target_arch = "wasm32"))]
use glutin::{surface::{Surface, WindowSurface}, context::PossiblyCurrentContext, prelude::*};

#[cfg_attr(coverage_nightly, coverage(off))] //#[cfg(not(tarpaulin_include))]
fn main() -> Result<(), Box<dyn Error>> {
    eprintln!(r"{} v{}-g{}, {}, {} 🦀", env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"),
        env!("BUILD_GIT_HASH"), env!("BUILD_TIMESTAMP"), env!("CARGO_PKG_AUTHORS"));
        //build_time::build_time_local!("%H:%M:%S%:z %Y-%m-%d"), //option_env!("ENV_VAR_NAME");
    println!("Usage: {} [<path-to-file>]", env::args().next().unwrap());

    let mut app = WinitApp::new();
    app.load_file(env::args().nth(1).unwrap_or("".to_owned()))?;
    let event_loop = EventLoop::new()?;
    //use winit::platform::run_on_demand::EventLoopExtRunOnDemand;
    //event_loop.set_control_flow(ControlFlow::Poll);
    event_loop.run_app(&mut app)?;  Ok(())
}

impl ApplicationHandler for WinitApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        #[cfg(feature = "b2d")] {
            self.init_blctx(event_loop, "SVG Viewer - Blend2D demo").unwrap(); return
        }
        if let Err(err) =
            self.init_state(event_loop, "Lottie/SVG Viewer - Femtovg") {
                eprintln!("Failed to initialize: {err:?}"); };
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _wid: WindowId, event: WindowEvent) {
        //if !self.window.as_ref().is_some_and(|window| window.id() == wid) { return }
        use winit::{keyboard::{Key, NamedKey}, event::*};

        match event {   //WindowEvent::Destroyed => dbg!(),
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Focused(bl) => self.focused = bl,

            #[cfg(not(target_arch = "wasm32"))] WindowEvent::Resized(size) => {
                if let Some((surface, // move into resize_viewport?
                    glctx)) = &self.state {
                    surface.resize(glctx, size.width .try_into().unwrap(),
                                          size.height.try_into().unwrap());
                }   self.mouse_pos = Default::default();
                self.resize_viewport(Some((size.width as _, size.height as _)));
            }   // first occur on window creation
            WindowEvent::KeyboardInput { event: KeyEvent { logical_key,
                state: ElementState::Pressed, .. }, .. } => match logical_key.as_ref() {
                Key::Named(NamedKey::Escape) => event_loop.exit(),
                Key::Named(NamedKey::Space)  => {   self.paused = !self.paused;
                                                    self.prevt = Instant::now(); }
                #[cfg(feature =  "lottie")]
                Key::Character(ch) => if self.paused { match ch {
                    "n" | "N" => {  use std::time::Duration;  // XXX:
                        let AnimGraph::Lottie(lottie) =
                            &self.graph else { return };
                        self.prevt = Instant::now() -
                            Duration::from_millis((1000. / lottie.fr) as _);
                        self.request_redraw();
                    }   _ => (),
                } }     _ => (),
            }
            WindowEvent::MouseInput { state, button: MouseButton::Left, .. } => {
                self.dragging = matches!(state, ElementState::Pressed);
                #[cfg(feature = "rive-rs")]
                if let AnimGraph::Rive((scene, viewport)) = &mut self.graph {
                    match state {
                        ElementState::Pressed  =>
                            scene.pointer_down(self.mouse_pos.0, self.mouse_pos.1, viewport),
                        ElementState::Released =>
                            scene.pointer_up  (self.mouse_pos.0, self.mouse_pos.1, viewport),
                    }
                }
            }
            WindowEvent::MouseWheel { delta: MouseScrollDelta::LineDelta(_, y), .. } => {
                let Some(ctx2d) = &mut self.ctx2d else { return };
                let pt = ctx2d.transform().inversed()
                    .transform_point(self.mouse_pos.0, self.mouse_pos.1);
                let scale = y / 10. + 1.;   ctx2d.translate( pt.0,  pt.1);
                ctx2d.scale(scale, scale);       ctx2d.translate(-pt.0, -pt.1);
            }
            WindowEvent::CursorMoved { position, .. } => {
                if self.dragging {
                    if let Some(ctx2d) = &mut self.ctx2d {
                        let trfm = ctx2d.transform().inversed();
                        let p0 = trfm.transform_point(
                            self.mouse_pos.0, self.mouse_pos.1);
                        let p1 = trfm.transform_point(
                            position.x as _, position.y as _);
                        ctx2d.translate(p1.0 - p0.0, p1.1 - p0.1);
                    }
                }   self.mouse_pos = (position.x as _, position.y as _);

                #[cfg(feature = "rive-rs")]
                if let AnimGraph::Rive((scene, viewport)) = &mut self.graph {
                    scene.pointer_move(self.mouse_pos.0, self.mouse_pos.1, viewport);
                }
            }
            WindowEvent::DroppedFile(path) => {
                self.mouse_pos = Default::default();     let _ = self.load_file(path);
                self.resize_viewport(None);                      self.request_redraw();
            }
            WindowEvent::RedrawRequested => {   self.redraw();
                #[cfg(feature = "b2d")] self.redraw_b2d();
            }   _ => ()
        }
    }

    fn about_to_wait(&mut self, _loop: &ActiveEventLoop) {
        if !self.paused && self.focused { self.request_redraw(); }
    }
}

struct WinitApp {
    paused: bool,
    focused: bool,
    dragging: bool,
    mouse_pos: (f32, f32),

    prevt: Instant,
    perf: PerfGraph,
    graph: AnimGraph,

    #[cfg(feature = "b2d")] blctx: Option<(BLContext, BLImage)>,
    #[cfg(feature = "b2d")] surface: Option<softbuffer::Surface<Rc<Window>, Rc<Window>>>,

    ctx2d: Option<Canvas<OpenGl>>,
    #[cfg(not(target_arch = "wasm32"))]
    state: Option<(Surface<WindowSurface>, PossiblyCurrentContext)>,
    window: Option<Window>,
}

#[cfg(feature =  "lottie")] use inlottie::schema::Animation;
#[cfg(feature = "rive-rs")] use inlottie::rive_nvg::RiveNVG;

enum AnimGraph {
    #[cfg(feature =  "lottie")] Lottie(Box<Animation>),
    #[cfg(feature = "rive-rs")]
    Rive((Box<dyn rive_rs::Scene<RiveNVG<OpenGl>>>, rive_rs::Viewport)),
    #[allow(clippy::upper_case_acronyms)] SVG(Box<usvg::Tree>),
    None, // for logo/testcase
}

impl WinitApp {
    fn new() -> Self {
        Self { paused: false, focused: true, dragging: false, //exit: false,
            #[cfg(feature = "b2d")] surface: None, #[cfg(feature = "b2d")] blctx: None,
            perf: PerfGraph::new(), mouse_pos: Default::default(), prevt: Instant::now(),
            graph: AnimGraph::None, ctx2d: None, state: None, window: None,
        }
    }

    #[inline] fn request_redraw(&self) {
        #[cfg(feature = "b2d")] if let Some(surface) =
            &self.surface { surface.window().request_redraw(); }
        if let Some(window) = &self.window { window.request_redraw(); }
    }

    #[cfg(feature = "b2d")] fn init_blctx(&mut self, event_loop: &ActiveEventLoop,
        title: &str) -> Result<(), Box<dyn Error>> {
        let mut wsize = event_loop.primary_monitor().unwrap().size();
        wsize.width  /= 2;  wsize.height /= 2;

        let window = Rc::new(event_loop.create_window(
            Window::default_attributes().with_transparent(true)
                .with_inner_size(wsize).with_title(title))?);

        let surface = softbuffer::Surface::new(
            &softbuffer::Context::new(window.clone())?, window.clone())?;
        self.surface = Some(surface);   Ok(())
    }

    #[cfg(feature = "b2d")] fn resize_b2d(&mut self, wsize: Option<(f32, f32)>) {
        let Some(surface) =
            self.surface.as_mut() else { return };  use std::num::NonZeroU32;

        let wsize = if let Some(wsize) = wsize {
            surface.resize(NonZeroU32::new(wsize.0 as _).unwrap(),
                           NonZeroU32::new(wsize.1 as _).unwrap()).unwrap();    wsize
        } else {
            let wsize = surface.window().inner_size();
            (wsize.width as _, wsize.height as _)
        };

        let csize = match &self.graph {
            AnimGraph::SVG(tree) => (tree.size().width(), tree.size().height()),
            AnimGraph::None => (480., 480.),    // for Blend2D logo
            _ => return,
        };

        let scale = (wsize.0 / csize.0).min(wsize.1 / csize.1) * 0.98;  // XXX:
        let csize = (csize.0 * scale, csize.1 * scale);

        /* let mut buffer = surface.buffer_mut().unwrap();
        buffer.iter_mut().for_each(|pix| *pix = 0xff636363);
        let  orig = ((wsize.0 - csize.0) / 2., (wsize.1 - csize.1) / 2.);

        #[allow(clippy::missing_transmute_annotations)]
        let frame = unsafe { std::mem::transmute(
            &mut buffer[(orig.1 as usize * wsize.0 as usize + orig.0 as usize) ..]) };

        // build BLImage over softbuffer frame, need to keep until present out to screen
        let mut blimg = BLImage::from_buffer(csize.0 as _, csize.1 as _,
            BLFormat::BL_FORMAT_PRGB32, frame, wsize.0 as u32 * 4); */
        let mut blimg = BLImage::new(csize.0 as _, csize.1 as _,
            BLFormat::BL_FORMAT_PRGB32);
        let mut blctx = BLContext::new(&mut blimg);

        // blctx.translate(orig.into());
        blctx.scale((scale as _, scale as _));
        self.blctx = Some((blctx, blimg));
    }

    #[cfg(feature = "b2d")] fn redraw_b2d(&mut self) {
        let Some((blctx, blimg)) = &mut self.blctx else { return };
        let Some(surface) =
            self.surface.as_mut() else { return };
        let wsize = surface.window().inner_size();

        let imgd = blimg.get_data();
        let loff = ((wsize.width  - imgd.width())  / 2) as usize;
        let topl = ((wsize.height - imgd.height()) / 2) as usize;

        self.prevt = Instant::now();
        match &mut self.graph {
            AnimGraph::SVG(tree) => {   //blctx.clear_all();
                blctx.fill_all_rgba32((99, 99, 99, 255).into());

                let scale = blctx.get_transform(1).get_scaling().0 as f32; // to screen viewport
                let mouse = ((self.mouse_pos.0 - loff as f32) / scale,
                                         (self.mouse_pos.1 - topl as f32) / scale);
                b2d_svg::render_nodes(blctx, mouse, tree.root(), &usvg::Transform::identity());
            }
            AnimGraph::None => b2d_svg::blend2d_logo(blctx),
            _ => return,
        }

        self.perf.update(self.prevt.elapsed().as_secs_f32());
        self.perf.render_b2d(blctx, (3., 3.));

        let mut buffer = surface.buffer_mut().unwrap();
        buffer.iter_mut().for_each(|pix| *pix = 0xff636363);    // XXX:

        //blimg.to_rgba_inplace(); // 0xAARRGGGBB -> 0xAABBGGRR
        for (src, dst) in imgd.pixels().chunks_exact(imgd.stride()
            as usize).zip(buffer.chunks_exact_mut(wsize.width as _).skip(topl)) {
            unsafe { std::slice::from_raw_parts(src.as_ptr() as *const u32, imgd.width() as _)
            }.iter().zip(dst.iter_mut().skip(loff)).for_each(
                |(src, dst)| *dst = *src)
                //.swap_bytes().rotate_right(8) // 0xAARRGGGBB -> 0xAABBGGRR
        }   let _ = buffer.present();
    }

    #[cfg(not(target_arch = "wasm32"))]
    // https://github.com/rust-windowing/glutin/blob/master/glutin_examples/src/lib.rs
    fn init_state(&mut self, event_loop: &ActiveEventLoop, title: &str) ->
        Result<(), Box<dyn Error>> {
        use glutin::{config::ConfigTemplateBuilder, surface::SurfaceAttributesBuilder,
            context::{ContextApi, ContextAttributesBuilder}, display::GetGlDisplay};
        use {raw_window_handle::HasWindowHandle, glutin_winit::DisplayBuilder};

        let mut wsize = event_loop.primary_monitor().unwrap().size();
        wsize.width  /= 2;  wsize.height /= 2;   use std::num::NonZeroU32;

        let (window, gl_config) = DisplayBuilder::new()
            .with_window_attributes(Some(Window::default_attributes()
                .with_transparent(true).with_inner_size(wsize).with_title(title)))
            .build(event_loop, ConfigTemplateBuilder::new()
                .with_transparency(cfg!(target_os = "macos")).with_alpha_size(8),
                // Find the config with maximum number of samples, so our triangle will be smooth.
                |configs|
                    configs.reduce(|config, accum| {
                        if (config.supports_transparency().unwrap_or(false) &
                            !accum.supports_transparency().unwrap_or(false)) ||
                            config.num_samples() < accum.num_samples() { config } else { accum }
                    }).unwrap())?;

        self.window = window;   let window = self.window.as_ref().unwrap();
        let raw_window_handle = window.window_handle()
            .ok().map(|handle| handle.as_raw());
        let gl_display = gl_config.display();

        let wsize = window.inner_size();
        let surface = unsafe {
            gl_display.create_window_surface(&gl_config,
                &SurfaceAttributesBuilder::<WindowSurface>::new().build(
                    raw_window_handle.unwrap(), NonZeroU32::new(wsize. width).unwrap(),
                                                NonZeroU32::new(wsize.height).unwrap()))?
        };

        let glctx = unsafe {
            gl_display.create_context(&gl_config,
                &ContextAttributesBuilder::new().build(raw_window_handle)).or_else(|_|
            gl_display.create_context(&gl_config,
                &ContextAttributesBuilder::new().with_context_api(
                         ContextApi::Gles(None)).build(raw_window_handle)))?
        }.make_current(&surface)?;

        self.state  = Some((surface, glctx));
        let mut ctx2d = Canvas::new(unsafe { OpenGl::new_from_function_cstr(
            |s| gl_display.get_proc_address(s) as *const _) }?)?;
        #[cfg(target_os = "macos")] let _ = ctx2d.add_font_dir("/Library/fonts");
        let _ = ctx2d.add_font_dir("data/fonts")?;
        self.ctx2d = Some(ctx2d);   Ok(())
    }

    #[cfg(target_arch = "wasm32")]
    fn init_state(&mut self, event_loop: &ActiveEventLoop) -> Result<(), Box<dyn Error>> {
        use winit::platform::web::WindowAttributesExtWebSys;
        use wasm_bindgen::JsCast;

        let  window = web_sys::window().unwrap();
        let  canvas = window.document().unwrap().get_element_by_id("canvas")
            .unwrap().dyn_into::<web_sys::HtmlCanvasElement>();
        self.window = event_loop.create_window(Window::default_attributes()
            .with_title("Winit window").with_append(true).with_canvas(canvas)).ok();

        let scale = window.device_pixel_ratio();
        canvas.set_width ((canvas.client_width()  as f64 * scale) as _);
        canvas.set_height((canvas.client_height() as f64 * scale) as _);
        // XXX: this is matter for hidpi/retina rendering

        self.ctx2d = Canvas::new(OpenGl::new_from_html_canvas(
             ctx2d.as_ref().unwrap())?).ok();
    }

    /* fn draw_offscreen(&mut self/*, path: P*/) -> Result<(), Box<dyn Error>> {
        self.ctx2d = Some(Canvas::new(Self::get_renderer()?)?);
        let  ctx2d = self.ctx2d.as_mut().unwrap();
        self.resize_viewport(Some(1024.0, 768.0));
        self.redraw();

        /* let (width, height, mut path) = (640, 480, Path::new());
        ctx2d.set_size(width, height, 1.);
        ctx2d.clear_rect(0, 0, width * 4, height * 4, Color::rgbf(0.9, 0.0, 0.9));

        path.rect(0., 0., width as _, height as _);
        ctx2d.fill_path(&path, &Paint::linear_gradient(0., 0., width as _, 0.,
            Color::rgba(255, 0, 0, 255), Color::rgba(0, 0, 255, 255)));
        ctx2d.flush(); */

        //let buf = ctx2d.screenshot()?.pixels().flat_map(|pixel|
        //    pixel.iter()).collect::<Vec<_>>();
        let buf = ctx2d.screenshot()?.into_contiguous_buf();
        let buf = unsafe { std::slice::from_raw_parts(buf.0.as_ptr() as *const u8,
            (width * height * 4) as _) };

        let mut encoder = png::Encoder::new(std::io::BufWriter::new(
            fs::File::create("target/foo.png")?), width, height);   // env!("OUT_DIR")
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);

        encoder.set_source_gamma(png::ScaledFloat::new(1. / 2.2));
        //    png::ScaledFloat::from_scaled(45455)  // 1. / 2.2 scaled by 100000
        //let source_chromaticities = png::SourceChromaticities::new( // unscaled instant
        //    (0.3127, 0.3290), (0.6400, 0.3300), (0.3000, 0.6000), (0.1500, 0.0600));
        //encoder.set_source_chromaticities(source_chromaticities);
        encoder.write_header()?.write_image_data(buf)?;     Ok(())
    }

    // https://github.com/Ionizing/femtovg-offscreen/blob/master/src/main.rs
    // https://github.com/servo/surfman/blob/master/surfman/examples/offscreen.rs
    // https://github.com/nobuyuki83/del-gl/blob/master/src/glutin/off_screen_render.rs
    #[cfg(target_os = "macos")] fn get_renderer() -> Result<OpenGl, Box<dyn Error>> {
        //use glutin::{Context, ContextCurrentState, CreationError};
        use glutin::{context::GlProfile, ContextBuilder, GlRequest};

        let ctx = ContextBuilder::new()
            .with_gl_profile(GlProfile::Core).with_gl(GlRequest::Latest)
            .build_headless(&EventLoop::new(), winit::dpi::PhysicalSize::new(1, 1))?;

        Ok(unsafe { OpenGl::new_from_function_cstr(|s|
            ctx.make_current()?.get_proc_address(s) as *const _) }?)
    }

    // https://github.com/rust-windowing/glutin/blob/master/glutin_examples/examples/egl_device.rs
    #[cfg(not(target_arch = "wasm32"))] #[cfg(not(target_os = "macos"))]
    fn get_renderer() -> Result<OpenGl, Box<dyn Error>> {
        use glutin::config::{ConfigSurfaceTypes, ConfigTemplateBuilder};
        use glutin::context::{ContextApi, ContextAttributesBuilder};
        use glutin::api::egl::{device::Device, display::Display};
        use glutin::{display::GetGlDisplay, prelude::*};

        let devices = Device::query_devices()?.collect::<Vec<_>>();
        devices.iter().enumerate().for_each(|(index, device)|
            println!("Device {}: Name: {} Vendor: {}", index,
                device.name().unwrap_or("UNKNOWN"), device.vendor().unwrap_or("UNKNOWN")));
        let display = unsafe { Display::with_device(devices.first()?, None) }?;

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
                .or_else(|_| display.create_context(&config,
            &ContextAttributesBuilder::new()
                .with_context_api(ContextApi::Gles(None)).build(None)))?
        }.make_current_surfaceless()?;

        Ok(unsafe { OpenGl::new_from_function_cstr(|s|
            display.get_proc_address(s) as *const _) }?)
    } */

    fn load_file<P: AsRef<std::path::Path>>(&mut self, path: P) -> Result<(), Box<dyn Error>> {
        let path = path.as_ref();

        //path.rfind('.').map_or("", |i| &path[1 + i..])
        //if fs::metadata(&path).is_ok() {} //if path.exists() {}
        self.graph = match path.extension().and_then(|ext| ext.to_str()) {
            #[cfg(feature =  "lottie")] Some("json") =>
                AnimGraph::Lottie(Box::new(Animation::from_reader(fs::File::open(path)?)?)),

            #[cfg(feature = "rive-rs")] Some("riv")  =>
                AnimGraph::Rive((RiveNVG::new_scene(
                    &fs::read(path)?).unwrap(), Default::default())),

            Some("svg") => {
                let mut usvg_opts = usvg::Options::default();
                        usvg_opts.fontdb_mut().load_system_fonts();
                AnimGraph::SVG(Box::new(usvg::Tree::from_data(&fs::read(path)?, &usvg_opts)?))
            }
            _ => { eprintln!("No file or unsupported: {}", path.display());  AnimGraph::None }
        };  Ok(())
    }

    fn resize_viewport(&mut self, wsize: Option<(f32, f32)>) {  // maximize & centralize
        #[cfg(feature = "b2d")] { self.resize_b2d(wsize); return }
        let Some(ctx2d) = &mut self.ctx2d else { return };

        let wsize = if let Some(wsize) = wsize {
            ctx2d.set_size(wsize.0 as _, wsize.1 as _, 1.);    wsize
        } else { (ctx2d.width() as _, ctx2d.height() as _) };

        let csize = match &mut self.graph {
            #[cfg(feature =  "lottie")]
            AnimGraph::Lottie(lottie) => (lottie.w as _, lottie.h as _),

            #[cfg(feature = "rive-rs")] AnimGraph::Rive((_, viewport)) => {
                viewport.resize(wsize.0 as _, wsize.1 as _);
                ctx2d.reset_transform();    return
            }
            AnimGraph::SVG(tree) => (tree.size().width(), tree.size().height()),
            AnimGraph::None => { ctx2d.reset_transform();   return }    // (480., 480.)?
        };

        ctx2d.reset_transform();
        let scale = (wsize.0 / csize.0).min(wsize.1  / csize.1) * 0.98;     // XXX:
        ctx2d.translate( (wsize.0 - csize.0 * scale) / 2.,
                         (wsize.1 - csize.1 * scale) / 2.);
        ctx2d.scale(scale, scale);
    }

    fn redraw(&mut self) {
        let Some(ctx2d) = &mut self.ctx2d else { return };
        let _elapsed = self.prevt.elapsed();    self.prevt = Instant::now();

        match &mut self.graph {
            #[cfg(feature =  "lottie")] AnimGraph::Lottie(lottie) =>
                if !(lottie.render_next_frame(ctx2d, _elapsed.as_secs_f32())) { return }
                // TODO: draw frame time (lottie.fnth) on screen?

            #[cfg(feature = "rive-rs")]
            AnimGraph::Rive((scene, viewport)) =>
                if !scene.advance_and_maybe_draw(&mut RiveNVG::new(ctx2d),
                    _elapsed, viewport) { return }

            AnimGraph::SVG(tree) => {
                ctx2d.clear_rect(0, 0, ctx2d.width(), ctx2d.height(),
                    femtovg::Color::rgbf(0.4, 0.4, 0.4)); // XXX: limit to viewport/viewbox?
                render_nodes(ctx2d, //ctx2d.transform().inversed().transform_point()
                    self.mouse_pos, tree.root(), &usvg::Transform::identity());
            }

            AnimGraph::None => some_test_case(ctx2d),
        }

        self.perf.render(ctx2d, (3., 3.));
        ctx2d.flush(); // Tell renderer to execute all drawing commands
        self.perf.update(self.prevt.elapsed().as_secs_f32());

        #[cfg(not(target_arch = "wasm32"))]
        if let Some((surface, glctx)) = &self.state {
            surface.swap_buffers(glctx).expect("Could not swap buffers");
        }   // Display what just rendered
    }
}

pub struct PerfGraph {
    que: VecDeque<f32>, max: f32, sum: f32/*, time: Instant*/,
    #[cfg(feature = "b2d")] font: Option<BLFont>,
}

impl PerfGraph { #[allow(clippy::new_without_default)]
    pub fn new() -> Self { Self {
            que: VecDeque::with_capacity(100), max: 0., sum: 0./*, time: Instant::now()*/,
            #[cfg(feature = "b2d")] font: BLFontFace::from_file(
                "data/Roboto-Regular.ttf").map(|face|
                    BLFont::new(&face, 14.)).ok()
    } }

    pub fn update(&mut self, ft: f32) { //debug_assert!(f32::EPSILON < ft);
        //let ft = self.time.elapsed().as_secs_f32();   self.time = Instant::now();
        let fps = 1. / ft;  if self.max <  fps { self.max = fps } // (ft + f32::EPSILON)
        if self.que.len() == 100 {  self.sum -= self.que.pop_front().unwrap_or(0.); }
        self.que.push_back(fps);    self.sum += fps;
    }

    pub fn render<T: Renderer>(&self, ctx2d: &mut Canvas<T>, pos: (f32, f32)) {
        let (rw, rh, mut path) = (100., 20., Path::new());
        let mut paint = Paint::color(Color::rgba(0, 0, 0, 99));
        path.rect(0., 0., rw, rh);  use femtovg::{Path, Color, Paint};

        let last_trfm = ctx2d.transform();  //ctx2d.save();
        ctx2d.reset_transform();     ctx2d.translate(pos.0, pos.1);
        ctx2d.fill_path(&path, &paint);    // to clear the exact area?

        path = Path::new();     path.move_to(0., rh);
        for i in 0..self.que.len() {  // self.que[i].min(100.) / 100.
            path.line_to(rw * i as f32 / self.que.len() as f32, rh - rh * self.que[i] / self.max);
        }   path.line_to(rw, rh);   paint.set_color(Color::rgba(255, 192, 0, 128));
        ctx2d.fill_path(&path, &paint);

        paint.set_color(Color::rgba(240, 240, 240, 255));
        paint.set_text_baseline(femtovg::Baseline::Top);
        paint.set_text_align(femtovg::Align::Right);
        paint.set_font_size(14.0); // some fixed values can be moved into the structure

        let fps = self.sum / self.que.len() as f32; // self.que.iter().sum::<f32>()
        let _ = ctx2d.fill_text(rw - 10., 0., format!("{fps:.2} FPS"), &paint);
        ctx2d.reset_transform();    ctx2d.set_transform(&last_trfm);    //ctx2d.restore();
    }

    #[cfg(feature = "b2d")] pub fn render_b2d(&self, blctx: &mut BLContext, pos: (f32, f32)) {
        let (rw, rh, mut path) = (100., 20., BLPath::new());
        path.add_rect(&(0., 0., rw, rh).into());

        let last_trfm = blctx.get_transform(1);
        blctx.translate(pos.into());
        blctx.fill_geometry_rgba32(&path, (0, 0, 0, 99).into());  // to clear the exact area?
        path.reset();   path.move_to((0., rh).into());
        for i in 0..self.que.len() {  // self.que[i].min(100.) / 100.
            path.line_to((rw * i as f32 / self.que.len() as f32,
                rh - rh * self.que[i] / self.max).into());
        }   path.line_to((rw, rh).into());
        blctx.fill_geometry_rgba32(&path, (255, 192, 0, 128).into());

        //paint.set_color(Color::rgba(240, 240, 240, 255));
        //paint.set_text_baseline(femtovg::Baseline::Top);
        //paint.set_text_align(femtovg::Align::Right);
        //paint.set_font_size(14.0); // some fixed values can be moved into the structure

        let fps = self.sum / self.que.len() as f32; // self.que.iter().sum::<f32>()
        if let Some(font) = &self.font {
            blctx.fill_utf8_text_d_rgba32((10., 15.).into(), font,  // XXX:
                &format!("{fps:.2} FPS"), (240, 240, 240, 255).into());
        }   blctx.reset_transform(Some(&last_trfm));
    }

}

fn render_nodes<T: Renderer>(ctx2d: &mut Canvas<T>, mouse: (f32, f32),
    parent: &usvg::Group, trfm: &usvg::Transform) {
    use femtovg::{Path, Color, Paint};

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
            // https://github.com/RazrFalcon/resvg/blob/master/crates/resvg/src/path.rs#L179
            usvg::Paint::Color(color) => {
                let mut fc = Color::rgb(color.red, color.green, color.blue);
                fc.set_alphaf(opacity.get());   Paint::color(fc)
            }

            usvg::Paint::LinearGradient(grad) =>
                Paint::linear_gradient_stops(grad.x1(), grad.y1(), grad.x2(), grad.y2(),
                    convert_stops(grad.stops(), opacity)),
            usvg::Paint::RadialGradient(grad) => {
                Paint::radial_gradient_stops(grad.fx(), grad.fy(),  // XXX: 1./0.
                    (grad.cx() - grad.fx()).hypot(grad.cy() - grad.fy()),
                     grad.r().get(), convert_stops(grad.stops(), opacity))
            }
        })
    }

    for child in parent.children() { match child {
        usvg::Node::Group(group) =>     // trfm is needed on rendering only
            render_nodes(ctx2d, mouse, group, &trfm.pre_concat(group.transform())),
            // TODO: deal with group.clip_path()/mask()/filters()

        usvg::Node::Path(path) => if path.is_visible() {
            let tpath = if trfm.is_identity() { None
            } else { path.data().clone().transform(*trfm) };    // XXX:
            let mut fpath = Path::new();

            for seg in tpath.as_ref().unwrap_or(path.data()).segments() {
                use usvg::tiny_skia_path::PathSegment::*;
                match seg {     Close => fpath.close(),
                    MoveTo(pt) => fpath.move_to(pt.x, pt.y),
                    LineTo(pt) => fpath.line_to(pt.x, pt.y),

                    QuadTo(ctrl, end) =>
                        fpath.quad_to  (ctrl.x, ctrl.y, end.x, end.y),
                    CubicTo(ctrl0, ctrl1, end) =>
                        fpath.bezier_to(ctrl0.x, ctrl0.y, ctrl1.x, ctrl1.y, end.x, end.y),
                }
            }

            use femtovg::{FillRule, LineCap, LineJoin};
            let fpaint = path.fill().and_then(|fill|
                convert_paint(fill.paint(), fill.opacity(), trfm).map(|mut paint| {
                    paint.set_fill_rule(match fill.rule() {
                        usvg::FillRule::NonZero => FillRule::NonZero,
                        usvg::FillRule::EvenOdd => FillRule::EvenOdd,
                    }); paint
                })
            );

            let lpaint = path.stroke().and_then(|stroke|
                convert_paint(stroke.paint(), stroke.opacity(), trfm).map(|mut paint| {
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
                    }); paint
                })
            );

            match path.paint_order() {
                usvg::PaintOrder::FillAndStroke => {
                    if let Some(paint) = fpaint { ctx2d.  fill_path(&fpath, &paint); }
                    if let Some(paint) = lpaint { ctx2d.stroke_path(&fpath, &paint); }
                }
                usvg::PaintOrder::StrokeAndFill => {
                    if let Some(paint) = lpaint { ctx2d.stroke_path(&fpath, &paint); }
                    if let Some(paint) = fpaint { ctx2d.  fill_path(&fpath, &paint); }
                }
            }

            if  ctx2d.contains_point(&fpath, mouse.0, mouse.1, FillRule::NonZero) {
                ctx2d.stroke_path(&fpath, &Paint::color(Color::rgb(32, 240, 32))
                    .with_line_width(1. / ctx2d.transform()[0]));
            }
        }

        usvg::Node::Image(img) => if img.is_visible() {
            match img.kind() {
                usvg::ImageKind::GIF(_) | usvg::ImageKind::WEBP(_) |
                usvg::ImageKind::PNG(_) | usvg::ImageKind::JPEG(_) => todo!(),
                // https://github.com/linebender/vello_svg/blob/main/src/lib.rs#L212
                usvg::ImageKind::SVG(svg) => render_nodes(ctx2d, mouse, svg.root(), trfm),
            }
        }

        usvg::Node::Text(text) => { let group = text.flattened();
            render_nodes(ctx2d, mouse, group, &trfm.pre_concat(group.transform()));
        }
    } }
}

fn some_test_case<T: Renderer>(ctx2d: &mut Canvas<T>) {
    let (w, h) = (ctx2d.width(), ctx2d.height());
    let (w, h) = (w as f32, h as f32);
    use femtovg::{Path, Color, Paint};

    let imgid = ctx2d.create_image_empty(w as _, h as _,
        femtovg::PixelFormat::Rgba8, femtovg::ImageFlags::FLIP_Y).unwrap();
    ctx2d.set_render_target(femtovg::RenderTarget::Image(imgid));
    ctx2d.clear_rect(0, 0, w as _, h as _, femtovg::Color::rgbaf(0., 0., 0., 0.));

    let (lx, ty) = (w / 4., h / 4.);
    let mut path = Path::new();     path.rect(lx, ty, w / 2., h / 2.);
    ctx2d.stroke_path(&path, &Paint::color(Color::rgbaf(0., 0., 1., 1.)).with_line_width(1.));
    ctx2d.  fill_path(&path, &Paint::color(Color::rgbaf(1., 0.5, 0.5, 1.)));

    let mskid = ctx2d.create_image_empty(w as _, h as _,
        femtovg::PixelFormat::Rgba8, femtovg::ImageFlags::FLIP_Y).unwrap();
    ctx2d.set_render_target(femtovg::RenderTarget::Image(mskid));
    ctx2d.clear_rect(0, 0, w as _, h as _, femtovg::Color::rgbaf(0., 0., 0., 0.));

    let (mut path, rx, by) = (Path::new(), w - lx, h - ty - 10.);
    path.move_to(w / 2., ty); path.line_to(rx, by); path.line_to(lx, by); path.close();
    ctx2d.fill_path(&path, &Paint::color(Color::rgbaf(0., 1., 0., 1.)));

    path = Path::new();  path.rect(0., 0., w, h);
    ctx2d.global_composite_operation(femtovg::CompositeOperation::DestinationIn);
    ctx2d.set_render_target(femtovg::RenderTarget::Image(imgid));
    let paint = femtovg::Paint::image(mskid, 0., 0., w, h, 0., 1.);
    ctx2d.fill_path(&path, &paint);     ctx2d.flush();  ctx2d.delete_image(mskid);

    ctx2d.global_composite_operation(femtovg::CompositeOperation::SourceOver);
    ctx2d.set_render_target(femtovg::RenderTarget::Screen);
    let paint = femtovg::Paint::image(imgid, 0., 0., w, h, 0., 1.);
    ctx2d.fill_path(&path, &paint);     ctx2d.flush();  ctx2d.delete_image(imgid);
}

#[cfg(feature = "b2d")] mod b2d_svg {   use intvg::blend2d::*;

pub fn blend2d_logo(ctx: &mut BLContext) {
    //let mut img = BLImage::new(480, 480, BLFormat::BL_FORMAT_PRGB32); // 0xAARRGGBB
    ctx.clear_all();     //let mut ctx = BLContext::new(&mut img);
    let mut radial = BLGradient::new(&BLRadialGradientValues::new(
        (180, 180).into(), (180, 180).into(), (180.0, 0.)));
    radial.add_stop(0.0, 0xFFFFFFFF.into());
    radial.add_stop(1.0, 0xFFFF6F3F.into());

    ctx.fill_geometry_ext(&BLCircle::new((180, 180).into(), 160.0), &radial);

    let mut linear = BLGradient::new(&BLLinearGradientValues::new(
        (195, 195).into(), (470, 470).into()));
    linear.add_stop(0.0, 0xFFFFFFFF.into());
    linear.add_stop(1.0, 0xFF3F9FFF.into());

    ctx.set_comp_op(BLCompOp::BL_COMP_OP_DIFFERENCE);
    ctx.fill_geometry_ext(&BLRoundRect::new(&(195, 195, 270, 270).into(), 25.0), &linear);
    ctx.set_comp_op(BLCompOp::BL_COMP_OP_SRC_OVER);   // restore to default

    //let _ = img.write_to_file("target/logo_b2d.png");
}

pub fn render_nodes(blctx: &mut BLContext, mouse: (f32, f32),
    parent: &usvg::Group, trfm: &usvg::Transform) {
    fn convert_paint(paint: &usvg::Paint, opacity: usvg::Opacity,
        _trfm: &usvg::Transform) -> Option<Box<dyn B2DStyle>> {
        fn convert_stops(grad: &mut BLGradient, stops: &[usvg::Stop], opacity: usvg::Opacity) {
            stops.iter().for_each(|stop| {   let color = stop.color();
                let color = (color.red, color.green, color.blue,
                    (stop.opacity() * opacity).to_u8()).into();
                grad.add_stop(stop.offset().get() as _, color);
            });
        }

        Some(match paint { usvg::Paint::Pattern(_) => { // trfm should be applied here
                eprintln!("Not support pattern painting"); return None }
            // https://github.com/RazrFalcon/resvg/blob/master/crates/resvg/src/path.rs#L179
            usvg::Paint::Color(color) => Box::new(BLSolidColor::init_rgba32(
                    (color.red, color.green, color.blue, opacity.to_u8()).into())),

            usvg::Paint::LinearGradient(grad) => {
                let mut linear = BLGradient::new(&BLLinearGradientValues::new(
                    (grad.x1(), grad.y1()).into(), (grad.x2(), grad.y2()).into()));
                convert_stops(&mut linear, grad.stops(), opacity);     Box::new(linear)
            }
            usvg::Paint::RadialGradient(grad) => {
                let mut radial = BLGradient::new(&BLRadialGradientValues::new(
                    (grad.cx(), grad.cy()).into(), (grad.fx(), grad.fy()).into(),
                    (grad.r().get() as _, 0.)));
                    //(grad.cx() - grad.fx()).hypot(grad.cy() - grad.fy())
                convert_stops(&mut radial, grad.stops(), opacity);     Box::new(radial)
            }
        })
    }

    for child in parent.children() { match child {
        usvg::Node::Group(group) =>     // trfm is needed on rendering only
            render_nodes(blctx, mouse, group, &trfm.pre_concat(group.transform())),

        usvg::Node::Path(path) => if path.is_visible() {
            let tpath = if trfm.is_identity() { None
            } else { path.data().clone().transform(*trfm) };    // XXX:
            let mut fpath = BLPath::new();

            for seg in tpath.as_ref().unwrap_or(path.data()).segments() {
                use usvg::tiny_skia_path::PathSegment::*;
                match seg {     Close => fpath.close(),
                    MoveTo(pt) => fpath.move_to((pt.x, pt.y).into()),
                    LineTo(pt) => fpath.line_to((pt.x, pt.y).into()),

                    QuadTo(cp, end) =>
                        fpath.quad_to ((cp.x, cp.y).into(), (end.x, end.y).into()),
                    CubicTo(c1, c2, end) =>
                        fpath.cubic_to((c1.x, c1.y).into(),
                                       (c2.x, c2.y).into(), (end.x, end.y).into()),
                }
            }

            let fpaint = path.fill().and_then(|fill| {
                blctx.set_fill_rule(match fill.rule() {
                    usvg::FillRule::NonZero => BLFillRule::BL_FILL_RULE_NON_ZERO,
                    usvg::FillRule::EvenOdd => BLFillRule::BL_FILL_RULE_EVEN_ODD,
                }); convert_paint(fill.paint(), fill.opacity(), trfm)
            });

            let lpaint = path.stroke().and_then(|stroke| {
                blctx.set_stroke_miter_limit(stroke.miterlimit().get() as _);
                blctx.set_stroke_width(stroke.width().get() as _);

                blctx.set_stroke_join(match stroke.linejoin() {
                    usvg::LineJoin::MiterClip => BLStrokeJoin::BL_STROKE_JOIN_MITER_CLIP,
                    usvg::LineJoin::Miter => BLStrokeJoin::BL_STROKE_JOIN_MITER_BEVEL,
                    usvg::LineJoin::Round => BLStrokeJoin::BL_STROKE_JOIN_ROUND,
                    usvg::LineJoin::Bevel => BLStrokeJoin::BL_STROKE_JOIN_BEVEL,
                });
                blctx.set_stroke_caps(match stroke.linecap () {
                    usvg::LineCap::Butt   => BLStrokeCap::BL_STROKE_CAP_BUTT,
                    usvg::LineCap::Round  => BLStrokeCap::BL_STROKE_CAP_ROUND,
                    usvg::LineCap::Square => BLStrokeCap::BL_STROKE_CAP_SQUARE,
                }); convert_paint(stroke.paint(), stroke.opacity(), trfm)
            });

            match path.paint_order() {
                usvg::PaintOrder::FillAndStroke => {
                    if let Some(paint) = fpaint {
                        blctx.fill_geometry_ext(&fpath, paint.as_ref());
                    }
                    if let Some(paint) = lpaint {
                        blctx.stroke_geometry_ext(&fpath, paint.as_ref());
                    }
                }
                usvg::PaintOrder::StrokeAndFill => {
                    if let Some(paint) = lpaint {
                        blctx.stroke_geometry_ext(&fpath, paint.as_ref());
                    }
                    if let Some(paint) = fpaint {
                        blctx.fill_geometry_ext(&fpath, paint.as_ref());
                    }
                }
            }

            if  matches!(fpath.hit_test(mouse.into(),
                BLFillRule::BL_FILL_RULE_NON_ZERO), BLHitTest::BL_HIT_TEST_IN) {
                blctx.set_stroke_width(2. / blctx.get_transform(1).get_scaling().0);
                blctx.stroke_geometry_rgba32(&fpath, (32, 240, 32, 128).into());
            }
        }

        usvg::Node::Image(img) => if img.is_visible() {
            match img.kind() {
                usvg::ImageKind::GIF(_) | usvg::ImageKind::WEBP(_) |
                usvg::ImageKind::PNG(_) | usvg::ImageKind::JPEG(_) => todo!(),
                // https://github.com/linebender/vello_svg/blob/main/src/lib.rs#L212
                usvg::ImageKind::SVG(svg) => render_nodes(blctx, mouse, svg.root(), trfm),
            }
        }

        usvg::Node::Text(text) => { let group = text.flattened();
            render_nodes(blctx, mouse, group, &trfm.pre_concat(group.transform()));
        }
    } }
}

}
