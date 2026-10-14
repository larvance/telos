//! Build script for Telos Launcher and Tauri asset bundle compilation.

fn main() {
    if std::path::Path::new("src/input.css").exists() {
        println!("cargo:rerun-if-changed=src/input.css");
        println!("cargo:rerun-if-changed=ui/index.html");
        println!("cargo:rerun-if-changed=ui/app.js");

        let _ = std::process::Command::new("npx")
            .args([
                "tailwindcss",
                "-i",
                "src/input.css",
                "-o",
                "ui/styles.css",
                "--minify",
            ])
            .status();
    }

    tauri_build::build();
}
