use super::*;

#[test]
fn full_width_line_encodes_visible_cells_without_control_bytes() {
    let mapping = BTreeMap::from([('가', 1), ('나', 2)]);
    assert_eq!(line(&"가나".repeat(7), &mapping).unwrap(), [1, 2].repeat(7));
}

#[test]
fn line_rejects_overflow_controls_and_unmapped_characters() {
    let mapping = BTreeMap::from([('가', 1)]);
    for invalid in [
        "".to_owned(),
        "가".repeat(15),
        "가\n".to_owned(),
        "나".to_owned(),
    ] {
        assert!(line(&invalid, &mapping).is_err());
    }
}

#[test]
fn shorter_name_erases_all_reveal_cells_at_the_original_position() {
    let mapping = BTreeMap::from([('가', 1), ('나', 2), (' ', 9)]);
    assert_eq!(
        name_rows(&[0xb0, 3, 4, 5, 6, 7], "가나", &mapping).unwrap(),
        vec![0xb0, 9, 1, 2, 9, 9]
    );
    assert!(name_rows(&[3, 0xfe, 4], "가", &mapping).is_err());
}

#[test]
fn longer_name_moves_left_and_still_covers_old_cells() {
    let mapping = BTreeMap::from([('가', 1), (' ', 9)]);
    // Old: x=16+48, two cells. New four cells start one cell left and end one cell right.
    assert_eq!(name_rows(&[0xb0, 3, 4], "가가가가", &mapping).unwrap(), vec![0xa0, 1, 1, 1, 1]);
    // A title-row name keeps bit 6 on its first position byte even at the left edge.
    assert_eq!(name_rows(&[0xc0, 3], "가가", &mapping).unwrap(), vec![0xc0, 1, 1]);
    assert!(name_rows(&[0xb0, 3], &"가".repeat(15), &mapping).is_err());
}

#[test]
fn multi_row_names_keep_row_breaks() {
    let mapping = BTreeMap::from([('가', 1), (' ', 9)]);
    assert_eq!(
        name_rows(&[0xb0, 3, 0xfe, 0xa0, 4], "가\n가", &mapping).unwrap(),
        vec![0xb0, 1, 0xfe, 0xa0, 1]
    );
    assert!(name_rows(&[0xb0, 3, 0xfe, 0xa0, 4], "가", &mapping).is_err());
}
