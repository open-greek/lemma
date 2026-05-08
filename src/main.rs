// Lemma - Greek Kindle Dictionary Generator.
// CLI entry point.

use clap::Parser;
use lemma::generator::{self, GeneratorOptions};
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(about = "Lemma - Greek Dictionary Generator. Produces EPUB and optional MOBI for sideloading.")]
struct Cli {
    /// Source Wiktionary language: 'en' (English) or 'el' (Greek).
    #[arg(short = 's', long = "source", default_value = "en")]
    source: String,

    /// Limit to first PERCENT% of words (for testing).
    #[arg(short = 'l', long = "limit")]
    limit: Option<f64>,

    /// Max inflections per headword. Default: 255
    #[arg(short = 'i', long = "inflections")]
    inflections: Option<usize>,

    /// Also generate .mobi via kindling (for sideloading).
    #[arg(short = 'm', long = "mobi", default_value_t = false)]
    mobi: bool,

    /// Also generate a StarDict bundle (.ifo/.idx/.dict/.syn directory plus a
    /// matching .zip in dist/) for use with GoldenDict, GoldenDict-ng,
    /// KOReader, sdcv, and other non-Kindle readers. The short flag is
    /// `-s` for `--source` (collision-free), so use the long form here.
    #[arg(long = "stardict", default_value_t = false)]
    stardict: bool,

    /// Path to a JSON file that overrides front-matter fields.
    #[arg(long = "front-matter", value_name = "PATH")]
    front_matter: Option<PathBuf>,
}

fn main() {
    let cli = Cli::parse();

    if cli.source != "en" && cli.source != "el" {
        eprintln!("Error: source must be 'en' or 'el'");
        std::process::exit(1);
    }

    if let Some(l) = cli.limit {
        if l <= 0.0 || l > 100.0 {
            eprintln!("Error: Limit must be between 0 and 100");
            std::process::exit(1);
        }
    }

    let opts = GeneratorOptions {
        source_lang: cli.source,
        limit_percent: cli.limit,
        generate_mobi: cli.mobi,
        generate_stardict: cli.stardict,
        max_inflections: cli.inflections,
        front_matter_path: cli.front_matter,
    };

    if let Err(e) = generator::run(opts) {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }
}
