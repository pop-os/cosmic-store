use crate::{AppId, backend::BackendName};

/// Determine source priority
pub fn priority(backend_name: BackendName, source_id: &str, id: &AppId) -> i32 {
    let mut priority = 0;
    if id.is_system() {
        // For system packages, prefer the packagekit backend
        if backend_name == BackendName::Packagekit {
            priority += 2;
        }
        return priority;
    }
    match id.normalized() {
        // These ids prefer the packagekit backend
        "net.lutris.Lutris" | "com.valvesoftware.Steam" => {
            if backend_name == BackendName::Packagekit {
                priority += 2;
            }
        }
        // All other sources prefer the flatpak-user backend
        _ => {
            if backend_name == BackendName::FlatpakUser {
                priority += 2;

                // Among flatpak-user sources, the flathub source is preferred
                if source_id == "flathub" {
                    priority += 1;
                }
            }
        }
    }
    priority
}
