use super::validate;

#[test]
fn preserves_flow_delay_and_prices_while_removing_kana_mode() {
    let original = b"$@\xa1 10g$r2$w\xff$k\xff";
    let translated = "가격 10g<NEWLINE>2<JOSA:topic><WAIT_TICKS:FF><WAIT_INPUT><END>";
    validate(original, translated).unwrap();
    for invalid in [
        translated.replace("10g", "11g"),
        translated.replace("10g", "10"),
        translated.replace("FF>", "FE>"),
        translated.replace("<NEWLINE>", ""),
        translated.replace("<WAIT_INPUT>", "<CLEAR_WINDOW>"),
    ] {
        assert!(validate(original, &invalid).is_err(), "{invalid}");
    }
}

#[test]
fn rejects_bad_source_and_unreviewable_authoring() {
    for raw in [
        b"A".as_slice(),
        b"A\xffB",
        b"$",
        b"$w",
        b"$z\xff",
        b"\0\xff",
    ] {
        assert!(validate(raw, "가<END>").is_err());
    }
    for text in [
        "\u{1100}\u{1161}<END>",
        "あ<END>",
        "漢<END>",
        "$<END>",
        "가<END>나",
    ] {
        assert!(validate(b"A\xff", text).is_err(), "{text}");
    }
}

#[test]
fn keeps_numeric_grouping_across_removed_controls() {
    validate(b"1$r2g\xff", "12g<NEWLINE><END>").unwrap();
    assert!(validate(b"1\xa12g\xff", "12g<END>").is_err());
    assert!(validate(b"10g 2\xff", "2 10g<END>").is_err());
}
