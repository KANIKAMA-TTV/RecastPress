#[cfg(not(target_os = "linux"))]
#[test]
fn unsupported_platform_cannot_be_reported_as_zero_rss() {
    use recastpress_server::process_memory_observation::observe_self_smaps_rollup_rss;
    assert_eq!(
        observe_self_smaps_rollup_rss().unwrap_err().kind(),
        std::io::ErrorKind::Unsupported
    );
}
