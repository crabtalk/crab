use crabtalk_sdk::format;

#[test]
fn byte_counts_use_scaled_units() {
    for (size, expected) in [
        (0, "0 B"),
        (1, "1 B"),
        (1023, "1023 B"),
        (1024, "1 KB"),
        (1536, "1.5 KB"),
        (1 << 20, "1 MB"),
        (128 << 20, "128 MB"),
        (1 << 30, "1 GB"),
        (1_074_306_017, "1 GB"),
        (5 << 30, "5 GB"),
        (1 << 40, "1 TB"),
        (u64::MAX, "16777216 TB"),
    ] {
        assert_eq!(format::bytes(size), expected, "{size}");
    }
}
