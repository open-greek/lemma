// Handles StarDict bundle generation via the kindling library.
//
// Produces a `<source_lang>_stardict/` directory next to the EPUB containing
// the four StarDict 2.4.2 files (`.ifo`, `.idx`, `.dict`, `.syn`) and a
// matching `.zip` of that directory under `dist/` for release attachment.
//
// StarDict bundles are consumed by GoldenDict, GoldenDict-ng, KOReader, sdcv,
// and other non-Kindle dictionary readers. They are the natural distribution
// format for any e-reader that runs KOReader (Kobo, reMarkable, Boox, etc.)
// and for desktop GoldenDict users.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

const LEMMA_REPO_URL: &str = "https://github.com/ciscoriordan/lemma";
const LEMMA_CONTACT_EMAIL: &str = "cisco.riordan@gmail.com";

pub struct StarDictGenerator<'a> {
    pub output_dir: &'a Path,
    pub source_lang: &'a str,
    pub opf_filename: &'a str,
}

impl<'a> StarDictGenerator<'a> {
    pub fn generate(&self) {
        println!("\nGenerating StarDict bundle...");

        let opf_path = self.output_dir.join(self.opf_filename);
        // The bundle directory's stem becomes the file stem inside it, so
        // <output_dir>/<stem>/<stem>.ifo, .idx, .dict, .syn.
        let stem = self.opf_filename.replace(".opf", "_stardict");
        let bundle_dir = self.output_dir.join(&stem);

        if bundle_dir.exists() {
            println!("Removing existing StarDict bundle: {}", stem);
            let _ = fs::remove_dir_all(&bundle_dir);
        }

        println!("Running kindling stardict on {}", self.opf_filename);

        let options = kindling::stardict::StarDictOptions {
            website: Some(LEMMA_REPO_URL.to_string()),
            email: Some(LEMMA_CONTACT_EMAIL.to_string()),
            description: Some(stardict_description(self.source_lang)),
            ..Default::default()
        };
        let result = kindling::stardict::build_stardict(&opf_path, &bundle_dir, &options);

        match result {
            Ok(report) => {
                println!(
                    "\nSuccess! Generated StarDict bundle: {}/ ({} headwords, {} inflection redirects)",
                    stem, report.wordcount, report.synwordcount
                );
                let total_bytes = total_bundle_bytes(&report);
                println!("Bundle size: {:.2} MB", total_bytes as f64 / 1024.0 / 1024.0);
                self.archive_to_dist(&bundle_dir, &stem);
                println!(
                    "Drop {0}/ into KOReader's koreader/data/dict/ or GoldenDict's dictionary path.",
                    stem
                );
            }
            Err(e) => {
                println!("\nError: kindling stardict failed: {}", e);
            }
        }
    }

    /// Pack the bundle directory into `dist/<stem>.zip` for release
    /// attachment. We use zip (already a dependency) over the StarDict-
    /// community-standard `.tar.bz2` because zip is universally supported,
    /// every target reader (GoldenDict-ng, KOReader) accepts an
    /// extracted-zip layout, and the size delta is negligible against
    /// `.dict` which is already mostly incompressible HTML.
    fn archive_to_dist(&self, bundle_dir: &Path, stem: &str) {
        let dist_dir = PathBuf::from("dist");
        if let Err(e) = fs::create_dir_all(&dist_dir) {
            println!("Warning: could not create dist/: {}", e);
            return;
        }
        let archive_path = dist_dir.join(format!("{}.zip", stem));
        if let Err(e) = write_bundle_zip(bundle_dir, &archive_path, stem) {
            println!("Warning: could not write {}: {}", archive_path.display(), e);
            return;
        }
        if let Ok(meta) = fs::metadata(&archive_path) {
            println!(
                "Wrote {} ({:.2} MB)",
                archive_path.display(),
                meta.len() as f64 / 1024.0 / 1024.0
            );
        } else {
            println!("Wrote {}", archive_path.display());
        }
    }
}

fn total_bundle_bytes(report: &kindling::stardict::StarDictReport) -> u64 {
    let mut total = 0u64;
    for path in [&report.ifo_path, &report.idx_path, &report.dict_path, &report.syn_path] {
        if let Ok(meta) = fs::metadata(path) {
            total += meta.len();
        }
    }
    total
}

/// Zip the four StarDict files into `archive_path` under a top-level
/// directory named `stem` so unzipping reproduces the original bundle
/// layout. Files that do not exist (e.g. `.syn` for inflection-free
/// dictionaries) are silently skipped.
fn write_bundle_zip(
    bundle_dir: &Path,
    archive_path: &Path,
    stem: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let file = fs::File::create(archive_path)?;
    let mut zip = zip::ZipWriter::new(file);
    let options: zip::write::SimpleFileOptions =
        zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);

    for ext in ["ifo", "idx", "dict", "syn"] {
        let src = bundle_dir.join(format!("{}.{}", stem, ext));
        if !src.exists() {
            continue;
        }
        let bytes = fs::read(&src)?;
        let entry_name = format!("{}/{}.{}", stem, stem, ext);
        zip.start_file(entry_name, options)?;
        zip.write_all(&bytes)?;
    }

    zip.finish()?;
    Ok(())
}

/// StarDict 2.4.2 has no `license` field, so license info is folded into
/// `description`. KOReader and GoldenDict show this in the dictionary's info
/// panel. `<br>` is the spec's line-break sequence.
fn stardict_description(source_lang: &str) -> String {
    let (gloss_lang, source_wiktionary) = match source_lang {
        "el" => ("Greek", "Greek Wiktionary (el.wiktionary.org)"),
        _ => ("English", "English Wiktionary (en.wiktionary.org)"),
    };
    format!(
        "Greek-{gloss} dictionary built from {src}, with full inflection \
         lookup via .syn redirects.<br>\
         License: CC-BY-SA 4.0 (inherited from Wiktionary).<br>\
         Generator: kindling (https://github.com/ciscoriordan/kindling, MIT).",
        gloss = gloss_lang,
        src = source_wiktionary,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn description_picks_english_wiktionary_for_en_source() {
        let d = stardict_description("en");
        assert!(d.contains("English Wiktionary"));
        assert!(d.contains("CC-BY-SA 4.0"));
        assert!(d.contains("github.com/ciscoriordan/kindling"));
    }

    #[test]
    fn description_picks_greek_wiktionary_for_el_source() {
        let d = stardict_description("el");
        assert!(d.contains("Greek Wiktionary"));
    }
}
