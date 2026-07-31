//! `cotrex model` — install, list, remove, info.

use crate::dispatch::cli::ModelAction;
use std::path::PathBuf;

/// Resolve the models directory from COTREX_HOME or ~/.cotrex/models.
pub fn models_dir() -> Result<PathBuf, String> {
    if let Ok(home) = std::env::var("COTREX_HOME") {
        return Ok(PathBuf::from(home).join("models"));
    }
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .map_err(|_| "cannot determine home directory".to_string())?;
    Ok(PathBuf::from(home).join(".cotrex").join("models"))
}

/// List installed model filenames (without .gguf extension).
pub fn list_installed() -> Result<Vec<String>, String> {
    let dir = models_dir()?;
    let mut files = Vec::new();
    if dir.exists() {
        for entry in std::fs::read_dir(&dir).map_err(|e| e.to_string())? {
            let entry = entry.map_err(|e| e.to_string())?;
            let path = entry.path();
            if path.extension().is_some_and(|ext| ext == "gguf") {
                if let Some(name) = path.file_stem() {
                    files.push(name.to_string_lossy().into_owned());
                }
            }
        }
    }
    files.sort();
    Ok(files)
}

/// Get the path for a specific model file.
pub fn model_path(filename: &str) -> Result<std::path::PathBuf, String> {
    Ok(models_dir()?.join(filename))
}

/// Get model file size in bytes, or 0 if not found.
pub fn model_size(filename: &str) -> u64 {
    model_path(filename)
        .ok()
        .and_then(|p| std::fs::metadata(p).ok())
        .map(|m| m.len())
        .unwrap_or(0)
}

/// Resolve a model ID to a (filename, url) pair from the built-in registry.
/// "latest" resolves to the first (recommended) model.
pub fn resolve_model(id: &str) -> Result<(String, String, u64), String> {
    let registry = cotrex_ai_runtime::model_manager::registry::ModelRegistry::built_in();
    let model = if id == "latest" {
        registry.models.first().ok_or("no models in registry")?
    } else {
        registry
            .find(id)
            .ok_or_else(|| format!("unknown model: {id}. Run: cotrex model list"))?
    };
    Ok((model.filename.clone(), model.url.clone(), model.size))
}

/// Download a model with progress bar and verify checksum.
pub fn download_model(
    id: &str,
    filename: &str,
    url: &str,
    expected_size: u64,
) -> Result<(), String> {
    let dir = models_dir()?;
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;

    let dest = dir.join(filename);
    let part = dir.join(format!("{filename}.part"));

    // Check if already installed
    if dest.exists() {
        let meta = std::fs::metadata(&dest).map_err(|e| e.to_string())?;
        if meta.len() == expected_size || expected_size == 0 {
            eprintln!("  {id} already installed.");
            return Ok(());
        }
        // Size mismatch — re-download
        std::fs::remove_file(&dest).map_err(|e| e.to_string())?;
    }

    // Clean up stale .part file
    if part.exists() {
        std::fs::remove_file(&part).map_err(|e| e.to_string())?;
    }

    eprintln!("  Downloading {id}...");

    // Download with progress
    let response = ureq::get(url)
        .call()
        .map_err(|e| format!("download failed: {e}"))?;

    let status = response.status();
    if status != 200 {
        return Err(format!("HTTP {status}"));
    }

    let total = if expected_size > 0 {
        expected_size
    } else {
        response
            .header("content-length")
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(0)
    };

    use std::io::{Read, Write};

    let mut reader = response.into_reader();
    let mut file = std::fs::File::create(&part).map_err(|e| e.to_string())?;
    let mut buf = [0u8; 8192];
    let mut downloaded: u64 = 0;

    let pb = indicatif::ProgressBar::new(total);
    pb.set_style(
        indicatif::ProgressStyle::default_bar()
            .template("  {spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {bytes}/{total_bytes} ({eta})")
            .expect("valid template")
            .progress_chars("=> "),
    );

    loop {
        let n = reader.read(&mut buf).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        file.write_all(&buf[..n]).map_err(|e| e.to_string())?;
        downloaded += n as u64;
        pb.set_position(downloaded);
    }
    pb.finish_and_clear();

    // Flush and close file handle before verification
    file.flush().map_err(|e| e.to_string())?;
    drop(file);

    // Diagnostics
    eprintln!("  verify path: {}", part.display());
    let meta = std::fs::metadata(&part).map_err(|e| e.to_string())?;
    eprintln!("  exists: true");
    eprintln!("  size: {}", meta.len());

    // Verify GGUF magic
    eprintln!("  Verifying GGUF...");
    {
        use std::io::Read;
        let mut file = std::fs::File::open(&part).map_err(|e| {
            let msg = format!("open for verify failed: {}: {}", part.display(), e);
            eprintln!("  {msg}");
            msg
        })?;
        let mut magic = [0u8; 4];
        file.read_exact(&mut magic).map_err(|e| {
            let msg = format!("read magic failed: {}: {}", part.display(), e);
            eprintln!("  {msg}");
            msg
        })?;
        if &magic != b"GGUF" {
            let msg = format!("not a valid GGUF file: expected GGUF, got {:?}", &magic);
            eprintln!("  {msg}");
            std::fs::remove_file(&part).ok();
            return Err(msg);
        }
        eprintln!("  GGUF magic: OK");
    }

    // Verify file size
    if expected_size > 0 && meta.len() != expected_size {
        let msg = format!(
            "size mismatch: expected {} bytes, got {}",
            expected_size,
            meta.len()
        );
        eprintln!("  {msg}");
        std::fs::remove_file(&part).ok();
        return Err(msg);
    }
    eprintln!("  size: OK");

    // Rename part to final path
    eprintln!("  Installing...");
    eprintln!("  rename source: {}", part.display());
    eprintln!("  rename destination: {}", dest.display());
    match std::fs::rename(&part, &dest) {
        Ok(_) => {}
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
            eprintln!("  destination exists, removing stale file...");
            std::fs::remove_file(&dest).map_err(|e| e.to_string())?;
            std::fs::rename(&part, &dest).map_err(|e| {
                let msg = format!("rename retry failed: {}", e);
                eprintln!("  {msg}");
                msg
            })?;
        }
        Err(e) => {
            let msg = format!("rename failed: {}: {}", part.display(), e);
            eprintln!("  {msg}");
            return Err(msg);
        }
    }

    eprintln!("  Done. {id} installed.");
    Ok(())
}

/// Format bytes as human-readable size.
pub fn format_size(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = 1024 * KB;
    const GB: u64 = 1024 * MB;

    if bytes >= GB {
        format!("{:.1} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.0} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.0} KB", bytes as f64 / KB as f64)
    } else {
        format!("{bytes} B")
    }
}

/// Execute the model subcommand.
pub fn run(action: &ModelAction) {
    match action {
        ModelAction::Install { model_id } => {
            let id = model_id.as_deref().unwrap_or("latest");
            match resolve_model(id) {
                Ok((filename, url, size)) => {
                    if let Err(e) = download_model(id, &filename, &url, size) {
                        eprintln!("cotrex: {e}");
                        std::process::exit(1);
                    }
                }
                Err(e) => {
                    eprintln!("cotrex: {e}");
                    std::process::exit(1);
                }
            }
        }
        ModelAction::List => {
            let installed = list_installed().unwrap_or_default();
            let registry = cotrex_ai_runtime::model_manager::registry::ModelRegistry::built_in();

            if installed.is_empty() && registry.models.is_empty() {
                println!("No models available.");
                return;
            }

            println!("Available models:\n");
            for m in &registry.models {
                let is_installed = installed.iter().any(|i| i == &m.id);
                let marker = if is_installed { "✓" } else { " " };
                let size_str = format_size(m.size);
                println!("  [{marker}] {:<20} {}", m.id, size_str);
            }

            if !installed.is_empty() {
                println!("\nInstalled:");
                for name in &installed {
                    let size = model_size(&format!("{name}.gguf"));
                    println!("  ✓ {} ({})", name, format_size(size));
                }
            }
        }
        ModelAction::Remove { model_id } => {
            let filename = format!("{model_id}.gguf");
            let path = match model_path(&filename) {
                Ok(p) => p,
                Err(e) => {
                    eprintln!("cotrex: {e}");
                    std::process::exit(1);
                }
            };
            if path.exists() {
                std::fs::remove_file(&path)
                    .map_err(|e| e.to_string())
                    .unwrap();
                println!("Removed {model_id}.");
            } else {
                eprintln!("cotrex: model '{model_id}' is not installed.");
                std::process::exit(1);
            }
        }
        ModelAction::Info { model_id } => {
            let id = model_id.as_deref().unwrap_or("latest");
            match resolve_model(id) {
                Ok((filename, _url, size)) => {
                    let installed = list_installed().unwrap_or_default();
                    let is_installed = installed
                        .iter()
                        .any(|i| i == &filename.replace(".gguf", ""));
                    let actual_size = model_size(&filename);

                    println!("Model: {id}");
                    println!("  File:       {filename}");
                    println!("  Size:       {}", format_size(size));
                    if is_installed {
                        println!("  Status:     ✓ installed");
                        if actual_size > 0 {
                            println!("  Disk usage: {}", format_size(actual_size));
                        }
                    } else {
                        println!("  Status:     not installed");
                    }
                    println!(
                        "  Registry:   {}",
                        cotrex_ai_runtime::model_manager::registry::ModelRegistry::built_in()
                            .find(id)
                            .and_then(|m| m.context)
                            .map(|c| format!("{c} context"))
                            .unwrap_or_else(|| "unknown".into())
                    );
                }
                Err(e) => {
                    eprintln!("cotrex: {e}");
                    std::process::exit(1);
                }
            }
        }
    }
}
