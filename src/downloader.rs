// Handles downloading dictionary data from Kaikki with cascading fallbacks.

use chrono::{DateTime, Utc};
use regex::Regex;
use serde_json::json;
use std::fs::{self, File};
use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::time::Instant;

pub struct Downloader {
    pub source_lang: String,
    pub extraction_date: Option<String>,
}

const KAIKKI_URL_EN: &str = "https://kaikki.org/dictionary/Greek/kaikki.org-dictionary-Greek.jsonl";
const KAIKKI_URL_EL: &str = "https://kaikki.org/elwiktionary/Greek/kaikki.org-dictionary-Greek.jsonl";

const KAIKKI_INDEX_EN: &str = "https://kaikki.org/dictionary/Greek/";
const KAIKKI_INDEX_EL: &str = "https://kaikki.org/elwiktionary/Greek/";

const LOCAL_KAIKKI_EN: &str = "en-el/kaikki.org-dictionary-Greek-words.jsonl";
const LOCAL_KAIKKI_EL: &str = "el/kaikki.org-dictionary-Greek.jsonl";

// The committed fallback JSONLs are snapshots from a fixed Wiktionary
// extraction date. The filenames intentionally do NOT carry the date
// (lemma's general rule: no dates in any names); the snapshot date lives
// in the constants below so the copyright page can still display it.
const LOCAL_FALLBACK_EN: &str = "greek_data_en.jsonl";
const LOCAL_FALLBACK_EL: &str = "greek_data_el.jsonl";

const LOCAL_FALLBACK_EXTRACTION_DATE_EN: &str = "2025-07-16";
const LOCAL_FALLBACK_EXTRACTION_DATE_EL: &str = "2025-07-17";

const GITHUB_EN: &str = "https://raw.githubusercontent.com/open-greek/lemma/main/greek_data_en.jsonl";
const GITHUB_EL: &str = "https://raw.githubusercontent.com/open-greek/lemma/main/greek_data_el.jsonl";

impl Downloader {
    pub fn new(source_lang: &str) -> Self {
        Self {
            source_lang: source_lang.to_string(),
            extraction_date: None,
        }
    }

    fn primary_url(&self) -> &'static str {
        if self.source_lang == "en" { KAIKKI_URL_EN } else { KAIKKI_URL_EL }
    }
    fn index_url(&self) -> &'static str {
        if self.source_lang == "en" { KAIKKI_INDEX_EN } else { KAIKKI_INDEX_EL }
    }
    fn local_kaikki_rel(&self) -> &'static str {
        if self.source_lang == "en" { LOCAL_KAIKKI_EN } else { LOCAL_KAIKKI_EL }
    }
    fn local_fallback(&self) -> &'static str {
        if self.source_lang == "en" { LOCAL_FALLBACK_EN } else { LOCAL_FALLBACK_EL }
    }
    fn local_fallback_extraction_date(&self) -> &'static str {
        if self.source_lang == "en" {
            LOCAL_FALLBACK_EXTRACTION_DATE_EN
        } else {
            LOCAL_FALLBACK_EXTRACTION_DATE_EL
        }
    }
    fn github_url(&self) -> &'static str {
        if self.source_lang == "en" { GITHUB_EN } else { GITHUB_EL }
    }

    /// Returns (success, filename). The downloader records the Wiktionary
    /// extraction date as `self.extraction_date` for the caller to read.
    pub fn download(&mut self) -> (bool, Option<String>) {
        let lang_desc = if self.source_lang == "en" { "English" } else { "Greek" };
        println!("Downloading Greek data from {} Wiktionary via Kaikki...", lang_desc);

        let target_filename = format!("greek_data_{}.jsonl", self.source_lang);

        // Use existing file (cached from a previous run).
        if Path::new(&target_filename).exists()
            && fs::metadata(&target_filename).map(|m| m.len() > 0).unwrap_or(false)
        {
            let line_count = count_lines(&target_filename);
            println!("Using existing file: {} ({} lines)", target_filename, line_count);
            self.extraction_date = load_sidecar(&target_filename);
            if let Some(d) = &self.extraction_date {
                println!("  Extraction date (from sidecar): {}", d);
            } else if Path::new(&target_filename).exists()
                && self.is_committed_fallback(&target_filename)
            {
                // Committed snapshot fallback: no sidecar, use the constant.
                self.extraction_date = Some(self.local_fallback_extraction_date().to_string());
                println!(
                    "  Extraction date (committed fallback constant): {}",
                    self.extraction_date.as_ref().unwrap()
                );
            }
            return (true, Some(target_filename));
        }

        // Try local kaikki dump
        if let Some(local_path) = self.find_local_kaikki_file() {
            println!("Using local kaikki dump: {}", local_path.display());
            if fs::copy(&local_path, &target_filename).is_ok() {
                let line_count = count_lines(&target_filename);
                println!("Copied {} lines to {}", line_count, target_filename);
                self.extraction_date = load_sidecar(local_path.to_str().unwrap_or(""))
                    .or_else(|| mtime_as_iso(&local_path));
                if let Some(d) = &self.extraction_date {
                    println!("  Extraction date (local dump): {}", d);
                    let abs = fs::canonicalize(&local_path).unwrap_or(local_path.clone());
                    let url = format!("file://{}", abs.display());
                    self.write_sidecar(&target_filename, &url);
                }
                return (true, Some(target_filename));
            }
        }

        // Try primary URL
        match self.download_from_url(self.primary_url(), &target_filename) {
            Ok(http_date) => {
                self.extraction_date = http_date.or_else(|| self.scrape_index_page_date());
                if let Some(d) = &self.extraction_date {
                    println!("  Extraction date (Kaikki): {}", d);
                    let url = self.primary_url().to_string();
                    self.write_sidecar(&target_filename, &url);
                } else {
                    println!("  Warning: could not determine Kaikki extraction date");
                }
                return (true, Some(target_filename));
            }
            Err(e) => {
                println!("Primary download failed: {}", e);
            }
        }

        // Try local committed fallback (a fixed Wiktionary snapshot whose date
        // lives in LOCAL_FALLBACK_EXTRACTION_DATE_*).
        let local_fallback = self.local_fallback().to_string();
        if Path::new(&local_fallback).exists() {
            println!("Primary download failed. Using local fallback file: {}", local_fallback);
            self.extraction_date = Some(self.local_fallback_extraction_date().to_string());
            println!(
                "  Extraction date (from fallback constant): {}",
                self.extraction_date.as_ref().unwrap()
            );
            return (true, Some(local_fallback));
        }

        // GitHub fallback. The URL points at a stable filename on main; if the
        // file is renamed or removed in the repo, this falls through to the
        // final error case.
        println!("Primary download failed and local fallback not found. Attempting GitHub fallback...");
        let gh_url = self.github_url().to_string();
        let fallback_filename = format!("greek_data_{}.jsonl", self.source_lang);

        match self.download_from_url(&gh_url, &fallback_filename) {
            Ok(_) => {
                self.extraction_date = Some(self.local_fallback_extraction_date().to_string());
                println!(
                    "GitHub fallback download successful. Extraction date (constant): {}",
                    self.extraction_date.as_ref().unwrap()
                );
                self.write_sidecar(&fallback_filename, &gh_url);
                (true, Some(fallback_filename))
            }
            Err(_) => {
                println!("Error: All download attempts failed.");
                println!("Try downloading manually from: {}", gh_url);
                println!("Save as: {}", fallback_filename);
                (false, None)
            }
        }
    }

    /// True if `path` looks like the committed fallback file (so we can
    /// trust the LOCAL_FALLBACK_EXTRACTION_DATE_* constant for it).
    fn is_committed_fallback(&self, path: &str) -> bool {
        Path::new(path).file_name().and_then(|n| n.to_str()) == Some(self.local_fallback())
    }

    fn find_local_kaikki_file(&self) -> Option<PathBuf> {
        let mut kaikki_dir = std::env::var("KAIKKI_LOCAL_DIR").unwrap_or_default();
        if kaikki_dir.is_empty() {
            if let Ok(contents) = fs::read_to_string(".env") {
                for line in contents.lines() {
                    let line = line.trim();
                    if line.is_empty() || line.starts_with('#') { continue; }
                    if let Some((k, v)) = line.split_once('=') {
                        if k.trim() == "KAIKKI_LOCAL_DIR" {
                            kaikki_dir = v.trim().to_string();
                        }
                    }
                }
            }
        }
        if kaikki_dir.is_empty() { return None; }

        let full_path = Path::new(&kaikki_dir).join(self.local_kaikki_rel());
        if !full_path.exists() { return None; }
        Some(full_path)
    }

    fn download_from_url(&self, url: &str, filename: &str) -> Result<Option<String>, String> {
        println!("Attempting to download from: {}", url);

        let start = Instant::now();
        let client = reqwest::blocking::Client::builder()
            .user_agent("Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36")
            .timeout(std::time::Duration::from_secs(300))
            .build()
            .map_err(|e| e.to_string())?;

        let resp = client.get(url).send().map_err(|e| e.to_string())?;
        let status = resp.status();
        println!("Response code: {}", status);
        if !status.is_success() {
            return Err(format!("HTTP {}", status));
        }

        let last_modified = resp.headers()
            .get(reqwest::header::LAST_MODIFIED)
            .and_then(|v| v.to_str().ok())
            .map(|s| s.to_string());
        if let Some(lm) = &last_modified {
            println!("Last-Modified: {}", lm);
        }
        let extraction_date = last_modified.as_deref().and_then(parse_http_date);

        let total_size = resp.content_length().unwrap_or(0);
        if total_size > 0 {
            println!("Content-Length: {} bytes ({:.2} MB)", total_size, total_size as f64 / 1024.0 / 1024.0);
        }

        let bytes = resp.bytes().map_err(|e| e.to_string())?;

        // Kaikki URLs sometimes return gzip; detect and decompress
        let content: Vec<u8> = if bytes.len() >= 2 && bytes[0] == 0x1f && bytes[1] == 0x8b {
            let mut decoder = flate2::read::GzDecoder::new(&bytes[..]);
            let mut out = Vec::new();
            decoder.read_to_end(&mut out).map_err(|e| e.to_string())?;
            out
        } else {
            bytes.to_vec()
        };

        let mut f = File::create(filename).map_err(|e| e.to_string())?;
        f.write_all(&content).map_err(|e| e.to_string())?;

        let elapsed = start.elapsed().as_secs_f64();
        let mb = content.len() as f64 / 1024.0 / 1024.0;
        println!("Download complete: {:.2} MB in {:.2} seconds", mb, elapsed);

        let line_count = count_lines(filename);
        println!("Downloaded {} lines to {}", line_count, filename);
        Ok(extraction_date)
    }

    fn scrape_index_page_date(&self) -> Option<String> {
        let client = reqwest::blocking::Client::builder()
            .user_agent("Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36")
            .timeout(std::time::Duration::from_secs(30))
            .build().ok()?;
        let body = client.get(self.index_url()).send().ok()?.text().ok()?;
        let re = Regex::new(r"extracted on (\d{4}-\d{2}-\d{2})").unwrap();
        re.captures(&body).and_then(|c| c.get(1)).map(|m| m.as_str().to_string())
    }

    fn write_sidecar(&self, jsonl_path: &str, source_url: &str) {
        let Some(extraction) = &self.extraction_date else { return; };
        let today = chrono::Local::now().format("%Y-%m-%d").to_string();
        let meta = json!({
            "extraction_date": extraction,
            "source_url": source_url,
            "downloaded_at": today,
        });
        let sidecar = format!("{}.meta", jsonl_path);
        if let Ok(mut f) = File::create(&sidecar) {
            let _ = f.write_all(serde_json::to_string_pretty(&meta).unwrap_or_default().as_bytes());
            let _ = f.write_all(b"\n");
        }
    }
}

fn count_lines(filename: &str) -> usize {
    let Ok(f) = File::open(filename) else { return 0 };
    BufReader::new(f).lines().count()
}

fn parse_http_date(header: &str) -> Option<String> {
    let dt: std::time::SystemTime = httpdate::parse_http_date(header).ok()?;
    let dt: DateTime<Utc> = dt.into();
    Some(dt.format("%Y-%m-%d").to_string())
}

fn parse_yyyymmdd_as_iso(value: &str) -> Option<String> {
    if value.len() == 8 && value.chars().all(|c| c.is_ascii_digit()) {
        Some(format!("{}-{}-{}", &value[..4], &value[4..6], &value[6..]))
    } else {
        None
    }
}

fn mtime_as_iso(path: &Path) -> Option<String> {
    let meta = fs::metadata(path).ok()?;
    let mtime = meta.modified().ok()?;
    let dt: DateTime<chrono::Local> = mtime.into();
    Some(dt.format("%Y-%m-%d").to_string())
}

fn load_sidecar(jsonl_path: &str) -> Option<String> {
    let sidecar = format!("{}.meta", jsonl_path);
    let contents = fs::read_to_string(&sidecar).ok()?;
    let meta: serde_json::Value = serde_json::from_str(&contents).ok()?;
    let raw = meta.get("extraction_date")?.as_str()?;
    if raw.len() == 10 && raw.chars().nth(4) == Some('-') {
        return Some(raw.to_string());
    }
    parse_yyyymmdd_as_iso(raw).or_else(|| Some(raw.to_string()))
}
