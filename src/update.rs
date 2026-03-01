use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result, bail};

const REPO: &str = "rucnyz/agents-sesame";
const BINARY: &str = if cfg!(target_os = "windows") {
    "ase.exe"
} else {
    "ase"
};
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
    let release_name = format!("ase-{latest}-{target}");
    let is_windows = cfg!(target_os = "windows");
    let archive_name = if is_windows {
        format!("{release_name}.zip")
    } else {
        format!("{release_name}.tar.gz")
    };
    let url = format!("https://github.com/{REPO}/releases/download/{latest}/{archive_name}");

    let tmpdir = tempdir()?;
    let archive_path = tmpdir.join(&archive_name);

    // Download
    curl_download(&url, &archive_path)
        .context("Failed to download release. Check your internet connection.")?;

    // Verify checksum if available
    let sha_url = format!("{url}.sha256");
    let sha_path = tmpdir.join(format!("{archive_name}.sha256"));
    if curl_download(&sha_url, &sha_path).is_ok() {
        verify_checksum(&tmpdir, &archive_name)?;
    }

    // Extract
    if is_windows {
        // Windows: use tar (bsdtar) which handles zip files
        let status = Command::new("tar")
            .args([
                "-xf",
                &archive_path.to_string_lossy(),
                "-C",
                &tmpdir.to_string_lossy(),
            ])
            .status()
            .context("Failed to extract archive")?;
        if !status.success() {
            bail!("Archive extraction failed");
        }
    } else {
        let status = Command::new("tar")
            .args([
                "-xzf",
                &archive_path.to_string_lossy(),
                "-C",
                &tmpdir.to_string_lossy(),
            ])
            .status()
            .context("Failed to extract archive")?;
        if !status.success() {
            bail!("tar extraction failed");
        }
    }

    // Find current binary path and replace
    let current_bin = current_exe_path()?;
    let new_bin = tmpdir.join(&release_name).join(BINARY);

    if !new_bin.exists() {
        bail!(
            "Binary not found in release archive at {}",
            new_bin.display()
        );
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

    let json: serde_json::Value =
        serde_json::from_slice(&output.stdout).context("Failed to parse GitHub API response")?;

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
        ("windows", "x86_64") => Ok("x86_64-pc-windows-msvc".into()),
        _ => bail!(
            "No pre-built binary for {os}-{arch}. Build from source: cargo install --git https://github.com/{REPO}"
        ),
    }
}

fn current_exe_path() -> Result<PathBuf> {
    env::current_exe()
        .context("Failed to determine current binary path")?
        .canonicalize()
        .context("Failed to resolve binary path")
}

fn tempdir() -> Result<PathBuf> {
    let dir = env::temp_dir().join(format!("ase-update-{}", std::process::id()));
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
    let sha_file = format!("{tarball}.sha256");

    // Read expected hash from .sha256 file (format: "hash  filename")
    let sha_content =
        fs::read_to_string(dir.join(&sha_file)).context("Failed to read checksum file")?;
    let expected_hash = sha_content
        .split_whitespace()
        .next()
        .unwrap_or("")
        .to_lowercase();

    if expected_hash.is_empty() {
        bail!("Empty checksum file");
    }

    // Try shasum (macOS), sha256sum (Linux), or certutil (Windows)
    let output = if Command::new("shasum").arg("--version").output().is_ok() {
        Command::new("shasum")
            .args(["-a", "256", tarball])
            .current_dir(dir)
            .output()
    } else if Command::new("sha256sum").arg("--version").output().is_ok() {
        Command::new("sha256sum")
            .arg(tarball)
            .current_dir(dir)
            .output()
    } else {
        // Windows: certutil -hashfile <file> SHA256
        Command::new("certutil")
            .args(["-hashfile", tarball, "SHA256"])
            .current_dir(dir)
            .output()
    };

    let output = output.context("No checksum tool available (shasum, sha256sum, or certutil)")?;
    if !output.status.success() {
        bail!("Checksum tool failed");
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    // certutil outputs hash on the second line; shasum/sha256sum output "hash  filename"
    let computed_hash = stdout
        .lines()
        .find_map(|line| {
            let trimmed = line.trim().to_lowercase();
            // Skip lines that are clearly not hashes (certutil headers/footers)
            if trimmed.len() == 64 && trimmed.chars().all(|c| c.is_ascii_hexdigit()) {
                Some(trimmed)
            } else {
                // shasum/sha256sum format: "hash  filename"
                let first = trimmed.split_whitespace().next().unwrap_or("");
                if first.len() == 64 && first.chars().all(|c| c.is_ascii_hexdigit()) {
                    Some(first.to_string())
                } else {
                    None
                }
            }
        })
        .unwrap_or_default();

    if computed_hash == expected_hash {
        Ok(())
    } else {
        bail!("Checksum mismatch: expected {expected_hash}, got {computed_hash}");
    }
}
