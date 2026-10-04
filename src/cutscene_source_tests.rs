use super::*;

#[test]
fn preserves_escaped_glyphs_controls_and_terminal_boundary() {
    let bytes = [
        16, 120, 0x80, 0xff, 0x81, 0xfd, 0xff, 0xfc, 0xff, 0xfe, 0xff, 0xfb, 8, 0xff, 0xff, 0x42,
    ];
    let (tokens, end) = text(&bytes, 0x4000).unwrap();
    assert_eq!(end, 15);
    assert_eq!(
        tokens.iter().map(|t| t.kind).collect::<Vec<_>>(),
        [
            "glyph",
            "glyph",
            "delay_tick",
            "clear_window",
            "newline",
            "newline_offset",
            "end"
        ]
    );
    assert_eq!(tokens[1].code, Some(0x81));
    assert_eq!(tokens[5].offset, Some(8));
    assert_eq!(
        tokens
            .iter()
            .map(|t| t.raw_hex.as_str())
            .collect::<String>(),
        "80ff81fdfffcfffefffb08ffff"
    );
}

#[test]
fn rejects_truncated_dialogue_and_invalid_script_paths() {
    for data in [
        &[16, 120, 0xff][..],
        &[16, 120, 0xff, 0xfb],
        &[16, 120, 0x80],
    ] {
        assert!(text(data, 0x4000).is_err());
    }
    assert!(text(&[0; 8], usize::MAX).is_err());
    assert!(text(&[0; 8], 0x3fff).is_err());
    let mut disk = vec![0; 0x8000];
    disk[0x4000] = 8;
    assert!(scripts(&disk, 1, &[0x4000], "test").is_err());
    disk[0x4000..0x4004].copy_from_slice(&[1, 0, 0, 0x80]);
    assert!(scripts(&disk, 1, &[0x4000], "test").is_err());
    disk[0x7fff] = 2;
    assert!(scripts(&disk, 1, &[0x7fff], "test").is_err());
    disk[0x7fff] = 0;
    disk[0x7ffd] = 7;
    assert!(scripts(&disk, 1, &[0x7ffd], "test").is_ok());
    disk[0x7ffe] = 7;
    assert!(scripts(&disk, 1, &[0x7ffe], "test").is_err());
}
