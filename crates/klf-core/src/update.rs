//! Update in one click (1.5.5, PUBLIC version only): pure decisions, tested here. The network
//! (WinHTTP), the checks (SHA-256 + ECDSA P-256 signature, Windows CNG) and the swap of the exe
//! live in klf-win/src/update.rs. No PowerShell, no telemetry.

/// Where the public releases are.
pub const API_URL: &str = "https://api.github.com/repos/breslev558/hebrew-keyboard-layout-fixer/releases/latest";
pub const EXE_NAME: &str = "KeyboardLayoutFixer.exe";
pub const SHA_NAME: &str = "KeyboardLayoutFixer.exe.sha256";
/// ECDSA P-256 signature (r||s, 128 hex characters) of the .sha256 file, made offline with the
/// update key: the exe is installed only if it matches the public key built into the program.
pub const SIG_NAME: &str = "KeyboardLayoutFixer.exe.sha256.sig";
/// The only hosts a download may come from (after GitHub's redirect).
pub const ALLOWED_HOSTS: &[&str] = &["github.com", "api.github.com", "objects.githubusercontent.com", "release-assets.githubusercontent.com"];
/// Larger answers / zips are refused.
pub const MAX_API_BYTES: usize = 256 * 1024;
pub const MAX_EXE_BYTES: u64 = 5 * 1024 * 1024;
/// A version that was refused or failed is not offered again automatically for 7 days.
pub const RETRY_AFTER_SECS: u64 = 7 * 24 * 3600;

/// What a release offers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Release {
    pub version: (u32, u32, u32),
    pub tag: String,
    pub exe_url: String,
    pub exe_size: u64,
    pub sha_url: String,
    pub sig_url: String,
}

/// "1.5.4", "v1.5.4", "1.5.4-noam" -> (1, 5, 4); anything else -> None.
pub fn parse_version(s: &str) -> Option<(u32, u32, u32)> {
    let s = s.trim().trim_start_matches(['v', 'V']);
    let core = s.split(['-', '+']).next()?;
    let mut it = core.split('.');
    let v = (it.next()?.parse().ok()?, it.next()?.parse().ok()?, it.next()?.parse().ok()?);
    if it.next().is_some() {
        return None;
    }
    Some(v)
}

/// Is the program allowed to look for updates at all? (no network request otherwise)
/// `exe_dir`: folder of the running exe; `local_programs`: %LOCALAPPDATA%\Programs\KeyboardLayoutFixer;
/// `policy_key_exists`: an administrator manages this PC (HKLM Policies key present).
pub fn may_check(setting_on: bool, policy_key_exists: bool, exe_dir: &str, local_programs: &str) -> bool {
    let norm = |p: &str| p.trim_end_matches(['\\', '/']).to_lowercase();
    setting_on && !policy_key_exists && !local_programs.is_empty() && norm(exe_dir) == norm(local_programs)
}

/// The GitHub answer for the latest release, if it offers a usable update.
pub fn parse_release(json: &str) -> Option<Release> {
    if json.len() > MAX_API_BYTES {
        return None;
    }
    let v: serde_json::Value = serde_json::from_str(json).ok()?;
    if v.get("draft").and_then(|d| d.as_bool()).unwrap_or(false) || v.get("prerelease").and_then(|d| d.as_bool()).unwrap_or(false) {
        return None;
    }
    let tag = v.get("tag_name")?.as_str()?.to_string();
    let version = parse_version(&tag)?;
    let assets = v.get("assets")?.as_array()?;
    let find = |name: &str| assets.iter().find(|a| a.get("name").and_then(|n| n.as_str()) == Some(name));
    let exe = find(EXE_NAME)?;
    let sha = find(SHA_NAME)?;
    let sig = find(SIG_NAME)?;
    let url = |a: &serde_json::Value| a.get("browser_download_url").and_then(|u| u.as_str()).filter(|u| u.starts_with("https://github.com/")).map(String::from);
    let exe_size = exe.get("size")?.as_u64()?;
    if exe_size == 0 || exe_size > MAX_EXE_BYTES {
        return None;
    }
    Some(Release { version, tag, exe_url: url(exe)?, exe_size, sha_url: url(sha)?, sig_url: url(sig)? })
}

/// Offer this release? Only a NEWER version, and not one refused/failed less than 7 days ago.
pub fn should_offer(current: (u32, u32, u32), r: &Release, last_failed: Option<(&str, u64)>, now_secs: u64) -> bool {
    if r.version <= current {
        return false;
    }
    !matches!(last_failed, Some((tag, at)) if tag == r.tag && now_secs.saturating_sub(at) < RETRY_AFTER_SECS)
}

/// The host of an https URL, if it is one of ALLOWED_HOSTS (checked again after redirects).
pub fn host_allowed(url: &str) -> bool {
    let Some(rest) = url.strip_prefix("https://") else { return false };
    let host = rest.split(['/', '?', '#']).next().unwrap_or("").to_ascii_lowercase();
    !host.contains(['@', ':']) && ALLOWED_HOSTS.contains(&host.as_str())
}

/// The signature file: 128 hex characters (r||s) -> 64 bytes.
pub fn parse_sig(text: &str) -> Option<[u8; 64]> {
    let h = text.trim();
    if h.len() != 128 || !h.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    let mut out = [0u8; 64];
    for (i, b) in out.iter_mut().enumerate() {
        *b = u8::from_str_radix(&h[2 * i..2 * i + 2], 16).ok()?;
    }
    Some(out)
}

/// The SHA-256 announced in the .sha256 file ("<64 hex>  KeyboardLayoutFixer.exe"), lower case.
pub fn parse_sha_file(text: &str) -> Option<String> {
    let h = text.split_whitespace().next()?.to_ascii_lowercase();
    (h.len() == 64 && h.bytes().all(|b| b.is_ascii_hexdigit())).then_some(h)
}

/// The downloaded exe may be installed: exact announced size, same SHA-256, and the version
/// inside the file is the one of the tag.
pub fn download_ok(r: &Release, size: u64, sha_hex: &str, announced: &str, file_version: Option<(u32, u32, u32)>) -> bool {
    size == r.exe_size && size <= MAX_EXE_BYTES && !announced.is_empty() && sha_hex.eq_ignore_ascii_case(announced) && file_version == Some(r.version)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn json(tag: &str, size: u64, pre: bool) -> String {
        format!(
            r#"{{"tag_name":"{tag}","draft":false,"prerelease":{pre},"assets":[
            {{"name":"KeyboardLayoutFixer.exe","size":{size},"browser_download_url":"https://github.com/x/y/releases/download/{tag}/KeyboardLayoutFixer.exe"}},
            {{"name":"KeyboardLayoutFixer.exe.sha256","size":90,"browser_download_url":"https://github.com/x/y/releases/download/{tag}/KeyboardLayoutFixer.exe.sha256"}},
            {{"name":"KeyboardLayoutFixer.exe.sha256.sig","size":128,"browser_download_url":"https://github.com/x/y/releases/download/{tag}/KeyboardLayoutFixer.exe.sha256.sig"}}]}}"#
        )
    }

    #[test]
    fn versions() {
        assert_eq!(parse_version("v1.5.4"), Some((1, 5, 4)));
        assert_eq!(parse_version("1.5.4-noam"), Some((1, 5, 4)));
        assert_eq!(parse_version("1.5"), None);
        assert_eq!(parse_version("v1.5.4.1"), None);
        assert_eq!(parse_version("x"), None);
        assert!((1, 10, 0) > (1, 9, 9), "numeric, not text");
    }

    #[test]
    fn no_check_at_school_or_when_switched_off() {
        let lp = r"C:\Users\a\AppData\Local\Programs\KeyboardLayoutFixer";
        assert!(may_check(true, false, lp, lp));
        assert!(may_check(true, false, &format!("{lp}\\"), &lp.to_uppercase()));
        assert!(!may_check(false, false, lp, lp), "setting off: no request");
        assert!(!may_check(true, true, lp, lp), "administrator policy: no request");
        assert!(!may_check(true, false, r"C:\Program Files\KeyboardLayoutFixer", lp), "school install: no request");
        assert!(!may_check(true, false, lp, ""));
    }

    #[test]
    fn release_parsing_and_offer() {
        let r = parse_release(&json("v1.5.5", 340_000, false)).unwrap();
        assert_eq!(r.version, (1, 5, 5));
        assert!(should_offer((1, 5, 4), &r, None, 0));
        assert!(!should_offer((1, 5, 5), &r, None, 0), "same version: nothing");
        assert!(!should_offer((1, 6, 0), &r, None, 0), "older: nothing");
        assert!(!should_offer((1, 5, 4), &r, Some(("v1.5.5", 100)), 100 + 3600), "failed 1 h ago: not again");
        assert!(should_offer((1, 5, 4), &r, Some(("v1.5.5", 100)), 100 + RETRY_AFTER_SECS), "after 7 days: offered again");
        assert!(parse_release(&json("v1.5.5", 340_000, true)).is_none(), "pre-release: nothing");
        assert!(parse_release(&json("v1.5.5", MAX_EXE_BYTES + 1, false)).is_none(), "too big");
        assert!(parse_release(r#"{"tag_name":"v1.5.5","assets":[]}"#).is_none(), "assets missing");
        let other_host = json("v1.5.5", 1000, false).replace("https://github.com/", "https://evil.example/");
        assert!(parse_release(&other_host).is_none(), "download only from github.com");
    }

    #[test]
    fn download_is_refused_if_size_or_sha_differs() {
        let r = parse_release(&json("v1.5.5", 1000, false)).unwrap();
        let sha = "a".repeat(64);
        let ann = parse_sha_file(&format!("{sha}  KeyboardLayoutFixer.exe\n")).unwrap();
        let v = Some((1, 5, 5));
        assert!(download_ok(&r, 1000, &sha.to_uppercase(), &ann, v));
        assert!(!download_ok(&r, 999, &sha, &ann, v), "wrong size");
        assert!(!download_ok(&r, 1000, &"b".repeat(64), &ann, v), "wrong SHA");
        assert!(!download_ok(&r, 1000, &sha, &ann, Some((1, 5, 4))), "file version is not the tag");
        assert!(!download_ok(&r, 1000, &sha, &ann, None));
        assert!(parse_sha_file("not a hash").is_none());
        assert!(!download_ok(&r, 1000, &sha, "", v));
    }

    #[test]
    fn hosts_and_signature_format() {
        assert!(host_allowed("https://github.com/x/y/releases/download/v1/KeyboardLayoutFixer.exe"));
        assert!(host_allowed("https://objects.githubusercontent.com/abc?x=1"));
        assert!(!host_allowed("http://github.com/x"), "https only");
        assert!(!host_allowed("https://github.com.evil.example/x"));
        assert!(!host_allowed("https://evil@github.com/x"));
        assert!(!host_allowed("https://github.com:444/x"));
        assert_eq!(parse_sig(&"ab".repeat(64)).unwrap()[0], 0xab);
        assert!(parse_sig("abc").is_none());
        assert!(parse_sig(&"zz".repeat(64)).is_none());
    }
}
