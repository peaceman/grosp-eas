use futures::future::FutureObj;
use futures::task::{Spawn, SpawnError};

pub mod node_groups;

#[derive(Debug, Copy, Clone, Default)]
pub struct Runtime;

impl Spawn for Runtime {
    fn spawn_obj(&self, future: FutureObj<'static, ()>) -> Result<(), SpawnError> {
        tokio::spawn(future);
        Ok(())
    }

    fn status(&self) -> Result<(), SpawnError> {
        Ok(())
    }
}
