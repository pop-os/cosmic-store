use std::sync::Arc;

use crate::{AppId, AppInfo};

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum OperationKind {
    Install,
    Uninstall,
    Update,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct Operation {
    pub kind: OperationKind,
    pub backend_name: &'static str,
    pub package_ids: Vec<AppId>,
    pub infos: Vec<Arc<AppInfo>>,
}

impl Operation {
    pub fn pending_text(&self, progress: i32) -> String {
        //TODO: translate
        let verb = match self.kind {
            OperationKind::Install => "Installing",
            OperationKind::Uninstall => "Uninstalling",
            OperationKind::Update => "Updating",
        };
        format!(
            "{} {} from {} ({}%)...",
            verb, self.infos[0].name, self.infos[0].source_name, progress
        )
    }

    pub fn completed_text(&self) -> String {
        //TODO: translate
        let verb = match self.kind {
            OperationKind::Install => "Installed",
            OperationKind::Uninstall => "Uninstalled",
            OperationKind::Update => "Updated",
        };
        format!(
            "{} {} from {}",
            verb, self.infos[0].name, self.infos[0].source_name
        )
    }

    pub fn failed_dialog(&self, err: &str) -> (String, String) {
        //TODO: translate
        let verb = match self.kind {
            OperationKind::Install => "install",
            OperationKind::Uninstall => "uninstall",
            OperationKind::Update => "update",
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
