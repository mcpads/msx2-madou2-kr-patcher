use super::*;

#[test]
fn packs_rows_into_two_byte_cells_across_cell_bands() {
    let mut rows = [0; 16];
    rows[0] = 0x8041;
    rows[15] = 0x1234;
    let packed = pack(32, &BTreeMap::from([(0x80, rows), (0x90, rows)])).unwrap();
    assert_eq!(&packed[..4], &[0, 4, 0x80, 0x41]);
    assert_eq!(&packed[482..484], &[0x12, 0x34]);
    assert_eq!(&packed[514..516], &[0x80, 0x41]);
    assert_eq!(&packed[994..996], &[0x12, 0x34]);
    assert!(packed[4..34].iter().all(|&b| b == 0));
    for (cells, code) in [(16, 0x90), (128, 0xfc), (16, 0x7f)] {
        assert!(pack(cells, &BTreeMap::from([(code, rows)])).is_err());
    }
}
