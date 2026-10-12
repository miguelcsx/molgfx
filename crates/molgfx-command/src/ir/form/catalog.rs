//! The molecular form catalogue and controls.

use super::{
    CartoonProfile, CartoonStyle, ContentHash, ControlValue, Finite, GapStyle, Look, NonNegative,
    OptionError, OptionInfo, Positive, RepresentationSpec, SurfaceKind, SurfaceStyle,
};
use serde::{Deserialize, Serialize};

forms! {
    /// Fitted circular helices and discrete beta cells.
    Cartoon = "cartoon" via cartoon {
        gaps: GapStyle => "presentation of real polymer gaps",
        width: Positive => "ribbon width in ångström",
        helix_radius: Positive => "fitted circular helix radius in ångström",
        style: CartoonStyle => "ribbon recipe",
        aspect_ratio: Positive => "helix and sheet width divided by thickness",
        arrow_factor: NonNegative => "strand arrow shoulder scale; zero disables arrows",
        direction_wedges: bool => "source-anchored polymer direction wedges",
        helix_profile: CartoonProfile => "protein helix cross-section",
        nucleic_profile: CartoonProfile => "nucleic backbone cross-section",
    }
    /// Peptide-oriented secondary-structure ribbons along the curved backbone.
    NewCartoon = "new_cartoon" via new_cartoon {
        gaps: GapStyle => "presentation of real polymer gaps",
        width: Positive => "ribbon width in ångström",
        aspect_ratio: Positive => "helix and sheet width divided by thickness",
        arrow_factor: NonNegative => "strand arrow shoulder scale; zero disables arrows",
        direction_wedges: bool => "source-anchored polymer direction wedges",
        helix_profile: CartoonProfile => "protein helix cross-section",
        nucleic_profile: CartoonProfile => "nucleic backbone cross-section",
    }
    /// Smooth atom-centred Gaussian density boundary.
    QuickSurf = "quick_surf" via quick_surf {
        radius: Positive => "van der Waals radius scale",
        sigma: Positive => "Gaussian standard deviation relative to atomic radius",
        isolevel: Positive => "density threshold",
        grid_spacing: Positive => "requested field spacing in ångström",
        cutoff: Positive => "Gaussian support in standard deviations",
    }
    /// Neighbor triplets around selected central atoms.
    Polyhedra = "polyhedra" via polyhedra {
        cutoff: Positive => "central-atom-to-neighbor distance cutoff in ångström",
    }
    /// Flat sheet with two independently splined border tubes.
    Ribbons = "ribbons" via ribbons {
        width: Positive => "ribbon width in ångström",
        edge_radius: NonNegative => "border tube radius in ångström",
        gaps: GapStyle => "presentation of real polymer gaps",
    }
    /// Constant-width peptide-oriented polymer ribbon.
    NewRibbons = "new_ribbons" via new_ribbons {
        width: Positive => "ribbon width in ångström",
        aspect_ratio: Positive => "width divided by thickness",
        profile: CartoonProfile => "extruded ribbon cross-section",
        gaps: GapStyle => "presentation of real polymer gaps",
    }
    /// Backbone trace through polymer guide atoms.
    Backbone = "backbone" via backbone {
        gaps: GapStyle => "presentation of real polymer gaps",
        width: Positive => "trace radius in ångström",
    }
    /// Straight segments between polymer guide atoms.
    Trace = "trace" via trace {
        gaps: GapStyle => "presentation of real polymer gaps",
        radius: NonNegative => "trace radius in ångström; zero draws a pixel-width line",
    }
    /// Round smooth polymer tube.
    Tube = "tube" via tube {
        gaps: GapStyle => "presentation of real polymer gaps",
        radius: NonNegative => "tube radius in ångström; zero draws a pixel-width line",
    }
    /// B-factor-driven variable-radius tube.
    Putty = "putty" via putty {
        gaps: GapStyle => "presentation of real polymer gaps",
        domain_min: Finite => "lower B-factor domain",
        domain_max: Finite => "upper B-factor domain",
        radius_min: Positive => "radius at the lower domain bound",
        radius_max: Positive => "radius at the upper domain bound",
    }
    /// Small atom spheres joined by bond capsules.
    BallAndStick = "ball_and_stick" via ball_and_stick {
        radius: Positive => "atom sphere scale relative to the van der Waals radius",
        bond_radius: Positive => "bond radius in ångström",
    }
    /// Van der Waals spheres.
    Spacefill = "spacefill" via spacefill {
        radius: Positive => "sphere scale relative to the van der Waals radius",
    }
    /// Uniform-radius sticks.
    Licorice = "licorice" via licorice {
        radius: Positive => "atom junction scale",
        bond_radius: Positive => "stick radius in ångström",
    }
    /// Distance-only links on the active interpolated coordinates.
    DynamicBonds = "dynamic_bonds" via dynamic_bonds {
        cutoff: Positive => "instantaneous pairwise distance cutoff in ångström",
        bond_radius: Positive => "capsule radius in ångström",
    }
    /// Bond capsules without atom spheres.
    Bonds = "bonds" via bonds {
        bond_radius: Positive => "bond radius in ångström",
    }
    /// Pucker-coloured ring bipyramids.
    PaperChain = "paper_chain" via paper_chain {
        height: Positive => "total bipyramid height in ångström",
        max_ring_size: u8 => "largest molecular ring, between three and twelve atoms",
    }
    /// Pixel-stable bond wires.
    Lines = "lines" via lines {
        width: Positive => "line width in pixels",
    }
    /// One point per atom.
    Points = "points" via points {
        size: Positive => "point diameter in pixels",
    }
    /// Analytic point/dot markers.
    Dots = "dots" via dots {
        size: Positive => "dot diameter in pixels",
        samples: u16 => "samples per atomic sphere, between one and 4096",
    }
    /// Sampled solvent-accessible atomic boundary.
    Solvent = "solvent" via solvent {
        probe_radius: Positive => "solvent probe radius in ångström",
        size: Positive => "sample diameter in physical pixels",
        samples: u16 => "samples per probe-inflated sphere, between one and 4096",
    }
    /// A molecular surface.
    Surface = "surface" via surface {
        kind: SurfaceKind => "which molecular boundary",
        style: SurfaceStyle => "how the boundary is drawn",
        probe_radius: Positive => "solvent probe radius in ångström",
        isolevel: Finite => "level-set threshold",
        blob_spread: NonNegative => "soft-union blend span in ångström",
    }
    /// Varshney rolling-probe triangles from a separately bound scientific result.
    Surf = "surf" via surf(molgfx_scene::DataSource::new("")) {
        source: ContentHash => "content identity of the separately bound scientific geometry",
        probe_radius: Positive => "rolling solvent-probe radius in ångström",
        wire: bool => "display the actual triangle edges",
        width: Positive => "triangle-edge width in physical pixels",
    }
    /// Sanner rolling-probe triangulation from a separately bound scientific result.
    Msms = "msms" via msms(molgfx_scene::DataSource::new("")) {
        source: ContentHash => "content identity of the separately bound scientific geometry",
        probe_radius: Positive => "rolling solvent-probe radius in ångström",
        density: Positive => "vertices per square ångström",
        all_components: bool => "include disconnected components and internal cavities",
        wire: bool => "display the actual triangle edges",
        width: Positive => "triangle-edge width in physical pixels",
    }
    /// Nucleic-acid backbone ribbon.
    NucleicAcid = "nucleic_acid" via nucleic_acid {
        width: Positive => "ribbon width in ångström",
    }
    /// Nucleic-acid bases.
    Bases = "bases" via bases {
        radius: Positive => "atom sphere scale",
    }
    /// Base-pair slabs.
    BasePairs = "base_pairs" via base_pairs {
        radius: Positive => "atom sphere scale",
        bond_radius: Positive => "bond radius in ångström",
    }
    /// Glycan tree ribbon.
    Glycan = "glycan" via glycan {
        width: Positive => "ribbon width in ångström",
    }
    /// One sphere per residue.
    Beads = "beads" via beads {
        radius: Positive => "enclosing-sphere scale",
    }
}
