use anyhow::{Result, Context, anyhow};
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;
use reqwest::blocking::Client;
use reqwest::header::{HeaderMap, HeaderValue, RANGE, USER_AGENT};

#[derive(Clone)]
pub struct DownloadProgress {
    pub total_bytes: Arc<AtomicU64>,
    pub downloaded_bytes: Arc<AtomicU64>,
    pub is_finished: Arc<AtomicU64>,
}

impl DownloadProgress {
    pub fn new(total: u64) -> Self {
        Self {
            total_bytes: Arc::new(AtomicU64::new(total)),
            downloaded_bytes: Arc::new(AtomicU64::new(0)),
            is_finished: Arc::new(AtomicU64::new(0)),
        }
    }

    pub fn progress(&self) -> f64 {
        let total = self.total_bytes.load(Ordering::SeqCst);
        let downloaded = self.downloaded_bytes.load(Ordering::SeqCst);
        if total == 0 {
            0.0
        } else {
            (downloaded as f64 / total as f64) * 100.0
        }
    }

    pub fn is_finished(&self) -> bool {
        self.is_finished.load(Ordering::SeqCst) == 1
    }

    pub fn mark_finished(&self) {
        self.is_finished.store(1, Ordering::SeqCst);
    }
}

pub struct Downloader {
    client: Client,
}

impl Downloader {
    pub fn new() -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(300))
            .build()
            .unwrap();
        Self { client }
    }

    pub fn download(
        &self,
        url: &str,
        dest_dir: &Path,
        filename: Option<&str>,
        token: Option<&str>,
    ) -> Result<(PathBuf, DownloadProgress)> {
        let final_filename = if let Some(name) = filename {
            name.to_string()
        } else {
            url.split('/')
                .last()
                .unwrap_or("download.zip")
                .to_string()
        };

        let dest_path = dest_dir.join(&final_filename);
        let temp_path = dest_dir.join(format!("{}.tmp", final_filename));

        let mut headers = HeaderMap::new();
        headers.insert(USER_AGENT, HeaderValue::from_static("woGAer"));
        if let Some(tok) = token {
            headers.insert(
                "Authorization",
                HeaderValue::from_str(&format!("Bearer {}", tok))?,
            );
        }

        let mut request = self.client.get(url).headers(headers.clone());

        if temp_path.exists() {
            let existing_size = std::fs::metadata(&temp_path)?.len();
            if existing_size > 0 {
                headers.insert(RANGE, HeaderValue::from_str(&format!("bytes={}-", existing_size))?);
                request = self.client.get(url).headers(headers.clone());
            }
        }

        let response = request.send().context("Download request failed")?;

        if !response.status().is_success() && response.status().as_u16() != 206 {
            let status = response.status();
            let text = response.text().unwrap_or_default();
            return Err(anyhow!("Download failed: {} - {}", status, text));
        }

        let total_size = response
            .headers()
            .get("content-range")
            .and_then(|v| v.to_str().ok())
            .and_then(|s| {
                s.split('/')
                    .last()
                    .and_then(|v| v.parse::<u64>().ok())
            })
            .or_else(|| {
                response
                    .headers()
                    .get("content-length")
                    .and_then(|v| v.to_str().ok())
                    .and_then(|v| v.parse::<u64>().ok())
            })
            .unwrap_or(0);

        let existing_size = if temp_path.exists() {
            std::fs::metadata(&temp_path)?.len()
        } else {
            0
        };

        let total_bytes = total_size.max(existing_size);
        let progress = DownloadProgress::new(total_bytes);
        progress.downloaded_bytes.store(existing_size, Ordering::SeqCst);

        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&temp_path)?;

        let bytes = response.bytes()?;
        file.write_all(&bytes)?;
        let new_size = existing_size + bytes.len() as u64;
        progress.downloaded_bytes.store(new_size, Ordering::SeqCst);

        if new_size >= total_bytes && total_bytes > 0 {
            progress.mark_finished();
            std::fs::rename(&temp_path, &dest_path)?;
            Ok((dest_path, progress))
        } else {
            progress.mark_finished();
            std::fs::rename(&temp_path, &dest_path)?;
            Ok((dest_path, progress))
        }
    }

    pub fn download_multi(
        &self,
        artifacts: Vec<(&str, Option<&str>)>,
        dest_dir: &Path,
        token: Option<&str>,
    ) -> Result<Vec<(String, PathBuf)>> {
        let mut results = Vec::new();

        for (url, filename) in artifacts {
            let (path, _) = self.download(url, dest_dir, filename, token)?;
            let name = path.file_name().unwrap_or_default().to_string_lossy().to_string();
            results.push((name, path));
        }

        Ok(results)
    }

    pub fn download_with_callback<F>(
        &self,
        url: &str,
        dest_dir: &Path,
        filename: Option<&str>,
        token: Option<&str>,
        mut callback: F,
    ) -> Result<PathBuf>
    where
        F: FnMut(f64, u64, u64) -> bool,
    {
        let final_filename = if let Some(name) = filename {
            name.to_string()
        } else {
            url.split('/')
                .last()
                .unwrap_or("download.zip")
                .to_string()
        };

        let dest_path = dest_dir.join(&final_filename);
        let temp_path = dest_dir.join(format!("{}.tmp", final_filename));

        let mut headers = HeaderMap::new();
        headers.insert(USER_AGENT, HeaderValue::from_static("woGAer"));
        if let Some(tok) = token {
            headers.insert(
                "Authorization",
                HeaderValue::from_str(&format!("Bearer {}", tok))?,
            );
        }

        let existing_size = if temp_path.exists() {
            std::fs::metadata(&temp_path)?.len()
        } else {
            0
        };

        if existing_size > 0 {
            headers.insert(RANGE, HeaderValue::from_str(&format!("bytes={}-", existing_size))?);
        }

        let mut request = self.client.get(url).headers(headers);

        let response = request.send().context("Download request failed")?;

        if !response.status().is_success() && response.status().as_u16() != 206 {
            let status = response.status();
            let text = response.text().unwrap_or_default();
            return Err(anyhow!("Download failed: {} - {}", status, text));
        }

        let total_size = response
            .headers()
            .get("content-range")
            .and_then(|v| v.to_str().ok())
            .and_then(|s| {
                s.split('/')
                    .last()
                    .and_then(|v| v.parse::<u64>().ok())
            })
            .or_else(|| {
                response
                    .headers()
                    .get("content-length")
                    .and_then(|v| v.to_str().ok())
                    .and_then(|v| v.parse::<u64>().ok())
            })
            .unwrap_or(0);

        let total = total_size.max(existing_size);
        let mut downloaded = existing_size;

        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&temp_path)?;

        let bytes = response.bytes()?;
        let chunk_size = bytes.len() as u64;
        file.write_all(&bytes)?;
        downloaded += chunk_size;

        let progress = if total > 0 {
            (downloaded as f64 / total as f64) * 100.0
        } else {
            0.0
        };

        let should_continue = callback(progress, downloaded, total);
        if !should_continue {
            return Err(anyhow!("Download cancelled by callback"));
        }

        if downloaded >= total && total > 0 {
            std::fs::rename(&temp_path, &dest_path)?;
        } else {
            std::fs::rename(&temp_path, &dest_path)?;
        }

        Ok(dest_path)
    }

    pub fn cleanup_temp(&self, dest_dir: &Path) -> Result<()> {
        let entries = std::fs::read_dir(dest_dir)?;
        for entry in entries {
            let entry = entry?;
            let path = entry.path();
            if let Some(ext) = path.extension() {
                if ext == "tmp" {
                    let _ = std::fs::remove_file(&path);
                }
            }
        }
        Ok(())
    }
}