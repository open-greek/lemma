// MOBI binary structure validation for Kindle dictionary recognition.
//
// Checks that each MOBI has the binary structure Kindle requires for a
// dictionary: PalmDB header, MOBI header with dictionary type, EXTH records
// with language metadata and matching content identifiers, and at least one
// INDX record after the text section.
//
// Also handles PalmDB name uniqueness across distinct filename families,
// where a "family" is a basename with its `_YYYYMMDD` date stamp stripped:
//
//     lemma_greek_en_20260410.mobi  -> lemma_greek_en.mobi
//
// Two files in the same family are expected to share a PalmDB name (kindling
// derives it from the stable OPF title), so we dedupe to the newest file per
// family by mtime before validating. Only collisions across distinct families
// are real problems on the device — the Kindle FSCK will rename both files,
// hiding them from the dictionary list.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use regex::Regex;

/// Strip a `_YYYYMMDD` segment from a filename to get its family identity.
pub fn family_key(path: impl AsRef<Path>) -> String {
    let base = path
        .as_ref()
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    // Strip a `_YYYYMMDD` segment followed by `_`, `.`, or end of string.
    // The regex crate doesn't support lookahead, so do it manually.
    let re = Regex::new(r"_\d{8}").unwrap();
    let mut result = String::with_capacity(base.len());
    let mut last_end = 0;
    for m in re.find_iter(&base) {
        let after = m.end();
        let next = base.as_bytes().get(after).copied();
        let is_terminator = matches!(next, None | Some(b'_') | Some(b'.'));
        if is_terminator {
            result.push_str(&base[last_end..m.start()]);
            last_end = after;
        }
    }
    result.push_str(&base[last_end..]);
    result
}

/// Keep only the newest file per family, by filesystem mtime.
///
/// Stale older versions sitting in `dist/` should not gate deploys, so we sort
/// by mtime (newest first) and keep the first file we see per family key.
pub fn dedupe_to_newest_per_family(paths: &[PathBuf]) -> Vec<PathBuf> {
    let mut entries: Vec<(SystemTime, PathBuf)> = paths
        .iter()
        .map(|p| {
            let mtime = fs::metadata(p)
                .and_then(|m| m.modified())
                .unwrap_or(SystemTime::UNIX_EPOCH);
            (mtime, p.clone())
        })
        .collect();
    // Newest first.
    entries.sort_by(|a, b| b.0.cmp(&a.0));
    let mut seen: HashMap<String, PathBuf> = HashMap::new();
    for (_, path) in entries {
        let key = family_key(&path);
        seen.entry(key).or_insert(path);
    }
    let mut result: Vec<PathBuf> = seen.into_values().collect();
    result.sort();
    result
}

/// Per-file validation outcome.
#[derive(Debug, Clone)]
pub struct FileValidation {
    pub path: PathBuf,
    pub palmdb_name: String,
    pub family: String,
    pub errors: Vec<String>,
}

impl FileValidation {
    pub fn ok(&self) -> bool {
        self.errors.is_empty()
    }
}

/// Aggregate result of validating a list of MOBI files.
pub struct ValidationReport {
    pub files: Vec<FileValidation>,
    pub passed: usize,
    pub failed: usize,
}

impl ValidationReport {
    pub fn ok(&self) -> bool {
        self.failed == 0
    }
}

/// Validate a list of MOBI files (after deduping to newest per family).
///
/// Returns a per-file report. The PalmDB-name uniqueness check fires across
/// distinct families only.
pub fn validate_mobi_files(paths: &[PathBuf]) -> ValidationReport {
    let deduped = dedupe_to_newest_per_family(paths);
    let mut palmdb_names: HashMap<String, (String, PathBuf)> = HashMap::new(); // name -> (family, path)
    let mut files = Vec::with_capacity(deduped.len());
    let mut passed = 0usize;
    let mut failed = 0usize;

    // Sort for deterministic output, matching the Python pass.
    let mut sorted = deduped;
    sorted.sort();

    for path in sorted {
        let mut report = validate_one(&path);

        // Cross-file uniqueness check.
        if !report.palmdb_name.is_empty() {
            if let Some((existing_family, existing_path)) = palmdb_names.get(&report.palmdb_name) {
                if existing_family != &report.family {
                    let other = existing_path
                        .file_name()
                        .map(|n| n.to_string_lossy().into_owned())
                        .unwrap_or_default();
                    report.errors.push(format!(
                        "PalmDB name '{}' conflicts with '{}' - Kindle FSCK will rename both files, hiding them from the dictionary list",
                        report.palmdb_name, other
                    ));
                }
            } else {
                palmdb_names.insert(
                    report.palmdb_name.clone(),
                    (report.family.clone(), path.clone()),
                );
            }
        }

        if report.ok() {
            passed += 1;
        } else {
            failed += 1;
        }
        files.push(report);
    }

    ValidationReport {
        files,
        passed,
        failed,
    }
}

/// Validate a single MOBI file's binary structure.
///
/// Does NOT perform the cross-file PalmDB name uniqueness check (that requires
/// the full file list). Use [`validate_mobi_files`] for the full pipeline.
pub fn validate_one(path: &Path) -> FileValidation {
    let mut errors: Vec<String> = Vec::new();
    let family = family_key(path);
    let mut palmdb_name = String::new();

    let data = match fs::read(path) {
        Ok(d) => d,
        Err(e) => {
            errors.push(format!("Cannot read file: {e}"));
            return FileValidation {
                path: path.to_path_buf(),
                palmdb_name,
                family,
                errors,
            };
        }
    };

    if data.len() < 78 {
        errors.push(format!(
            "File too small ({} bytes), minimum PalmDB header is 78 bytes",
            data.len()
        ));
        return FileValidation {
            path: path.to_path_buf(),
            palmdb_name,
            family,
            errors,
        };
    }

    // 1. PalmDB header checks.
    let name_slot = &data[0..32];
    let nul = name_slot.iter().position(|&b| b == 0).unwrap_or(name_slot.len());
    let name_bytes = &name_slot[..nul];
    // Latin-1 decoding: each byte is its U+00xx code point.
    palmdb_name = name_bytes.iter().map(|&b| b as char).collect();

    let db_type = &data[60..64];
    let db_creator = &data[64..68];
    let num_records = read_u16_be(&data, 76);

    if db_type != b"BOOK" {
        errors.push(format!(
            "PalmDB type is '{}', expected 'BOOK'",
            String::from_utf8_lossy(db_type)
        ));
    }
    if db_creator != b"MOBI" {
        errors.push(format!(
            "PalmDB creator is '{}', expected 'MOBI'",
            String::from_utf8_lossy(db_creator)
        ));
    }
    if num_records == 0 {
        errors.push("PalmDB has 0 records".to_string());
    }
    if name_bytes.len() > 31 {
        errors.push(format!(
            "PalmDB name too long ({} bytes, max 31)",
            name_bytes.len()
        ));
    }

    // 2. Record 0 / MOBI header checks.
    if num_records > 0 {
        let rec0_offset = read_u32_be(&data, 78) as usize;
        if rec0_offset + 280 > data.len() {
            errors.push("Record 0 offset out of bounds".to_string());
        } else {
            let rec0 = &data[rec0_offset..];

            // PalmDOC compression
            let compression = read_u16_be(rec0, 0);
            if compression != 1 && compression != 2 && compression != 17480 {
                errors.push(format!("Invalid compression type: {compression}"));
            }

            // MOBI magic at offset 16
            let mobi_magic = &rec0[16..20];
            if mobi_magic != b"MOBI" {
                errors.push(format!(
                    "No MOBI header (expected 'MOBI', got {})",
                    debug_bytes(mobi_magic)
                ));
            } else {
                let header_length = read_u32_be(rec0, 20) as usize;
                let mobi_type = read_u32_be(rec0, 24);
                let text_encoding = read_u32_be(rec0, 28);

                if mobi_type != 2 {
                    errors.push(format!("MOBI type is {mobi_type}, expected 2 (MOBI Book)"));
                }
                if text_encoding != 65001 {
                    errors.push(format!(
                        "Text encoding is {text_encoding}, expected 65001 (UTF-8)"
                    ));
                }

                // 3. EXTH checks
                let exth_flag = read_u32_be(rec0, 128);
                let has_exth = (exth_flag & 0x40) != 0;

                if !has_exth {
                    errors.push("EXTH flag not set - dictionary metadata missing".to_string());
                } else {
                    let exth_offset = 16 + header_length;
                    if exth_offset + 12 > rec0.len() {
                        errors.push(format!(
                            "EXTH header missing at expected offset {exth_offset}"
                        ));
                    } else {
                        let exth_magic = &rec0[exth_offset..exth_offset + 4];
                        if exth_magic != b"EXTH" {
                            errors.push(format!(
                                "EXTH header missing at expected offset {exth_offset}"
                            ));
                        } else {
                            let exth_count = read_u32_be(rec0, exth_offset + 8) as usize;
                            let mut exth_records: HashMap<u32, Vec<u8>> = HashMap::new();
                            let mut pos = exth_offset + 12;
                            for _ in 0..exth_count {
                                if pos + 8 > rec0.len() {
                                    break;
                                }
                                let rec_type = read_u32_be(rec0, pos);
                                let rec_len = read_u32_be(rec0, pos + 4) as usize;
                                if rec_len < 8 || pos + rec_len > rec0.len() {
                                    break;
                                }
                                exth_records
                                    .entry(rec_type)
                                    .or_insert_with(|| rec0[pos + 8..pos + rec_len].to_vec());
                                pos += rec_len;
                            }

                            // 531/532 select the dictionary by language. Matching,
                            // nonempty 113/504 values give the sideloaded dictionary a
                            // stable identity so Vocabulary Builder records its lookups.
                            if !exth_records.contains_key(&531) {
                                errors.push("EXTH 531 (DictionaryInLanguage) missing".to_string());
                            }
                            if !exth_records.contains_key(&532) {
                                errors.push("EXTH 532 (DictionaryOutLanguage) missing".to_string());
                            }
                            let id_113 = exth_records.get(&113).filter(|value| !value.is_empty());
                            let id_504 = exth_records.get(&504).filter(|value| !value.is_empty());
                            if id_113.is_none() {
                                errors.push(
                                    "EXTH 113 (dictionary content identifier) missing or empty"
                                        .to_string(),
                                );
                            }
                            if id_504.is_none() {
                                errors.push(
                                    "EXTH 504 (dictionary content key) missing or empty".to_string(),
                                );
                            }
                            if matches!((id_113, id_504), (Some(left), Some(right)) if left != right)
                            {
                                errors.push(
                                    "EXTH 113/504 dictionary content identifiers do not match"
                                        .to_string(),
                                );
                            }
                        }
                    }
                }

                // 4. INDX record checks.
                //
                // The MOBI header field at offset 0x50 ("First Non-book index"
                // per the MobileRead MOBI spec) is the first record that is
                // not part of the compressed text. It is NOT guaranteed to be
                // the first INDX record. Kindle dictionaries commonly place
                // cover / HD-image-container records between the text and the
                // INDX records, so the record at first_non_book is often a
                // JPEG. The Kindle finds the INDX section via EXTH and header
                // pointers, not by positional assumption.
                //
                // So we scan the range [first_non_book, num_records) for any
                // record whose payload begins with the 'INDX' magic. If none
                // is found, the dictionary truly has no index and is broken.
                let first_non_book = read_u32_be(rec0, 80) as usize;
                let num_records_usize = num_records as usize;
                let mut found_indx = false;
                for rec_idx in first_non_book..num_records_usize {
                    let rec_off_pos = 78 + rec_idx * 8;
                    if rec_off_pos + 4 > data.len() {
                        break;
                    }
                    let rec_off = read_u32_be(&data, rec_off_pos) as usize;
                    if rec_off + 4 > data.len() {
                        continue;
                    }
                    if &data[rec_off..rec_off + 4] == b"INDX" {
                        found_indx = true;
                        break;
                    }
                }
                if !found_indx {
                    errors.push(format!(
                        "No INDX record found after text section (scanned records {}..{})",
                        first_non_book,
                        num_records_usize.saturating_sub(1)
                    ));
                }
            }
        }
    }

    FileValidation {
        path: path.to_path_buf(),
        palmdb_name,
        family,
        errors,
    }
}

fn read_u16_be(data: &[u8], offset: usize) -> u16 {
    u16::from_be_bytes([data[offset], data[offset + 1]])
}

fn read_u32_be(data: &[u8], offset: usize) -> u32 {
    u32::from_be_bytes([
        data[offset],
        data[offset + 1],
        data[offset + 2],
        data[offset + 3],
    ])
}

fn debug_bytes(b: &[u8]) -> String {
    let mut s = String::from("b'");
    for &c in b {
        if c.is_ascii_graphic() || c == b' ' {
            s.push(c as char);
        } else {
            s.push_str(&format!("\\x{:02x}", c));
        }
    }
    s.push('\'');
    s
}

/// Print a per-file validation report. Returns true if all files passed.
pub fn print_report(report: &ValidationReport) -> bool {
    println!(
        "\nRunning MOBI validation on {} file(s)...\n",
        report.files.len()
    );
    for f in &report.files {
        let filename = f
            .path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        if f.ok() {
            // Python prints nothing on pass, only the final summary.
        } else {
            println!("  FAIL  {}", filename);
            for err in &f.errors {
                println!("        {}", err);
            }
        }
    }
    print!(
        "\nMOBI validation results: {}/{} passed",
        report.passed,
        report.passed + report.failed
    );
    if report.failed > 0 {
        print!(", {} FAILED", report.failed);
    }
    println!();
    report.ok()
}
