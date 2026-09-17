//! Interactive focus-and-context pocket: a ligand held inside a clipped
//! molecular surface, the bulk faded to cartoon, the lining residues drawn as
//! lines — and a cutaway plane you slide through the pocket in real time.
//!
//! Usage: `cargo run --example focus_cutaway --release --features semantic -- [structure.cif] [LIGAND] [inspection|illustrative|cinematic]`
//! Defaults load haemoglobin and focus its haem, so a bare run shows something.
//!
//! Controls once the window is open:
//!   left-drag  orbit        wheel  zoom
//!   [ / ]      slide the cutaway plane back / forward through the pocket
//!   S          hide / show the pocket surface
//!   L          hide / show the lining residues
//!   R          recentre the camera on the ligand

use molgfx::{
    ArcballController, AtomSelection, BoundingSphere, Button, Camera, ClipPlane, ClipSet, Engine,
    EngineConfig, InputEvent, RenderProfile, RepresentationHandle, Rgba8, Scene, Vec3,
};
use molgfx_recipes::{FocusScene, FocusStyle, FocusSurfaceExtent};
use std::sync::Arc;
use winit::application::ApplicationHandler;
use winit::dpi::PhysicalSize;
use winit::event::{ElementState, MouseButton, MouseScrollDelta, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::window::{Window, WindowId};

/// Everything the pocket needs that outlives a single frame: the scene, the
/// handles the key commands mutate, and the fixed cut geometry we slide along.
struct App {
    scene: Scene,
    surface: RepresentationHandle,
    lining: RepresentationHandle,
    cut_center: Vec3,
    cut_normal: Vec3,
    cut_offset: f32,
    surface_opacity: f32,
    lining_opacity: f32,
    surface_on: bool,
    lining_on: bool,
    frame: BoundingSphere,
    view: Vec3,
    up: Vec3,
    profile: RenderProfile,
    state: Option<Running>,
}

struct Running {
    window: Arc<Window>,
    engine: Engine,
    camera: Camera,
    controller: ArcballController,
    /// Frames still owed to the GPU before the view settles. The loop is
    /// event-driven, not free-running: an idle window renders nothing, so
    /// memory holds flat instead of climbing over thousands of wasted frames.
    frames_remaining: u32,
    dragging: bool,
}

/// Frames drawn after a change. One would do for the realtime path, but a short
/// burst lets progressive cinematic accumulation converge before the loop
/// parks — bounded either way, so it never grows without limit.
const SETTLE_FRAMES: u32 = 24;

impl Running {
    /// Mark the view dirty: redraw now and keep redrawing until it settles.
    fn wake(&mut self) {
        self.frames_remaining = SETTLE_FRAMES;
        self.window.request_redraw();
    }
}

fn aspect(size: PhysicalSize<u32>) -> f32 {
    let pixels = size.cast::<f32>();
    pixels.width.max(1.0) / pixels.height.max(1.0)
}

impl App {
    /// Recompute the cutaway plane from the current slider offset and push it
    /// onto the surface. The plane's normal is fixed to the ligand view axis,
    /// so orbiting looks *into* a stationary slice rather than dragging it.
    fn apply_cut(&mut self) {
        let point = self.cut_center + self.cut_normal * self.cut_offset;
        let Ok(plane) = ClipPlane::from_point_normal(point, -self.cut_normal) else {
            return;
        };
        let Ok(set) = ClipSet::new(&[plane]) else {
            return;
        };
        if let Some(surface) = self.scene.representation_mut(self.surface) {
            surface.clipping = set;
        }
    }

    fn set_opacity(&mut self, handle: RepresentationHandle, opacity: f32) {
        if let Some(representation) = self.scene.representation_mut(handle) {
            representation.material.opacity = opacity;
        }
    }

    fn camera_for(&self, size: PhysicalSize<u32>) -> Camera {
        let mut camera = Camera::framing(&self.frame, aspect(size));
        let distance = camera.eye.distance(camera.target);
        camera.eye = camera.target + self.view * distance;
        camera.up = self.up;
        camera
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.state.is_some() {
            return;
        }
        let attributes = Window::default_attributes().with_title("molgfx — focus cutaway");
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
            profile: self.scene_profile(),
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
        // Park the loop between events; input wakes it. Without this winit
        // polls and we would redraw forever.
        event_loop.set_control_flow(ControlFlow::Wait);
        let camera = self.camera_for(size);
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
                if let Some(state) = &mut self.state {
                    // Only the orbit drag moves the camera; a bare hover leaves
                    // the view unchanged, so it must not wake the loop.
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
                    .fit_near_far(state.camera.eye, &self.frame);
                match state.engine.render(&self.scene, &state.camera) {
                    Ok(_) => {}
                    Err(e) => eprintln!("frame error ({}): {e}", e.code()),
                }
                // Keep drawing only while frames are still owed; then park.
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
    fn scene_profile(&self) -> RenderProfile {
        self.profile.clone()
    }

    fn on_key(&mut self, key: PhysicalKey) {
        let step = self.frame.radius * 0.06;
        match key {
            PhysicalKey::Code(KeyCode::BracketRight) => {
                self.cut_offset += step;
                self.apply_cut();
            }
            PhysicalKey::Code(KeyCode::BracketLeft) => {
                self.cut_offset -= step;
                self.apply_cut();
            }
            PhysicalKey::Code(KeyCode::KeyS) => {
                self.surface_on = !self.surface_on;
                let opacity = if self.surface_on {
                    self.surface_opacity
                } else {
                    0.0
                };
                self.set_opacity(self.surface, opacity);
            }
            PhysicalKey::Code(KeyCode::KeyL) => {
                self.lining_on = !self.lining_on;
                let opacity = if self.lining_on {
                    self.lining_opacity
                } else {
                    0.0
                };
                self.set_opacity(self.lining, opacity);
            }
            PhysicalKey::Code(KeyCode::KeyR) => {
                if let Some(size) = self.state.as_ref().map(|s| s.window.inner_size()) {
                    let camera = self.camera_for(size);
                    if let Some(state) = &mut self.state {
                        state.camera = camera;
                    }
                }
            }
            _ => return,
        }
        if let Some(state) = &mut self.state {
            state.wake();
        }
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    let path = args
        .first()
        .map_or("benchmarks/scenes/4hhb.cif", String::as_str);
    let ligand_name = args.get(1).map_or("HEM", String::as_str);
    let profile = match args.get(2).map(String::as_str) {
        Some("inspection") => RenderProfile::inspection(),
        Some("illustrative") => RenderProfile::illustrative(),
        _ => RenderProfile::cinematic(),
    };

    let parsed = pdbiox::read(path).map_err(|d| format!("could not read {path}: {d:?}"))?;
    let structure = pdbiox::infer_bonds(
        &parsed,
        pdbiox::BondInference::default(),
        &pdbiox::ExecutionContext::default(),
    )
    .map_err(|d| format!("bond inference failed: {d:?}"))?
    .structure;

    let (ligand_atoms, ligand_points) = ligand_selection(&structure, ligand_name)?;
    let point_count = u16::try_from(ligand_points.len()).map_or(u16::MAX, |value| value);
    let center = ligand_points
        .iter()
        .copied()
        .fold(Vec3::ZERO, |sum, point| sum + point)
        / f32::from(point_count);
    let view = view_direction(&ligand_points, center);
    let up = up_direction(&ligand_points, center, view);

    let mut scene = Scene::from_structure(&structure)?;
    let selection = scene.add_selection(AtomSelection::Sparse(ligand_atoms));
    // Tuned for the dark studio backdrop the cinematic profile selects: the
    // demoted context has to stay legible without reading as bright scribbles
    // against it, so it sits dim and desaturated rather than near-white.
    let focus = scene.focus_with(
        selection,
        FocusStyle {
            context_opacity: 0.30,
            context_color: Rgba8::opaque(74, 78, 82),
            // Ordered waters read as loose debris floating around the subject
            // at this framing; the pocket is the subject here, so they stay
            // out of the composition rather than competing with it.
            solvent_opacity: 0.0,
            surface_extent: FocusSurfaceExtent::Pocket,
            ..FocusStyle::default()
        },
    )?;

    // The lining residues read better as thin sticks than as an opaque wall.
    let lining_opacity = 0.6;
    if let Some(lining) = scene.representation_mut(focus.near_representation) {
        lining.kind = molgfx::RepresentationKind::Lines;
        lining.params.line_width_pixels = 1.4;
        lining.material.opacity = lining_opacity;
    }
    let surface_opacity = match scene.representation_mut(focus.pocket_representation) {
        Some(surface) => {
            surface.material.specular = 0.2;
            surface.material.opacity
        }
        None => 1.0,
    };

    let ligand_sphere = BoundingSphere::from_points(&ligand_points);
    let frame = BoundingSphere {
        center: ligand_sphere.center,
        radius: ligand_sphere.radius * 1.55,
    };

    let mut app = App {
        scene,
        surface: focus.pocket_representation,
        lining: focus.near_representation,
        cut_center: center,
        cut_normal: view,
        cut_offset: 0.0,
        surface_opacity,
        lining_opacity,
        surface_on: true,
        lining_on: true,
        frame,
        view,
        up,
        profile,
        state: None,
    };
    app.apply_cut();

    println!("loaded {path}: focusing {ligand_name}");
    println!(
        "controls:  left-drag orbit · wheel zoom · [ ] slide cut · S surface · L lining · R recentre"
    );

    let event_loop = EventLoop::new()?;
    event_loop.run_app(&mut app)?;
    Ok(())
}

fn ligand_selection(
    structure: &pdbiox::Structure,
    name: &str,
) -> Result<(Vec<u32>, Vec<Vec3>), Box<dyn std::error::Error>> {
    let Some(residue) = structure.data().residues().find(|r| r.name() == Some(name)) else {
        return Err(format!("component {name} is absent from the structure").into());
    };
    let atoms = residue.atoms().map(|a| a.index().get()).collect::<Vec<_>>();
    let points = atoms
        .iter()
        .filter_map(|index| {
            structure
                .positions()
                .get(usize::try_from(*index).ok()?)
                .copied()
                .map(Vec3::from_array)
        })
        .collect::<Vec<_>>();
    if points.is_empty() {
        return Err(format!("component {name} has no positioned atoms").into());
    }
    Ok((atoms, points))
}

/// A view axis roughly normal to the flat of the ligand, so the opening frames
/// the pocket face-on rather than edge-on.
fn view_direction(points: &[Vec3], center: Vec3) -> Vec3 {
    let primary = points
        .iter()
        .map(|p| *p - center)
        .max_by(|a, b| a.length_squared().total_cmp(&b.length_squared()))
        .map_or(Vec3::X, |v| v);
    let secondary = points
        .iter()
        .map(|p| *p - center)
        .max_by(|a, b| {
            primary
                .cross(*a)
                .length_squared()
                .total_cmp(&primary.cross(*b).length_squared())
        })
        .map_or(Vec3::Y, |v| v);
    let mut normal = primary.cross(secondary).normalize_or_zero();
    if normal.length_squared() < 0.5 {
        normal = Vec3::Z;
    }
    if normal.dot(Vec3::Z) < 0.0 {
        -normal
    } else {
        normal
    }
}

fn up_direction(points: &[Vec3], center: Vec3, view: Vec3) -> Vec3 {
    points
        .iter()
        .map(|p| (*p - center).reject_from_normalized(view))
        .max_by(|a, b| a.length_squared().total_cmp(&b.length_squared()))
        .map_or(Vec3::Y, Vec3::normalize_or_zero)
}
