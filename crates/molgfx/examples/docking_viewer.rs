//! Interactive docking viewer: plain, holo-pocket, or multi-ligand comparison.
//!
//! Three modes are supported depending on command-line arguments:
//!
//! 1. Plain structure view (default):
//!    `cargo run --example docking_viewer --release -- [structure.cif]`
//!
//! 2. Holo pocket view — ligand inside a clipped molecular surface:
//!    `cargo run --example docking_viewer --release --features semantic -- [protein.cif] [ligand_name]`
//!
//! 3. Multi-structure comparison — several poses side by side:
//!    `cargo run --example docking_viewer --release -- --compare [pose1.cif] [pose2.cif] ...`
//!
//! 4. Headless representation audit:
//!    `cargo run --example docking_viewer --release -- --audit-representations [output-dir] [--4k]`
//!
//! Controls (all modes):
//!   left drag  orbit
//!   wheel      zoom
//!   1-0        switch representation (spacefill / ball-stick / cartoon /
//!              licorice / lines / analytic SAS / twister / trace / tube / rocket)
//!   B/O/G/U    beads / points / paper-chain / putty
//!   , .        previous / next representation
//!   M          cycle surface style (solid / mesh / contour / dots / filled)
//!   C          cycle colour scheme (element → chain → secondary structure)
//!   R          reset camera
//!   P          toggle pocket surface on/off  (holo mode only)
//!   L          toggle lining on/off          (holo mode only)
//!   [ ]        slide cut plane              (holo mode only)

use molgfx::{
    ArcballController, BoundingSphere, Button, Camera, ClipPlane, ClipSet, ColorScheme, Engine,
    EngineConfig, InputEvent, RepresentationHandle, RepresentationKind, Scene, SurfaceStyle, Vec3,
};
use std::sync::Arc;
use winit::application::ApplicationHandler;
use winit::dpi::PhysicalSize;
use winit::event::{ElementState, MouseButton, MouseScrollDelta, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::window::{Window, WindowId};

#[path = "docking_viewer/audit.rs"]
mod audit;
#[path = "docking_viewer/catalog.rs"]
mod catalog;
#[path = "common/mod.rs"]
mod common;
#[path = "docking_viewer/scenes.rs"]
mod scenes;

struct App {
    scene: Scene,
    representations: Vec<RepresentationHandle>,
    representation_index: usize,
    surface_style_index: usize,
    putty_domain: Option<[f32; 2]>,
    color_index: usize,
    available_choices: [bool; 14],
    bound: BoundingSphere,
    state: Option<Running>,
    // Holo mode state
    holo: Option<HoloState>,
}

struct HoloState {
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
const LARGE_SURFACE_ATOMS: usize = 131_072;

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
            "molgfx — holo pocket"
        } else {
            "molgfx — docking viewer"
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
            let choice = catalog::choice_for_key(code).or_else(|| match code {
                KeyCode::Comma => self.cycled_choice(catalog::CycleDirection::Previous),
                KeyCode::Period => self.cycled_choice(catalog::CycleDirection::Next),
                _ => None,
            });
            match code {
                KeyCode::KeyC => {
                    self.cycle_color();
                    changed = true;
                }
                KeyCode::KeyR => {
                    if let Some(state) = &mut self.state {
                        let size = state.window.inner_size();
                        state.camera = Camera::framing(&self.bound, aspect(size));
                        changed = true;
                    }
                }
                KeyCode::KeyM => changed = self.cycle_surface_style(),
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
                }
                KeyCode::BracketRight => {
                    if let Some(holo) = &mut self.holo {
                        holo.cut_offset += self.bound.radius * 0.06;
                        self.apply_cut();
                        changed = true;
                    }
                }
                KeyCode::BracketLeft => {
                    if let Some(holo) = &mut self.holo {
                        holo.cut_offset -= self.bound.radius * 0.06;
                        self.apply_cut();
                        changed = true;
                    }
                }
                _ => {}
            }
            if let Some(choice) = choice {
                changed |= self.set_representation(choice);
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
        for handle in &self.representations {
            if let Some(rep) = self.scene.representation_mut(*handle) {
                rep.color = schemes[self.color_index];
            }
        }
    }

    fn choice_available(&self, choice: catalog::RepresentationChoice) -> bool {
        match self.available_choices.get(choice.index()) {
            Some(available) => *available,
            None => false,
        }
    }

    fn cycled_choice(
        &self,
        direction: catalog::CycleDirection,
    ) -> Option<catalog::RepresentationChoice> {
        let mut index = self.representation_index;
        for _ in 0..catalog::choices().len() {
            let choice = catalog::cycled_choice(index, direction);
            index = choice.index();
            if self.choice_available(choice) {
                return Some(choice);
            }
        }
        None
    }

    fn set_representation(&mut self, choice: catalog::RepresentationChoice) -> bool {
        if !self.choice_available(choice) {
            println!(
                "representation unavailable for this structure: {}",
                choice.name()
            );
            return false;
        }
        self.representation_index = choice.index();
        self.surface_style_index = 0;
        let large_surface = choice
            == catalog::RepresentationChoice::Kind(RepresentationKind::Surface)
            && self
                .scene
                .structures()
                .map(|(_, placed)| {
                    usize::try_from(placed.atoms.len()).map_or(usize::MAX, |count| count)
                })
                .sum::<usize>()
                >= LARGE_SURFACE_ATOMS;
        for handle in &self.representations {
            if let Some(rep) = self.scene.representation_mut(*handle) {
                choice.apply(rep, self.putty_domain);
                if large_surface {
                    // Exact solid SAS routes through culled sphere impostors:
                    // no full-screen BVH walk and no molecular voxel field.
                    rep.params.surface_style = SurfaceStyle::Solid;
                }
            }
        }
        if large_surface {
            println!("representation: surface (large-scene exact SAS)");
        } else {
            println!("representation: {}", choice.name());
        }
        true
    }

    fn cycle_surface_style(&mut self) -> bool {
        let styles = [
            SurfaceStyle::SoftUnion,
            SurfaceStyle::Solid,
            SurfaceStyle::Mesh,
            SurfaceStyle::Contour,
            SurfaceStyle::Dots,
            SurfaceStyle::FilledContour,
        ];
        self.surface_style_index = (self.surface_style_index + 1) % styles.len();
        let mut changed = false;
        for handle in &self.representations {
            if let Some(rep) = self.scene.representation_mut(*handle)
                && rep.kind == RepresentationKind::Surface
            {
                rep.params.surface_style = styles[self.surface_style_index];
                changed = true;
            }
        }
        if changed {
            println!("surface style: {:?}", styles[self.surface_style_index]);
        }
        changed
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.first().map(String::as_str) == Some("--audit-representations") {
        let output = args
            .get(1)
            .map_or("target/visual-checks/docking-viewer", String::as_str);
        let four_k = args.get(2).is_some_and(|value| value == "--4k");
        let filter_start = if four_k { 3 } else { 2 };
        let filters = args.get(filter_start..).map_or(&[][..], |filters| filters);
        let image = if four_k {
            audit::FOUR_K_IMAGE
        } else {
            audit::PREVIEW_IMAGE
        };
        audit::run(output, filters, image)?;
        return Ok(());
    }

    // Determine mode from arguments
    let result = match args.first().map(String::as_str) {
        Some("--compare") => {
            let paths: Vec<String> = args.into_iter().skip(1).collect();
            if paths.is_empty() {
                return Err(
                    std::io::Error::other("usage: --compare pose1.cif pose2.cif ...").into(),
                );
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
        Err(e) => return Err(std::io::Error::other(e).into()),
    };

    println!("{description}");
    println!("controls:");
    println!("  left-drag  orbit");
    println!("  wheel      zoom");
    println!("  1-0        switch representation (spacefill / ball-stick / cartoon /");
    println!("               licorice / lines / surface / twister / trace / tube / rocket)");
    println!("  B/O/G/U    beads / points / paper-chain / putty");
    println!("  , .        previous / next representation");
    println!("  M          cycle surface style (solid / mesh / contour / dots / filled)");
    println!("  C          cycle colour scheme");
    println!("  R          reset camera");
    if app.holo.is_some() {
        println!("  P          toggle pocket surface");
        println!("  L          toggle lining");
        println!("  [ ]        slide cut plane");
    }

    let event_loop = EventLoop::new()?;
    event_loop.run_app(&mut { app })?;
    Ok(())
}
