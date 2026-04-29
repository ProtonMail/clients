use tokio::sync::oneshot;

/// Caller-side handle to request worker shutdown.
pub struct WorkerShutdownHandle {
    tx: Option<oneshot::Sender<()>>,
}

/// Worker-side shutdown signal.
pub struct WorkerShutdownSignal {
    rx: oneshot::Receiver<()>,
}

impl WorkerShutdownHandle {
    /// Build a paired shutdown handle/signal.
    #[must_use]
    pub fn pair() -> (Self, WorkerShutdownSignal) {
        let (tx, rx) = oneshot::channel();
        (Self { tx: Some(tx) }, WorkerShutdownSignal { rx })
    }

    /// Request shutdown. Returns `true` when signal was sent.
    pub fn request_shutdown(mut self) -> bool {
        self.tx.take().is_some()
    }
}

impl WorkerShutdownSignal {
    /// Completes when shutdown is requested or sender is dropped.
    pub async fn cancelled(&mut self) {
        let _ = (&mut self.rx).await;
    }
}
