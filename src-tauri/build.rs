use std::{env, fs};

fn main() {
    println!("cargo:rerun-if-changed=../.env");
    let mut update_base = env::var("VITE_UPDATE_BASE_URL").unwrap_or_default();
    if update_base.is_empty() {
        if let Ok(contents) = fs::read_to_string("../.env") {
            for line in contents.lines() {
                if let Some((key, value)) = line.split_once('=') {
                    if key.trim() == "VITE_UPDATE_BASE_URL" {
                        update_base = value.trim().trim_matches(|c| c == '\"' || c == '\'').to_string();
                        break;
                    }
                }
            }
        }
    }
    if !update_base.is_empty() {
        println!("cargo:rustc-env=PR2PO_UPDATE_BASE_URL={update_base}");
    }
    tauri_build::build()
}
