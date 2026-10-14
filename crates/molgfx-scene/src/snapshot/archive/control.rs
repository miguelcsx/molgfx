use crate::Error;
use molframe::CancellationToken;

pub(super) fn check(cancellation: &CancellationToken) -> Result<(), Error> {
    if cancellation.is_cancelled() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::Interrupted,
            "scientific archive operation cancelled",
        )
        .into());
    }
    Ok(())
}

pub(super) fn collect<T>(
    values: impl IntoIterator<Item = T>,
    cancellation: &CancellationToken,
) -> Result<Vec<T>, Error> {
    let values = values.into_iter();
    let mut result = Vec::with_capacity(values.size_hint().0);
    for (index, value) in values.enumerate() {
        if index.is_multiple_of(1024) {
            check(cancellation)?;
        }
        result.push(value);
    }
    check(cancellation)?;
    Ok(result)
}

pub(super) fn surface(
    value: &crate::MolecularSurfaceGeometry,
    cancellation: &CancellationToken,
) -> Result<crate::source::MolecularSurfaceDescription, Error> {
    let mesh = value.mesh();
    Ok(crate::source::MolecularSurfaceDescription {
        vertices: collect(mesh.geometry.vertices.iter().copied(), cancellation)?,
        normals: collect(mesh.geometry.vertex_normals.iter().copied(), cancellation)?,
        faces: collect(mesh.geometry.faces.iter().map(|face| face.0), cancellation)?,
        atom_rows: collect(mesh.atom_rows.iter().copied(), cancellation)?,
        discarded_degenerate_faces: mesh.discarded_degenerate_faces,
    })
}
