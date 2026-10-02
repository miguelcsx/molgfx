use super::*;

fn tile(width: usize, height: usize, value: u8) -> Tile {
    Tile {
        width,
        height,
        rgba: vec![value; width * height * 4],
    }
}

fn pixel(sheet: &(usize, usize, Vec<u8>), x: usize, y: usize) -> u8 {
    sheet.2[(y * sheet.0 + x) * 4]
}

#[test]
fn tiles_sit_left_to_right_separated_by_a_white_gap() {
    let sheet = compose(&[Some(tile(4, 3, 10)), Some(tile(4, 3, 20))], [4, 3]);
    assert_eq!((sheet.0, sheet.1), (4 + GAP + 4, 3));
    assert_eq!(pixel(&sheet, 0, 0), 10);
    assert_eq!(pixel(&sheet, 4, 1), u8::MAX);
    assert_eq!(pixel(&sheet, 4 + GAP, 2), 20);
}

#[test]
fn a_missing_recipe_image_becomes_a_mid_grey_tile_of_the_case_extent() {
    let sheet = compose(&[Some(tile(2, 2, 10)), None], [3, 2]);
    assert_eq!((sheet.0, sheet.1), (2 + GAP + 3, 2));
    assert_eq!(pixel(&sheet, 2 + GAP, 1), MISSING_GREY);
}
