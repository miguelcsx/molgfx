//! One-call pocket-and-pose composition around a focused subject.

use crate::color::uniform;
use crate::representation::{Selection, SurfaceKind, SurfaceStyle};
use crate::{Color, Error, RepresentationSpec, StructureId, rep, sel};

/// Distances and presentation for [`pocket_representations`].
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct PocketStyle {
    /// Radius of the interaction shell in ångström.
    pub near: f32,
    /// Radius of the orienting shell in ångström; beyond it is far context.
    pub mid: f32,
    /// Pocket-surface opacity.
    pub pocket_opacity: f32,
    /// Far-context opacity.
    pub context_opacity: f32,
    /// Local-solvent opacity.
    pub solvent_opacity: f32,
    /// Pocket-surface colour.
    pub pocket_color: Color,
    /// Far-context colour.
    pub context_color: Color,
    /// Local-solvent colour.
    pub solvent_color: Color,
}

impl Default for PocketStyle {
    fn default() -> Self {
        Self {
            near: 4.0,
            mid: 10.0,
            pocket_opacity: 0.34,
            context_opacity: 0.30,
            solvent_opacity: 0.42,
            pocket_color: Color::rgb(95, 126, 132),
            context_color: Color::rgb(96, 116, 138),
            solvent_color: Color::rgb(72, 140, 156),
        }
    }
}

impl PocketStyle {
    fn validate(self) -> Result<(), Error> {
        let opacities = [
            self.pocket_opacity,
            self.context_opacity,
            self.solvent_opacity,
        ];
        let ordered = self.near.is_finite()
            && self.mid.is_finite()
            && 0.0 <= self.near
            && self.near < self.mid;
        if !ordered {
            return Err(Error::InvalidSpec(
                "pocket distances must be finite and satisfy 0 <= near < mid".to_owned(),
            ));
        }
        if opacities
            .iter()
            .any(|opacity| !opacity.is_finite() || !(0.0..=1.0).contains(opacity))
        {
            return Err(Error::InvalidSpec(
                "pocket opacities must lie in the closed interval zero to one".to_owned(),
            ));
        }
        Ok(())
    }
}

/// Builds the focus-and-context representations around `focus`.
///
/// Bands are distances to the focused atoms: the focus draws at full detail, the
/// complete residues within `near` as thin ball-and-stick, the residues out to
/// `mid` as faint points, and everything beyond as a demoted tube. A translucent
/// solvent-excluded surface bounds the pocket, and water inside the orienting
/// shell is kept as sparse context; water farther away is never drawn.
///
/// The result is ordinary representation specifications, so every piece is
/// independently editable and one scene patch adds or removes the lot.
///
/// # Errors
///
/// Returns an error for non-finite or unordered distances or an opacity outside
/// zero to one.
pub fn pocket_representations(
    focus: &Selection,
    structure: StructureId,
    style: PocketStyle,
) -> Result<Vec<RepresentationSpec>, Error> {
    style.validate()?;
    let focus_source = focus.source();
    let water = sel::water();
    let water = water.source();
    let near_shell = residues_within(style.near, focus_source);
    let mid_shell = residues_within(style.mid, focus_source);
    let solvent = both(&mid_shell, water);
    let near = both(&both(&near_shell, &not(focus_source)), &not(water));
    let mid = both(&both(&mid_shell, &not(&near_shell)), &not(water));
    let context = both(&not(&mid_shell), &not(water));
    let pocket = both(
        &both(&within(style.near, focus_source), &not(focus_source)),
        &not(water),
    );
    Ok(vec![
        rep::tube(context)
            .radius(0.16)
            .opacity(style.context_opacity)
            .color(uniform(style.context_color))
            .structure(structure)
            .into(),
        rep::points(mid)
            .size(2.0)
            .opacity(0.08)
            .structure(structure)
            .into(),
        rep::surface(pocket)
            .kind(SurfaceKind::SolventExcluded)
            .style(SurfaceStyle::Solid)
            .opacity(style.pocket_opacity)
            .color(uniform(style.pocket_color))
            .structure(structure)
            .into(),
        rep::ball_and_stick(near)
            .radius(0.11)
            .bond_radius(0.07)
            .opacity(0.48)
            .structure(structure)
            .into(),
        rep::spacefill(solvent)
            .radius(0.24)
            .opacity(style.solvent_opacity)
            .color(uniform(style.solvent_color))
            .structure(structure)
            .into(),
        rep::ball_and_stick(focus.clone())
            .structure(structure)
            .into(),
    ])
}

// Composed in the same canonical text the typed builder produces, so a caller's
// textual focus query nests without being parsed here; the planner compiles it.
fn within(radius: f32, target: &str) -> String {
    format!("within {radius} of ({target})")
}

fn residues_within(radius: f32, target: &str) -> String {
    format!("byres ({})", within(radius, target))
}

fn both(left: &str, right: &str) -> String {
    format!("({left}) and ({right})")
}

fn not(target: &str) -> String {
    format!("not ({target})")
}
