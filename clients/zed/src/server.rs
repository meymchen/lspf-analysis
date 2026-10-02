use std::fs;
use std::path::{Path, PathBuf};

use serde_json::{Value, json};
use zed_extension_api::{Architecture, Os, Result};

pub fn version() -> &'static str {
    include_str!("../server-version").trim()
}

pub fn target(os: Os, arch: Architecture) -> Result<&'static str> {
    match (os, arch) {
        (Os::Windows, Architecture::X8664) => Ok("x86_64-pc-windows-msvc"),
        (Os::Windows, Architecture::Aarch64) => Ok("aarch64-pc-windows-msvc"),
        (Os::Mac, Architecture::X8664) => Ok("x86_64-apple-darwin"),
        (Os::Mac, Architecture::Aarch64) => Ok("aarch64-apple-darwin"),
        (Os::Linux, Architecture::X8664) => Ok("x86_64-unknown-linux-musl"),
        (Os::Linux, Architecture::Aarch64) => Ok("aarch64-unknown-linux-musl"),
        _ => Err("LSPF Analysis downloads support only x64 and ARM64. Configure a local binary.path for other architectures.".into()),
    }
}

fn asset(target: &str) -> String {
    let suffix = if target.ends_with("windows-msvc") {
        ".exe"
    } else {
        ""
    };
    format!("lspf-analysis-{}-{target}{suffix}", version())
}

pub fn cache_path(target: &str) -> PathBuf {
    PathBuf::from(format!("server-v{}", version())).join(asset(target))
}

pub fn download_url(target: &str) -> String {
    format!(
        "https://github.com/meymchen/lspf-analysis/releases/download/server-v{}/{}",
        version(),
        asset(target)
    )
}

/// Publish a completed download with a rename. A failed or interrupted download
/// never becomes an executable cache entry; an existing cache works offline.
pub fn ensure_cached(path: &Path, download: impl FnOnce(&Path) -> Result<()>) -> Result<()> {
    let nonempty_file = |path: &Path| {
        fs::metadata(path).is_ok_and(|metadata| metadata.is_file() && metadata.len() > 0)
    };
    if nonempty_file(path) {
        return Ok(());
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    // Append rather than replace an extension: Unix assets contain dots in
    // their version but have no filename extension.
    let mut temporary = path.as_os_str().to_os_string();
    temporary.push(".download");
    let temporary = PathBuf::from(temporary);
    download(&temporary)?;
    if !nonempty_file(&temporary) {
        return Err("The server download is empty".into());
    }
    fs::rename(temporary, path).map_err(|error| error.to_string())
}

pub fn configuration(settings: Option<Value>) -> Result<Value> {
    let settings = settings.unwrap_or_else(|| json!({}));
    if !settings.is_object() {
        return Err("lsp.lspf-analysis.settings must be a JSON object".into());
    }
    if let Some(section) = settings.get("lspfAnalysis") {
        if !section.is_object() {
            return Err("lsp.lspf-analysis.settings.lspfAnalysis must be a JSON object".into());
        }
        Ok(settings)
    } else {
        Ok(json!({ "lspfAnalysis": settings }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static NEXT: AtomicUsize = AtomicUsize::new(0);

    fn test_path() -> PathBuf {
        // Stay in Cargo's ignored output, including on restricted Windows hosts.
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("target/cache-tests")
            .join(format!(
                "{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ))
            .join("server.exe")
    }

    #[test]
    fn completed_cache_needs_no_network_and_partial_download_is_never_used() {
        let path = test_path();
        let interrupted = ensure_cached(&path, |temporary| {
            assert_eq!(temporary.file_name().unwrap(), "server.exe.download");
            fs::write(temporary, b"partial").unwrap();
            Err("offline".into())
        });
        assert_eq!(interrupted, Err("offline".into()));
        assert!(!path.exists());
        ensure_cached(&path, |temporary| {
            fs::write(temporary, b"complete").map_err(|error| error.to_string())
        })
        .unwrap();
        assert_eq!(fs::read(&path).unwrap(), b"complete");
        ensure_cached(&path, |_| panic!("cached startup must not use the network")).unwrap();
        assert!(!path.with_file_name("server.exe.download").exists());
    }

    #[test]
    fn empty_download_cannot_become_cached_executable() {
        let path = test_path();
        assert!(
            ensure_cached(&path, |temporary| {
                fs::write(temporary, []).map_err(|error| error.to_string())
            })
            .is_err()
        );
        assert!(!path.exists());
    }

    #[test]
    fn every_supported_host_selects_its_own_versioned_asset() {
        for (os, arch, expected) in [
            (
                Os::Windows,
                Architecture::X8664,
                "x86_64-pc-windows-msvc.exe",
            ),
            (
                Os::Windows,
                Architecture::Aarch64,
                "aarch64-pc-windows-msvc.exe",
            ),
            (Os::Mac, Architecture::X8664, "x86_64-apple-darwin"),
            (Os::Mac, Architecture::Aarch64, "aarch64-apple-darwin"),
            (Os::Linux, Architecture::X8664, "x86_64-unknown-linux-musl"),
            (
                Os::Linux,
                Architecture::Aarch64,
                "aarch64-unknown-linux-musl",
            ),
        ] {
            let target = target(os, arch).unwrap();
            let url = download_url(target);
            assert!(url.ends_with(expected), "{url}");
            assert!(url.contains(&format!("/server-v{}/", version())));
            assert_eq!(
                cache_path(target).file_name().unwrap(),
                url.rsplit('/').next().unwrap()
            );
        }
        assert!(target(Os::Windows, Architecture::X86).is_err());
    }

    #[test]
    fn configuration_preserves_overrides_and_empty_settings_reset_defaults() {
        let bare = json!({"locale": "zh-cn", "health": {"qualityWarn": 80}});
        let wrapped = json!({"lspfAnalysis": bare});
        assert_eq!(configuration(Some(bare)).unwrap(), wrapped);
        assert_eq!(configuration(Some(wrapped.clone())).unwrap(), wrapped);
        assert_eq!(configuration(None).unwrap(), json!({"lspfAnalysis": {}}));
        assert!(configuration(Some(json!({"lspfAnalysis": null}))).is_err());
        assert!(configuration(Some(json!([]))).is_err());
    }
}
