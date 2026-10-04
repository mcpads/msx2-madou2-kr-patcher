use super::*;

#[test]
fn sparse_names_resolve_to_bounded_strings_and_leave_fallback_holes() {
    let names = BTreeMap::from([(1, vec![0x21, 255]), (3, vec![1, 255, 255])]);
    let data = pack_names(&names).unwrap();
    assert_eq!(&data[0..4], &[0; 4]);
    assert_eq!(&data[8..12], &[0; 4]);
    for (index, expected) in names {
        let start =
            usize::from(u16::from_le_bytes([data[index * 4], data[index * 4 + 1]])) - 0x7c00;
        let len = usize::from(u16::from_le_bytes([
            data[index * 4 + 2],
            data[index * 4 + 3],
        ]));
        assert_eq!(&data[start..start + len], expected);
    }
    assert!(pack_names(&BTreeMap::from([(0, vec![255; 1021])])).is_err());
    assert!(pack_names(&BTreeMap::from([(0, vec![0x21])])).is_err());
}
