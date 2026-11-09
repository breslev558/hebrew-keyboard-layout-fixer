//! Any comment and any technical names: the anonymous report stays valid JSON, within the size
//! limit, with only the expected fields and never a path.
#![no_main]
use klf_core::diag::Attempt;
use klf_core::report::{build, build_crash, pretty, MAX_BYTES};
use libfuzzer_sys::fuzz_target;

const FIELDS: [&str; 14] = ["kind", "version", "windows", "msg", "action", "step", "total_ms", "steps", "exe", "window_class", "focus_class", "selection", "chars", "comment"];

fn check(j: &str) {
    assert!(j.len() <= MAX_BYTES, "report too big: {}", j.len());
    let v: serde_json::Value = serde_json::from_str(j).expect("valid JSON");
    for k in v.as_object().expect("object").keys() {
        assert!(FIELDS.contains(&k.as_str()), "unexpected field {k}");
    }
    for k in ["exe", "window_class", "focus_class", "version", "windows", "kind"] {
        if let Some(s) = v.get(k).and_then(|x| x.as_str()) {
            assert!(!s.contains(['\\', '/', ' ']), "{k} not cleaned: {s:?}");
        }
    }
    let _ = pretty(j);
}

fuzz_target!(|data: &str| {
    let parts: Vec<&str> = data.splitn(5, '\u{1}').collect();
    let get = |i: usize| parts.get(i).copied().unwrap_or("");
    let a = Attempt { exe: get(0).into(), window_class: get(1).into(), focus_class: get(2).into(), ..Default::default() };
    check(&build(get(3), "1.6.3", "10.0.1", Some(&a), get(4)));
    check(&build("signalement", get(0), get(1), None, data));
    check(&build_crash("1.6.3", "10.0.1", data));
});
