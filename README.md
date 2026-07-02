# Lemma: Modern Greek Dictionary for Kindle

<p align="center">
  <img width="700" alt="Lemma - Modern Greek to English Dictionary for Kindle" src="images/lemma_banner.png">
</p>

A free Modern Greek-English dictionary for Kindle e-readers. 31K headwords, 568K inflected form lookups, built from Wiktionary data using [Kindling](https://github.com/ciscoriordan/kindling) (a reverse-engineered, ~7,000x faster *kindlegen* replacement). The generator and all helper tools are written in Rust.

<p align="center">
  <img width="600" alt="Lemma dictionary screenshot" src="images/screenshot.jpg">
</p>

Lemma ships as a single unified edition with every feature the generator supports - definitions, inflections, monotonic and polytonic lookup, gender and declension info, etymology, usage examples, and clickable cross-references.

## Quick Install

### Installing on Your Kindle

1. **Connect your Kindle** to your computer via USB cable
2. **Open the Kindle drive** on your computer
3. **Navigate to the `documents/dictionaries` folder** on your Kindle
   - If the `dictionaries` folder doesn't exist, create it inside `documents`
4. **Download `lemma_greek_en.mobi`** from [GitHub Releases](https://github.com/open-greek/lemma/releases) and copy it to `documents/dictionaries`
   - Alternatively, you can build the `.mobi` locally by running with the `-m` flag (see below)
5. **Safely eject your Kindle** from your computer
6. **Restart your Kindle**:
   - Hold the power button for 40 seconds, or
   - Go to Settings > Device Options > Restart
7. The dictionary will be available after restart

### Setting as Default Greek Dictionary

On your Kindle, go to **Settings → Language & Dictionaries → Dictionaries → Greek** and pick **"Lemma Greek Dictionary"**. From then on, long-pressing any Greek word in any book will use Lemma for the lookup popup.

(On older Kindles the path may be slightly different, e.g. *Home → Menu → Settings → Device Options → Language and Dictionaries → Dictionaries → Greek*.)

## Pre-built Dictionary

Ready-to-use dictionary files are available on the [Releases page](https://github.com/open-greek/lemma/releases):

- `lemma_greek_en.mobi` - the full dictionary for sideloading to Kindle devices
- `lemma_greek_en.epub` - the source EPUB (most users want the MOBI)
- `lemma_greek_en_stardict_v<version>.zip` - StarDict bundle for [GoldenDict](https://github.com/xiaoyifang/goldendict-ng), [KOReader](https://koreader.rocks/), [sdcv](https://dushistov.github.io/sdcv/), and other non-Kindle dictionary readers (Kobo, reMarkable, Boox, desktop). The version suffix is intentional, see the install note below.

Filenames are stable across versions, so each new release replaces the previous file in `documents/dictionaries/` on your Kindle in place. Check the **Build Info** section on the dictionary's copyright page to see which build you have installed (lemma generator version, build date, and Wiktionary extraction date).

### Installing the StarDict bundle

For non-Kindle readers, download the latest `lemma_greek_en_stardict_v<version>.zip` from the Releases page and unzip it. The archive contains a single `lemma_greek_en_stardict_v<version>/` directory with four files (`.ifo`, `.idx`, `.dict`, `.syn`). Drop that whole directory into your reader's dictionary path:

- **KOReader** (Kobo, reMarkable, Boox, jailbroken Kindle): `koreader/data/dict/`
- **GoldenDict-ng** (desktop): add the unzipped directory under *Edit, Dictionaries, Sources, Files*
- **sdcv** (terminal): `~/.stardict/dic/`

If you previously installed an older version, remove the old `lemma_greek_en_stardict_v*` directory first. The version suffix is part of the directory name because GoldenDict-ng on Linux otherwise keeps a stale copy of the metadata in its index cache (see [xiaoyifang/goldendict-ng#2829](https://github.com/xiaoyifang/goldendict-ng/issues/2829)); a fresh stem on each release sidesteps that bug.

Inflection lookup, headword search, and cross-entry links (rewritten to StarDict's `bword://` scheme) all work out of the box.

## Features
- **Inflection Support**: Automatically links inflected forms to their lemmas, with 2.74M form-to-lemma mappings from [Dilemma](https://github.com/open-greek/dilemma) when available
- **Lemma Equivalences**: Bridges cases where Wiktionary and Dilemma use different canonical forms for the same word (e.g., `τρώω`/`τρώγω`, `λέω`/`λέγω`), recovering ~742K additional inflections via 6,281 auto-generated equivalence pairs
- **Pre-Ranked Inflections**: When [Dilemma](https://github.com/open-greek/dilemma)'s `mg_ranked_forms.json` is available (from [HuggingFace Hub](https://huggingface.co/datasets/open-greek/dilemma-data) or locally), inflections arrive pre-ranked by corpus frequency and case-deduplicated. Case variants (φας/Φας) are added after the inflection cap, not before, so each slot goes to a unique form. Falls back to local ranking via [FrequencyWords](https://github.com/hermitdave/FrequencyWords) (OpenSubtitles 2018) if ranked forms aren't available
- **Polytonic Support**: Corpus-attested polytonic forms from Greek Wikisource, enabling lookups in pre-1982 polytonic texts
- **Gender and Variants**: POS line shows gender and key forms (e.g., "noun, feminine (plural θάλασσες)")
- **Etymology**: Word origins with transliterations stripped for clean display
- **Cross-References**: Clickable links between related entries (rewritten to `bword://` for StarDict readers, so cross-entry links resolve in GoldenDict, KOReader, and sdcv as well as on Kindle)
- **Clean Formatting**: Optimized for Kindle's dictionary popup interface
- **StarDict Output**: Optional four-file StarDict 2.4.2 bundle (`.ifo`/`.idx`/`.dict`/`.syn`) for use with GoldenDict, GoldenDict-ng, KOReader, sdcv, and other non-Kindle dictionary readers; built via `--stardict`
- **Testing Mode**: Create smaller dictionaries for testing (1-100% of entries)

## Building from Source

### Prerequisites

- Rust 1.80+ (edition 2024)
- [Kindling](https://github.com/ciscoriordan/kindling) (the `kindling-mobi` crate) is pulled automatically from crates.io as a normal dependency; no local checkout is needed. lemma uses Kindling as a library for MOBI generation, and the MOBI step is always invoked via the library (there is no shell-out to `kindling-cli`).
- Works on macOS, Linux, and Windows

### Installation

```bash
git clone https://github.com/open-greek/lemma.git
cd lemma

# Release build (kindling-mobi is fetched from crates.io automatically)
cargo build --release

# Run the generator (produces EPUB by default)
./target/release/lemma [options]
```

To build against a local Kindling checkout instead of the published crate (for testing unreleased Kindling changes), run `./scripts/setup-local-kindling.sh` first. It writes a gitignored `.cargo/config.toml` that patches the `kindling-mobi` crates.io dependency to your local path; delete that file to go back to the published crate.

### Options

```bash
# Generate dictionary (EPUB output)
cargo run --release

# Also generate .mobi for sideloading
cargo run --release -- -m

# Also generate a StarDict bundle for GoldenDict / KOReader / sdcv
cargo run --release -- --stardict

# Emit the .epub as a valid EPUB3 dictionary (the format KDP accepts)
cargo run --release -- --epub3

# Generate every output (EPUB + MOBI + StarDict)
cargo run --release -- -m --stardict

# Generate a test dictionary with only 10% of entries
cargo run --release -- -l 10
```

### Command Line Arguments

- `-l, --limit PERCENT`: Limit to first X% of words (useful for testing)
- `-m, --mobi`: Also generate `.mobi` via Kindling (for sideloading)
- `--stardict`: Also generate a StarDict bundle (`<output>/lemma_greek_en_stardict_v<version>/` plus a matching `dist/lemma_greek_en_stardict_v<version>.zip`) for GoldenDict, GoldenDict-ng, KOReader, sdcv, and other non-Kindle readers. The version suffix forces GoldenDict-ng's path-keyed metadata cache to invalidate on each release. Full builds (without `--limit`) refuse to write a bundle below 1000 headwords as a safeguard against shipping a partial test build by accident.
- `--epub3`: Emit `lemma_greek_en.epub` as a valid [EPUB 3.3](https://www.w3.org/TR/epub-33/) dictionary, using the [EPUB Dictionaries and Glossaries](https://www.w3.org/TR/epub-dictionaries/) profile (XHTML content documents plus a Search Key Map, `dc:type=dictionary`, source/target-language metadata; validated clean by `epubcheck` under its DICT profile), instead of the legacy idx-HTML EPUB 2.0.1 markup. This is the format KDP's current converter accepts. The EPUB3 is built by the Kindling library (`kindling::epub_build::build_epub3`), which reads lemma's idx OPF directly and auto-detects the el->en dictionary; lemma no longer renders the EPUB3 markup itself. The MOBI/idx path is unaffected: the `.html` content files and OPF the MOBI build reads are untouched.
- `-i, --inflections N`: Max inflections per headword (default: 255)
- `--front-matter PATH`: Override the copyright/usage front-matter fields (edition name, tagline, features, copyright holder, extra copyright lines, data sources) from a JSON file. Unspecified fields fall through to the built-in defaults.
- `-h, --help`: Show help message

Cross-references, etymology, usage examples, and polytonic lookups are always enabled — lemma ships a single unified edition with the full feature set.

## Data Sources

The dictionaries are built from:

- **Primary Source**: [Kaikki.org](https://kaikki.org/) - Machine-readable Wiktionary data (definitions, POS, etymology)
- **Inflection Data** (optional): [Dilemma](https://github.com/open-greek/dilemma) - Greek lemmatizer with 2.74M Modern Greek form-to-lemma mappings compiled from English and Greek Wiktionary, treebank corpora, and LSJ expansion
- **Ranked Inflections** (optional): Dilemma's `mg_ranked_forms.json` from the [`open-greek/dilemma-data`](https://huggingface.co/datasets/open-greek/dilemma-data) HuggingFace dataset provides pre-ranked, case-deduplicated inflection lists per lemma. Downloaded automatically if `huggingface_hub` is installed.
- **Frequency Data** (fallback): [FrequencyWords](https://github.com/hermitdave/FrequencyWords) - Word frequency lists derived from OpenSubtitles 2018 corpus, used to rank inflections when pre-ranked forms are not available
- **Fallback Data**: Pre-downloaded JSONL files in the repository

### Optional Configuration

To use local kaikki dumps or Dilemma inflection data, create a `.env` file in the project root:

```
KAIKKI_LOCAL_DIR=/path/to/kaikki/dumps
DILEMMA_DATA_DIR=/path/to/dilemma/data
```

When `DILEMMA_DATA_DIR` is set and `mg_lookup_scored.json` (or `mg_lookup.json`) is found, the generator will supplement kaikki-derived inflections with Dilemma's more comprehensive mappings. Without it, inflections are extracted from kaikki data only.

#### Kaikki Extraction Date

The downloader records the real Kaikki extraction date (the date Wiktionary was snapshotted, not the date you ran the build) and renders it on the dictionary's copyright page. The cascade is:

1. HTTP `Last-Modified` header on the Kaikki JSONL URL (direct download).
2. The "extracted on YYYY-MM-DD" line on Kaikki's language index page (direct download, fallback).
3. The file mtime of the local dump (when `KAIKKI_LOCAL_DIR` is used).
4. The hardcoded constant for any committed fallback snapshot.

On a successful download the downloader writes a tiny `greek_data_<lang>.jsonl.meta` sidecar next to the dump containing `{"extraction_date": ..., "source_url": ..., "downloaded_at": ...}`. Subsequent builds that reuse the cached dump read the sidecar so the extraction date survives across runs. Sidecars are gitignored.

The generator also automatically looks for `mg_ranked_forms.json` (pre-ranked inflections) in three locations: `data/` in this project, the `DILEMMA_DATA_DIR`, or the [`open-greek/dilemma-data`](https://huggingface.co/datasets/open-greek/dilemma-data) HuggingFace dataset (requires `pip install huggingface_hub`).

#### Lemma Equivalences

Wiktionary and Dilemma sometimes disagree on the canonical lemma for a word (e.g., Wiktionary uses `τρώω` for "eat" while Dilemma files all 165 inflections under `τρώγω`). To bridge this, run:

```bash
cargo run --release --bin generate_mg_equivalences
```

This cross-references the two data sources, uses corpus frequency as a tiebreaker, and writes `data/mg_lemma_equivalences.json`. The dictionary generator loads this automatically. Without it, inflections filed under a different canonical form in Dilemma will be missed.

### Related Projects

- [Kindling](https://github.com/ciscoriordan/kindling) - MOBI generator for Kindle dictionaries, books, and comics.
- [Dilemma](https://github.com/open-greek/dilemma) - Greek lemmatizer (provides the inflection lookup tables used by Lemma) plus a POS tagger and dependency parser via `dilemma[tagger]`.

## Dictionary Content

The dictionary includes:

- **Headwords**: Main dictionary entries
- **Inflected Forms**: Automatically redirect to their lemmas
- **Part of Speech**: Grammatical category, gender, and key forms
- **Definitions**: Multiple numbered definitions where applicable
- **Etymology**: Word origins and history
- **Usage Examples**: Attested example sentences with translations
- **Cross-References**: Clickable links between related entries
- **Domain Tags**: Subject area indicators (e.g., γλωσσολογία, γραμματική)

### Inflection Limit

Each headword includes up to 255 unique inflected forms (`MAX_INFLECTIONS` in `src/html_gen.rs`), ranked by corpus frequency when pre-ranked forms from Dilemma are available. Use `-i N` to adjust.

Each headword also carries up to 255 polytonic variants (`MAX_POLYTONIC`), sourced from attested forms in Greek Wikisource via Dilemma's `mg_polytonic_ranked.json`. This enables lookups in polytonic Modern Greek texts (pre-1982 orthography, Katharevousa literature, etc.).

### Excluded Content

The following are filtered out as they cannot be selected in Kindle texts:

- Prefixes and suffixes (e.g., `-ικός`, `προ-`)
- Combining forms and clitics
- Individual letters and symbols
- Abbreviations and contractions

## Troubleshooting

### Dictionary Not Appearing

- Ensure the `.mobi` file(s) are in the `documents/dictionaries` folder
- **Always restart your Kindle** after adding new dictionaries
- If still not appearing, try a hard restart (hold power button for 40 seconds)

### Lookup Not Working

- Make sure you've set the dictionary as default for Greek
- Some older Kindle models may have limited Greek support

### Building Issues

- **Kindling not found**: Only needed for `.mobi` generation (`-m` flag). Download from [Kindling releases](https://github.com/ciscoriordan/kindling/releases)
- **Download freezes**: Use pre-downloaded data files from the repository
- **Memory issues**: Use the `-l` option to build smaller test dictionaries first

## Dictionary Layout

Dictionary content is split across `content_NN.html` files: `content_00.html` holds non-Greek headwords, and the Greek letters Α…Ω follow in order. Each file is a standalone XHTML document with the Kindle dictionary `<idx:*>` markup. Any letter with more than ~2,500 entries is spread across several consecutive files so no single file is oversized; Α alone (the privative α-/αν- plus the ανα-/απο-/αντι- prefixes give it ~4.5x the headwords of any other letter) becomes about five files, so file numbering is sequential rather than one number per letter. Cross-reference links are file-qualified (e.g. `content_15.html#hw_λέγω`), so a headword in one file links into whichever file holds its target. The NCX exposes a one-entry-per-letter jump-to-letter TOC.

Splitting is required by [Kindle Publishing Guidelines §15.5](https://kindlegen.s3.amazonaws.com/AmazonKindlePublishingGuidelines.pdf) - Amazon's server-side dictionary converter can time out on a single very large XHTML file (before this, Α was one ~19 MB file). The per-file cap lives in `src/html_gen.rs` as `MAX_ENTRIES_PER_FILE`; lower it if the converter still struggles. Kindling reads every spine entry and produces a single MOBI, so the MOBI output is still one file per edition.

## Git Hooks

A pre-commit hook runs [Kindling's](https://github.com/ciscoriordan/kindling) `kindling validate` against any dictionary OPF whose directory has staged changes, so broken manuscripts can't be committed. It checks against the Amazon Kindle Publishing Guidelines (KPG 2026.1).

Install it once per clone:

```bash
./scripts/install-hooks.sh
```

This symlinks `.git/hooks/pre-commit` to the tracked `scripts/pre-commit`, so future updates to the hook are picked up automatically.

Behavior:

- If the commit doesn't touch any `lemma_greek_en_*/` directory, the hook exits immediately.
- For each changed dict directory, it finds the `.opf` and runs `kindling validate` on it. A non-zero exit aborts the commit with the failing findings.
- If `kindling-cli` isn't on `PATH` (and isn't at `~/Documents/kindling/target/release/kindling-cli`), the hook prints a warning and lets the commit through.
- Bypass with `git commit --no-verify` when you need to commit in spite of validation output.

## License

MIT - © 2025-2026 Open Greek

- **Dictionary content and data**: [Creative Commons Attribution-ShareAlike 4.0](https://creativecommons.org/licenses/by-sa/4.0/) (derived from Wiktionary)
- **Frequency data** (`data/el_full.txt`): [MIT License](https://github.com/hermitdave/FrequencyWords/blob/master/LICENSE) (from FrequencyWords/OpenSubtitles)

## Acknowledgments

- Wiktionary contributors for the source data
- [Kaikki.org](https://kaikki.org/) for providing machine-readable Wiktionary dumps
- [Dilemma](https://github.com/open-greek/dilemma) for Greek lemmatization and inflection data
- [Kindling](https://github.com/ciscoriordan/kindling) for MOBI generation
- [FrequencyWords](https://github.com/hermitdave/FrequencyWords) for corpus frequency data (MIT license)
