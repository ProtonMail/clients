use crate::stash::{PooledTether, TracedOperation, spawn_read_worker};
pub use rusqlite;
use rusqlite::{Connection, Error, OpenFlags};
use sqlite_watcher::connection::State;
use sqlite_watcher::statement::Statement;
use sqlite_watcher::watcher::Watcher;
use std::path::PathBuf;
use std::sync::Arc;
use tempfile::TempDir;
use tracing::{Span, debug};

const DEFAULT_READ_WORKERS: usize = 8;
const READ_CHANNEL_CAPACITY: usize = 32;

#[derive(Debug)]
enum Source {
    File(PathBuf),
    TmpFile(TempDir),
}

type InitFn = dyn Fn(&mut Connection) -> Result<(), Error> + Send + Sync + 'static;

pub struct InitFns {
    pub rw: Box<InitFn>,
    pub ro: Box<InitFn>,
}

const RO_OPEN_FLAGS: OpenFlags = OpenFlags::SQLITE_OPEN_READ_ONLY
    .union(OpenFlags::SQLITE_OPEN_NO_MUTEX)
    .union(OpenFlags::SQLITE_OPEN_URI);

pub struct StashConnectionPool {
    pub(crate) ro_sender: flume::Sender<TracedOperation>,
    span: Span,
    pub(crate) write_worker: PooledTether,
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
        init_fns: InitFns,
        watcher: &Arc<Watcher>,
    ) -> Result<Arc<Self>, Error> {
        Self::new(
            Source::File(path.into()),
            read_worker_count,
            init_fns,
            watcher,
        )
    }

    pub fn tmp_file(
        read_worker_count: Option<usize>,
        init_fns: InitFns,
        watcher: &Arc<Watcher>,
    ) -> Result<Arc<Self>, Error> {
        let tmp_dir = TempDir::new().expect("failed to create temp dir");
        Self::new(
            Source::TmpFile(tmp_dir),
            read_worker_count,
            init_fns,
            watcher,
        )
    }

    fn new(
        source: Source,
        read_worker_count: Option<usize>,
        init_fns: InitFns,
        watcher: &Arc<Watcher>,
    ) -> Result<Arc<Self>, Error> {
        let read_worker_count = read_worker_count.unwrap_or_else(|| {
            std::thread::available_parallelism()
                .map(|n| n.get().saturating_sub(1).max(1))
                .unwrap_or(DEFAULT_READ_WORKERS)
        });

        // Write connection MUST be opened (and initialized) first. It is the only
        // one with `SQLITE_OPEN_CREATE`, and it runs `PRAGMA journal_mode = WAL`
        // and `PRAGMA page_size = ...` — both of which need write access and only
        // take effect before the file has been touched. Opening an RO connection
        // first against a brand-new file would fail outright (no CREATE) or, in
        // theoretical edge cases where the file exists but is not yet in WAL mode,
        // lock in the wrong page size.
        let write_conn = Self::create_connection(&source, &init_fns.rw, OpenFlags::default())?;
        let write_worker = PooledTether::new(write_conn, watcher, read_worker_count);

        let (ro_sender, ro_receiver) = flume::bounded(READ_CHANNEL_CAPACITY);

        for i in 0..read_worker_count {
            let conn = Self::create_connection(&source, &init_fns.ro, RO_OPEN_FLAGS)?;
            spawn_read_worker(conn, ro_receiver.clone(), i);
        }

        Ok(Arc::new(Self {
            _source: source,
            ro_sender,
            span: Span::current(),
            write_worker,
        }))
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
