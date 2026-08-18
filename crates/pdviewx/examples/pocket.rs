//! Interactive viewer example: load a structure, draw it, orbit it.
//!
//! Usage: `cargo run --example pocket --release -- path/to/structure.cif`
//! The engine is windowing-agnostic; this example owns the winit event
//! loop and translates its events into the engine's abstract input.

use pdviewx::{
    ArcballController, AtomSelection, BoundingSphere, Button, Camera, Engine, EngineConfig,
    InputEvent, RepresentationKind, Scene,
};
use std::sync::Arc;
use winit::application::ApplicationHandler;
use winit::dpi::PhysicalSize;
use winit::event::{ElementState, MouseButton, MouseScrollDelta, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::window::{Window, WindowId};

struct App {
    scene: Scene,
    state: Option<Running>,
}

struct Running {
    window: Arc<Window>,
    engine: Engine,
    camera: Camera,
    bound: BoundingSphere,
    controller: ArcballController,
    /// Frames still owed before the view settles. The loop is event-driven:
    /// an idle window renders nothing, so memory stays flat instead of
    /// climbing over thousands of identical frames.
    frames_remaining: u32,
    dragging: bool,
}

/// Frames drawn after each change — a short bounded burst, never unbounded.
const SETTLE_FRAMES: u32 = 24;

impl Running {
    fn wake(&mut self) {
        self.frames_remaining = SETTLE_FRAMES;
        self.window.request_redraw();
    }
}

fn aspect(size: PhysicalSize<u32>) -> f32 {
    let pixels = size.cast::<f32>();
    pixels.width.max(1.0) / pixels.height.max(1.0)
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.state.is_some() {
            return;
        }
        let attributes = Window::default_attributes().with_title("pdviewx");
        let window = match event_loop.create_window(attributes) {
            Ok(window) => Arc::new(window),
            Err(e) => {
                eprintln!("window creation failed: {e}");
                event_loop.exit();
                return;
            }
        };
        let size = window.inner_size();
        let config = EngineConfig {
            width: size.width.max(1),
            height: size.height.max(1),
            ..EngineConfig::default()
        };
        let engine = match Engine::new(&config, Some(window.clone())) {
            Ok(engine) => engine,
            Err(e) => {
                eprintln!("engine start failed ({}): {e}", e.code());
                event_loop.exit();
                return;
            }
        };
        event_loop.set_control_flow(ControlFlow::Wait);
        let bound = self.scene.world_aabb();
        let camera = Camera::framing_aabb(&bound, aspect(size));
        window.request_redraw();
        self.state = Some(Running {
            window,
            engine,
            camera,
            bound: bound.bounding_sphere(),
            controller: ArcballController::default(),
            frames_remaining: SETTLE_FRAMES,
            dragging: false,
        });
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        let Some(state) = &mut self.state else {
            return;
        };
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) => {
                state.engine.resize(size.width, size.height);
                state.camera.projection.set_aspect(aspect(size));
                state.wake();
            }
            WindowEvent::CursorMoved { position, .. } => {
                // A bare hover leaves the camera unchanged; only the orbit drag
                // needs a redraw.
                if state.dragging {
                    let position = position.cast::<f32>();
                    state.controller.update(
                        InputEvent::PointerMove {
                            x: position.x,
                            y: position.y,
                        },
                        &mut state.camera,
                    );
                    state.wake();
                }
            }
            WindowEvent::MouseInput {
                state: pressed,
                button,
                ..
            } => {
                let button = match button {
                    MouseButton::Left => Button::Left,
                    MouseButton::Right => Button::Right,
                    _ => Button::Middle,
                };
                let down = pressed == ElementState::Pressed;
                if button == Button::Left {
                    state.dragging = down;
                }
                state.controller.update(
                    InputEvent::PointerButton {
                        button,
                        pressed: down,
                        x: 0.0,
                        y: 0.0,
                    },
                    &mut state.camera,
                );
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let amount = match delta {
                    MouseScrollDelta::LineDelta(_, y) => y,
                    MouseScrollDelta::PixelDelta(p) => p.cast::<f32>().y / 40.0,
                };
                state
                    .controller
                    .update(InputEvent::Scroll { delta: amount }, &mut state.camera);
                state.wake();
            }
            WindowEvent::RedrawRequested => {
                // Refit depth to the scene each frame; cheap, and it keeps
                // reversed depth optimal while zooming.
                state
                    .camera
                    .projection
                    .fit_near_far(state.camera.eye, &state.bound);
                match state.engine.render(&self.scene, &state.camera) {
                    Ok(_) => {}
                    Err(e) => eprintln!("frame error ({}): {e}", e.code()),
                }
                // Keep drawing only while frames are owed; then park.
                state.frames_remaining = state.frames_remaining.saturating_sub(1);
                if state.frames_remaining > 0 {
                    state.window.request_redraw();
                }
            }
            _ => {}
        }
    }
}

fn main() {
    let path = std::env::args().nth(1);
    let mut scene = Scene::new();
    if let Some(path) = path {
        match pdbiox::read(&path) {
            Ok(structure) => {
                if let Err(e) = scene.add_structure(&structure) {
                    eprintln!("structure not renderable ({}): {e}", e.code());
                    return;
                }
                let selection = scene.add_selection(AtomSelection::All);
                if let Err(e) = scene.represent(selection, RepresentationKind::Spacefill) {
                    eprintln!("representation failed ({}): {e}", e.code());
                    return;
                }
                let atom_count = match scene.first_atoms() {
                    Some(table) => table.len(),
                    None => 0,
                };
                println!("loaded {path}: {atom_count} atoms");
            }
            Err(diagnostics) => {
                eprintln!("could not read {path}:");
                for d in diagnostics.iter().take(8) {
                    eprintln!("  {d:?}");
                }
                return;
            }
        }
    } else {
        println!("no structure given; showing the empty gradient");
        println!("usage: cargo run --example pocket --release -- structure.cif");
    }

    let event_loop = match EventLoop::new() {
        Ok(el) => el,
        Err(e) => {
            eprintln!("event loop failed: {e}");
            return;
        }
    };
    let mut app = App { scene, state: None };
    if let Err(e) = event_loop.run_app(&mut app) {
        eprintln!("event loop error: {e}");
    }
}
