// Handles MOBI file generation via the kindling library.

use std::fs;
use std::path::{Path, PathBuf};

pub struct MobiGenerator<'a> {
    pub output_dir: &'a Path,
    pub source_lang: &'a str,
    pub opf_filename: &'a str,
    /// False for `--limit` test builds, which share the same dist filename as
    /// the full build and would otherwise overwrite the release artifact.
    pub is_full_build: bool,
}

impl<'a> MobiGenerator<'a> {
    pub fn generate(&self) {
        println!("\nGenerating MOBI file...");

        let opf_path = self.output_dir.join(self.opf_filename);
        let mobi_filename = self.opf_filename.replace(".opf", ".mobi");
        let mobi_path = self.output_dir.join(&mobi_filename);

        if mobi_path.exists() {
            println!("Removing existing MOBI file: {}", mobi_filename);
            let _ = fs::remove_file(&mobi_path);
        }

        println!("Running kindling on {}", self.opf_filename);
        println!("This may take several minutes for large dictionaries...");

        // Pre-flight validation (skipped here - let build_mobi handle errors)
        let result = kindling::mobi::build_mobi(
            &opf_path,
            &mobi_path,
            false, // no_compress
            false, // headwords_only
            None,  // srcs_data
            false, // include_cmet
            false, // no_hd_images
            false, // creator_tag
            false, // kf8_only
            None,  // doc_type
            true,  // kindle_limits (default ON for dictionaries)
            false, // self_check
            false, // kindlegen_parity (comic-path only; ignored for dict builds)
            false, // strict_accents (off = fold diacritics at lookup, like kindlegen)
        );

        match result {
            Ok(_) if mobi_path.exists() => {
                println!("\nSuccess! Generated {}", mobi_filename);
                let dict_type = "Greek-English";
                println!("Dictionary type: {}", dict_type);
                if let Ok(meta) = fs::metadata(&mobi_path) {
                    println!("File size: {:.2} MB", meta.len() as f64 / 1024.0 / 1024.0);
                }
                self.copy_to_dist(&mobi_path);
                println!("You can now transfer this file to your Kindle device.");
            }
            Ok(_) => {
                println!("\nWarning: kindling reported success but MOBI file not found.");
            }
            Err(e) => {
                println!("\nError: kindling failed: {}", e);
            }
        }
    }

    fn copy_to_dist(&self, mobi_path: &Path) {
        if !self.is_full_build {
            println!("Skipping dist/ copy (--limit test build)");
            return;
        }
        let dist_dir = PathBuf::from("dist");
        if fs::create_dir_all(&dist_dir).is_err() { return; }
        let Some(fname) = mobi_path.file_name() else { return; };
        let dest = dist_dir.join(fname);
        if fs::copy(mobi_path, &dest).is_ok() {
            println!("Copied {} to dist/", fname.to_string_lossy());
        }
    }
}
