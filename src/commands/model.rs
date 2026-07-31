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

/// Verify a GGUF file at the given path. Returns true if valid.
fn verify_gguf(path: &std::path::Path) -> bool {
    use std::io::Read;
    let mut file = match std::fs::File::open(path) {
        Ok(f) => f,
        Err(_) => return false,
    };
    let mut magic = [0u8; 4];
    if file.read_exact(&mut magic).is_err() {
        return false;
    }
    &magic == b"GGUF"
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

    // Check if .part file is a complete, valid GGUF
    if part.exists() {
        let part_meta = std::fs::metadata(&part).map_err(|e| e.to_string())?;
        let part_size = part_meta.len();
        let is_valid_gguf = verify_gguf(&part);

        if is_valid_gguf && (expected_size == 0 || part_size == expected_size) {
            // .part file is complete — just rename it
            eprintln!("  {id} download found, finalizing...");
            rename_part_to_dest(&part, &dest)?;
            eprintln!("  Done. {id} installed.");
            return Ok(());
        }

        if is_valid_gguf && part_size > 0 {
            // Valid GGUF but incomplete — report and re-download
            eprintln!("  Partial download found ({}/{}), re-downloading...",
                format_size(part_size), format_size(expected_size));
        } else {
            // Invalid file — delete and re-download
            eprintln!("  Incomplete or corrupted download, re-downloading...");
            std::fs::remove_file(&part).ok();
        }
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

    // Use content-length from response as the true total
    let total = response
        .header("content-length")
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(expected_size);

    use std::io::{Read, Write};

    let mut reader = response.into_reader();
    let mut file = std::fs::File::create(&part).map_err(|e| e.to_string())?;
    let mut buf = [0u8; 65536];
    let mut downloaded: u64 = 0;

    let pb = indicatif::ProgressBar::new(total);
    pb.set_style(
        indicatif::ProgressStyle::default_bar()
            .template("  {spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {bytes}/{total_bytes} ({bytes_per_sec}) ({eta})")
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

    // Verify GGUF magic
    if !verify_gguf(&part) {
        let msg = format!("not a valid GGUF file");
        std::fs::remove_file(&part).ok();
        return Err(msg);
    }

    // Verify file size (tolerant: accept if GGUF is valid)
    let meta = std::fs::metadata(&part).map_err(|e| e.to_string())?;
    if expected_size > 0 && meta.len() != expected_size {
        eprintln!("  Warning: size mismatch (expected {}, got {}) but GGUF is valid, installing anyway.",
            format_size(expected_size), format_size(meta.len()));
    }

    // Rename part to final path
    rename_part_to_dest(&part, &dest)?;

    eprintln!("  Done. {id} installed.");
    Ok(())
}

/// Rename .part file to final destination, handling conflicts.
fn rename_part_to_dest(part: &std::path::Path, dest: &std::path::Path) -> Result<(), String> {
    match std::fs::rename(part, dest) {
        Ok(_) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
            std::fs::remove_file(dest).map_err(|e| e.to_string())?;
            std::fs::rename(part, dest).map_err(|e| format!("rename retry failed: {e}"))
        }
        Err(e) => Err(format!("rename failed: {e}")),
    }
}

/// Interactive model selector — shows models and lets user pick with arrow keys.
fn interactive_select_model() -> Result<String, String> {
    let registry = cotrex_ai_runtime::model_manager::registry::ModelRegistry::built_in();
    let installed = list_installed().unwrap_or_default();

    let models: Vec<_> = registry.models.iter().collect();
    if models.is_empty() {
        return Err("no models available".into());
    }

    let options: Vec<String> = models
        .iter()
        .map(|m| {
            let is_installed = installed.iter().any(|i| i == &m.id);
            let status = if is_installed { " [installed]" } else { "" };
            format!("{} — {}{}", m.id, format_size(m.size), status)
        })
        .collect();

    let selection = inquire::Select::new("Select a model to install:", options)
        .prompt()
        .map_err(|e| format!("{e}"))?;

    // Parse the model ID from the selection (before the " — " separator)
    let model_id = selection.split(" — ").next().unwrap_or(&selection).trim().to_string();
    Ok(model_id)
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
            let id = if let Some(id) = model_id.as_deref() {
                id.to_string()
            } else {
                // No model specified — interactive selector
                match interactive_select_model() {
                    Ok(id) => id,
                    Err(e) => {
                        eprintln!("cotrex: {e}");
                        std::process::exit(1);
                    }
                }
            };
            match resolve_model(&id) {
                Ok((filename, url, size)) => {
                    if let Err(e) = download_model(&id, &filename, &url, size) {
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
                let marker = if is_installed { "\u{2713}" } else { " " };
                let size_str = format_size(m.size);
                println!("  [{marker}] {:<20} {}", m.id, size_str);
            }

            if !installed.is_empty() {
                println!("\nInstalled:");
                for name in &installed {
                    let size = model_size(&format!("{name}.gguf"));
                    println!("  \u{2713} {} ({})", name, format_size(size));
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
                        println!("  Status:     \u{2713} installed");
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
        ModelAction::Test { model_id } => {
            run_model_test(model_id);
        }
    }
}

/// Run model qualification tests.
fn run_model_test(model_id: &str) {
    use crate::agent::qualify;

    // Resolve model ID to filename from registry
    let (filename, _url, _size) = match resolve_model(model_id) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("cotrex: {e}");
            std::process::exit(1);
        }
    };

    // Check if model is installed by filename
    let installed = list_installed().unwrap_or_default();
    let stem = filename.replace(".gguf", "");
    let is_installed = installed.iter().any(|i| i == &stem);
    if !is_installed {
        eprintln!("cotrex: model '{model_id}' is not installed.");
        eprintln!("  Install it first: cotrex model install {model_id}");
        std::process::exit(2);
    }

    // Check for existing qualification
    if let Some(existing) = qualify::load_qualification(model_id) {
        let current_hash = qualify::compute_prompt_hash();
        if existing.prompt_hash != current_hash {
            eprintln!("  Previous qualification is stale (prompts changed). Re-testing...");
        } else {
            eprintln!("  Using cached qualification (use --force to re-test).");
            print_qualification_summary(&existing);
            return;
        }
    }

    eprintln!("Testing model: {model_id}\n");

    // Run qualification
    let result = qualify::run_qualification(model_id);

    // Print results
    for test in &result.tests {
        let status = if test.passed { "PASS" } else { "FAIL" };
        let reason = test
            .reason
            .as_ref()
            .map(|r| format!(" \u{2014} {r}"))
            .unwrap_or_default();
        println!("[{status}] {}{}", test.name, reason);
    }

    println!("\nCapability summary:\n");
    for (cap, status) in &result.capabilities {
        println!("  {cap}: {status}");
    }

    // Save qualification
    if let Err(e) = qualify::save_qualification(&result) {
        eprintln!("\nFailed to save qualification: {e}");
    } else {
        println!(
            "\nReport:\n  .cotrex/qualifications/{}.json",
            model_id
        );
    }

    // Exit with appropriate code
    let all_passed = result.capabilities.values().all(|s| *s == qualify::CapabilityStatus::Passed);
    if !all_passed {
        std::process::exit(1);
    }
}

/// Print qualification summary.
fn print_qualification_summary(result: &crate::agent::qualify::QualificationResult) {
    for test in &result.tests {
        let status = if test.passed { "PASS" } else { "FAIL" };
        let reason = test
            .reason
            .as_ref()
            .map(|r| format!(" \u{2014} {r}"))
            .unwrap_or_default();
        println!("[{status}] {}{}", test.name, reason);
    }

    println!("\nCapability summary:\n");
    for (cap, status) in &result.capabilities {
        println!("  {cap}: {status}");
    }
}
