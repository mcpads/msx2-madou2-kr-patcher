use super::*;

#[test]
fn preserves_prefix_and_full_original_through_overlapping_restore() {
    let mut original: Vec<u8> = (0..512).map(|n| (n % 128) as u8).collect();
    original.extend(vec![b'Z'; 1000]);
    let prefix = b"font-input";
    let packed = pack(&original, prefix).unwrap();
    assert_eq!(&packed.bytes[..prefix.len()], prefix);
    assert_eq!(packed.bytes.len(), original.len());
    let mut stream = packed.bytes[packed.stream_offset..].to_vec();
    stream.push(0);
    assert_eq!(rle::decode(&stream, original.len()).unwrap(), original);
}

#[test]
fn rejects_unread_tail_overwrite_despite_fitting_total_size() {
    let mut original = vec![b'Z'; 1000];
    original.extend((0..512).map(|n| (n % 128) as u8));
    assert!(rle::encode(&original).len() + 4 < original.len());
    assert!(pack(&original, b"font").err().unwrap().contains("unread"));
    assert!(pack(&[b'Z'; 1000], &[1; 1000]).is_err());
    assert!(pack(&[], &[]).is_err());
}
