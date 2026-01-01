use std::hash::{Hash, Hasher};

/// Normalize app IDs
fn normalize_id(id_raw: &str) -> &str {
    id_raw.trim_end_matches(".desktop")
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize, bitcode::Decode, bitcode::Encode)]
pub struct AppId(String);

impl AppId {
    pub fn new(id_raw: &str) -> Self {
        // The raw ID is stored for use by the backends
        Self(id_raw.to_string())
    }

    pub fn system() -> Self {
        Self("__SYSTEM__".to_string())
    }

    pub fn is_system(&self) -> bool {
        self.0 == "__SYSTEM__"
    }

    /// Get the raw ID
    pub fn raw(&self) -> &str {
        &self.0
    }

    /// Get the normalized ID
    pub fn normalized(&self) -> &str {
        normalize_id(&self.0)
    }
}

// Compare using the normalized ID
impl PartialEq for AppId {
    fn eq(&self, other: &Self) -> bool {
        self.normalized() == other.normalized()
    }
}
impl Eq for AppId {}

// Hash using the normalized ID
impl Hash for AppId {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.normalized().hash(state);
    }
}
