fn main() {
    #[cfg(target_os = "macos")]
    {
        use std::env;
        use std::path::PathBuf;
        use std::process::Command;

        let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
        let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
        let swift_src = manifest_dir.join("native/typoscope-ocr.swift");
        let helper = out_dir.join("typoscope-ocr");
        let target = env::var("TARGET").unwrap();
        let swift_target = match target.as_str() {
            "aarch64-apple-darwin" => "arm64-apple-macosx10.15",
            "x86_64-apple-darwin" => "x86_64-apple-macosx10.15",
            other => panic!("unsupported macOS OCR target: {other}"),
        };

        println!("cargo:rerun-if-changed={}", swift_src.display());

        let status = Command::new("swiftc")
            .arg("-O")
            .args(["-target", swift_target])
            .arg("-o")
            .arg(&helper)
            .arg(&swift_src)
            .status()
            .expect("failed to invoke swiftc — install Xcode CLT");

        if !status.success() {
            panic!("swiftc failed building typoscope-ocr helper");
        }

        // Tauri bundles this target-suffixed sidecar beside the app executable.
        let binaries = manifest_dir.join("binaries");
        std::fs::create_dir_all(&binaries).expect("create sidecar directory");
        std::fs::copy(&helper, binaries.join(format!("typoscope-ocr-{target}")))
            .expect("copy OCR sidecar for bundling");
    }

    tauri_build::build()
}
