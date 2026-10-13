use crate::placement::rigid;

#[test]
fn rotation_is_transposed_into_column_major_storage() {
    let rotation = [[0.0, -1.0, 0.0], [1.0, 0.0, 0.0], [0.0, 0.0, 1.0]];
    let matrix = rigid(rotation, [1.0, 2.0, 3.0]);
    assert_eq!(matrix[..3], [0.0, 1.0, 0.0]);
    assert_eq!(matrix[4..7], [-1.0, 0.0, 0.0]);
    assert_eq!(matrix[12..], [1.0, 2.0, 3.0, 1.0]);
}
