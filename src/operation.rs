use std::sync::Arc;

use crate::AppInfo;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum OperationKind {
    Install,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct Operation {
    pub kind: OperationKind,
    pub backend_name: &'static str,
    pub package_id: String,
    pub info: Arc<AppInfo>,
}
