use super::*;

#[test]
fn bdf_places_pixels_with_bearings_and_baseline() {
    let bdf = "STARTCHAR example\nENCODING 44032\nBBX 3 2 2 1\nBITMAP\nA0\n40\nENDCHAR\n";
    let glyphs = read_glyphs(bdf, &['가']).unwrap();
    assert_eq!(glyphs[&'가'], [0, 0, 0, 0, 0x28, 0x10, 0, 0]);
    assert!(read_glyphs(&bdf.replace("3 2 2 1", "3 2 6 1"), &['가']).is_err());
    assert!(read_glyphs(&bdf.replace("A0\n40", "A0"), &['가']).is_err());
    assert!(read_glyphs(bdf, &['나']).is_err());
    assert!(read_glyphs(&format!("{bdf}{bdf}"), &['가']).is_err());
}
