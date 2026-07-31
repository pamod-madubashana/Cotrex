//! `cotrex doctor` — system diagnostics.

pub fn run() {
    println!("Doctor Report\n");

    let checks = super::checks::run_all();
    for check in &checks {
        let icon = if check.ok { "✓" } else { "✗" };
        println!("  {icon} {}: {}", check.label, check.message);
    }

    let models = super::checks::installed_models();
    if models.is_empty() {
        println!("\n  ⚠ No models installed\n");
        println!("  Run: cotrex init");
    } else {
        println!("\n  Installed models:");
        for m in &models {
            let size = super::model::model_size(&format!("{m}.gguf"));
            println!("    ✓ {} ({})", m, super::model::format_size(size));
        }
    }
}
