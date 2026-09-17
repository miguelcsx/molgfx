//! Browser engine lifecycle, resolution policy and allocation-stable frame entry point.

use crate::browser::{WebCamera, WebScene};
use crate::browser_frame::{WebFrameReport, WebFrameTiming, WebRenderExtent};
use crate::browser_types::{
    WebCapabilities, WebEngineConfig, WebLifecycleState, WebPick, WebProfile, WebRenderMode,
    WebResolutionPolicy,
};
use molgfx::{
    DerivedCacheBudget, Engine, EngineConfig, FrameStatus, ImageConfig, ResidencyBudget,
    ResidencyConfig, UploadRingConfig,
};
use wasm_bindgen::prelude::*;
use web_sys::HtmlCanvasElement;

const BYTES_PER_SURFACE_PIXEL: u64 = 4;

/// WebGPU engine bound to a host-owned canvas.
#[wasm_bindgen]
#[derive(Debug)]
pub struct WebEngine {
    inner: Option<Engine>,
    canvas: HtmlCanvasElement,
    config: WebEngineConfig,
    capabilities: WebCapabilities,
    extent: WebRenderExtent,
    lifecycle: WebLifecycleState,
}

#[wasm_bindgen]
#[allow(clippy::missing_errors_doc)]
impl WebEngine {
    /// Opens WebGPU asynchronously against the supplied canvas.
    #[wasm_bindgen]
    pub async fn create(
        canvas: HtmlCanvasElement,
        config: WebEngineConfig,
    ) -> Result<WebEngine, JsValue> {
        let extent = resolve_extent(config.width, config.height, 1.0, &config)?;
        canvas.set_width(extent.width);
        canvas.set_height(extent.height);
        let inner = Engine::new_async(&engine_config(&config, extent), Some(canvas.clone()))
            .await
            .map_err(|error| render_error(&error))?;
        let capabilities = (*inner.capabilities()).into();
        Ok(Self {
            inner: Some(inner),
            canvas,
            config,
            capabilities,
            extent,
            lifecycle: WebLifecycleState::Ready,
        })
    }

    /// Resizes using logical pixels and device-pixel ratio under the configured policy.
    #[wasm_bindgen]
    pub fn resize(
        &mut self,
        width: u32,
        height: u32,
        device_pixel_ratio: f32,
    ) -> Result<(), JsValue> {
        self.ensure_live()?;
        let extent = resolve_extent(width, height, device_pixel_ratio, &self.config)?;
        self.canvas.set_width(extent.width);
        self.canvas.set_height(extent.height);
        if let Some(inner) = &mut self.inner {
            inner.resize(extent.width, extent.height);
        }
        self.extent = extent;
        Ok(())
    }

    /// Renders into one caller-owned report without allocating a JS wrapper per frame.
    #[wasm_bindgen(js_name = renderInto)]
    pub fn render_into(
        &mut self,
        scene: &WebScene,
        camera: &WebCamera,
        output: &mut WebFrameReport,
    ) -> Result<(), JsValue> {
        self.ensure_live()?;
        let Some(inner) = &mut self.inner else {
            return Err(lifecycle_error(self.lifecycle));
        };
        match inner.render(&scene.inner, &camera.inner) {
            Ok(report) => {
                self.lifecycle = if report.status == FrameStatus::Skipped {
                    WebLifecycleState::SurfaceRecovering
                } else {
                    WebLifecycleState::Ready
                };
                output.write(report, self.extent);
                Ok(())
            }
            Err(error) => {
                if error.code() == "MOLGFX-E0002" {
                    self.lifecycle = WebLifecycleState::DeviceLost;
                }
                Err(render_error(&error))
            }
        }
    }

    /// Convenience renderer that creates a new report object.
    pub fn render(
        &mut self,
        scene: &WebScene,
        camera: &WebCamera,
    ) -> Result<WebFrameReport, JsValue> {
        let mut report = WebFrameReport::new();
        self.render_into(scene, camera, &mut report)?;
        Ok(report)
    }

    /// Measures one off-screen frame asynchronously when timestamp queries are available.
    #[wasm_bindgen(js_name = profileFrame)]
    pub async fn profile_frame(
        &mut self,
        scene: &WebScene,
        camera: &WebCamera,
    ) -> Result<WebFrameTiming, JsValue> {
        self.ensure_live()?;
        let Some(inner) = &mut self.inner else {
            return Err(lifecycle_error(self.lifecycle));
        };
        inner
            .profile_frame_async(
                &scene.inner,
                &camera.inner,
                ImageConfig {
                    width: self.extent.width,
                    height: self.extent.height,
                },
            )
            .await
            .map(WebFrameTiming::from)
            .map_err(|error| render_error(&error))
    }

    /// Resolves one visible entity from integer picking attachments.
    pub async fn pick(&mut self, x: u32, y: u32) -> Result<Option<WebPick>, JsValue> {
        self.ensure_live()?;
        let Some(inner) = &mut self.inner else {
            return Err(lifecycle_error(self.lifecycle));
        };
        inner
            .pick_async(x, y)
            .await
            .map(|pick| pick.map(WebPick::from))
            .map_err(|error| render_error(&error))
    }

    /// Recreates a lost device while preserving caller-owned scene and camera values.
    #[wasm_bindgen(js_name = recoverDevice)]
    pub async fn recover_device(&mut self) -> Result<(), JsValue> {
        if self.lifecycle == WebLifecycleState::Destroyed {
            return Err(lifecycle_error(self.lifecycle));
        }
        self.inner = None;
        self.lifecycle = WebLifecycleState::DeviceLost;
        let inner = Engine::new_async(
            &engine_config(&self.config, self.extent),
            Some(self.canvas.clone()),
        )
        .await
        .map_err(|error| render_error(&error))?;
        self.capabilities = (*inner.capabilities()).into();
        self.inner = Some(inner);
        self.lifecycle = WebLifecycleState::Ready;
        Ok(())
    }

    /// Releases GPU resources immediately. A destroyed engine is terminal.
    pub fn destroy(&mut self) {
        self.inner = None;
        self.lifecycle = WebLifecycleState::Destroyed;
    }

    /// Immutable limits and optional acceleration paths for the current device.
    pub fn capabilities(&self) -> WebCapabilities {
        self.capabilities
    }

    #[wasm_bindgen(getter, js_name = lifecycleState)]
    /// Current typed device and surface lifecycle state.
    pub fn lifecycle_state(&self) -> WebLifecycleState {
        self.lifecycle
    }

    /// Changes rendering mode without changing the physical canvas extent.
    #[wasm_bindgen(js_name = setMode)]
    pub fn set_mode(&mut self, mode: WebRenderMode) -> Result<(), JsValue> {
        self.ensure_live()?;
        self.config.mode = mode;
        if let Some(inner) = &mut self.inner {
            inner.set_render_mode(self.config.mode());
        }
        Ok(())
    }

    /// Changes presentation profile without changing the physical canvas extent.
    #[wasm_bindgen(js_name = setProfile)]
    pub fn set_profile(&mut self, profile: WebProfile) -> Result<(), JsValue> {
        self.ensure_live()?;
        let Some(inner) = &mut self.inner else {
            return Err(lifecycle_error(self.lifecycle));
        };
        let mut config = self.config.clone();
        config.profile = profile;
        inner
            .set_render_profile(config.profile())
            .map_err(|error| render_error(&error))?;
        self.config.profile = profile;
        Ok(())
    }

    /// Caller scheduler target; the engine owns no animation loop.
    #[wasm_bindgen(getter, js_name = targetFps)]
    pub fn target_fps(&self) -> u16 {
        self.config.target_fps
    }

    /// Configured total engine memory ceiling.
    #[wasm_bindgen(getter, js_name = totalMemoryBytes)]
    pub fn total_memory_bytes(&self) -> u64 {
        self.config.total_memory_bytes
    }

    fn ensure_live(&self) -> Result<(), JsValue> {
        if matches!(
            self.lifecycle,
            WebLifecycleState::Destroyed | WebLifecycleState::DeviceLost
        ) {
            Err(lifecycle_error(self.lifecycle))
        } else {
            Ok(())
        }
    }
}

#[allow(clippy::cast_precision_loss, clippy::cast_possible_truncation)]
fn resolve_extent(
    width: u32,
    height: u32,
    device_pixel_ratio: f32,
    config: &WebEngineConfig,
) -> Result<WebRenderExtent, JsValue> {
    let dpr = if device_pixel_ratio.is_finite() {
        device_pixel_ratio.max(0.1)
    } else {
        1.0
    };
    let requested_width = physical_extent(width, dpr);
    let requested_height = physical_extent(height, dpr);
    let required = u64::from(requested_width)
        .saturating_mul(u64::from(requested_height))
        .saturating_mul(BYTES_PER_SURFACE_PIXEL);
    if required <= config.total_memory_bytes {
        return Ok(WebRenderExtent {
            width: requested_width,
            height: requested_height,
            scale: 1.0,
        });
    }
    if matches!(config.resolution_policy, WebResolutionPolicy::NativeStrict) {
        return Err(web_error(
            "MOLGFX-WEB-E0001",
            &format!(
                "native surface requires {required} bytes; budget is {} bytes",
                config.total_memory_bytes
            ),
        ));
    }
    let scale = ((config.total_memory_bytes as f64) / (required as f64)).sqrt();
    Ok(WebRenderExtent {
        width: scaled_extent(requested_width, scale),
        height: scaled_extent(requested_height, scale),
        scale: scale as f32,
    })
}

fn engine_config(config: &WebEngineConfig, extent: WebRenderExtent) -> EngineConfig {
    const MIB: u64 = 1024 * 1024;
    const PAGED_BUFFER_CAPACITIES: u64 = 5;
    let total = config.total_memory_bytes;
    let gpu_source_physical = (total / 8 * 3).clamp(16 * MIB, 64 * MIB);
    let residency_capacity = gpu_source_physical / PAGED_BUFFER_CAPACITIES;
    let staging = (total / 32).clamp(MIB, 16 * MIB);
    let page_size = 64 * 1024;
    let page_count = match u32::try_from(residency_capacity / page_size) {
        Ok(value) => value,
        Err(_) => u32::MAX,
    };
    let staging_usize = match usize::try_from(staging) {
        Ok(value) => value,
        Err(_) => usize::MAX,
    };
    EngineConfig {
        power: config.power(),
        width: extent.width,
        height: extent.height,
        resource_memory_limit_bytes: Some(total),
        mode: config.mode(),
        profile: config.profile(),
        residency: ResidencyConfig {
            page_size,
            page_count,
            uploads: UploadRingConfig {
                capacity_bytes: staging_usize,
                ticket_capacity: 256,
                epoch_budget_bytes: staging_usize,
                in_flight_budget_bytes: staging_usize,
                alignment: 256,
            },
            command_capacity: 1024,
            machine_capacity: 1024,
        },
        source_budget: ResidencyBudget {
            cpu: total / 8 * 3,
            staging,
            gpu_hot: residency_capacity * 5 / 6,
            gpu_warm: residency_capacity / 6,
            in_flight: staging,
        },
        derived_cache: DerivedCacheBudget {
            cpu_bytes: total / 16,
            gpu_bytes: total / 8,
        },
        picking_page_capacity: 1024,
    }
}

#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn physical_extent(logical: u32, scale: f32) -> u32 {
    (f64::from(logical.max(1)) * f64::from(scale))
        .round()
        .clamp(1.0, f64::from(u32::MAX)) as u32
}

#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn scaled_extent(value: u32, scale: f64) -> u32 {
    (f64::from(value) * scale)
        .floor()
        .clamp(1.0, f64::from(u32::MAX)) as u32
}

fn render_error(error: &molgfx::RenderError) -> JsValue {
    web_error(error.code(), &error.to_string())
}

fn lifecycle_error(state: WebLifecycleState) -> JsValue {
    web_error(
        "MOLGFX-WEB-E0002",
        &format!("engine lifecycle state is {state:?}"),
    )
}

fn web_error(code: &str, message: &str) -> JsValue {
    let value = js_sys::Error::new(message);
    value.set_name(code);
    value.into()
}
