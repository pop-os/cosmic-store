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
    pub package_id: AppId,
    pub info: Arc<AppInfo>,
}

impl Operation {
    pub fn failed_dialog(&self, err: &str) -> (String, String) {
        //TODO: translate
        let verb = match self.kind {
            OperationKind::Install => "install",
            OperationKind::Uninstall => "uninstall",
            OperationKind::Update => "update",
        };
        (
            format!(
                "Failed to {verb} {} from {}",
                self.info.name, self.info.source_name
            ),
            format!(
                "Failed to {verb} {} ({}) from {} ({}):\n{err}",
                self.info.name,
                self.package_id.raw(),
                self.info.source_name,
                self.info.source_id
            ),
        )
    }
}
