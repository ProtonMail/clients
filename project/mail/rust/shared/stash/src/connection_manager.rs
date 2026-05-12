use crate::stash::{
    PooledTether, PooledTetherInterruptNotifier, TracedOperation, spawn_read_worker,
};
use parking_lot::Mutex;
pub use rusqlite;
use rusqlite::{Connection, Error, InterruptHandle, OpenFlags};
use sqlite_watcher::connection::State;
use sqlite_watcher::statement::Statement;
use sqlite_watcher::watcher::Watcher;
use std::path::PathBuf;
use std::sync::Arc;
use tempfile::TempDir;
use tracing::{Span, debug, info};

const DEFAULT_READ_WORKERS: usize = 8;
const READ_CHANNEL_CAPACITY: usize = 32;

#[derive(Debug)]
enum Source {
    File(PathBuf),
    TmpFile(TempDir),
}

type InitFn = dyn Fn(&mut Connection) -> Result<(), Error> + Send + Sync + 'static;

pub struct StashConnectionPool {
    pub(crate) ro_sender: flume::Sender<TracedOperation>,
    interrupts: Vec<InterruptData>,
    interrupted: Mutex<bool>,
    span: Span,
    pub(crate) write_worker: PooledTether,
    write_worker_interrupt: InterruptData,
    /// Keeps the temp directory alive for the lifetime of the pool
    _source: Source,
}

impl Drop for StashConnectionPool {
    fn drop(&mut self) {
        let _entered = self.span.enter();

        debug!("Connection pool dropped");
    }
}

impl StashConnectionPool {
    pub fn file<P: Into<PathBuf>>(
        path: P,
        read_worker_count: Option<usize>,
        init_fn: Box<InitFn>,
        watcher: &Arc<Watcher>,
    ) -> Result<Arc<Self>, Error> {
        Self::new(
            Source::File(path.into()),
            read_worker_count,
            init_fn,
            watcher,
        )
    }

    pub fn tmp_file(
        read_worker_count: Option<usize>,
        init_fn: Box<InitFn>,
        watcher: &Arc<Watcher>,
    ) -> Result<Arc<Self>, Error> {
        let tmp_dir = TempDir::new().expect("failed to create temp dir");
        Self::new(
            Source::TmpFile(tmp_dir),
            read_worker_count,
            init_fn,
            watcher,
        )
    }

    fn new(
        source: Source,
        read_worker_count: Option<usize>,
        init_fn: Box<InitFn>,
        watcher: &Arc<Watcher>,
    ) -> Result<Arc<Self>, Error> {
        let read_worker_count = read_worker_count.unwrap_or_else(|| {
            std::thread::available_parallelism()
                .map(|n| n.get().saturating_sub(1).max(1))
                .unwrap_or(DEFAULT_READ_WORKERS)
        });

        let (ro_sender, ro_receiver) = flume::bounded(READ_CHANNEL_CAPACITY);

        let mut interrupts = Vec::with_capacity(read_worker_count);
        for i in 0..read_worker_count {
            let conn = Self::create_connection(&source, &init_fn, OpenFlags::default())?;
            let handle = spawn_read_worker(conn, ro_receiver.clone(), watcher, i);
            interrupts.push(InterruptData {
                handle,
                interrupt_notifier: PooledTetherInterruptNotifier::new(ro_sender.clone()),
            });
        }

        let write_conn = Self::create_connection(&source, &init_fn, OpenFlags::default())?;
        let write_handle = write_conn.get_interrupt_handle();
        let write_worker = PooledTether::new(write_conn, watcher, read_worker_count);
        let write_worker_interrupt = InterruptData {
            handle: write_handle,
            interrupt_notifier: write_worker.interrupt_notifier(),
        };

        Ok(Arc::new(Self {
            _source: source,
            ro_sender,
            interrupts,
            interrupted: Default::default(),
            span: Span::current(),
            write_worker,
            write_worker_interrupt,
        }))
    }

    /// Interrupt all ongoing queries and rollback any active transactions.
    ///
    /// This method is useful on iOS to ensure that the db file locks are released and no new
    /// transaction is started until `resume()` is called.
    pub fn interrupt(&self) {
        let was_interrupted = {
            let mut interrupted = self.interrupted.lock();
            let old_value = *interrupted;
            *interrupted = true;
            drop(interrupted);
            old_value
        };

        if !was_interrupted {
            info!("Interrupting mail_stash");
            for handle in &self.interrupts {
                handle.interrupt()
            }
            self.write_worker_interrupt.interrupt();
        }
    }

    /// Resume execution and allow the tethers to proceed.
    pub fn resume(&self) {
        info!("Resuming mail_stash");
        let mut is_interrupted = self.interrupted.lock();
        *is_interrupted = false;
    }

    fn create_connection(
        source: &Source,
        init_fn: &InitFn,
        flags: OpenFlags,
    ) -> Result<Connection, Error> {
        match source {
            Source::File(path) => Connection::open_with_flags(path, flags),
            Source::TmpFile(tmp) => Connection::open_with_flags(tmp.path().join("test"), flags),
        }
        .and_then(|mut c| {
            (init_fn)(&mut c)?;
            State::set_pragmas().execute(&c)?;
            State::start_tracking().execute(&c)?;
            Ok(c)
        })
    }
}

struct InterruptData {
    handle: InterruptHandle,
    interrupt_notifier: PooledTetherInterruptNotifier,
}

impl InterruptData {
    fn interrupt(&self) {
        self.handle.interrupt();
        self.interrupt_notifier.interrupt();
    }
}
