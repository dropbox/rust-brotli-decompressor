// Compiles the reference C brotli implementation (with BROTLI_EXPERIMENTAL,
// so serialized shared dictionaries are supported) into the fuzz binary for
// the differential fuzz targets. Only active with --features c-compat.
//
// The checkout location is taken from $BROTLI_C_ROOT, falling back to
// ../../google-brotli (a sibling of this repository).

fn main() {
    if std::env::var_os("CARGO_FEATURE_C_COMPAT").is_none() {
        return;
    }
    let root = std::env::var("BROTLI_C_ROOT").unwrap_or_else(|_| {
        let manifest = std::env::var("CARGO_MANIFEST_DIR").unwrap();
        format!("{}/../../google-brotli", manifest)
    });
    let root = std::path::PathBuf::from(root);
    if !root.join("c/include/brotli/decode.h").exists() {
        panic!(
            "google/brotli checkout not found at {:?}; set BROTLI_C_ROOT to a \
             checkout of https://github.com/google/brotli",
            root
        );
    }
    println!("cargo:rerun-if-env-changed=BROTLI_C_ROOT");
    let mut build = cc::Build::new();
    build
        .include(root.join("c/include"))
        .define("BROTLI_EXPERIMENTAL", "1")
        .opt_level(1)
        .warnings(false);
    for dir in ["c/common", "c/dec", "c/enc"].iter() {
        for entry in std::fs::read_dir(root.join(dir)).unwrap() {
            let path = entry.unwrap().path();
            if path.extension().map_or(false, |e| e == "c") {
                println!("cargo:rerun-if-changed={}", path.display());
                build.file(path);
            }
        }
    }
    build.compile("brotli_c_reference");
}
