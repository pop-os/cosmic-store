use crate::AppId;

/// Determine source priority
pub fn priority(backend_name: &str, source_id: &str, id: &AppId) -> i32 {
    let mut priority = 0;
    match id.normalized() {
        // These ids prefer the packagekit backend
        "net.lutris.Lutris" | "com.valvesoftware.Steam" => {
            if backend_name == "packagekit" {
                priority += 2;
            }
        }
        // All other sources prefer the flatpak backend
        _ => {
            if backend_name == "flatpak" {
                priority += 2;

                // Among flatpak sources, the flathub source is preferred
                if source_id == "flathub" {
                    priority += 1;
                }
            }
        }
    }
    priority
}
