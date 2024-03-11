use cosmic::iced::futures::{channel::mpsc, SinkExt};
use std::{collections::HashMap, error::Error, sync::Arc};

use crate::{Backend, Message};

fn err_str<T: ToString>(err: T) -> String {
    err.to_string()
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum Operation {
    Install {
        backend_name: &'static str,
        package_id: String,
    },
}

impl Operation {
    /// Perform the operation
    pub async fn perform(
        self,
        id: u64,
        backends: HashMap<&'static str, Arc<dyn Backend>>,
        msg_tx: &mut mpsc::Sender<Message>,
    ) -> Result<(), String> {
        let _ = msg_tx.send(Message::PendingProgress(id, 0.0)).await;

        match self {
            Self::Install {
                backend_name,
                package_id,
            } => match backends.get(backend_name) {
                Some(backend) => {
                    /*
                    tokio::task::spawn_blocking(move || {
                        backend.install(package_id, |progress| {
                            let _ = msg_tx.send(Message::PendingProgress(id, i as _)).await;
                        })
                    })
                    .await
                    .map_err(err_str)?;
                    */
                    log::warn!("TODO: INSTALL {package_id} from {backend_name}");
                    for i in 1..100 {
                        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
                        let _ = msg_tx.send(Message::PendingProgress(id, i as _)).await;
                    }
                }
                None => {
                    Err(format!("backend {:?} not found", backend_name))?;
                }
            },
        }

        let _ = msg_tx.send(Message::PendingProgress(id, 100.0)).await;

        Ok(())
    }
}
