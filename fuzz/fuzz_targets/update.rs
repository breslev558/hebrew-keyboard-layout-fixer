//! Any answer from the network: parsing the release, the signature and the checksum file never
//! panics, and an allowed address is always an https address of an allowed host.
#![no_main]
use klf_core::update::{host_allowed, parse_release, parse_sha_file, parse_sig, parse_version};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &str| {
    let _ = parse_release(data);
    let _ = parse_sig(data);
    let _ = parse_sha_file(data);
    let _ = parse_version(data);
    if host_allowed(data) {
        assert!(data.starts_with("https://"));
        let host = data[8..].split(['/', '?', '#']).next().unwrap_or("");
        assert!(!host.contains(['@', ':']), "allowed with user or port: {data:?}");
    }
});
