//! Ordered render-profile resolution kept separate from the public recipe types.

use super::{
    BackdropStyle, BloomStyle, DepthOfField, DisplayTransform, IllustrationStyle,
    LightingEnvironment, MotionBlur, PresentationEffect, RenderProfile, ResolvedRenderPlan,
};

impl RenderProfile {
    pub(crate) fn resolve(&self) -> ResolvedRenderPlan {
        let mut order = (0..self.layers.len()).collect::<Vec<_>>();
        order.sort_by_key(|index| self.layers[*index].priority);
        let mut illustration = IllustrationStyle::default();
        let mut depth_of_field = None;
        let mut motion_blur = None;
        let mut bloom = None;
        let mut backdrop = BackdropStyle::default();
        let mut lighting = LightingEnvironment::default();
        let mut display = DisplayTransform::default();
        for index in order {
            let layer = self.layers[index];
            match layer.effect {
                PresentationEffect::Illustration(style) => {
                    illustration = illustration.blend(style.sanitize(), layer.weight);
                }
                PresentationEffect::DepthOfField(settings) => {
                    let settings = settings.sanitize();
                    let baseline = match depth_of_field {
                        Some(current) => current,
                        None => DepthOfField {
                            max_blur_pixels: 0.0,
                            ..settings
                        },
                    };
                    let resolved = baseline.blend(settings, layer.weight);
                    depth_of_field = (resolved.max_blur_pixels > 0.0).then_some(resolved);
                }
                PresentationEffect::MotionBlur(settings) => {
                    let settings = settings.sanitize();
                    let baseline = match motion_blur {
                        Some(current) => current,
                        None => MotionBlur {
                            shutter: 0.0,
                            max_blur_pixels: 0.0,
                        },
                    };
                    let resolved = baseline.blend(settings, layer.weight);
                    motion_blur = (resolved.shutter > 0.0 && resolved.max_blur_pixels > 0.0)
                        .then_some(resolved);
                }
                PresentationEffect::Backdrop(style) => {
                    backdrop = backdrop.blend(style.sanitize(), layer.weight);
                }
                PresentationEffect::Lighting(environment) => {
                    lighting = lighting.blend(environment.sanitize(), layer.weight);
                }
                PresentationEffect::Display(transform) => {
                    display = display.blend(transform.sanitize(), layer.weight);
                }
                PresentationEffect::Bloom(style) => {
                    let style = style.sanitize();
                    let baseline = match bloom {
                        Some(current) => current,
                        None => BloomStyle {
                            intensity: 0.0,
                            ..style
                        },
                    };
                    let resolved = baseline.blend(style, layer.weight);
                    bloom = (resolved.intensity > 0.0).then_some(resolved);
                }
            }
        }
        ResolvedRenderPlan {
            illustration,
            depth_of_field,
            motion_blur,
            bloom,
            backdrop,
            lighting,
            display,
        }
    }
}
