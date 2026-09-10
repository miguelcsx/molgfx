//! Complete molecular-representation catalogue for the interactive viewer.

use pdviewx::{Representation, RepresentationKind, SurfaceKind, SurfaceStyle, TubeRadiusMapping};
use winit::keyboard::KeyCode;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum RepresentationChoice {
    Kind(RepresentationKind),
    Putty,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum CycleDirection {
    Previous,
    Next,
}

const CHOICES: [RepresentationChoice; 14] = [
    RepresentationChoice::Kind(RepresentationKind::Spacefill),
    RepresentationChoice::Kind(RepresentationKind::BallAndStick),
    RepresentationChoice::Kind(RepresentationKind::Cartoon),
    RepresentationChoice::Kind(RepresentationKind::Licorice),
    RepresentationChoice::Kind(RepresentationKind::Lines),
    RepresentationChoice::Kind(RepresentationKind::Surface),
    RepresentationChoice::Kind(RepresentationKind::Twister),
    RepresentationChoice::Kind(RepresentationKind::Trace),
    RepresentationChoice::Kind(RepresentationKind::Tube),
    RepresentationChoice::Kind(RepresentationKind::Rocket),
    RepresentationChoice::Kind(RepresentationKind::Beads),
    RepresentationChoice::Kind(RepresentationKind::Points),
    RepresentationChoice::Kind(RepresentationKind::PaperChain),
    RepresentationChoice::Putty,
];

pub(super) const fn choices() -> &'static [RepresentationChoice] {
    &CHOICES
}

pub(super) fn choice_for_key(key: KeyCode) -> Option<RepresentationChoice> {
    Some(match key {
        KeyCode::Digit1 => CHOICES[0],
        KeyCode::Digit2 => CHOICES[1],
        KeyCode::Digit3 => CHOICES[2],
        KeyCode::Digit4 => CHOICES[3],
        KeyCode::Digit5 => CHOICES[4],
        KeyCode::Digit6 => CHOICES[5],
        KeyCode::Digit7 => CHOICES[6],
        KeyCode::Digit8 => CHOICES[7],
        KeyCode::Digit9 => CHOICES[8],
        KeyCode::Digit0 => CHOICES[9],
        KeyCode::KeyB => CHOICES[10],
        KeyCode::KeyO => CHOICES[11],
        KeyCode::KeyG => CHOICES[12],
        KeyCode::KeyU => CHOICES[13],
        _ => return None,
    })
}

pub(super) fn cycled_choice(index: usize, direction: CycleDirection) -> RepresentationChoice {
    let index = match direction {
        CycleDirection::Previous => match index.checked_sub(1) {
            Some(value) => value,
            None => CHOICES.len() - 1,
        },
        CycleDirection::Next => (index + 1) % CHOICES.len(),
    };
    CHOICES[index]
}

impl RepresentationChoice {
    pub(super) fn index(self) -> usize {
        CHOICES
            .iter()
            .position(|candidate| *candidate == self)
            .map_or(0, |index| index)
    }

    pub(super) const fn name(self) -> &'static str {
        match self {
            Self::Kind(kind) => kind.stable_name(),
            Self::Putty => "putty",
        }
    }

    pub(super) fn apply(self, representation: &mut Representation, domain: Option<[f32; 2]>) {
        let kind = match self {
            Self::Kind(kind) => kind,
            Self::Putty => RepresentationKind::Tube,
        };
        let defaults = Representation::new(representation.target, kind);
        representation.params = defaults.params;
        representation.material = defaults.material;
        representation.kind = kind;
        if kind == RepresentationKind::Surface {
            // The interactive catalogue promises a smooth, resolution-
            // independent surface even for very large structures. SAS is the
            // exact analytic union of probe-inflated atoms and needs no dense
            // 3D field; SES remains available through the explicit API where
            // its rolling-probe semantics are requested deliberately.
            representation.params.surface_kind = SurfaceKind::SolventAccessible;
            representation.params.surface_style = SurfaceStyle::SoftUnion;
        }
        if self == Self::Putty
            && let Some(domain) = domain
            && let Ok(mapping) = TubeRadiusMapping::b_factor(domain, [0.18, 0.72])
        {
            representation.params.tube_radius_mapping = mapping;
        }
    }
}

#[cfg(test)]
#[path = "catalog_tests.rs"]
mod tests;
