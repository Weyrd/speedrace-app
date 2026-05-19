// build.rs — forward BACKEND_URL from .env to the crate via option_env!()
// Place a `.env` file next to this build.rs (i.e. inside src-tauri/) and add:
//   BACKEND_URL=https://api.momentum.app
// The file is gitignored; CI sets the variable directly in the environment.

fn main() {
    // Read a local .env file if present (dev only, not required in CI)
    if let Ok(contents) =
        std::fs::read_to_string(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(".env"))
    {
        for line in contents.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            if let Some((key, value)) = line.split_once('=') {
                // Only export known variables so we don't pollute the env
                if key == "BACKEND_URL" {
                    println!("cargo:rustc-env={}={}", key.trim(), value.trim());
                }
            }
        }
    }

    // Always re-run if .env changes (or appears / disappears)
    println!("cargo:rerun-if-changed=.env");

    tauri_build::build()
}
