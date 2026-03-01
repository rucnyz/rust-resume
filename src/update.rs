use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result, bail};

const REPO: &str = "rucnyz/rust-resume";
const BINARY: &str = "fr-rs";
const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");

pub fn self_update() -> Result<()> {
    eprintln!("Checking for updates...");

    let latest = fetch_latest_version()?;
    let latest_clean = latest.trim_start_matches('v');

    if latest_clean == CURRENT_VERSION {
        eprintln!("Already up to date (v{CURRENT_VERSION}).");
        return Ok(());
    }

    eprintln!("Update available: v{CURRENT_VERSION} -> {latest}");
    eprintln!("Downloading...");

    let target = detect_target()?;
    let release_name = format!("{BINARY}-{latest}-{target}");
    let tarball = format!("{release_name}.tar.gz");
    let url = format!("https://github.com/{REPO}/releases/download/{latest}/{tarball}");

    let tmpdir = tempdir()?;
    let tar_path = tmpdir.join(&tarball);

    // Download
    curl_download(&url, &tar_path)
        .context("Failed to download release. Check your internet connection.")?;

    // Verify checksum if available
    let sha_url = format!("{url}.sha256");
    let sha_path = tmpdir.join(format!("{tarball}.sha256"));
    if curl_download(&sha_url, &sha_path).is_ok() {
        verify_checksum(&tmpdir, &tarball)?;
    }

    // Extract
    let status = Command::new("tar")
        .args(["-xzf", &tar_path.to_string_lossy(), "-C", &tmpdir.to_string_lossy()])
        .status()
        .context("Failed to extract archive")?;
    if !status.success() {
        bail!("tar extraction failed");
    }

    // Find current binary path and replace
    let current_bin = current_exe_path()?;
    let new_bin = tmpdir.join(&release_name).join(BINARY);

    if !new_bin.exists() {
        bail!("Binary not found in release archive at {}", new_bin.display());
    }

    // Replace: rename old -> .bak, copy new, remove .bak
    let backup = current_bin.with_extension("bak");
    fs::rename(&current_bin, &backup)
        .context("Failed to back up current binary. Check file permissions.")?;

    match fs::copy(&new_bin, &current_bin) {
        Ok(_) => {
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let _ = fs::set_permissions(&current_bin, fs::Permissions::from_mode(0o755));
            }
            let _ = fs::remove_file(&backup);
        }
        Err(e) => {
            // Rollback
            let _ = fs::rename(&backup, &current_bin);
            bail!("Failed to install new binary: {e}");
        }
    }

    // Cleanup
    let _ = fs::remove_dir_all(&tmpdir);

    eprintln!("Updated to {latest} successfully!");
    Ok(())
}

fn fetch_latest_version() -> Result<String> {
    let output = Command::new("curl")
        .args([
            "-fsSL",
            &format!("https://api.github.com/repos/{REPO}/releases/latest"),
        ])
        .output()
        .context("Failed to run curl. Is curl installed?")?;

    if !output.status.success() {
        bail!("Failed to fetch release info from GitHub");
    }

    let json: serde_json::Value = serde_json::from_slice(&output.stdout)
        .context("Failed to parse GitHub API response")?;

    let tag = json["tag_name"]
        .as_str()
        .context("No tag_name in GitHub API response")?
        .to_string();

    Ok(tag)
}

fn detect_target() -> Result<String> {
    let os = env::consts::OS;
    let arch = env::consts::ARCH;

    match (os, arch) {
        ("linux", "x86_64") => Ok("x86_64-unknown-linux-musl".into()),
        ("linux", "aarch64") => Ok("aarch64-unknown-linux-gnu".into()),
        ("macos", "x86_64") => Ok("x86_64-apple-darwin".into()),
        ("macos", "aarch64") => Ok("aarch64-apple-darwin".into()),
        _ => bail!("No pre-built binary for {os}-{arch}. Build from source: cargo install --git https://github.com/{REPO}"),
    }
}

fn current_exe_path() -> Result<PathBuf> {
    env::current_exe()
        .context("Failed to determine current binary path")?
        .canonicalize()
        .context("Failed to resolve binary path")
}

fn tempdir() -> Result<PathBuf> {
    let dir = env::temp_dir().join(format!("fr-rs-update-{}", std::process::id()));
    fs::create_dir_all(&dir)?;
    Ok(dir)
}

fn curl_download(url: &str, dest: &Path) -> Result<()> {
    let status = Command::new("curl")
        .args(["-fSL", "-o", &dest.to_string_lossy(), url])
        .status()
        .context("Failed to run curl")?;
    if !status.success() {
        bail!("curl download failed for {url}");
    }
    Ok(())
}

fn verify_checksum(dir: &Path, tarball: &str) -> Result<()> {
    // Try shasum (macOS) then sha256sum (Linux)
    let sha_tool = if Command::new("shasum").arg("--version").output().is_ok() {
        "shasum"
    } else {
        "sha256sum"
    };

    let status = if sha_tool == "shasum" {
        Command::new("shasum")
            .args(["-a", "256", "-c", &format!("{tarball}.sha256")])
            .current_dir(dir)
            .status()
    } else {
        Command::new("sha256sum")
            .args(["-c", &format!("{tarball}.sha256")])
            .current_dir(dir)
            .status()
    };

    match status {
        Ok(s) if s.success() => Ok(()),
        _ => bail!("Checksum verification failed"),
    }
}
