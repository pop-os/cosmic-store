use std::sync::Arc;

use crate::AppInfo;

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
    pub package_id: String,
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
            format!("Failed to {verb} {}", self.info.name),
            format!(
                "Failed to {verb} {} ({}):\n{err}",
                self.info.name, self.package_id
            ),
        )
    }
}
