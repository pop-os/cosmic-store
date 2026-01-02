use std::collections::HashMap;
use std::fs;

pub struct OsInfo {
    #[allow(dead_code)]
    pub id: String,              // "pop" or "ubuntu"
    #[allow(dead_code)]
    pub version_id: String,      // "24.04"
    pub version_codename: String, // "noble"
    pub ubuntu_codename: String, // "noble" (for Pop!_OS)
}

impl OsInfo {
    /// Detect OS information from /etc/os-release
    pub fn detect() -> Result<Self, Box<dyn std::error::Error>> {
        let contents = fs::read_to_string("/etc/os-release")?;

        let mut map = HashMap::new();
        for line in contents.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }

            if let Some((key, value)) = line.split_once('=') {
                let value = value.trim_matches('"');
                map.insert(key.to_string(), value.to_string());
            }
        }

        let id = map.get("ID").map(|s| s.as_str()).unwrap_or("ubuntu").to_string();
        let version_id = map
            .get("VERSION_ID")
            .map(|s| s.as_str())
            .unwrap_or("22.04")
            .to_string();
        let version_codename = map
            .get("VERSION_CODENAME")
            .map(|s| s.as_str())
            .unwrap_or("jammy")
            .to_string();
        let ubuntu_codename = map
            .get("UBUNTU_CODENAME")
            .map(|s| s.to_string())
            .unwrap_or_else(|| version_codename.clone());

        Ok(OsInfo {
            id,
            version_id,
            version_codename,
            ubuntu_codename,
        })
    }

    /// Get the OS codename (prefers UBUNTU_CODENAME for Pop!_OS)
    pub fn codename(&self) -> &str {
        if !self.ubuntu_codename.is_empty() {
            &self.ubuntu_codename
        } else {
            &self.version_codename
        }
    }
}
