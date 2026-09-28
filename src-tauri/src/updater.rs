//! Updating thumbdeck to its latest release. Releases are built by GitHub Actions and published
//! (with a latest.json saying what's newest) to github.com/Nozeren/thumbdeck-releases. A
//! download is only installed when its signature is from thumbdeck's key (updater.pub) and
//! for the version it claims to be; then it replaces the installed copy: the binary on Linux,
//! the .app on macOS.

use base64::Engine;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;

const LATEST: &str = "https://github.com/Nozeren/thumbdeck-releases/releases/latest/download/latest.json";
/// Public half of the key releases are signed with (`tauri signer`, a minisign key)
const PUBLIC_KEY: &str = include_str!("updater.pub");

#[derive(Deserialize, Debug)]
struct Latest {
    version: String,
    #[serde(default)]
    notes: String,
    #[serde(default)]
    pub_date: String,
    platforms: HashMap<String, Download>,
}

#[derive(Deserialize, Debug, Clone)]
struct Download {
    url: String,
    signature: String,
}

/// A newer version than the one running
#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct Update {
    pub version: String,
    pub current: String,
    /// What's new (markdown)
    pub notes: String,
    pub date: String,
}

/// This build's name in latest.json
fn platform() -> Option<&'static str> {
    match (std::env::consts::OS, std::env::consts::ARCH) {
        ("macos", _) => Some("darwin-universal"),
        ("linux", "x86_64") => Some("linux-x86_64"),
        _ => None,
    }
}

/// Whether this copy can update itself: a release build, installed where you can write
/// (not a dev build, not a system package)
fn installed_copy() -> Option<PathBuf> {
    if cfg!(debug_assertions) {
        return None;
    }
    let exe = std::env::current_exe().ok()?.canonicalize().ok()?;
    let target = if cfg!(target_os = "macos") {
        // thumbdeck.app/Contents/MacOS/thumbdeck
        let app = exe.parent()?.parent()?.parent()?.to_path_buf();
        app.extension().is_some_and(|e| e == "app").then_some(app)?
    } else {
        exe
    };
    let dir = target.parent()?;
    let writable = std::fs::metadata(dir).is_ok_and(|m| !m.permissions().readonly())
        && tempfile_in(dir).is_ok();
    writable.then_some(target)
}

fn tempfile_in(dir: &Path) -> std::io::Result<()> {
    let probe = dir.join(format!(".thumbdeck-write-test-{}", std::process::id()));
    std::fs::write(&probe, b"")?;
    std::fs::remove_file(probe)
}

/// "1.2.3" (or "v1.2.3", "1.2.3-beta") newer than another
fn newer(candidate: &str, current: &str) -> bool {
    let parse = |v: &str| -> Vec<u64> {
        v.trim().trim_start_matches('v').split(['-', '+']).next().unwrap_or("").split('.').map(|n| n.parse().unwrap_or(0)).collect()
    };
    let (a, b) = (parse(candidate), parse(current));
    (0..a.len().max(b.len())).map(|i| (a.get(i).copied().unwrap_or(0), b.get(i).copied().unwrap_or(0))).find(|(x, y)| x != y).is_some_and(|(x, y)| x > y)
}

/// Where to look for releases. THUMBDECK_UPDATE_URL points elsewhere (e.g. a file:// folder, to
/// try an update before publishing it); what's downloaded must still be signed with the key.
fn latest_url() -> (String, &'static str) {
    match std::env::var("THUMBDECK_UPDATE_URL") {
        Ok(url) if !url.is_empty() => (url, "=https,file"),
        _ => (LATEST.to_string(), "=https"),
    }
}

fn curl(args: &[&str]) -> Result<Vec<u8>, String> {
    let out = Command::new("curl")
        .args(["-fsSL", "--proto", latest_url().1, "--retry", "2"])
        .args(args)
        .env_clear()
        .envs(crate::runner::shell_env().iter().cloned()) // proxies, certificates
        .output()
        .map_err(|e| format!("couldn't run curl: {e}"))?;
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr);
        return Err(format!("download failed: {}", err.trim().lines().last().unwrap_or("curl failed")));
    }
    Ok(out.stdout)
}

fn latest() -> Result<Latest, String> {
    let body = curl(&["--max-time", "20", &latest_url().0])?;
    serde_json::from_slice(&body).map_err(|e| format!("latest.json isn't readable: {e}"))
}

/// The newest release, if it's newer than this one and there's a build for this computer.
/// None also when this copy can't update itself (a dev build, a system install).
pub fn check(current: &str) -> Result<Option<Update>, String> {
    if installed_copy().is_none() {
        return Ok(None);
    }
    let Some(platform) = platform() else { return Ok(None) };
    Ok(available(&latest()?, current, platform))
}

fn available(latest: &Latest, current: &str, platform: &str) -> Option<Update> {
    (newer(&latest.version, current) && latest.platforms.contains_key(platform)).then(|| Update {
        version: latest.version.trim_start_matches('v').to_string(),
        current: current.to_string(),
        notes: latest.notes.clone(),
        date: latest.pub_date.clone(),
    })
}

/// Whether `data` is signed with thumbdeck's key, for `version`. Signatures are base64, as
/// `tauri signer sign --app-version` writes them; the version is in the signed part, so an
/// older release can't be passed off as a newer one.
fn verify(data: &[u8], signature: &str, public_key: &str, version: &str) -> Result<(), String> {
    let text = |b64: &str, what| {
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(b64.trim())
            .map_err(|_| format!("the {what} isn't base64"))?;
        String::from_utf8(bytes).map_err(|_| format!("the {what} isn't text"))
    };
    let key = minisign_verify::PublicKey::decode(&text(public_key, "public key")?).map_err(|e| format!("bad public key: {e}"))?;
    let sig = minisign_verify::Signature::decode(&text(signature, "signature")?).map_err(|e| format!("bad signature: {e}"))?;
    key.verify(data, &sig, false).map_err(|_| "the download isn't signed by thumbdeck's key".to_string())?;
    let signed_for = sig.trusted_comment().split('\t').find_map(|f| f.strip_prefix("version:"));
    if signed_for != Some(version) {
        return Err(format!("the download is signed for version {}, not {version}", signed_for.unwrap_or("?")));
    }
    Ok(())
}

/// Put `new` (a file or a folder) where `target` is, keeping `target` if anything fails.
/// Both are in the same folder, so the renames can't be half done.
fn replace(new: &Path, target: &Path) -> Result<(), String> {
    let old = target.with_file_name(format!(".{}.old", target.file_name().and_then(|n| n.to_str()).unwrap_or("thumbdeck")));
    let remove = |p: &Path| if p.is_dir() { std::fs::remove_dir_all(p) } else { std::fs::remove_file(p) };
    if old.exists() {
        remove(&old).map_err(|e| format!("couldn't clear {}: {e}", old.display()))?;
    }
    std::fs::rename(target, &old).map_err(|e| format!("couldn't move the installed copy aside: {e}"))?;
    if let Err(e) = std::fs::rename(new, target) {
        let _ = std::fs::rename(&old, target);
        return Err(format!("couldn't put the new version in place: {e}"));
    }
    let _ = remove(&old);
    Ok(())
}

/// Download the latest release, check it, and install it over this copy. Returns the version
/// installed; it runs after a restart.
pub fn install(current: &str) -> Result<String, String> {
    let target = installed_copy().ok_or("this copy of thumbdeck can't update itself (a dev build or a system install)")?;
    let platform = platform().ok_or("there are no releases for this computer")?;
    let latest = latest()?;
    let update = available(&latest, current, platform).ok_or("thumbdeck is up to date")?;
    let download = &latest.platforms[platform];

    // Everything happens next to the installed copy (same disk: the final rename is instant)
    let stage = target.with_file_name(format!(".thumbdeck-update-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&stage);
    std::fs::create_dir_all(&stage).map_err(|e| format!("couldn't make {}: {e}", stage.display()))?;
    let result = (|| {
        let archive = stage.join("update.tar.gz");
        curl(&["--max-time", "600", "-o", &archive.to_string_lossy(), &download.url])?;
        let data = std::fs::read(&archive).map_err(|e| e.to_string())?;
        verify(&data, &download.signature, PUBLIC_KEY, &update.version)?;
        let unpacked = stage.join("unpacked");
        std::fs::create_dir_all(&unpacked).map_err(|e| e.to_string())?;
        let tar = Command::new("tar").arg("-xzf").arg(&archive).arg("-C").arg(&unpacked).output().map_err(|e| e.to_string())?;
        if !tar.status.success() {
            return Err(format!("couldn't unpack the update: {}", String::from_utf8_lossy(&tar.stderr).trim()));
        }
        let new = unpacked.join(target.file_name().ok_or("no file name")?);
        if !new.exists() {
            return Err(format!("the update has no {}", new.file_name().unwrap_or_default().to_string_lossy()));
        }
        replace(&new, &target)
    })();
    let _ = std::fs::remove_dir_all(&stage);
    result.map(|_| update.version)
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEST_KEY: &str = include_str!("testdata/updater/test.pub");
    const DATA: &[u8] = include_bytes!("testdata/updater/update.tar.gz");
    const UNVERSIONED_SIG: &str = include_str!("testdata/updater/update.tar.gz.sig");
    const VERSIONED: &[u8] = include_bytes!("testdata/updater/versioned.tar.gz");
    const VERSIONED_SIG: &str = include_str!("testdata/updater/versioned.tar.gz.sig");

    #[test]
    fn versions() {
        assert!(newer("0.2.0", "0.1.0"));
        assert!(newer("v1.0.0", "0.9.9"));
        assert!(newer("0.1.10", "0.1.9"), "numbers, not text");
        assert!(newer("1.0", "0.9.9"));
        assert!(!newer("0.1.0", "0.1.0"));
        assert!(!newer("0.1.0", "0.2.0"));
        assert!(!newer("0.2.0-beta", "0.2.0"));
    }

    #[test]
    fn signatures() {
        assert_eq!(verify(VERSIONED, VERSIONED_SIG, TEST_KEY, "9.9.9"), Ok(()));
        let mut tampered = VERSIONED.to_vec();
        tampered[0] ^= 1;
        assert!(verify(&tampered, VERSIONED_SIG, TEST_KEY, "9.9.9").unwrap_err().contains("isn't signed"));
        assert!(verify(VERSIONED, VERSIONED_SIG, TEST_KEY, "9.9.10").unwrap_err().contains("signed for version 9.9.9"));
        assert!(verify(DATA, UNVERSIONED_SIG, TEST_KEY, "9.9.9").unwrap_err().contains("signed for version ?"),
                "a signature without a version isn't accepted");
        assert!(verify(VERSIONED, VERSIONED_SIG, PUBLIC_KEY, "9.9.9").unwrap_err().contains("isn't signed"),
                "the test key isn't thumbdeck's key");
        assert!(verify(VERSIONED, "%%%", TEST_KEY, "9.9.9").is_err());
    }

    #[test]
    fn latest_json() {
        let latest: Latest = serde_json::from_str(r#"{
            "version": "0.2.0", "notes": "Logs tab", "pub_date": "2026-09-26T16:00:00Z",
            "platforms": { "linux-x86_64": { "url": "https://x/l.tar.gz", "signature": "s" },
                           "darwin-universal": { "url": "https://x/m.tar.gz", "signature": "s" } } }"#).unwrap();
        let update = available(&latest, "0.1.0", "linux-x86_64").unwrap();
        assert_eq!((update.version.as_str(), update.current.as_str(), update.notes.as_str()), ("0.2.0", "0.1.0", "Logs tab"));
        assert_eq!(available(&latest, "0.2.0", "linux-x86_64"), None, "up to date");
        assert_eq!(available(&latest, "0.1.0", "windows-x86_64"), None, "no build for this computer");
    }

    #[test]
    fn replacing_a_file_or_a_folder_in_place() {
        let d = crate::testutil::TempDir::new("updater-replace");
        d.file("thumbdeck", "old").file(".update/thumbdeck", "new");
        replace(&d.0.join(".update/thumbdeck"), &d.0.join("thumbdeck")).unwrap();
        assert_eq!(std::fs::read_to_string(d.0.join("thumbdeck")).unwrap(), "new");
        assert!(!d.0.join(".thumbdeck.old").exists(), "the old copy is removed");

        d.file("thumbdeck.app/Contents/Info.plist", "old").file(".update/thumbdeck.app/Contents/Info.plist", "new");
        replace(&d.0.join(".update/thumbdeck.app"), &d.0.join("thumbdeck.app")).unwrap();
        assert_eq!(std::fs::read_to_string(d.0.join("thumbdeck.app/Contents/Info.plist")).unwrap(), "new");

        let err = replace(&d.0.join(".update/missing"), &d.0.join("thumbdeck")).unwrap_err();
        assert!(err.contains("couldn't put the new version in place"));
        assert_eq!(std::fs::read_to_string(d.0.join("thumbdeck")).unwrap(), "new", "the installed copy is kept");
    }

    #[test]
    fn dev_builds_dont_update_themselves() {
        assert_eq!(installed_copy(), None);
        assert_eq!(check("0.0.1"), Ok(None), "without even asking GitHub");
    }
}
