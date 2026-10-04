use super::*;

fn fixture() -> (Entry, Reference) {
    let raw = [16, 120, 0x80, 0xff, 0xfb, 8, 0xfc, 0x81, 0xff, 0xff];
    let (tokens, _) = cutscene_source::text(&raw, 0x4000).unwrap();
    let reference = Reference {
        id: "intro-4000-4010".into(),
        script: 0x4000,
        command_address: 0x4010,
        pointer_disk_offset: 18,
        address: 0x4000,
        disk_offset: 0,
        initial_xy: [16, 120],
        raw_hex: hex::encode(raw),
        tokens,
    };
    let entry: Entry = serde_json::from_value(json!({"id":"cutscene-00000","group":"intro",
        "source_disk_offset":0,"source_raw_sha256":media::sha256(&raw),
        "references":[reference.id],"text":"가<NEWLINE_OFFSET:8><CLEAR_WINDOW>나<END>",
        "state":"needs_review","layout":{"newline_offsets":[32]}}))
    .unwrap();
    (entry, reference)
}

#[test]
fn preserves_controls_with_explicit_layout_and_clear_keeps_y() {
    let (entry, reference) = fixture();
    let (text, measurement, chars) = prepare_entry(&entry, &reference).unwrap();
    assert_eq!(measurement["last_y"], 144);
    assert_eq!(measurement["right_edge"], 32);
    assert_eq!(chars.into_iter().collect::<String>(), "가나");
    assert!(matches!(&text.units[1], Unit::Control(raw) if raw == &[0xff,0xfb,32]));
}

#[test]
fn rejects_changed_controls_implicit_layout_and_overflow() {
    for text in [
        "가<END>",
        "가<NEWLINE_OFFSET:9><CLEAR_WINDOW>나<END>",
        "가\n<NEWLINE_OFFSET:8><CLEAR_WINDOW>나<END>",
        "가<NEWLINE_OFFSET:8><CLEAR_WINDOW>나<END><END>",
        "가가가가가가가가가가가가가가가<NEWLINE_OFFSET:8><CLEAR_WINDOW>나<END>",
    ] {
        let (mut entry, reference) = fixture();
        entry.text = text.into();
        assert!(prepare_entry(&entry, &reference).is_err(), "{text}");
    }
    let (mut entry, reference) = fixture();
    entry.layout.initial_xy = Some([16, 168]);
    assert!(prepare_entry(&entry, &reference).is_err());
    entry.layout.initial_xy = None;
    entry.layout.newline_offsets = Some(vec![]);
    assert!(prepare_entry(&entry, &reference).is_err());
    assert!(serde_json::from_value::<Layout>(json!({"initial_xy":[true,120]})).is_err());
    assert!(serde_json::from_value::<Layout>(json!({"newline_offsets":[256]})).is_err());
    assert!(serde_json::from_value::<Layout>(json!({"other":0})).is_err());
}

#[test]
fn validates_source_identity_aliases_and_span_population() {
    let (entry, reference) = fixture();
    let source = [0u8; 8];
    let draft = Draft {
        source_sha256: media::sha256(&source),
        input_policy: "non-distribution-development".into(),
        entries: vec![entry],
    };
    assert!(validate(&source, b"draft", draft, &[reference]).is_ok());
    for mutation in 0..4 {
        let (mut entry, reference) = fixture();
        match mutation {
            0 => entry.references.clear(),
            1 => entry.source_raw_sha256 = "wrong".into(),
            2 => entry.source_disk_offset = 1,
            _ => entry.group = "ending".into(),
        }
        let draft = Draft {
            source_sha256: media::sha256(&source),
            input_policy: "non-distribution-development".into(),
            entries: vec![entry],
        };
        assert!(validate(&source, b"draft", draft, &[reference]).is_err());
    }
}
