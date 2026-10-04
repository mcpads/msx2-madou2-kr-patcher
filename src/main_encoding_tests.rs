use super::*;

#[test]
fn preserves_opaque_payloads_and_escapes_literal_dollar() {
    let mapping = Mapping(vec![
        ('가', vec![5]),
        ('힣', vec![1, 255]),
        ('끝', vec![2, 36]),
    ]);
    assert_eq!(
        mapping
            .encode("가힣끝$<WAIT_TICKS:FF><JOSA:object><NEWLINE><END>")
            .unwrap(),
        b"\x05\x01\xff\x02\x24$$$w\xff\x04\x03$r\xff"
    );
}

#[test]
fn ranks_by_frequency_then_unicode_and_includes_unprinted_particles() {
    let mapping = Mapping::from_texts(["나가나<JOSA:topic><END>"]).unwrap();
    assert_eq!(
        mapping.0.iter().map(|(c, _)| *c).collect::<String>(),
        "나가는은"
    );
    assert_eq!(mapping.encode("가나<END>").unwrap(), [6, 5, 255]);
}

#[test]
fn rejects_missing_glyphs_and_malformed_controls() {
    let mapping = Mapping::from_texts(["가<END>"]).unwrap();
    for text in [
        "힣<END>",
        "가",
        "가<END>가",
        "<END><END>",
        "<WAIT_TICKS><END>",
        "<WAIT_TICKS:ff><END>",
        "<NEWLINE:01><END>",
        "<JOSA:unknown><END>",
        "<TOGGLE_KANA><END>",
        "<END",
        "<<END>",
        "가>나<END>",
    ] {
        assert!(mapping.encode(text).is_err(), "{text}");
    }
}

#[test]
fn extension_boundary_and_capacity_are_checked() {
    let text: String = (0..349)
        .map(|n| char::from_u32(0xac00 + n).unwrap())
        .collect();
    let mapping = Mapping::from_texts([format!("{text}<END>").as_str()]).unwrap();
    let selected: String = [91, 92, 347, 348]
        .map(|n| char::from_u32(0xac00 + n).unwrap())
        .iter()
        .collect();
    assert_eq!(
        mapping.encode(&format!("{selected}<END>")).unwrap(),
        [254, 1, 0, 1, 255, 2, 0, 255]
    );
    let oversized: String = (0..861)
        .map(|n| char::from_u32(0xac00 + n).unwrap())
        .collect();
    assert!(Mapping::from_texts([format!("{oversized}<END>").as_str()]).is_err());
}
