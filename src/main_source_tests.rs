use super::*;

#[test]
fn follows_terminator_and_normalizes_kana_runs_in_each_mode() {
    let data = b"\0\xb6\xde$@\xb6\xde$w\xff$$\xfftail";
    let (tokens, end) = parse(data, 1, data.len()).unwrap();
    assert_eq!(annotate(&tokens), "が<TOGGLE_KANA>ガ<WAIT_TICKS:FF>$<END>");
    assert_eq!(&data[end..], b"tail");
    assert!(parse(data, 1, end - 1).is_err());
}

#[test]
fn rejects_out_of_bounds_and_incomplete_units() {
    for data in [b"".as_slice(), b"A", b"$", b"$t", b"$z\xff", b"\0\xff"] {
        assert!(parse(data, 0, data.len()).is_err());
    }
    assert!(parse(b"A\xff", 0, 3).is_err());
    assert!(word(b"\0", 0).is_err());
    assert!(word(b"\0\0", usize::MAX).is_err());
}
