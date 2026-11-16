//! Carry `site/` inside the binary: one (name, bytes) per file, so a release is one artifact.
use std::path::Path;

fn walk(base: &Path, dir: &Path, out: &mut Vec<String>) {
    let mut es: Vec<_> = std::fs::read_dir(dir).map(|r| r.filter_map(Result::ok).collect()).unwrap_or_default();
    es.sort_by_key(|e| e.path());
    for e in es {
        let p = e.path();
        if p.is_dir() {
            walk(base, &p, out);
        } else {
            let rel = p.strip_prefix(base).unwrap().to_string_lossy().replace('\\', "/");
            out.push(format!("({rel:?}, include_bytes!({:?}) as &[u8]),", p.canonicalize().unwrap().to_string_lossy()));
        }
    }
}

fn main() {
    let site = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../site");
    println!("cargo:rerun-if-changed={}", site.display());
    let mut v = Vec::new();
    walk(&site, &site, &mut v);
    let src = format!("pub static SITE: &[(&str, &[u8])] = &[\n{}\n];\n", v.join("\n"));
    std::fs::write(Path::new(&std::env::var("OUT_DIR").unwrap()).join("site.rs"), src).unwrap();
}
