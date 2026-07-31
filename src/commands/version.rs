//! `cotrex version` — detailed version information.

pub fn run() {
    let version = env!("CARGO_PKG_VERSION");
    let installed = super::model::list_installed().unwrap_or_default();
    let registry = cotrex_ai_runtime::model_manager::registry::ModelRegistry::built_in();

    let backend = if cfg!(feature = "local-model") {
        "Local (llama.cpp)"
    } else {
        "Remote (API)"
    };

    let features = {
        let mut f = Vec::new();
        if cfg!(feature = "local-model") {
            f.push("MCP");
            f.push("Local Inference");
        }
        f.push("Workspace Intelligence");
        f.push("Git Context");
        f.join(", ")
    };

    println!("Cotrex {version}\n");
    println!("Runtime:  cotrex-ai");
    println!("Backend:  {backend}");
    println!(
        "Models:   {} registered, {} installed",
        registry.models.len(),
        installed.len()
    );
    println!("Features: {features}");
}
