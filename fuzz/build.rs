// Links the system brotli (>= 1.1.0, e.g. Debian/Ubuntu libbrotli-dev) into
// the fuzz binary for the differential fuzz targets. Only active with
// --features c-compat.
//
// Note the system library is built without BROTLI_EXPERIMENTAL, so it has no
// serialized shared dictionary support; the differential targets only
// exercise raw (custom) dictionaries. Serialized dictionaries are covered by
// the fixture corpus under testdata/dict_corpus instead.

fn main() {
    if std::env::var_os("CARGO_FEATURE_C_COMPAT").is_none() {
        return;
    }
    // pkg-config supplies the link-search paths when brotli is installed off
    // the default linker path; the -l flags below cover the common case.
    if let Ok(out) = std::process::Command::new("pkg-config")
        .args(["--libs-only-L", "libbrotlienc", "libbrotlidec"])
        .output()
    {
        if out.status.success() {
            for flag in String::from_utf8_lossy(&out.stdout).split_whitespace() {
                if let Some(path) = flag.strip_prefix("-L") {
                    println!("cargo:rustc-link-search=native={}", path);
                }
            }
        }
    }
    println!("cargo:rustc-link-lib=brotlienc");
    println!("cargo:rustc-link-lib=brotlidec");
    println!("cargo:rustc-link-lib=brotlicommon");
}
