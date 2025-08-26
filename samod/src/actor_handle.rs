use std::sync::{Arc, Mutex};

use crate::{ActorTask, DocActorInner, DocHandle};

// Trait for sending actor tasks across different channel types
pub(crate) trait ActorSender: Send + Sync {
    fn send(&self, task: ActorTask) -> Result<(), std::sync::mpsc::SendError<ActorTask>>;
}

// Implementation for std::sync::mpsc channels
impl ActorSender for std::sync::mpsc::Sender<ActorTask> {
    fn send(&self, task: ActorTask) -> Result<(), std::sync::mpsc::SendError<ActorTask>> {
        self.send(task)
    }
}

pub(crate) struct ActorHandle {
    #[allow(dead_code)]
    pub(crate) inner: Arc<Mutex<DocActorInner>>,
    pub(crate) tx: Box<dyn ActorSender>,
    pub(crate) doc: DocHandle,
}
