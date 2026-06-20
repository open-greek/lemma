// Packages the output directory into a valid EPUB file.

use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipWriter};

pub struct EpubGenerator<'a> {
    pub output_dir: &'a Path,
    pub source_lang: &'a str,
    pub opf_filename: &'a str,
    /// False for `--limit` test builds, which share the same dist filename as
    /// the full build and would otherwise overwrite the release artifact.
    pub is_full_build: bool,
}

impl<'a> EpubGenerator<'a> {
    pub fn generate(&self) -> std::io::Result<PathBuf> {
        println!("\nGenerating EPUB file...");

        // Derive the EPUB filename from the OPF filename.
        let epub_name = self.opf_filename.replace(".opf", ".epub");
        let epub_path = self.output_dir.join(&epub_name);

        let file = File::create(&epub_path)?;
        let mut zw = ZipWriter::new(file);

        // mimetype first, stored uncompressed
        let stored = SimpleFileOptions::default().compression_method(CompressionMethod::Stored);
        zw.start_file("mimetype", stored)?;
        zw.write_all(b"application/epub+zip")?;

        let deflated = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);

        let container_xml = "<?xml version=\"1.0\"?>\n<container version=\"1.0\" xmlns=\"urn:oasis:names:tc:opendocument:xmlns:container\">\n  <rootfiles>\n    <rootfile full-path=\"OEBPS/content.opf\" media-type=\"application/oebps-package+xml\"/>\n  </rootfiles>\n</container>\n";
        zw.start_file("META-INF/container.xml", deflated)?;
        zw.write_all(container_xml.as_bytes())?;

        // OPF -> OEBPS/content.opf
        let opf_path = self.output_dir.join(self.opf_filename);
        let mut opf_bytes = Vec::new();
        File::open(&opf_path)?.read_to_end(&mut opf_bytes)?;
        zw.start_file("OEBPS/content.opf", deflated)?;
        zw.write_all(&opf_bytes)?;

        let fixed_files = ["toc.ncx", "cover.jpg", "usage.html", "copyright.html"];
        for filename in &fixed_files {
            let fp = self.output_dir.join(filename);
            if fp.exists() {
                let mut buf = Vec::new();
                File::open(&fp)?.read_to_end(&mut buf)?;
                zw.start_file(format!("OEBPS/{}", filename), deflated)?;
                zw.write_all(&buf)?;
            } else {
                println!("  Warning: expected file not found: {}", filename);
            }
        }

        // Dictionary content is split across per-letter content_NN.html files.
        // Collect whatever the generator emitted and add them in sorted order
        // so the EPUB zip is deterministic.
        let mut content_files: Vec<PathBuf> = fs::read_dir(self.output_dir)?
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| {
                p.file_name()
                    .and_then(|n| n.to_str())
                    .map(|n| n.starts_with("content_") && n.ends_with(".html"))
                    .unwrap_or(false)
            })
            .collect();
        content_files.sort();
        if content_files.is_empty() {
            println!("  Warning: no content_*.html files found");
        }
        for fp in &content_files {
            let filename = fp.file_name().and_then(|n| n.to_str()).unwrap_or("");
            let mut buf = Vec::new();
            File::open(fp)?.read_to_end(&mut buf)?;
            zw.start_file(format!("OEBPS/{}", filename), deflated)?;
            zw.write_all(&buf)?;
        }

        zw.finish()?;

        let size_mb = fs::metadata(&epub_path)?.len() as f64 / 1024.0 / 1024.0;
        println!("  Created {} ({:.2} MB)", epub_name, size_mb);

        self.copy_to_dist(&epub_path, &epub_name);

        Ok(epub_path)
    }

    /// Build a valid EPUB3 dictionary at the same `.epub` filename used by the
    /// idx path, delegating to kindling's `epub_build::build_epub3`. kindling
    /// reads lemma's idx OPF (`content_NN.html` + `<x-metadata>` el->en), and
    /// with `DictMode::Auto` auto-detects the dictionary and its source/target
    /// languages, emitting an epubcheck-clean (DICT profile) EPUB3. lemma no
    /// longer renders its own EPUB3 markup; the idx/MOBI output is untouched.
    pub fn generate_epub3(&self) -> std::io::Result<PathBuf> {
        println!("\nGenerating EPUB3 dictionary file...");

        let epub_name = self.opf_filename.replace(".opf", ".epub");
        let epub_path = self.output_dir.join(&epub_name);

        let opf_path = self.output_dir.join(self.opf_filename);
        let opf = kindling::opf::OPFData::parse(&opf_path)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))?;
        kindling::epub_build::build_epub3(
            &opf,
            &epub_path,
            &kindling::epub_build::EpubMeta::default(),
        )
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))?;

        let size_mb = fs::metadata(&epub_path)?.len() as f64 / 1024.0 / 1024.0;
        println!("  Created {} ({:.2} MB)", epub_name, size_mb);

        self.copy_to_dist(&epub_path, &epub_name);

        Ok(epub_path)
    }

    fn copy_to_dist(&self, epub_path: &Path, epub_name: &str) {
        if !self.is_full_build {
            println!("  Skipping dist/ copy (--limit test build)");
            return;
        }
        let dist_dir = PathBuf::from("dist");
        if fs::create_dir_all(&dist_dir).is_err() { return; }
        let dest = dist_dir.join(epub_name);
        if fs::copy(epub_path, &dest).is_ok() {
            println!("  Copied {} to dist/", epub_name);
        }
    }
}
