//! Reads only fixed invented sentinel filenames from explicit fixture source roots.
use diary_api::FolderCheck;
use diary_infrastructure::config::Config;
use std::io::Read;
pub fn folders(config: &Config) -> Vec<FolderCheck> {
    config
        .sources
        .iter()
        .filter(|source| matches!(source.alias.as_str(), "smoke-readable" | "smoke-denied"))
        .map(|source| {
            let path = source.root.join("sentinel.txt");
            let outcome = match std::fs::File::open(path) {
                Ok(file) => {
                    let mut bytes = Vec::new();
                    match file.take(64).read_to_end(&mut bytes) {
                        Ok(_) if bytes == b"synthetic scaffold sentinel\n" => "readable",
                        _ => "unexpected",
                    }
                }
                Err(error) if error.kind() == std::io::ErrorKind::PermissionDenied => {
                    eprintln!(
                        "SMOKE server_folder_denied errno={}",
                        error.raw_os_error().unwrap_or(0)
                    );
                    "denied"
                }
                Err(_) => "unexpected",
            };
            if outcome == "readable" {
                eprintln!("SMOKE server_folder_readable");
            }
            if outcome == "denied" {
                eprintln!("SMOKE server_folder_denied");
            }
            FolderCheck {
                alias: source.alias.clone(),
                outcome: outcome.into(),
            }
        })
        .collect()
}
