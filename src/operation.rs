use std::{fmt, sync::Arc};

use crate::{AppId, AppInfo, backend::BackendName};

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum OperationKind {
    Install,
    Uninstall { purge_data: bool },
    Update,
    RepositoryAdd(Vec<RepositoryAdd>),
    RepositoryRemove(Vec<RepositoryRemove>, bool),
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct Operation {
    pub kind: OperationKind,
    pub backend_name: BackendName,
    pub package_ids: Vec<AppId>,
    pub infos: Vec<Arc<AppInfo>>,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct RepositoryAdd {
    pub id: String,
    pub data: Vec<u8>,
}

impl RepositoryAdd {
    fn ids(adds: &[Self]) -> Vec<String> {
        adds.iter().map(|x| x.id.clone()).collect()
    }
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct RepositoryRemove {
    pub id: String,
    pub name: String,
}

impl RepositoryRemove {
    fn ids(rms: &[Self]) -> Vec<String> {
        rms.iter().map(|x| x.id.clone()).collect()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RepositoryRemoveError {
    pub rms: Vec<RepositoryRemove>,
    pub installed: Vec<(String, String)>,
}

impl fmt::Display for RepositoryRemoveError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "failed to remove repositories {:?} as it still has {} installed {}",
            RepositoryRemove::ids(&self.rms),
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
            OperationKind::Uninstall { .. } => "Uninstalling",
            OperationKind::Update => "Updating",
            OperationKind::RepositoryAdd(adds) => {
                return format!(
                    "Adding repositories {:?} ({}%)",
                    RepositoryAdd::ids(adds),
                    progress
                );
            }
            OperationKind::RepositoryRemove(rms, _force) => {
                return format!(
                    "Removing repositories {:?} ({}%)",
                    RepositoryRemove::ids(rms),
                    progress
                );
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
            OperationKind::Uninstall { .. } => "Uninstalled",
            OperationKind::Update => "Updated",
            OperationKind::RepositoryAdd(adds) => {
                return format!("Added repositories {:?}", RepositoryAdd::ids(adds));
            }
            OperationKind::RepositoryRemove(rms, _force) => {
                return format!("Removed repositories {:?}", RepositoryRemove::ids(rms));
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
            OperationKind::Uninstall { .. } => "uninstall",
            OperationKind::Update => "update",
            OperationKind::RepositoryAdd(adds) => {
                return (
                    "Failed to add repositories".to_string(),
                    format!(
                        "Failed to add repositories {:?}:\n{err}",
                        RepositoryAdd::ids(adds)
                    ),
                );
            }
            OperationKind::RepositoryRemove(rms, _force) => {
                return (
                    "Failed to remove repositories".to_string(),
                    format!(
                        "Failed to remove repositories {:?}:\n{err}",
                        RepositoryRemove::ids(rms)
                    ),
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
