// Regression tests for lemma::mobi_validator.
//
// These tests cover two false-positive bugs that were previously blocking
// deploy:
//
// 1. PalmDB name uniqueness check fired across different versions of the same
//    dictionary family (e.g. yesterday's and today's build both sitting in
//    `dist/`). Fix: dedupe and key uniqueness by filename family, where a
//    family is the basename with its `_YYYYMMDD` date stamp stripped.
//
// 2. INDX record check assumed the record at MOBI header offset 0x50
//    ("first non-book record") must start with 'INDX'. In practice Kindle
//    dictionaries place cover / HD-image-container records between the text
//    and the INDX records, so that record is often a JPEG. Fix: scan the
//    post-text record range for any 'INDX' record.
//
// The tests build minimal fake MOBI byte buffers and pass them through the
// real validator. They do NOT invoke kindling.

use std::fs;
use std::path::PathBuf;

use lemma::mobi_validator::{dedupe_to_newest_per_family, family_key, validate_mobi_files};

// A minimal record 0 is 280 bytes (the validator requires
// `rec0_offset + 280 <= len(data)`). We size it to fit a small EXTH section
// after the MOBI header, with the four EXTH records required by the validator.
const REC0_SIZE: usize = 512;
const MOBI_HEADER_LENGTH: u32 = 232; // header_length written at rec0[20:24]
const EXTH_OFFSET_IN_REC0: usize = 16 + MOBI_HEADER_LENGTH as usize; // = 248
const CONTENT_ID: &[u8] = b"bc7a0762-d59c-46e0-95d6-51e3cde6e418";

// JPEG SOI + APP0 marker - the exact bytes the old check misidentified.
const JPEG_MAGIC: [u8; 4] = [0xff, 0xd8, 0xff, 0xe0];
const INDX_MAGIC: [u8; 4] = [b'I', b'N', b'D', b'X'];

fn write_u16_be(buf: &mut [u8], offset: usize, value: u16) {
    let bytes = value.to_be_bytes();
    buf[offset..offset + 2].copy_from_slice(&bytes);
}

fn write_u32_be(buf: &mut [u8], offset: usize, value: u32) {
    let bytes = value.to_be_bytes();
    buf[offset..offset + 4].copy_from_slice(&bytes);
}

fn build_record0_with_identity(
    first_non_book: u32,
    id_113: Option<&[u8]>,
    id_504: Option<&[u8]>,
) -> Vec<u8> {
    let mut rec0 = vec![0u8; REC0_SIZE];

    // PalmDOC header: compression=1 (no compression). Rest zero.
    write_u16_be(&mut rec0, 0, 1);

    // MOBI header magic at offset 16
    rec0[16..20].copy_from_slice(b"MOBI");
    // header_length
    write_u32_be(&mut rec0, 20, MOBI_HEADER_LENGTH);
    // mobi_type = 2 (MOBI Book)
    write_u32_be(&mut rec0, 24, 2);
    // text_encoding = 65001 (UTF-8)
    write_u32_be(&mut rec0, 28, 65001);
    // first_non_book at offset 80
    write_u32_be(&mut rec0, 80, first_non_book);
    // EXTH flag at offset 128
    write_u32_be(&mut rec0, 128, 0x40);

    // EXTH header at 16 + header_length
    let off = EXTH_OFFSET_IN_REC0;
    rec0[off..off + 4].copy_from_slice(b"EXTH");
    let mut records: Vec<(u32, &[u8])> = vec![(531, b"grc"), (532, b"en")];
    if let Some(value) = id_113 {
        records.push((113, value));
    }
    if let Some(value) = id_504 {
        records.push((504, value));
    }
    write_u32_be(&mut rec0, off + 8, records.len() as u32);

    let mut entry_pos = off + 12;
    for (record_type, value) in records {
        let record_len = 8 + value.len();
        write_u32_be(&mut rec0, entry_pos, record_type);
        write_u32_be(&mut rec0, entry_pos + 4, record_len as u32);
        rec0[entry_pos + 8..entry_pos + record_len].copy_from_slice(value);
        entry_pos += record_len;
    }
    write_u32_be(&mut rec0, off + 4, (entry_pos - off) as u32);

    rec0
}

/// Build a minimal MOBI file as bytes.
///
/// Records `0..first_non_book` are "text" records (all-zero payload, no
/// content that the validator looks at). Records from `first_non_book` onward
/// take their leading bytes from `post_text_record_magics`, in order.
fn build_mobi(
    palmdb_name: &str,
    num_total_records: u32,
    first_non_book: u32,
    post_text_record_magics: &[[u8; 4]],
) -> Vec<u8> {
    build_mobi_with_identity(
        palmdb_name,
        num_total_records,
        first_non_book,
        post_text_record_magics,
        Some(CONTENT_ID),
        Some(CONTENT_ID),
    )
}

fn build_mobi_with_identity(
    palmdb_name: &str,
    num_total_records: u32,
    first_non_book: u32,
    post_text_record_magics: &[[u8; 4]],
    id_113: Option<&[u8]>,
    id_504: Option<&[u8]>,
) -> Vec<u8> {
    assert_eq!(
        post_text_record_magics.len(),
        (num_total_records - first_non_book) as usize
    );

    // PalmDB header is 78 bytes, then an 8-byte record-info entry per record.
    let header_len: usize = 78 + 8 * num_total_records as usize;

    let mut record_data: Vec<Vec<u8>> = Vec::with_capacity(num_total_records as usize);

    // Record 0: MOBI header + PalmDOC header (REC0_SIZE bytes).
    record_data.push(build_record0_with_identity(first_non_book, id_113, id_504));

    // Text records 1..first_non_book-1: 8-byte zero payloads.
    for _ in 1..first_non_book {
        record_data.push(vec![0u8; 8]);
    }

    // Post-text records: 8-byte payload each, leading with the requested magic.
    for magic in post_text_record_magics {
        let mut payload = vec![0u8; 8];
        payload[..4].copy_from_slice(magic);
        record_data.push(payload);
    }

    // Compute record offsets.
    let mut record_offsets: Vec<u32> = Vec::with_capacity(record_data.len());
    let mut off = header_len as u32;
    for rec in &record_data {
        record_offsets.push(off);
        off += rec.len() as u32;
    }

    // Assemble the PalmDB header.
    let mut buf = vec![0u8; header_len];

    let name_bytes = palmdb_name.as_bytes();
    let name_len = name_bytes.len().min(31);
    buf[..name_len].copy_from_slice(&name_bytes[..name_len]);
    // [32:60] attributes/version/etc - zero is fine.
    buf[60..64].copy_from_slice(b"BOOK");
    buf[64..68].copy_from_slice(b"MOBI");
    // [68:76] zero
    write_u16_be(&mut buf, 76, num_total_records as u16);

    // Record info list at offset 78. Each entry: 4 bytes offset, 4 bytes
    // attributes + unique id (we zero them).
    for (i, rec_off) in record_offsets.iter().enumerate() {
        write_u32_be(&mut buf, 78 + 8 * i, *rec_off);
    }

    let mut out = buf;
    for rec in record_data {
        out.extend_from_slice(&rec);
    }
    out
}

fn write_mobi(
    tmpdir: &std::path::Path,
    filename: &str,
    palmdb_name: &str,
    num_total_records: u32,
    first_non_book: u32,
    post_text_record_magics: &[[u8; 4]],
) -> PathBuf {
    let path = tmpdir.join(filename);
    let bytes = build_mobi(
        palmdb_name,
        num_total_records,
        first_non_book,
        post_text_record_magics,
    );
    fs::write(&path, &bytes).unwrap();
    path
}

fn write_mobi_with_identity(
    tmpdir: &std::path::Path,
    filename: &str,
    id_113: Option<&[u8]>,
    id_504: Option<&[u8]>,
) -> PathBuf {
    let path = tmpdir.join(filename);
    let bytes = build_mobi_with_identity(
        "Lemma_Greek_Dictionary",
        3,
        1,
        &[JPEG_MAGIC, INDX_MAGIC],
        id_113,
        id_504,
    );
    fs::write(&path, bytes).unwrap();
    path
}

/// Format a `ValidationReport`'s failure messages into a single string for
/// `assert!` diagnostic output.
fn format_failures(report: &lemma::mobi_validator::ValidationReport) -> String {
    let mut s = String::new();
    for f in &report.files {
        if !f.errors.is_empty() {
            s.push_str(&format!("{}: {:?}\n", f.path.display(), f.errors));
        }
    }
    s
}

// ---------------- family_key tests ----------------

#[test]
fn date_stripped_at_end() {
    assert_eq!(
        family_key("/tmp/lemma_greek_en_20260410.mobi"),
        "lemma_greek_en.mobi"
    );
}

#[test]
fn date_stripped_before_suffix() {
    assert_eq!(
        family_key("/tmp/lemma_greek_en_20260410_basic.mobi"),
        "lemma_greek_en_basic.mobi"
    );
}

#[test]
fn no_date_is_unchanged() {
    assert_eq!(family_key("/tmp/lemma_latin_en.mobi"), "lemma_latin_en.mobi");
}

#[test]
fn two_dates_same_family() {
    assert_eq!(
        family_key("/tmp/lemma_greek_en_20260409.mobi"),
        family_key("/tmp/lemma_greek_en_20260410.mobi"),
    );
}

// ---------------- PalmDB uniqueness tests ----------------

#[test]
fn same_family_shared_name_is_ok() {
    // Two date-stamped versions of the same dictionary must NOT error.
    let tmp = tempdir();

    let a = write_mobi(
        tmp.path(),
        "lemma_greek_en_20260409.mobi",
        "Lemma_Greek_Dictionary",
        3,
        1,
        &[JPEG_MAGIC, INDX_MAGIC],
    );
    let b = write_mobi(
        tmp.path(),
        "lemma_greek_en_20260410.mobi",
        "Lemma_Greek_Dictionary",
        3,
        1,
        &[JPEG_MAGIC, INDX_MAGIC],
    );

    // Make sure 20260410 has a strictly newer mtime so the dedupe keeps it as
    // the "newest" (timestamps can collide on fast FSs).
    set_mtime(&a, std::time::UNIX_EPOCH + std::time::Duration::from_secs(1000));
    set_mtime(&b, std::time::UNIX_EPOCH + std::time::Duration::from_secs(2000));

    let report = validate_mobi_files(&[a, b]);
    assert!(report.ok(), "expected PASS, got: {}", format_failures(&report));
    // Dedupe should have kept only the newest file.
    assert_eq!(report.files.len(), 1);
}

#[test]
fn different_families_shared_name_is_error() {
    // Genuine collision across dictionaries must still be reported.
    let tmp = tempdir();

    let a = write_mobi(
        tmp.path(),
        "lemma_greek_en_20260410.mobi",
        "Shared_Name",
        3,
        1,
        &[JPEG_MAGIC, INDX_MAGIC],
    );
    let b = write_mobi(
        tmp.path(),
        "lemma_latin_en_20260410.mobi",
        "Shared_Name",
        3,
        1,
        &[JPEG_MAGIC, INDX_MAGIC],
    );

    let report = validate_mobi_files(&[a, b]);
    assert!(!report.ok(), "expected FAIL, got pass");
    let any_conflict = report
        .files
        .iter()
        .any(|f| f.errors.iter().any(|e| e.contains("conflicts with")));
    assert!(any_conflict, "expected 'conflicts with' error in report");
}

// ---------------- INDX scan tests ----------------

#[test]
fn jpeg_then_indx_is_ok() {
    // first_non_book points at a JPEG but there is an INDX record later.
    let tmp = tempdir();
    let path = write_mobi(
        tmp.path(),
        "lemma_greek_en_20260410.mobi",
        "Lemma_Greek_Dictionary",
        4, // rec0 + 1 text + jpeg + indx
        2,
        &[JPEG_MAGIC, INDX_MAGIC],
    );

    let report = validate_mobi_files(&[path]);
    assert!(report.ok(), "expected PASS, got: {}", format_failures(&report));
}

#[test]
fn no_indx_anywhere_is_error() {
    // No INDX record in the post-text range must fail with the new error.
    let tmp = tempdir();
    let path = write_mobi(
        tmp.path(),
        "lemma_greek_en_20260410.mobi",
        "Lemma_Greek_Dictionary",
        4,
        2,
        &[JPEG_MAGIC, JPEG_MAGIC],
    );

    let report = validate_mobi_files(&[path]);
    assert!(!report.ok(), "expected FAIL, got pass");
    let any_no_indx = report.files.iter().any(|f| {
        f.errors
            .iter()
            .any(|e| e.contains("No INDX record found after text section"))
    });
    assert!(any_no_indx, "expected no-INDX error");
}

#[test]
fn indx_immediately_after_text_is_ok() {
    // The classic case: first non-book record is itself an INDX.
    let tmp = tempdir();
    let path = write_mobi(
        tmp.path(),
        "lemma_greek_en_20260410.mobi",
        "Lemma_Greek_Dictionary",
        3,
        1,
        &[INDX_MAGIC, [0, 0, 0, 0]],
    );

    let report = validate_mobi_files(&[path]);
    assert!(report.ok(), "expected PASS, got: {}", format_failures(&report));
}

// ---------------- Vocabulary Builder identity tests ----------------

#[test]
fn matching_exth_113_and_504_are_ok() {
    let tmp = tempdir();
    let path = write_mobi_with_identity(
        tmp.path(),
        "lemma_greek_en.mobi",
        Some(CONTENT_ID),
        Some(CONTENT_ID),
    );

    let report = validate_mobi_files(&[path]);
    assert!(report.ok(), "expected PASS, got: {}", format_failures(&report));
}

#[test]
fn kindling_dictionary_output_passes_vocabulary_builder_identity_validation() {
    let tmp = tempdir();
    let html = r#"<html xmlns:idx="http://www.mobipocket.com/idx">
<body><idx:entry><idx:orth value="λόγος">λόγος</idx:orth><b>λόγος</b> word</idx:entry></body>
</html>"#;
    let opf = r#"<?xml version="1.0" encoding="UTF-8"?>
<package version="2.0" xmlns="http://www.idpf.org/2007/opf" unique-identifier="BookId">
  <metadata xmlns:dc="http://purl.org/dc/elements/1.1/">
    <dc:title>Lemma Integration Dictionary</dc:title>
    <dc:creator>Open Greek</dc:creator>
    <dc:language>el</dc:language>
    <dc:identifier id="BookId">LemmaGreekENEL</dc:identifier>
    <x-metadata>
      <DictionaryInLanguage>el</DictionaryInLanguage>
      <DictionaryOutLanguage>en</DictionaryOutLanguage>
      <DefaultLookupIndex>default</DefaultLookupIndex>
    </x-metadata>
  </metadata>
  <manifest><item id="content" href="content.html" media-type="application/xhtml+xml"/></manifest>
  <spine><itemref idref="content"/></spine>
</package>"#;
    let opf_path = tmp.path().join("content.opf");
    let mobi_path = tmp.path().join("lemma_greek_en.mobi");
    fs::write(tmp.path().join("content.html"), html).unwrap();
    fs::write(&opf_path, opf).unwrap();

    kindling::mobi::build_mobi(
        &opf_path,
        &mobi_path,
        true,
        false,
        None,
        false,
        false,
        false,
        false,
        None,
        true,
        false,
        false,
        false,
        false,
        false,
    )
    .unwrap();

    let report = validate_mobi_files(&[mobi_path]);
    assert!(report.ok(), "expected PASS, got: {}", format_failures(&report));
}

#[test]
fn missing_exth_113_is_error() {
    let tmp = tempdir();
    let path = write_mobi_with_identity(
        tmp.path(),
        "lemma_greek_en.mobi",
        None,
        Some(CONTENT_ID),
    );

    let report = validate_mobi_files(&[path]);
    assert!(!report.ok(), "expected FAIL, got pass");
    assert!(report.files[0]
        .errors
        .iter()
        .any(|error| error.contains("EXTH 113")));
}

#[test]
fn missing_exth_504_is_error() {
    let tmp = tempdir();
    let path = write_mobi_with_identity(
        tmp.path(),
        "lemma_greek_en.mobi",
        Some(CONTENT_ID),
        None,
    );

    let report = validate_mobi_files(&[path]);
    assert!(!report.ok(), "expected FAIL, got pass");
    assert!(report.files[0]
        .errors
        .iter()
        .any(|error| error.contains("EXTH 504")));
}

#[test]
fn mismatched_exth_113_and_504_are_error() {
    let tmp = tempdir();
    let path = write_mobi_with_identity(
        tmp.path(),
        "lemma_greek_en.mobi",
        Some(CONTENT_ID),
        Some(b"3d9d7698-a961-46d0-ae55-cfdb688cf373"),
    );

    let report = validate_mobi_files(&[path]);
    assert!(!report.ok(), "expected FAIL, got pass");
    assert!(report.files[0]
        .errors
        .iter()
        .any(|error| error.contains("do not match")));
}

// ---------------- dedupe sanity check ----------------

#[test]
fn dedupe_keeps_newest_per_family() {
    let tmp = tempdir();
    let older = write_mobi(
        tmp.path(),
        "lemma_greek_en_20260409.mobi",
        "Lemma_Greek_Dictionary",
        3,
        1,
        &[JPEG_MAGIC, INDX_MAGIC],
    );
    let newer = write_mobi(
        tmp.path(),
        "lemma_greek_en_20260410.mobi",
        "Lemma_Greek_Dictionary",
        3,
        1,
        &[JPEG_MAGIC, INDX_MAGIC],
    );

    set_mtime(&older, std::time::UNIX_EPOCH + std::time::Duration::from_secs(1000));
    set_mtime(&newer, std::time::UNIX_EPOCH + std::time::Duration::from_secs(2000));

    let kept = dedupe_to_newest_per_family(&[older, newer.clone()]);
    assert_eq!(kept.len(), 1);
    assert_eq!(kept[0], newer);
}

// ---------------- helpers: minimal tempdir + mtime ----------------

/// Minimal scoped tempdir using std (no external crates needed).
struct TestDir {
    path: PathBuf,
}

impl TestDir {
    fn path(&self) -> &std::path::Path {
        &self.path
    }
}

impl Drop for TestDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

fn tempdir() -> TestDir {
    use std::sync::atomic::{AtomicUsize, Ordering};
    static COUNTER: AtomicUsize = AtomicUsize::new(0);
    let n = COUNTER.fetch_add(1, Ordering::SeqCst);
    let pid = std::process::id();
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let path = std::env::temp_dir().join(format!("lemma-mobi-test-{pid}-{nanos}-{n}"));
    fs::create_dir_all(&path).unwrap();
    TestDir { path }
}

fn set_mtime(path: &std::path::Path, when: std::time::SystemTime) {
    // Use libc utimes for portability across macOS/Linux without extra deps.
    use std::ffi::CString;
    use std::os::unix::ffi::OsStrExt;

    let cpath = CString::new(path.as_os_str().as_bytes()).unwrap();
    let secs = when
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);

    // libc::timeval { tv_sec, tv_usec } x 2 (atime, mtime)
    #[repr(C)]
    struct Timeval {
        tv_sec: i64,
        tv_usec: i64,
    }
    unsafe extern "C" {
        fn utimes(path: *const std::os::raw::c_char, times: *const Timeval) -> i32;
    }
    let times = [
        Timeval {
            tv_sec: secs,
            tv_usec: 0,
        },
        Timeval {
            tv_sec: secs,
            tv_usec: 0,
        },
    ];
    let ret = unsafe { utimes(cpath.as_ptr(), times.as_ptr()) };
    assert_eq!(ret, 0, "utimes failed for {}", path.display());
}
