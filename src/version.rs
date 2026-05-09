// The lemma generator version, sourced from Cargo.toml at compile time.
//
// Used in the stdout banner and embedded into the dictionary's copyright
// page and OPF metadata. lemma's naming convention is "no dates and no
// version numbers in filenames or build directories"; the StarDict bundle
// stem is the one carved-out exception, because GoldenDict-ng on Linux
// caches metadata by .ifo path and a stable stem causes stale info across
// upgrades (xiaoyifang/goldendict-ng#2829). See src/stardict.rs.

pub const LEMMA_VERSION: &str = env!("CARGO_PKG_VERSION");
