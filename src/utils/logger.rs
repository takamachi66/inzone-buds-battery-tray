use std::fs::{self, File, OpenOptions};
use std::io;
use std::sync::{Arc, Mutex};

use tracing_subscriber::fmt::writer::MakeWriter;

use crate::utils::paths::base_dir;

pub fn init_logger() -> anyhow::Result<()> {
    let log_path = base_dir().join("logs").join("app.log");
    if let Some(parent) = log_path.parent() {
        fs::create_dir_all(parent)?;
    }

    let file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(log_path)?;

    let writer = SharedFileWriter(Arc::new(Mutex::new(file)));

    tracing_subscriber::fmt()
        .with_ansi(false)
        .with_writer(writer)
        .with_target(false)
        .compact()
        .try_init()
        .map_err(|error| anyhow::anyhow!(error.to_string()))?;

    Ok(())
}

#[derive(Clone)]
struct SharedFileWriter(Arc<Mutex<File>>);

impl<'a> MakeWriter<'a> for SharedFileWriter {
    type Writer = SharedFileGuard;

    fn make_writer(&'a self) -> Self::Writer {
        SharedFileGuard(self.0.clone())
    }
}

struct SharedFileGuard(Arc<Mutex<File>>);

impl io::Write for SharedFileGuard {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.0
            .lock()
            .map_err(|_| io::Error::other("log file lock poisoned"))?
            .write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.0
            .lock()
            .map_err(|_| io::Error::other("log file lock poisoned"))?
            .flush()
    }
}
