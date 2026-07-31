//! Shared system checks used by both `init` and `doctor`.

/// Result of a single system check.
pub struct Check {
    pub label: &'static str,
    pub ok: bool,
    pub message: String,
}

/// Run all system checks and return results.
pub fn run_all() -> Vec<Check> {
    let checks = vec![check_config_dir(), check_models_dir(), check_rtk()];
    checks
}

fn check_config_dir() -> Check {
    let path = dirs::config_dir().map(|d| d.join("cotrex"));
    match path {
        Some(p) if p.exists() => Check {
            label: "Config directory",
            ok: true,
            message: format!("{}", p.display()),
        },
        Some(p) => {
            // Create it
            let _ = std::fs::create_dir_all(&p);
            Check {
                label: "Config directory",
                ok: true,
                message: format!("created {}", p.display()),
            }
        }
        None => Check {
            label: "Config directory",
            ok: false,
            message: "cannot determine config directory".into(),
        },
    }
}

fn check_models_dir() -> Check {
    match crate::commands::model::models_dir() {
        Ok(dir) if dir.exists() => Check {
            label: "Models directory",
            ok: true,
            message: format!("{}", dir.display()),
        },
        Ok(dir) => {
            let _ = std::fs::create_dir_all(&dir);
            Check {
                label: "Models directory",
                ok: true,
                message: format!("created {}", dir.display()),
            }
        }
        Err(e) => Check {
            label: "Models directory",
            ok: false,
            message: e,
        },
    }
}

fn check_rtk() -> Check {
    // RTK is resolved at runtime by the orchestration layer.
    // For now, just check if the embedded or data-dir copy exists.
    let data_dir = dirs::data_dir()
        .or_else(dirs::data_local_dir)
        .map(|d| d.join("cotrex"));
    match data_dir {
        Some(p) => {
            let rtk_path = p
                .join("rtk")
                .with_extension(std::env::consts::EXE_EXTENSION);
            if rtk_path.exists() {
                Check {
                    label: "RTK",
                    ok: true,
                    message: format!("detected at {}", rtk_path.display()),
                }
            } else {
                // RTK auto-downloads on first use, so this is not an error
                Check {
                    label: "RTK",
                    ok: true,
                    message: "will be downloaded on first use".into(),
                }
            }
        }
        None => Check {
            label: "RTK",
            ok: true,
            message: "will be downloaded on first use".into(),
        },
    }
}

/// Check if any models are installed.
pub fn installed_models() -> Vec<String> {
    crate::commands::model::list_installed().unwrap_or_default()
}
