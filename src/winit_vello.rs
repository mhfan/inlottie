// Copyright 2024 the Vello Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

// https://github.com/linebender/vello/blob/main/examples/simple/src/main.rs

use std::{fs, collections::VecDeque, time::Instant, sync::Arc};
use vello::util::{RenderContext, RenderSurface};
use vello::{AaConfig, Renderer, RendererOptions, Scene,
    kurbo::{Affine, BezPath, Rect}, peniko::{self, Color}};
use winit::{window::Window, event_loop::ControlFlow, event::*};

// Simple struct to hold the state of the renderer
pub struct ActiveRenderState<'s> {
    // The fields MUST be in this order, so that the surface is dropped before the window
    surface: RenderSurface<'s>,
    window: Arc<Window>,
}

enum RenderState<'s> {
    Active(ActiveRenderState<'s>),
    // Cache a window so that it can be reused when the app is resumed after being suspended
    Suspended(Option<Arc<Window>>),
}

fn main() -> anyhow::Result<()> {
    eprintln!(r"{} v{}-g{}, {}, {} 🦀", env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"),
        env!("BUILD_GIT_HASH"), env!("BUILD_TIMESTAMP"), env!("CARGO_PKG_AUTHORS"));
        //build_time::build_time_local!("%H:%M:%S%:z %Y-%m-%d"), //option_env!("ENV_VAR_NAME");
    println!("Usage: {} [<path-to-file>]", std::env::args().next().unwrap());

    //let mut lottie = None;
    //use inlottie::schema::Animation;

    let mut usvg_opts = usvg::Options::default();
    usvg_opts.fontdb_mut().load_system_fonts();     let mut tree = None;
    let path = std::env::args().nth(1).unwrap_or("data/tiger.svg".to_owned());

    //if fs::metadata(&path).is_ok() {} //if std::path::Path(&path).exists() {}
    match path.rfind('.').map_or("", |i| &path[1 + i..]) {
        //"json" => lottie = Animation::from_reader(fs::File::open(&path)?).ok(),
        "svg"  => tree  = usvg::Tree::from_data(&fs::read(&path)?, &usvg_opts).ok(),
        _ => eprintln!("File format is not supported: {path}"),
    }

    // Setup a bunch of state:

    // The vello RenderContext is a global context that lasts for the lifetime of the application
    let mut render_cx = RenderContext::new();

    // State for our example where we store the winit Window and the wgpu Surface
    let mut render_state = RenderState::Suspended(None);

    // An array of renderers, one per wgpu device
    let mut renderers: Vec<Option<Renderer>> = vec![];

    // A vello Scene is a data structure which allows one to build up a description of
    // a scene to be drawn (with paths, fills, images, text, etc) which is then passed to
    // a renderer for rendering
    let mut scene = Scene::new();

    let (mut fragment, mut trfm) = (Scene::new(), Affine::IDENTITY);
    let (mut perf, mut prevt) = (PerfGraph::new(), Instant::now());

    let mut focused = true;
    // Create and run a winit event loop
    let event_loop = winit::event_loop::EventLoop::new()?;
    event_loop.run(move |event, elwt| match event {
            // Setup renderer. In winit apps it is recommended to do setup in Event::Resumed
            // for best cross-platform compatibility
            Event::Resumed => {
                let RenderState::Suspended(cached_window) =
                    &mut render_state else { return };

                let mut wsize = elwt.primary_monitor().unwrap().size();
                wsize.width  /= 2;  wsize.height /= 2;

                // Get the window cached in a previous Suspended event or else create a new one
                let window = cached_window.take().unwrap_or_else(|| Arc::new(
                    winit::window::WindowBuilder::new().with_resizable(true)
                        .with_inner_size(wsize).with_title("Vello Demo").build(elwt).unwrap()
                    /*elwt.create_window(Window::default_attributes()
                        .with_inner_size(LogicalSize::new(1044, 800)) // XXX: winit v0.30
                        .with_resizable(true).with_title("Vello Shapes")).unwrap() */
                ));

                // Create a vello Surface
                //let wsize = window.inner_size();
                let surface_future =
                    render_cx.create_surface(window.clone(),
                        wsize.width, wsize.height, wgpu::PresentMode::AutoVsync);
                let surface = pollster::block_on(
                    surface_future).expect("Error creating surface");

                // Create a vello Renderer for the surface (using its device id)
                renderers.resize_with(render_cx.devices.len(), || None);
                renderers[surface.dev_id].get_or_insert_with(|| Renderer::new(
                    &render_cx.devices[surface.dev_id].device,
                    RendererOptions { surface_format: Some(surface.format), use_cpu: false,
                        antialiasing_support: vello::AaSupport::all(),
                        num_init_threads: std::num::NonZeroUsize::new(1) }
                ).expect("Couldn't create renderer"));

                // Save the Window and Surface to a state variable
                render_state = RenderState::Active(ActiveRenderState { window, surface });
                elwt.set_control_flow(ControlFlow::Poll);
            }

            Event::Suspended => {   // Save window state on suspend
                if let RenderState::Active(state) = &render_state {
                    render_state = RenderState::Suspended(Some(state.window.clone()));
                }   elwt.set_control_flow(ControlFlow::Wait);
            }

            Event::AboutToWait => if focused {
                let render_state = match &mut render_state {
                    RenderState::Active(state) => state,
                    _ => return,
                };  render_state.window.request_redraw();
            }
            Event::WindowEvent { event, window_id, } => {
                // Ignore the event (return from the function) if
                //   - we have no render_state
                //   - OR the window id of the event doesn't match the one of our render_state
                //
                // Else extract a mutable reference to the render state from
                // its containing option for use below
                let render_state = match &mut render_state {
                    RenderState::Active(state)
                        if state.window.id() == window_id => state,
                    _ => return,
                };

                match event {
                    // Exit the event loop when a close is requested
                    // (e.g. window's close button is pressed)
                    WindowEvent::CloseRequested => elwt.exit(),
                    WindowEvent::Focused(bl) => focused = bl,

                    // Resize the surface when the window is resized
                    WindowEvent::Resized(size) => {
                        if let Some(tree) = &tree {
                            let wsize = (size.width as f32, size.height as f32);
                            let csize = (tree.size().width(), tree.size().height());
                            let scale =  (wsize.0 / csize.0).min(wsize.1 / csize.1) * 0.95;
                            let origx = ((wsize.0 - csize.0 * scale) / 2.) as f64;
                            let origy = ((wsize.1 - csize.1 * scale) / 2.) as f64;
                            trfm = Affine::translate((origx, origy)) * Affine::scale(scale as _);
                        }

                        render_cx.resize_surface(&mut render_state.surface,
                            size.width, size.height);
                        render_state.window.request_redraw();
                    }

                    // This is where all the rendering happens
                    WindowEvent::RedrawRequested => {
                        let _ = prevt.elapsed();    prevt = Instant::now();
                        // Empty the scene of objects to draw. You could create a new Scene
                        // each time, but in this case the same Scene is reused so that
                        // the underlying memory allocation can also be reused.
                        scene.reset();
                        fragment.reset();
                        perf.render(&mut scene, 3., 3.);

                        // Re-add the objects to draw to the scene.
                        if let Some(tree) = &tree {
                            inlottie::vello_svg::render_tree(&mut fragment, tree);
                        } else { add_shapes_to_scene(&mut fragment); }
                        scene.append(&fragment, Some(trfm));

                        // Get the RenderSurface (surface + config)
                        let surface = &render_state.surface;

                        // Get the window size
                        let width  = surface.config.width;
                        let height = surface.config.height;

                        // Get the surface's texture
                        let surface_texture = surface.surface
                            .get_current_texture().expect("failed to get surface texture");

                        // Get a handle to the device
                        let device_handle = &render_cx.devices[surface.dev_id];

                        // Render to the surface's texture
                        renderers[surface.dev_id].as_mut().unwrap().render_to_surface(
                            &device_handle.device, &device_handle.queue,
                            &scene, &surface_texture,   // Background color
                            &vello::RenderParams { base_color: Color::rgb8(99, 99, 99), 
                                width, height, antialiasing_method: AaConfig::Msaa16 }
                        ).expect("failed to render to surface");
                        perf.update(prevt.elapsed().as_secs_f32());

                        // Queue the texture to be presented on the surface
                        surface_texture.present();
                        device_handle.device.poll(wgpu::Maintain::Poll);
                    }   _ => ()
                }
            }   _ => ()
    }).expect("Couldn't run event loop");   Ok(())
}

/// Add shapes to a vello scene. This does not actually render the shapes, but adds them
/// to the Scene data structure which represents a set of objects to draw.
fn add_shapes_to_scene(scene: &mut Scene) {
    use vello::kurbo::{Circle, Ellipse, Line, RoundedRect, Stroke};

    // Draw an outlined rectangle
    let stroke = Stroke::new(6.0);
    let rect = RoundedRect::new(10.0, 10.0, 240.0, 240.0, 20.0);
    let rect_stroke_color = Color::rgb(0.9804, 0.702, 0.5294);
    scene.stroke(&stroke, Affine::IDENTITY, rect_stroke_color, None, &rect);

    // Draw a filled circle
    let circle = Circle::new((420.0, 200.0), 120.0);
    let circle_fill_color = Color::rgb(0.9529, 0.5451, 0.6588);
    scene.fill(peniko::Fill::NonZero, Affine::IDENTITY, circle_fill_color, None, &circle);

    // Draw a filled ellipse
    let ellipse = Ellipse::new((250.0, 420.0), (100.0, 160.0), -90.0);
    let ellipse_fill_color = Color::rgb(0.7961, 0.651, 0.9686);
    scene.fill(peniko::Fill::NonZero, Affine::IDENTITY, ellipse_fill_color, None, &ellipse);

    // Draw a straight line
    let line = Line::new((260.0, 20.0), (620.0, 100.0));
    let line_stroke_color = Color::rgb(0.5373, 0.7059, 0.9804);
    scene.stroke(&stroke, Affine::IDENTITY, line_stroke_color, None, &line);
}

pub struct PerfGraph { que: VecDeque<f32>, max: f32, sum: f32
    /*, time: Instant*/, font: Option<peniko::Font> }

impl PerfGraph {
    #[allow(clippy::new_without_default)] pub fn new() -> Self {
        let font = std::fs::read("data/Roboto-Regular.ttf").ok()
            .map(|data| peniko::Font::new(peniko::Blob::new(Arc::new(data)), 0));
        Self { que: VecDeque::with_capacity(100), max: 0., sum: 0.
            /*, time: Instant::now()*/, font }
    }

    pub fn update(&mut self, ft: f32) { //debug_assert!(f32::EPSILON < ft);
        //let ft = self.time.elapsed().as_secs_f32();   self.time = Instant::now();
        let fps = 1. / ft;  if self.max <  fps { self.max = fps } // (ft + f32::EPSILON)
        if self.que.len() == 100 {  self.sum -= self.que.pop_front().unwrap_or(0.); }
        self.que.push_back(fps);    self.sum += fps;
    }

    pub fn render(&self, scene: &mut Scene, x: f32, y: f32) {
        let (rw, rh, mut path) = (100., 20., BezPath::new());
        let trfm = Affine::translate((x as f64, y as f64));

        scene.fill(peniko::Fill::NonZero, trfm, Color::rgba8(0, 0, 0, 99),
            None, &Rect::new(0., 0., rw, rh));  // to clear the exact area?

            path.move_to((0., rh));
        for i in 0..self.que.len() {  // self.que[i].min(100.) / 100.
            path.line_to((rw * i as f64 / self.que.len() as f64,
                rh - rh * self.que[i] as f64 / self.max  as f64));
        }   path.line_to((rw, rh));

        scene.fill(peniko::Fill::NonZero, trfm, Color::rgba8(255, 192, 0, 128), None, &path);
        let fps = self.sum / self.que.len() as f32; // self.que.iter().sum::<f32>()

        if self.font.is_none() { return }   let font = self.font.as_ref().unwrap();
        let (font_size, mut pen) = (14., (rw as f32 - 10., 0.));
        use vello::skrifa::{raw::FileRef, MetadataProvider, instance::Size};
        let font_ref = match FileRef::new(font.data.as_ref()).unwrap() {
            FileRef::Collection(collection) => collection.get(font.index).unwrap(),
            FileRef::Font(font) => font,
        };
        let var_loc = font_ref.axes().location(Vec::<(&str, f32)>::new().iter().copied());
        let metrics  = font_ref.metrics(Size::new(font_size), &var_loc);
        pen.1 += metrics.ascent - metrics.descent + metrics.leading;

        let metrics = font_ref.glyph_metrics(Size::new(font_size), &var_loc);
        let charmap  = font_ref.charmap();

        scene.draw_glyphs(font).font_size(font_size).transform(trfm)
             .brush(Color::rgba8(240, 240, 240, 255))
             .draw(peniko::Fill::NonZero, format!("{fps:.2} FPS").chars().rev().map(|ch| {
                let gid =  charmap.map(ch).unwrap_or_default();
                pen.0 -= metrics.advance_width(gid).unwrap_or_default();
                vello::glyph::Glyph { id: gid.to_u16() as _, x: pen.0, y: pen.1 }
             }));   // XXX: not as decent as canvas.fill_text(...) of femtovg
    }
}

