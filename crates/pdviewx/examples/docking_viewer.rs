//! Interactive docking viewer: plain, holo-pocket, or multi-ligand comparison.
//!
//! Three modes are supported depending on command-line arguments:
//!
//! 1. Plain structure view (default):
//!    `cargo run --example docking_viewer --release -- [structure.cif]`
//!
//! 2. Holo pocket view — ligand inside a clipped molecular surface:
//!    `cargo run --example docking_viewer --release -- [protein.cif] [ligand_name]`
//!
//! 3. Multi-structure comparison — several poses side by side:
//!    `cargo run --example docking_viewer --release -- --compare [pose1.cif] [pose2.cif] ...`
//!
//! Controls (all modes):
//!   left drag  orbit
//!   wheel      zoom
//!   1-0        switch representation (spacefill / ball-stick / cartoon /
//!              licorice / lines / surface / twister / trace / tube / rocket)
//!   C          cycle colour scheme (element → chain → secondary structure)
//!   R          reset camera
//!   P          toggle pocket surface on/off  (holo mode only)
//!   L          toggle lining on/off          (holo mode only)
//!   [ ]        slide cut plane              (holo mode only)

use pdviewx::{
    ArcballController, BoundingSphere, Button, Camera, ClipPlane, ClipSet, ColorScheme, Engine,
    EngineConfig, FocusView, InputEvent, RepresentationHandle, RepresentationKind, Scene, Vec3,
};
use std::sync::Arc;
use winit::application::ApplicationHandler;
use winit::dpi::PhysicalSize;
use winit::event::{ElementState, MouseButton, MouseScrollDelta, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::window::{Window, WindowId};

#[path = "common/mod.rs"]
mod common;
#[path = "docking_viewer/scenes.rs"]
mod scenes;

struct App {
    scene: Scene,
    representation: RepresentationHandle,
    color_index: usize,
    bound: BoundingSphere,
    state: Option<Running>,
    // Holo mode state
    holo: Option<HoloState>,
}

struct HoloState {
    _focus: FocusView,
    pocket: RepresentationHandle,
    lining: RepresentationHandle,
    _ligand: RepresentationHandle,
    cut_center: Vec3,
    cut_normal: Vec3,
    cut_offset: f32,
    pocket_on: bool,
    lining_on: bool,
    pocket_opacity: f32,
    lining_opacity: f32,
}

struct Running {
    window: Arc<Window>,
    engine: Engine,
    camera: Camera,
    controller: ArcballController,
    frames_remaining: u32,
    dragging: bool,
}

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
        let title = if self.holo.is_some() {
            "pdviewx — holo pocket"
        } else {
            "pdviewx — docking viewer"
        };
        let attributes = Window::default_attributes().with_title(title);
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
        let camera = Camera::framing(&self.bound, aspect(size));
        window.request_redraw();
        self.state = Some(Running {
            window,
            engine,
            camera,
            controller: ArcballController::default(),
            frames_remaining: SETTLE_FRAMES,
            dragging: false,
        });
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) => {
                if let Some(state) = &mut self.state {
                    state.engine.resize(size.width, size.height);
                    state.camera.projection.set_aspect(aspect(size));
                    state.wake();
                }
            }
            WindowEvent::CursorMoved { position, .. } => {
                if let Some(state) = &mut self.state
                    && state.dragging
                {
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
                if let Some(state) = &mut self.state {
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
            }
            WindowEvent::MouseWheel { delta, .. } => {
                if let Some(state) = &mut self.state {
                    let amount = match delta {
                        MouseScrollDelta::LineDelta(_, y) => y,
                        MouseScrollDelta::PixelDelta(p) => p.cast::<f32>().y / 40.0,
                    };
                    state
                        .controller
                        .update(InputEvent::Scroll { delta: amount }, &mut state.camera);
                    state.wake();
                }
            }
            WindowEvent::KeyboardInput { event, .. } => {
                if event.state == ElementState::Pressed {
                    self.on_key(event.physical_key);
                }
            }
            WindowEvent::RedrawRequested => {
                let Some(state) = &mut self.state else {
                    return;
                };
                state
                    .camera
                    .projection
                    .fit_near_far(state.camera.eye, &self.bound);
                match state.engine.render(&self.scene, &state.camera) {
                    Ok(_) => {}
                    Err(e) => eprintln!("frame error ({}): {e}", e.code()),
                }
                state.frames_remaining = state.frames_remaining.saturating_sub(1);
                if state.frames_remaining > 0 {
                    state.window.request_redraw();
                }
            }
            _ => {}
        }
    }
}

impl App {
    fn on_key(&mut self, key: PhysicalKey) {
        let mut changed = false;
        if let PhysicalKey::Code(code) = key {
            let new_kind = match code {
                KeyCode::Digit1 => Some(RepresentationKind::Spacefill),
                KeyCode::Digit2 => Some(RepresentationKind::BallAndStick),
                KeyCode::Digit3 => Some(RepresentationKind::Cartoon),
                KeyCode::Digit4 => Some(RepresentationKind::Licorice),
                KeyCode::Digit5 => Some(RepresentationKind::Lines),
                KeyCode::Digit6 => Some(RepresentationKind::Surface),
                KeyCode::Digit7 => Some(RepresentationKind::Twister),
                KeyCode::Digit8 => Some(RepresentationKind::Trace),
                KeyCode::Digit9 => Some(RepresentationKind::Tube),
                KeyCode::Digit0 => Some(RepresentationKind::Rocket),
                KeyCode::KeyC => {
                    self.cycle_color();
                    changed = true;
                    None
                }
                KeyCode::KeyR => {
                    if let Some(state) = &mut self.state {
                        let size = state.window.inner_size();
                        state.camera = Camera::framing(&self.bound, aspect(size));
                        changed = true;
                    }
                    None
                }
                // Holo-only controls
                KeyCode::KeyP => {
                    if let Some(holo) = &mut self.holo {
                        holo.pocket_on = !holo.pocket_on;
                        let opacity = if holo.pocket_on {
                            holo.pocket_opacity
                        } else {
                            0.0
                        };
                        if let Some(rep) = self.scene.representation_mut(holo.pocket) {
                            rep.material.opacity = opacity;
                        }
                        changed = true;
                    }
                    None
                }
                KeyCode::KeyL => {
                    if let Some(holo) = &mut self.holo {
                        holo.lining_on = !holo.lining_on;
                        let opacity = if holo.lining_on {
                            holo.lining_opacity
                        } else {
                            0.0
                        };
                        if let Some(rep) = self.scene.representation_mut(holo.lining) {
                            rep.material.opacity = opacity;
                        }
                        changed = true;
                    }
                    None
                }
                KeyCode::BracketRight => {
                    if let Some(holo) = &mut self.holo {
                        holo.cut_offset += self.bound.radius * 0.06;
                        self.apply_cut();
                        changed = true;
                    }
                    None
                }
                KeyCode::BracketLeft => {
                    if let Some(holo) = &mut self.holo {
                        holo.cut_offset -= self.bound.radius * 0.06;
                        self.apply_cut();
                        changed = true;
                    }
                    None
                }
                _ => None,
            };
            if let Some(kind) = new_kind
                && let Some(rep) = self.scene.representation_mut(self.representation)
            {
                rep.kind = kind;
                changed = true;
            }
        }
        if changed && let Some(state) = &mut self.state {
            state.wake();
        }
    }

    fn apply_cut(&mut self) {
        let Some(holo) = &self.holo else { return };
        let origin = holo.cut_center + holo.cut_normal * holo.cut_offset;
        let Ok(plane) = ClipPlane::from_point_normal(origin, -holo.cut_normal) else {
            return;
        };
        let Ok(set) = ClipSet::new(&[plane]) else {
            return;
        };
        if let Some(rep) = self.scene.representation_mut(holo.pocket) {
            rep.clipping = set;
        }
    }

    fn cycle_color(&mut self) {
        let schemes = [
            ColorScheme::ByElement,
            ColorScheme::ByChain,
            ColorScheme::BySecondaryStructure,
        ];
        self.color_index = (self.color_index + 1) % schemes.len();
        if let Some(rep) = self.scene.representation_mut(self.representation) {
            rep.color = schemes[self.color_index];
        }
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();

    // Determine mode from arguments
    let result = match args.first().map(String::as_str) {
        Some("--compare") => {
            let paths: Vec<String> = args.into_iter().skip(1).collect();
            if paths.is_empty() {
                eprintln!("usage: --compare pose1.cif pose2.cif ...");
                return;
            }
            scenes::build_compare_scene(&paths)
        }
        _ if args.len() >= 2 => {
            // protein.cif + ligand_name
            scenes::build_holo_scene(&args[0], &args[1])
        }
        _ => {
            let path = args
                .first()
                .map_or("benchmarks/scenes/3PTB.cif", String::as_str);
            scenes::build_plain_scene(path)
        }
    };

    let (app, description) = match result {
        Ok(v) => v,
        Err(e) => {
            eprintln!("{e}");
            return;
        }
    };

    println!("{description}");
    println!("controls:");
    println!("  left-drag  orbit");
    println!("  wheel      zoom");
    println!("  1-0        switch representation (spacefill / ball-stick / cartoon /");
    println!("               licorice / lines / surface / twister / trace / tube / rocket)");
    println!("  C          cycle colour scheme");
    println!("  R          reset camera");
    if app.holo.is_some() {
        println!("  P          toggle pocket surface");
        println!("  L          toggle lining");
        println!("  [ ]        slide cut plane");
    }

    let event_loop = match EventLoop::new() {
        Ok(el) => el,
        Err(e) => {
            eprintln!("event loop failed: {e}");
            return;
        }
    };
    if let Err(e) = event_loop.run_app(&mut { app }) {
        eprintln!("event loop error: {e}");
    }
}
