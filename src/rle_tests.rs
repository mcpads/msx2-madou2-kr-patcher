use super::*;

#[test]
fn round_trips_runs_literals_and_packet_boundaries() {
    for length in [0, 1, 2, 3, 126, 127, 128, 255, 256, 257, 1024] {
        for data in [
            vec![42; length],
            (0..length).map(|n| (n % 251) as u8).collect(),
        ] {
            assert_eq!(decode(&encode(&data), length).unwrap(), data);
        }
    }
    // The reader supports a 256-byte repeat even though our encoder uses <=127.
    assert_eq!(decode(&[0x80, 17, 0], 256).unwrap(), vec![17; 256]);
}

#[test]
fn refuses_truncation_size_mismatch_and_trailing_data() {
    for (bytes, size) in [
        (&[2, 1][..], 2),
        (&[0x83][..], 3),
        (&[1, 4][..], 1),
        (&[0][..], 1),
        (&[0, 1][..], 0),
        (&[0x83, 4, 0][..], 2),
    ] {
        assert!(decode(bytes, size).is_err());
    }
}

#[test]
fn prefix_decode_leaves_following_media_bytes_unconsumed() {
    let data = [3, 0x11, 0, 0x22, 0, 0x99];
    assert_eq!(
        super::decode_prefix(&data, 3).unwrap(),
        (vec![0x11, 0, 0x22], 5)
    );
    assert!(super::decode(&data, 3).is_err());
    assert!(super::decode_prefix(&data, 4).is_err());
}
