use berm_api::{Budget, Limits};

#[test]
fn headroom_rounds_up_in_the_manifest_units_and_does_not_clip_to_ceilings() {
    let limits = Limits::for_usage(Some(80_001), Some(0), None, (12 << 20) + 1);
    assert_eq!(limits.cpu, Some(120_002));
    assert_eq!(limits.read, Some(1));
    assert_eq!(limits.written, None);
    assert_eq!(limits.memory, Some(19));
    let most = Budget {
        cpu: 100_000,
        read: 1_000_000,
        written: 1_000_000,
        memory: 128 << 20,
    };
    assert_eq!(limits.over(most).past(&most), Some("cpu"));
    assert_eq!(most.past(&most), None);
}
