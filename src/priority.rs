//TODO: less hard-coded rules for repo priorities
pub fn priority(backend_name: &str, source_id: &str, _id: &str) -> i32 {
    let mut priority = 0;
    if backend_name == "flatpak" {
        priority += 2;
    }
    if source_id == "flathub" {
        priority += 1;
    }
    priority
}
