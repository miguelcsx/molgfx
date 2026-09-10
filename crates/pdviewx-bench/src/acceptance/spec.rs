//! Fixed acceptance-scene declarations from `docs/23-benchmarks.md`.

use pdviewx::{ImageConfig, RenderMode};
use serde::Serialize;

/// Stable golden-scene identity.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize)]
pub(crate) enum GoldenSceneId {
    #[serde(rename = "GS-001")]
    Gs001,
    #[serde(rename = "GS-002")]
    Gs002,
    #[serde(rename = "GS-003")]
    Gs003,
    #[serde(rename = "GS-004")]
    Gs004,
    #[serde(rename = "GS-005")]
    Gs005,
    #[serde(rename = "GS-006")]
    Gs006,
    #[serde(rename = "GS-007")]
    Gs007,
    #[serde(rename = "GS-008")]
    Gs008,
    #[serde(rename = "GS-009")]
    Gs009,
    #[serde(rename = "GS-010")]
    Gs010,
}

impl GoldenSceneId {
    /// Normative identifier.
    #[must_use]
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Gs001 => "GS-001",
            Self::Gs002 => "GS-002",
            Self::Gs003 => "GS-003",
            Self::Gs004 => "GS-004",
            Self::Gs005 => "GS-005",
            Self::Gs006 => "GS-006",
            Self::Gs007 => "GS-007",
            Self::Gs008 => "GS-008",
            Self::Gs009 => "GS-009",
            Self::Gs010 => "GS-010",
        }
    }
}

/// Declarative scene construction recipe.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum SceneRecipe {
    FocusPocket,
    CartoonLigand,
    Spacefill,
    TransparentSurface,
    CapsidRegion,
    SemanticLod,
    QualityPocket,
    Confidence,
    Difference,
    Publication4k,
}

/// One normative acceptance scene and its local fixture route.
#[derive(Clone, Copy, Debug)]
pub(crate) struct GoldenSceneSpec {
    pub(crate) id: GoldenSceneId,
    pub(crate) title: &'static str,
    pub(crate) recipe: SceneRecipe,
    pub(crate) default_fixture: Option<&'static str>,
    pub(crate) target_atoms: u64,
    pub(crate) config: ImageConfig,
    pub(crate) mode: RenderMode,
    pub(crate) gate_fps: Option<u32>,
    pub(crate) initial_frame_limit_ms: Option<u32>,
    pub(crate) fixture_requirement: &'static str,
}

const HD: ImageConfig = ImageConfig {
    width: 1_280,
    height: 960,
};

const GS001: GoldenSceneSpec = GoldenSceneSpec {
    id: GoldenSceneId::Gs001,
    title: "protein-ligand pocket",
    recipe: SceneRecipe::FocusPocket,
    default_fixture: Some("benchmarks/scenes/3PTB.cif"),
    target_atoms: 2_000,
    config: HD,
    mode: RenderMode::Realtime,
    gate_fps: Some(120),
    initial_frame_limit_ms: None,
    fixture_requirement: "fixture must contain a ligand; provider interactions and label payloads are required for full coverage",
};
const GS002: GoldenSceneSpec = GoldenSceneSpec {
    id: GoldenSceneId::Gs002,
    title: "enzyme cartoon and ligand",
    recipe: SceneRecipe::CartoonLigand,
    default_fixture: Some("benchmarks/scenes/4hhb.cif"),
    target_atoms: 30_000,
    config: HD,
    mode: RenderMode::Realtime,
    gate_fps: Some(120),
    initial_frame_limit_ms: None,
    fixture_requirement: "fixture must represent the approximately 30k-atom target scene",
};
const GS003: GoldenSceneSpec = GoldenSceneSpec {
    id: GoldenSceneId::Gs003,
    title: "large complex spacefill",
    recipe: SceneRecipe::Spacefill,
    default_fixture: Some("benchmarks/scenes/1AON.cif"),
    target_atoms: 100_000,
    config: HD,
    mode: RenderMode::Realtime,
    gate_fps: Some(60),
    initial_frame_limit_ms: None,
    fixture_requirement: "fixture must represent the approximately 100k-atom target scene",
};
const GS004: GoldenSceneSpec = GoldenSceneSpec {
    id: GoldenSceneId::Gs004,
    title: "transparent pocket surface",
    recipe: SceneRecipe::TransparentSurface,
    default_fixture: Some("benchmarks/scenes/4hhb.cif"),
    target_atoms: 30_000,
    config: HD,
    mode: RenderMode::Realtime,
    gate_fps: Some(90),
    initial_frame_limit_ms: None,
    fixture_requirement: "fixture must contain the target pocket and caller/provider surface contract",
};
const GS005: GoldenSceneSpec = GoldenSceneSpec {
    id: GoldenSceneId::Gs005,
    title: "viral capsid region",
    recipe: SceneRecipe::CapsidRegion,
    default_fixture: Some("benchmarks/scenes/6VXX.cif"),
    target_atoms: 1_000_000,
    config: HD,
    mode: RenderMode::Realtime,
    gate_fps: Some(60),
    initial_frame_limit_ms: None,
    fixture_requirement: "fixture must materialize the approximately 1M-atom capsid region",
};
const GS006: GoldenSceneSpec = GoldenSceneSpec {
    id: GoldenSceneId::Gs006,
    title: "ribosome or whole capsid",
    recipe: SceneRecipe::SemanticLod,
    default_fixture: None,
    target_atoms: 10_000_000,
    config: HD,
    mode: RenderMode::Realtime,
    gate_fps: Some(30),
    initial_frame_limit_ms: None,
    fixture_requirement: "explicit approximately 10M-atom fixture and paged LOD provider are required",
};
const GS007: GoldenSceneSpec = GoldenSceneSpec {
    id: GoldenSceneId::Gs007,
    title: "quality protein-ligand pocket",
    recipe: SceneRecipe::QualityPocket,
    default_fixture: Some("benchmarks/scenes/3PTB.cif"),
    target_atoms: 2_000,
    config: HD,
    mode: RenderMode::Cinematic,
    gate_fps: None,
    initial_frame_limit_ms: Some(100),
    fixture_requirement: "same complete semantic payload as GS-001 is required",
};
const GS008: GoldenSceneSpec = GoldenSceneSpec {
    id: GoldenSceneId::Gs008,
    title: "predicted model confidence",
    recipe: SceneRecipe::Confidence,
    default_fixture: None,
    target_atoms: 5_000,
    config: HD,
    mode: RenderMode::Realtime,
    gate_fps: None,
    initial_frame_limit_ms: None,
    fixture_requirement: "explicit predicted-model fixture with pLDDT in the deposited B-factor column is required",
};
const GS009: GoldenSceneSpec = GoldenSceneSpec {
    id: GoldenSceneId::Gs009,
    title: "two-pose difference",
    recipe: SceneRecipe::Difference,
    default_fixture: Some("benchmarks/scenes/3PTB.cif"),
    target_atoms: 2_000,
    config: HD,
    mode: RenderMode::Realtime,
    gate_fps: None,
    initial_frame_limit_ms: None,
    fixture_requirement: "caller correspondence uses stable rows only for topology-identical duplicated fixtures",
};
const GS010: GoldenSceneSpec = GoldenSceneSpec {
    id: GoldenSceneId::Gs010,
    title: "4K publication export",
    recipe: SceneRecipe::Publication4k,
    default_fixture: Some("benchmarks/scenes/4hhb.cif"),
    target_atoms: 30_000,
    config: ImageConfig {
        width: 3_840,
        height: 2_160,
    },
    mode: RenderMode::Cinematic,
    gate_fps: None,
    initial_frame_limit_ms: None,
    fixture_requirement: "fixture must represent the approximately 30k-atom publication scene",
};

/// Complete GS ladder without synthetic scientific substitutions.
#[must_use]
pub(crate) const fn golden_scene_specs() -> [GoldenSceneSpec; 10] {
    [
        GS001, GS002, GS003, GS004, GS005, GS006, GS007, GS008, GS009, GS010,
    ]
}
