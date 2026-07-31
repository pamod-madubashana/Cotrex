//! `cotrex init` — first-run onboarding.

/// Run the init command.
pub fn run(no_download: bool) {
    println!("Welcome to Cotrex\n");
    println!("Checking system...\n");

    // Run shared checks
    let checks = super::checks::run_all();
    for check in &checks {
        let icon = if check.ok { "✓" } else { "✗" };
        println!("  {icon} {}", check.label);
    }

    // Check installed models
    let installed = super::model::list_installed().unwrap_or_default();

    if !installed.is_empty() {
        println!("\n  ✓ Models already installed:");
        for m in &installed {
            let size = super::model::model_size(&format!("{m}.gguf"));
            println!("    {} ({})", m, super::model::format_size(size));
        }
        println!("\nCotrex is ready.");
        return;
    }

    // No models installed
    println!("\n  ⚠ No models installed.\n");

    if no_download {
        println!("Skipping download (--no-download).\n");
        println!("Install a model manually:\n");
        println!("  cotrex model install qwen2.5-0.5b\n");
        return;
    }

    // Resolve the recommended model
    let registry = cotrex_ai_runtime::model_manager::registry::ModelRegistry::built_in();
    let recommended = match registry.models.first() {
        Some(m) => m,
        None => {
            println!("No models available in registry.");
            return;
        }
    };

    println!(
        "Installing recommended model:\n  {} ({})\n",
        recommended.id,
        super::model::format_size(recommended.size)
    );

    match super::model::download_model(
        &recommended.id,
        &recommended.filename,
        &recommended.url,
        recommended.size,
    ) {
        Ok(()) => {
            println!("\nCotrex is ready.\n");
            println!("Run 'cotrex mcp' to start.");
        }
        Err(e) => {
            eprintln!("\nFailed to install model: {e}");
            eprintln!("\nTry manually:");
            eprintln!("  cotrex model install {}", recommended.id);
            std::process::exit(1);
        }
    }
}
