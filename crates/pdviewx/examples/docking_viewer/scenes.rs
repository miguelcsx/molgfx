//! Scene construction for the docking viewer modes.

#[cfg(test)]
#[path = "scenes_tests.rs"]
mod tests;

use super::{App, common};
use pdviewx::{AtomSelection, ColorScheme, Mat4, RepresentationKind, Scene, Vec3};

#[cfg(feature = "semantic")]
use super::HoloState;
#[cfg(feature = "semantic")]
use pdviewx::{
    Aabb, BoundingSphere, ClipPlane, ClipSet, FocusScene, FocusStyle, FocusSurfaceExtent, Rgba8,
    SurfaceStyle,
};

pub(super) fn build_plain_scene(path: &str) -> Result<(App, String), String> {
    let structure = common::read_structure(path)?;
    let mut scene = Scene::new();
    let handle = scene
        .add_structure(&structure)
        .map_err(|error| format!("scene rejected structure ({}): {error}", error.code()))?;
    let records = common::deposited_secondary_structure(path, &structure);
    if !records.is_empty() {
        let _ = scene.apply_secondary_structure(handle, &records);
    }
    let selection = match scene.add_structure_selection(handle, AtomSelection::All) {
        Ok(selection) => selection,
        Err(_) => scene.add_selection(AtomSelection::All),
    };
    let representation = scene
        .represent(selection, RepresentationKind::Cartoon)
        .or_else(|_| scene.represent(selection, RepresentationKind::Spacefill))
        .map_err(|error| format!("representation failed ({}): {error}", error.code()))?;
    if let Some(value) = scene.representation_mut(representation) {
        value.color = ColorScheme::ByChain;
    }
    let bound = scene.world_aabb().bounding_sphere();
    let app = App {
        scene,
        representations: vec![representation],
        representation_index: 2,
        surface_style_index: 0,
        putty_domain: guide_b_factor_domain(&structure),
        color_index: 1,
        bound,
        state: None,
        holo: None,
    };
    Ok((
        app,
        format!("loaded {path}: {} atoms", structure.data().atoms().count()),
    ))
}

#[cfg(feature = "semantic")]
pub(super) fn build_holo_scene(path: &str, ligand_name: &str) -> Result<(App, String), String> {
    let parsed = pdbiox::read(path)
        .map_err(|diagnostic| format!("could not read {path}: {diagnostic:?}"))?;
    let structure = pdbiox::infer_bonds(&parsed, pdbiox::BondInference::default())
        .map_err(|diagnostic| format!("bond inference failed: {diagnostic:?}"))?
        .structure;
    let selection = ligand_selection(&structure, ligand_name)?;
    let ligand_bound = Aabb::from_points(selection.points.iter().copied());
    let view_direction = ligand_view_direction(&selection.points, ligand_bound.center());
    let mut scene = Scene::from_structure(&structure)
        .map_err(|error| format!("scene from structure failed ({}): {error}", error.code()))?;
    let ligand = scene.add_selection(AtomSelection::Sparse(selection.atoms));
    let focus = scene
        .focus_with(
            ligand,
            FocusStyle {
                context_opacity: 0.30,
                context_color: Rgba8::opaque(74, 78, 82),
                solvent_opacity: 0.0,
                surface_extent: FocusSurfaceExtent::Pocket,
                ..FocusStyle::default()
            },
        )
        .map_err(|error| format!("focus failed ({}): {error}", error.code()))?;
    let cut = ClipPlane::from_point_normal(ligand_bound.center(), -view_direction)
        .map_err(|_| "invalid clip plane".to_owned())?;
    let cut_set = ClipSet::new(&[cut]).map_err(|_| "invalid clip set".to_owned())?;
    if let Some(surface) = scene.representation_mut(focus.pocket_representation) {
        surface.clipping = cut_set;
        surface.params.surface_style = SurfaceStyle::Solid;
        surface.color = ColorScheme::Uniform(Rgba8::opaque(48, 68, 67));
        surface.material.opacity = 1.0;
        surface.material.roughness = 0.74;
        surface.material.specular = 0.14;
    }
    let lining_opacity = 0.6;
    if let Some(lining) = scene.representation_mut(focus.near_representation) {
        lining.kind = RepresentationKind::Lines;
        lining.params.line_width_pixels = 1.4;
        lining.material.opacity = lining_opacity;
    }
    if let Some(ligand_representation) = scene.representation_mut(focus.focus_representation) {
        ligand_representation.params.radius_scale = 0.34;
        ligand_representation.params.bond_radius = 0.20;
        ligand_representation.material.roughness = 0.38;
        ligand_representation.material.specular = 0.34;
        ligand_representation.order = 1;
    }
    let ligand_sphere = BoundingSphere::from_points(&selection.points);
    let frame = BoundingSphere {
        center: ligand_sphere.center,
        radius: ligand_sphere.radius * 1.55,
    };
    let pocket_opacity = scene
        .representation(focus.pocket_representation)
        .map_or(1.0, |value| value.material.opacity);
    let holo = HoloState {
        pocket: focus.pocket_representation,
        lining: focus.near_representation,
        _ligand: focus.focus_representation,
        cut_center: ligand_bound.center(),
        cut_normal: view_direction,
        cut_offset: 0.0,
        pocket_on: true,
        lining_on: true,
        pocket_opacity,
        lining_opacity,
    };
    let app = App {
        scene,
        representations: vec![focus.context_representation],
        representation_index: 8,
        surface_style_index: 0,
        putty_domain: guide_b_factor_domain(&structure),
        color_index: 1,
        bound: frame,
        state: None,
        holo: Some(holo),
    };
    let description = format!(
        "holo view: {path} + ligand '{ligand_name}' ({} atoms)",
        structure.data().atoms().count(),
    );
    Ok((app, description))
}

#[cfg(not(feature = "semantic"))]
pub(super) fn build_holo_scene(_path: &str, _ligand_name: &str) -> Result<(App, String), String> {
    Err("holo mode requires `--features semantic`".to_owned())
}

pub(super) fn build_compare_scene(paths: &[String]) -> Result<(App, String), String> {
    let mut scene = Scene::new();
    let mut structures = Vec::with_capacity(paths.len());
    let mut handles = Vec::with_capacity(paths.len());
    for path in paths {
        let structure = common::read_structure(path)?;
        let handle = scene
            .add_structure(&structure)
            .map_err(|error| format!("scene rejected {path} ({}): {error}", error.code()))?;
        structures.push(structure);
        handles.push(handle);
    }
    for ((path, structure), handle) in paths.iter().zip(&structures).zip(&handles) {
        let records = common::deposited_secondary_structure(path, structure);
        if !records.is_empty() {
            let _ = scene.apply_secondary_structure(*handle, &records);
        }
    }
    let first = handles
        .first()
        .copied()
        .ok_or_else(|| "comparison needs at least one structure".to_owned())?;
    let spacing = scene
        .structure(first)
        .map_or(20.0, |placed| placed.world_aabb().half_extents().x * 2.0)
        + 8.0;
    let count = u16::try_from(handles.len())
        .map_err(|_| "comparison supports at most 65535 structures".to_owned())?;
    let center = f32::from(count.saturating_sub(1)) * 0.5;
    let mut representations = Vec::with_capacity(handles.len());
    for (index, handle) in handles.iter().enumerate() {
        let index = u16::try_from(index).map_err(|_| "comparison index overflow".to_owned())?;
        let offset = (f32::from(index) - center) * spacing;
        let placed = scene
            .structure_mut(*handle)
            .ok_or_else(|| "structure became stale".to_owned())?;
        placed.model_to_world = Mat4::from_translation(Vec3::new(offset, 0.0, 0.0));
        let selection = match scene.add_structure_selection(*handle, AtomSelection::All) {
            Ok(selection) => selection,
            Err(_) => scene.add_selection(AtomSelection::All),
        };
        let value = scene
            .represent(selection, RepresentationKind::Cartoon)
            .or_else(|_| scene.represent(selection, RepresentationKind::Spacefill))
            .map_err(|error| format!("representation failed ({}): {error}", error.code()))?;
        if let Some(item) = scene.representation_mut(value) {
            item.color = ColorScheme::ByChain;
        }
        representations.push(value);
    }
    if representations.is_empty() {
        return Err("no representations created".to_owned());
    }
    let putty_domain = structures
        .iter()
        .filter_map(guide_b_factor_domain)
        .reduce(|left, right| [left[0].min(right[0]), left[1].max(right[1])]);
    let bound = scene.world_aabb().bounding_sphere();
    Ok((
        App {
            scene,
            representations,
            representation_index: 2,
            surface_style_index: 0,
            putty_domain,
            color_index: 1,
            bound,
            state: None,
            holo: None,
        },
        format!("comparison view: {} structures side by side", paths.len()),
    ))
}

fn guide_b_factor_domain(structure: &pdbiox::Structure) -> Option<[f32; 2]> {
    let mut domain: Option<[f32; 2]> = None;
    for atom in structure
        .data()
        .atoms()
        .filter(|atom| matches!(atom.name(), Some("CA" | "C4'")))
    {
        let Some(value) = atom.b_factor().filter(|value| value.is_finite()) else {
            continue;
        };
        domain = Some(match domain {
            Some([minimum, maximum]) => [minimum.min(value), maximum.max(value)],
            None => [value, value],
        });
    }
    domain.filter(|domain| domain[0] < domain[1])
}

#[cfg(feature = "semantic")]
struct LigandSelection {
    atoms: Vec<u32>,
    points: Vec<Vec3>,
}

#[cfg(feature = "semantic")]
fn ligand_selection(
    structure: &pdbiox::Structure,
    ligand_name: &str,
) -> Result<LigandSelection, String> {
    let Some(residue) = structure
        .data()
        .residues()
        .find(|residue| residue.name() == Some(ligand_name))
    else {
        return Err(format!("component {ligand_name} is absent"));
    };
    let atoms: Vec<u32> = residue.atoms().map(|atom| atom.index().get()).collect();
    let points: Vec<Vec3> = atoms
        .iter()
        .filter_map(|index| {
            structure
                .positions()
                .get(usize::try_from(*index).ok()?)
                .copied()
                .map(Vec3::from_array)
        })
        .collect();
    if atoms.is_empty() || points.is_empty() {
        return Err("ligand selection has no positioned atoms".to_owned());
    }
    Ok(LigandSelection { atoms, points })
}

#[cfg(feature = "semantic")]
fn ligand_view_direction(points: &[Vec3], center: Vec3) -> Vec3 {
    let primary = points
        .iter()
        .map(|point| *point - center)
        .max_by(|left, right| left.length_squared().total_cmp(&right.length_squared()))
        .map_or(Vec3::X, |direction| direction);
    let secondary = points
        .iter()
        .map(|point| *point - center)
        .max_by(|left, right| {
            primary
                .cross(*left)
                .length_squared()
                .total_cmp(&primary.cross(*right).length_squared())
        })
        .map_or(Vec3::Y, |direction| direction);
    let normal = primary.cross(secondary).normalize_or_zero();
    let normal = if normal.length_squared() < 0.5 {
        Vec3::Z
    } else {
        normal
    };
    if normal.dot(Vec3::Z) < 0.0 {
        -normal
    } else {
        normal
    }
}
