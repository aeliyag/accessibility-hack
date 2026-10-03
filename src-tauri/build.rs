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

        println!("cargo:rerun-if-changed={}", swift_src.display());

        let status = Command::new("swiftc")
            .arg("-O")
            .arg("-o")
            .arg(&helper)
            .arg(&swift_src)
            .status()
            .expect("failed to invoke swiftc — install Xcode CLT");

        if !status.success() {
            panic!("swiftc failed building typoscope-ocr helper");
        }

        println!("cargo:rustc-env=TYPOSCOPE_OCR_HELPER={}", helper.display());
    }

    tauri_build::build()
}
