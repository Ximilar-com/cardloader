//! SANE through `scanimage` (sane-utils on Linux, `brew install sane-backends` on macOS).

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;

use tokio::io::{AsyncBufReadExt, AsyncReadExt, BufReader};
use tokio::process::Command;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use super::{side_for, DeviceInfo, ScannerSupport};
use crate::error::{Error, Result};
use crate::model::{ColorMode, MultifeedMode, ScanSettings};
use crate::pairing::Side;

/// Apps started from a desktop don't get the shell's PATH, so look in the usual places too.
const EXTRA_DIRS: &[&str] = &["/opt/homebrew/bin", "/usr/local/bin", "/usr/bin", "/bin"];

pub fn find_scanimage() -> Option<PathBuf> {
    let path = std::env::var_os("PATH").unwrap_or_default();
    std::env::split_paths(&path)
        .chain(EXTRA_DIRS.iter().map(PathBuf::from))
        .map(|dir| dir.join("scanimage"))
        .find(|p| p.is_file())
}

fn install_hint() -> String {
    if cfg!(target_os = "macos") {
        "scanimage not found — install SANE with `brew install sane-backends`".into()
    } else {
        "scanimage not found — install SANE (e.g. `sudo apt install sane-utils`)".into()
    }
}

pub fn support() -> ScannerSupport {
    let found = find_scanimage();
    ScannerSupport {
        backend: "sane".into(),
        available: found.is_some(),
        hint: if found.is_some() {
            None
        } else {
            Some(install_hint())
        },
    }
}

fn bin() -> Result<PathBuf> {
    find_scanimage().ok_or_else(|| Error::msg(install_hint()))
}

pub async fn list_devices() -> Result<Vec<DeviceInfo>> {
    let out = tokio::time::timeout(
        Duration::from_secs(60),
        Command::new(bin()?)
            .args(["-f", "%d|%v|%m%n"])
            .kill_on_drop(true)
            .output(),
    )
    .await
    .map_err(|_| Error::msg("scanimage -L timed out"))??;
    Ok(parse_devices(&String::from_utf8_lossy(&out.stdout)))
}

fn parse_devices(text: &str) -> Vec<DeviceInfo> {
    text.lines()
        .filter_map(|line| {
            let mut parts = line.splitn(3, '|');
            let id = parts.next()?.trim();
            if id.is_empty() {
                return None;
            }
            let vendor = parts.next().unwrap_or("").trim();
            let model = parts.next().unwrap_or("").trim();
            Some(DeviceInfo {
                id: id.into(),
                name: format!("{vendor} {model}").trim().to_string(),
                backend: "sane".into(),
            })
        })
        .collect()
}

/// The device's options from `scanimage -A`: option name → allowed values (for lists like `--source`).
async fn device_options(dev: &str) -> Result<HashMap<String, Vec<String>>> {
    let out = tokio::time::timeout(
        Duration::from_secs(60),
        Command::new(bin()?)
            .args(["-A", "-d", dev])
            .kill_on_drop(true)
            .output(),
    )
    .await
    .map_err(|_| Error::msg("scanimage -A timed out"))??;
    if !out.status.success() {
        return Err(Error::msg(format!(
            "scanner {dev} not available: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        )));
    }
    Ok(parse_options(&String::from_utf8_lossy(&out.stdout)))
}

fn parse_options(text: &str) -> HashMap<String, Vec<String>> {
    let mut opts = HashMap::new();
    for line in text.lines() {
        let line = line.trim();
        if !line.starts_with('-') {
            continue;
        }
        let (name, rest) = line.split_once(' ').unwrap_or((line, ""));
        let name = name.split('[').next().unwrap_or(name).to_string();
        let spec = rest.split(" [").next().unwrap_or("").trim();
        let values = spec
            .split('|')
            .map(|v| v.trim().to_string())
            .filter(|v| !v.is_empty())
            .collect();
        opts.insert(name, values);
    }
    opts
}

/// The scanimage arguments for a card batch, using only options the device has.
fn build_args(
    dev: &str,
    s: &ScanSettings,
    opts: &HashMap<String, Vec<String>>,
    batch: &str,
) -> Vec<String> {
    let has = |o: &str| opts.contains_key(o);
    let mut args = vec!["-d".to_string(), dev.to_string()];
    let mut push = |k: &str, v: String| {
        args.push(k.to_string());
        args.push(v);
    };

    if let Some(sources) = opts.get("--source") {
        let wanted = if s.duplex { "duplex" } else { "front" };
        let pick = sources
            .iter()
            .find(|v| v.to_lowercase().contains(wanted))
            .or_else(|| sources.iter().find(|v| v.to_lowercase().contains("adf")))
            .or_else(|| sources.first());
        if let Some(v) = pick {
            push("--source", v.clone());
        }
    }
    if let Some(modes) = opts.get("--mode") {
        let wanted = match s.color {
            ColorMode::Color => "color",
            ColorMode::Gray => "gray",
            ColorMode::Bw => "lineart",
        };
        if let Some(v) = modes.iter().find(|v| v.to_lowercase().contains(wanted)) {
            push("--mode", v.clone());
        }
    }
    if has("--resolution") {
        push("--resolution", s.dpi.to_string());
    }
    if has("--page-width") {
        push("--page-width", format!("{:.1}", s.page_width_mm));
    }
    if has("--page-height") {
        push("--page-height", format!("{:.1}", s.page_height_mm));
    }
    if has("-x") {
        push("-x", format!("{:.1}", s.page_width_mm));
    }
    if has("-y") {
        push("-y", format!("{:.1}", s.page_height_mm));
    }
    if s.multifeed == MultifeedMode::Off {
        // Cards are thick and stiff: ultrasonic/length multifeed detection reports false double feeds.
        if let Some(actions) = opts.get("--df-action") {
            if let Some(v) = actions.iter().find(|v| v.eq_ignore_ascii_case("continue")) {
                push("--df-action", v.clone());
            }
        }
        for flag in ["--df-thickness", "--df-length"] {
            if has(flag) {
                args.push(format!("{flag}=no"));
            }
        }
    }
    if has("--buffermode") {
        args.push("--buffermode=On".into());
    }
    if has("--prepick") {
        args.push("--prepick=On".into());
    }
    args.extend([
        "--format=png".to_string(),
        format!("--batch={batch}"),
        "--batch-print".to_string(),
    ]);
    args
}

pub async fn scan(
    dev: &str,
    settings: &ScanSettings,
    out_dir: &Path,
    prefix: &str,
    pages: mpsc::Sender<(PathBuf, Side)>,
    cancel: CancellationToken,
) -> Result<()> {
    let opts = device_options(dev).await?;
    let batch = out_dir
        .join(format!("{prefix}_%04d.png"))
        .to_string_lossy()
        .into_owned();
    let args = build_args(dev, settings, &opts, &batch);
    tracing::info!("scanimage {}", args.join(" "));
    let mut child = Command::new(bin()?)
        .args(&args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()?;
    let mut stdout = BufReader::new(child.stdout.take().expect("piped")).lines();
    let mut stderr = child.stderr.take().expect("piped");
    let err_task = tokio::spawn(async move {
        let mut text = String::new();
        let _ = stderr.read_to_string(&mut text).await;
        text
    });

    let mut index = 0;
    loop {
        tokio::select! {
            line = stdout.next_line() => match line? {
                Some(line) => {
                    let path = PathBuf::from(line.trim());
                    if path.is_file() {
                        index += 1;
                        if pages.send((path, side_for(index, settings.duplex))).await.is_err() {
                            break;
                        }
                    }
                }
                None => break,
            },
            _ = cancel.cancelled() => {
                let _ = child.start_kill();
                break;
            }
        }
    }
    let status = child.wait().await?;
    let stderr = err_task.await.unwrap_or_default();
    // 7 = SANE_STATUS_NO_DOCS: the feeder ran empty, the normal end of a batch.
    match status.code() {
        Some(0) | Some(7) => Ok(()),
        _ if cancel.is_cancelled() => Ok(()),
        _ if index > 0 && stderr.contains("Document feeder out of documents") => Ok(()),
        code => Err(Error::msg(format!(
            "scanimage failed ({}): {}",
            code.map(|c| c.to_string())
                .unwrap_or_else(|| "signal".into()),
            stderr.trim().lines().last().unwrap_or("no output")
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FUJITSU_A: &str = "All options specific to device `fujitsu:fi-8170:1234':
  Standard:
    --source ADF Front|ADF Back|ADF Duplex [ADF Front]
    --mode Lineart|Halftone|Gray|Color [Lineart]
    --resolution 50..600dpi (in steps of 1) [600]
  Geometry:
    -l 0..215.872mm [0]
    -x 0..215.872mm [215.872]
    --page-width 0..221.121mm [215.872]
    --page-height 0..1100mm [279.364]
  Sensors and Buttons:
    --df-action Default|Continue|Stop [Default]
    --df-thickness[=(yes|no)] [inactive]
    --df-length[=(yes|no)] [inactive]
    --prepick Default|Off|On [Default]
";

    #[test]
    fn parses_device_list() {
        let devs = parse_devices("fujitsu:fi-8170:1234|FUJITSU|fi-8170\n\n");
        assert_eq!(devs.len(), 1);
        assert_eq!(devs[0].name, "FUJITSU fi-8170");
    }

    #[test]
    fn builds_card_scan_args() {
        let opts = parse_options(FUJITSU_A);
        assert_eq!(
            opts["--source"],
            vec!["ADF Front", "ADF Back", "ADF Duplex"]
        );
        assert!(opts.contains_key("--df-thickness"));
        let args = build_args(
            "fujitsu:x",
            &ScanSettings::default(),
            &opts,
            "/tmp/s_%04d.png",
        )
        .join(" ");
        assert!(args.contains("--source ADF Duplex"), "{args}");
        assert!(args.contains("--mode Color"));
        assert!(args.contains("--resolution 600"));
        assert!(args.contains("--page-width 66.0"));
        assert!(args.contains("--df-action Continue"));
        assert!(args.contains("--df-thickness=no"));
        assert!(args.contains("--prepick=On"));
        assert!(!args.contains("--buffermode"));
        assert!(args.ends_with("--format=png --batch=/tmp/s_%04d.png --batch-print"));
    }

    #[test]
    fn duplex_sides_alternate() {
        assert_eq!(side_for(1, true), Side::Front);
        assert_eq!(side_for(2, true), Side::Back);
        assert_eq!(side_for(2, false), Side::Front);
    }
}
