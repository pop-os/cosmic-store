use std::{fmt, sync::Arc};

use crate::{AppId, AppInfo};

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum OperationKind {
    Install,
    Uninstall,
    Update,
    RepositoryAdd { id: String, data: Vec<u8> },
    RepositoryRemove { id: String, force: bool },
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct Operation {
    pub kind: OperationKind,
    pub backend_name: &'static str,
    pub package_ids: Vec<AppId>,
    pub infos: Vec<Arc<AppInfo>>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RepositoryRemoveError {
    pub id: String,
    pub installed: Vec<(String, String)>,
}

impl fmt::Display for RepositoryRemoveError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "failed to remove repository {} as it still has {} installed {}",
            self.id,
            self.installed.len(),
            if self.installed.len() == 1 {
                "item"
            } else {
                "items"
            }
        )
    }
}

impl std::error::Error for RepositoryRemoveError {}

impl Operation {
    pub fn pending_text(&self, progress: i32) -> String {
        //TODO: translate
        let verb = match &self.kind {
            OperationKind::Install => "Installing",
            OperationKind::Uninstall => "Uninstalling",
            OperationKind::Update => "Updating",
            OperationKind::RepositoryAdd { id, .. } => {
                return format!("Adding repository {} ({}%)", id, progress);
            }
            OperationKind::RepositoryRemove { id, .. } => {
                return format!("Removing repository {} ({}%)", id, progress);
            }
        };
        format!(
            "{} {} from {} ({}%)...",
            verb, self.infos[0].name, self.infos[0].source_name, progress
        )
    }

    pub fn completed_text(&self) -> String {
        //TODO: translate
        let verb = match &self.kind {
            OperationKind::Install => "Installed",
            OperationKind::Uninstall => "Uninstalled",
            OperationKind::Update => "Updated",
            OperationKind::RepositoryAdd { id, .. } => {
                return format!("Added repository {}", id);
            }
            OperationKind::RepositoryRemove { id, .. } => {
                return format!("Removed repository {}", id);
            }
        };
        format!(
            "{} {} from {}",
            verb, self.infos[0].name, self.infos[0].source_name
        )
    }

    pub fn failed_dialog(&self, err: &str) -> (String, String) {
        //TODO: translate
        let verb = match &self.kind {
            OperationKind::Install => "install",
            OperationKind::Uninstall => "uninstall",
            OperationKind::Update => "update",
            OperationKind::RepositoryAdd { id, .. } => {
                return (
                    format!("Failed to add repository {}", id),
                    format!("Failed to add repository {}:\n{err}", id),
                );
            }
            OperationKind::RepositoryRemove { id, .. } => {
                return (
                    format!("Failed to remove repository {}", id),
                    format!("Failed to remove repository {}:\n{err}", id),
                );
            }
        };
        //TODO: get ids and names from all packages
        (
            format!(
                "Failed to {verb} {} from {}",
                self.infos[0].name, self.infos[0].source_name
            ),
            format!(
                "Failed to {verb} {} ({}) from {} ({}):\n{err}",
                self.infos[0].name,
                self.package_ids[0].raw(),
                self.infos[0].source_name,
                self.infos[0].source_id
            ),
        )
    }
}
