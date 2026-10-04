use super::*;
#[test]
fn ending_copy_preserves_c5_rows_without_neighbor_pixels() {
    let mut ending = vec![0; 3074];
    for row in 0..16 {
        ending[2 + (64 + row) * 32 + 10..4 + (64 + row) * 32 + 10]
            .copy_from_slice(&[row as u8, 255 - row as u8]);
    }
    let correction = ending_correction(&ending).unwrap();
    for row in 0..16 {
        assert_eq!(
            &correction[2 + row * 32..4 + row * 32],
            &[row as u8, 255 - row as u8]
        );
        assert!(
            correction[4 + row * 32..34 + row * 32]
                .iter()
                .all(|&b| b == 0)
        );
    }
    assert!(ending_correction(&ending[..100]).is_err());
}
#[test]
fn font_write_rejects_changed_padding_and_preserves_shortened_stream_tail() {
    let original = rle::encode(&[1, 2, 3, 4]);
    let mut source = original.clone();
    source.extend([0; 8]);
    let write = font_write(&source, "test", 0, original.len(), source.len(), 4, &[9; 4]).unwrap();
    let shorter = rle::encode(&[9; 4]);
    assert_eq!(&write.replacement[..shorter.len()], shorter);
    assert_eq!(
        &write.replacement[shorter.len()..],
        &original[shorter.len()..]
    );
    source[original.len()] = 1;
    assert!(font_write(&source, "test", 0, original.len(), source.len(), 4, &[9; 4]).is_err());
    assert!(
        font_write(
            &original,
            "test",
            0,
            original.len(),
            original.len(),
            4,
            &[1; 300]
        )
        .is_err()
    );
}
