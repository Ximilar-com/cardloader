//! Downloading model files: resumable (HTTP Range on a `.part` file), cancellable, and checked against the
//! catalog's size and SHA-256 before the file takes its final name.

use std::path::{Path, PathBuf};

use futures::StreamExt;
use sha2::{Digest, Sha256};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio_util::sync::CancellationToken;

use super::catalog::ModelFile;
use crate::error::{Error, Result};

pub fn part_path(dest: &Path) -> PathBuf {
    let mut name = dest.file_name().unwrap_or_default().to_os_string();
    name.push(".part");
    dest.with_file_name(name)
}

/// Bytes already on disk for this file: complete, or a partial download.
pub fn present(dest: &Path) -> u64 {
    std::fs::metadata(dest)
        .or_else(|_| std::fs::metadata(part_path(dest)))
        .map(|m| m.len())
        .unwrap_or(0)
}

async fn hash_existing(path: &Path, hasher: &mut Sha256) -> Result<u64> {
    let mut file = match tokio::fs::File::open(path).await {
        Ok(f) => f,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(0),
        Err(e) => return Err(e.into()),
    };
    let mut buf = vec![0u8; 1 << 20];
    let mut total = 0;
    loop {
        let n = file.read(&mut buf).await?;
        if n == 0 {
            return Ok(total);
        }
        hasher.update(&buf[..n]);
        total += n as u64;
    }
}

/// Downloads `file` to `dest`, calling `progress` with the bytes of this file on disk so far.
pub async fn fetch(
    http: &reqwest::Client,
    file: &ModelFile,
    url: &str,
    dest: &Path,
    progress: &(dyn Fn(u64) + Send + Sync),
    cancel: &CancellationToken,
) -> Result<()> {
    if dest.is_file() {
        progress(file.size);
        return Ok(());
    }
    if let Some(dir) = dest.parent() {
        tokio::fs::create_dir_all(dir).await?;
    }
    let part = part_path(dest);
    let mut hasher = Sha256::new();
    let mut have = hash_existing(&part, &mut hasher).await?;
    if have > file.size {
        tokio::fs::remove_file(&part).await?;
        hasher = Sha256::new();
        have = 0;
    }

    if have < file.size {
        let mut request = http.get(url);
        if have > 0 {
            request = request.header(reqwest::header::RANGE, format!("bytes={have}-"));
        }
        let response = tokio::select! {
            r = request.send() => r.map_err(|e| Error::msg(format!("download failed: {e}")))?,
            _ = cancel.cancelled() => return Err(Error::msg("download cancelled")),
        };
        let status = response.status();
        if !status.is_success() {
            return Err(Error::msg(format!(
                "download failed: HTTP {status} for {}",
                file.name
            )));
        }
        let append = have > 0 && status == reqwest::StatusCode::PARTIAL_CONTENT;
        if !append {
            // The server sent the whole file: start over.
            hasher = Sha256::new();
            have = 0;
        }
        let mut out = tokio::fs::OpenOptions::new()
            .create(true)
            .write(true)
            .append(append)
            .truncate(!append)
            .open(&part)
            .await?;
        progress(have);
        let mut stream = response.bytes_stream();
        loop {
            let chunk = tokio::select! {
                c = stream.next() => c,
                _ = cancel.cancelled() => {
                    out.flush().await?;
                    return Err(Error::msg("download cancelled"));
                }
            };
            let Some(chunk) = chunk else { break };
            let chunk = chunk.map_err(|e| Error::msg(format!("download interrupted: {e}")))?;
            out.write_all(&chunk).await?;
            hasher.update(&chunk);
            have += chunk.len() as u64;
            if have > file.size {
                break;
            }
            progress(have);
        }
        out.flush().await?;
    }

    let digest = format!("{:x}", hasher.finalize());
    if have != file.size || digest != file.sha256 {
        let _ = tokio::fs::remove_file(&part).await;
        return Err(Error::msg(format!(
            "{} is damaged (expected {} bytes, got {have}; checksum {}) — download it again",
            file.name,
            file.size,
            if digest == file.sha256 {
                "ok"
            } else {
                "mismatch"
            }
        )));
    }
    tokio::fs::rename(&part, dest).await?;
    progress(file.size);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};
    use wiremock::matchers::{header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn spec(body: &[u8]) -> ModelFile {
        ModelFile {
            name: "m.gguf",
            url: "",
            size: body.len() as u64,
            sha256: Box::leak(format!("{:x}", Sha256::digest(body)).into_boxed_str()),
        }
    }

    #[tokio::test]
    async fn downloads_resumes_and_verifies() {
        let body: Vec<u8> = (0..200_000u32).map(|i| (i % 251) as u8).collect();
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/m.gguf"))
            .and(header("range", "bytes=50000-"))
            .respond_with(ResponseTemplate::new(206).set_body_bytes(body[50_000..].to_vec()))
            .mount(&server)
            .await;
        let dir = tempfile::tempdir().unwrap();
        let dest = dir.path().join("m.gguf");
        std::fs::write(part_path(&dest), &body[..50_000]).unwrap();

        let seen = AtomicU64::new(0);
        let file = spec(&body);
        let url = format!("{}/m.gguf", server.uri());
        fetch(
            &reqwest::Client::new(),
            &file,
            &url,
            &dest,
            &|n| seen.store(n, Ordering::Relaxed),
            &CancellationToken::new(),
        )
        .await
        .unwrap();
        assert_eq!(std::fs::read(&dest).unwrap(), body);
        assert!(!part_path(&dest).exists());
        assert_eq!(seen.load(Ordering::Relaxed), body.len() as u64);
        assert_eq!(present(&dest), body.len() as u64);
    }

    #[tokio::test]
    async fn rejects_a_damaged_file() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(b"not the model".to_vec()))
            .mount(&server)
            .await;
        let dir = tempfile::tempdir().unwrap();
        let dest = dir.path().join("m.gguf");
        let mut file = spec(b"the real model");
        file.size = 13;
        let err = fetch(
            &reqwest::Client::new(),
            &file,
            &format!("{}/m.gguf", server.uri()),
            &dest,
            &|_| {},
            &CancellationToken::new(),
        )
        .await
        .unwrap_err();
        assert!(err.to_string().contains("damaged"), "{err}");
        assert!(!dest.exists() && !part_path(&dest).exists());
    }
}
