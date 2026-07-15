fn main() {
    if let Ok(contents) =
        std::fs::read_to_string(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(".env"))
    {
        for line in contents.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            if let Some((key, value)) = line.split_once('=') {
                if key == "BACKEND_URL" {
                    println!("cargo:rustc-env={}={}", key.trim(), value.trim());
                }
            }
        }
    }

    println!("cargo:rerun-if-changed=.env");

    tauri_build::build()
}
