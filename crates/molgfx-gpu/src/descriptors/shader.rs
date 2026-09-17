//! Shader module descriptors.

/// A shader module from WGSL source. WGSL is the single shader language;
/// backends that need another form compile it themselves.
#[derive(Clone, Copy, Debug)]
pub struct ShaderModuleDesc<'a> {
    /// Debug label; also names the module in compile errors.
    pub label: &'static str,
    /// The WGSL source text.
    pub wgsl: &'a str,
}
