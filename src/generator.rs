// Main generator orchestrator.

use crate::dilemma::DilemmaInflections;
use crate::downloader::Downloader;
use crate::entry_processor::EntryProcessor;
use crate::epub::EpubGenerator;
use crate::front_matter::{self, FrontMatter};
use crate::html_gen::{BuildParams, HtmlGenerator};
use crate::mobi::MobiGenerator;
use crate::stardict::StarDictGenerator;
use crate::version::LEMMA_VERSION;
use chrono::Local;
use std::path::PathBuf;

pub struct GeneratorOptions {
    pub source_lang: String,
    pub limit_percent: Option<f64>,
    pub generate_mobi: bool,
    pub generate_stardict: bool,
    pub max_inflections: Option<usize>,
    pub front_matter_path: Option<PathBuf>,
}

pub fn run(opts: GeneratorOptions) -> Result<(), Box<dyn std::error::Error>> {
    if opts.source_lang != "en" && opts.source_lang != "el" {
        return Err("Source language must be 'en' or 'el'".into());
    }

    // Today's date is used for the human-readable "Dictionary created" line
    // on the copyright page, NOT for any filename.
    let build_date = Local::now().format("%Y%m%d").to_string();

    let front_matter: FrontMatter = front_matter::load_front_matter(opts.front_matter_path.as_deref())?;

    let source_desc = if opts.source_lang == "en" { "English" } else { "Greek" };
    println!("Lemma v{} - Greek Kindle Dictionary Generator", LEMMA_VERSION);
    println!("Initialized with:");
    println!("  Source: {} Wiktionary", source_desc);
    println!("  Build date: {}", build_date);
    if let Some(p) = opts.limit_percent {
        println!("  Word limit: {}% of entries", p);
    }
    if let Some(p) = &opts.front_matter_path {
        println!("  Front matter: {}", p.display());
    }
    let edition_preview = front_matter.get("edition_name")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .unwrap_or_else(|| "Lemma Greek Dictionary".to_string());
    println!("  Edition: {}", edition_preview);

    // Download
    let mut downloader = Downloader::new(&opts.source_lang);
    let (success, filename) = downloader.download();
    if !success {
        eprintln!("Error: Download failed");
        std::process::exit(1);
    }
    let filename = filename.expect("filename set on success");
    let extraction_date = downloader.extraction_date;

    // Load dilemma data
    let mut dilemma = DilemmaInflections::new();

    // Process entries
    let mut processor = EntryProcessor::new(
        &opts.source_lang,
        opts.limit_percent,
        &filename,
        Some(&mut dilemma),
    );
    processor.process();
    let final_extraction_date = extraction_date.or(processor.extraction_date.clone());
    let entries = std::mem::replace(&mut processor.entries, crate::entry_processor::EntryMap::new());
    drop(processor);

    // HTML generation. Lemma builds a single unified edition with all
    // features always enabled.
    let params = BuildParams {
        source_lang: opts.source_lang.clone(),
        build_date: build_date.clone(),
        extraction_date: final_extraction_date.clone(),
        limit_percent: opts.limit_percent,
        max_inflections: opts.max_inflections,
        front_matter,
    };

    let mut html_gen = HtmlGenerator::new(entries, params, Some(&dilemma));
    html_gen.create_output_files()?;
    let output_dir = html_gen.output_dir.clone();
    let opf_filename = html_gen.opf_filename.clone();
    drop(html_gen);

    // EPUB
    let epub = EpubGenerator {
        output_dir: &output_dir,
        source_lang: &opts.source_lang,
        opf_filename: &opf_filename,
    };
    epub.generate()?;

    // The MOBI build is the memory-heavy step (kindling holds the full
    // entry index in RAM); StarDict is much lighter, so the order here
    // does not matter much for peak memory. Free dilemma once we are
    // done with HTML generation regardless.
    if opts.generate_mobi || opts.generate_stardict {
        drop(dilemma); // free memory
    }

    if opts.generate_mobi {
        let mobi = MobiGenerator {
            output_dir: &output_dir,
            source_lang: &opts.source_lang,
            opf_filename: &opf_filename,
        };
        mobi.generate();
    }

    if opts.generate_stardict {
        let stardict = StarDictGenerator {
            output_dir: &output_dir,
            source_lang: &opts.source_lang,
            opf_filename: &opf_filename,
            is_full_build: opts.limit_percent.is_none(),
        };
        stardict.generate();
    }

    println!("\nDictionary generation complete!");
    println!("Files created in {}/", output_dir.display());
    if let Some(d) = final_extraction_date {
        println!("Wiktionary extraction date: {}", d);
    }

    Ok(())
}
