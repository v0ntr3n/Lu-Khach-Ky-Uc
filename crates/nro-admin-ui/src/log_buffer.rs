//! Thread-safe in-memory ring buffer capturing `tracing` log output for the GUI Logs tab.

use std::collections::VecDeque;
use std::io::{self, Write};
use std::sync::{Arc, Mutex};

use tracing_subscriber::fmt::MakeWriter;

const MAX_LOG_LINES: usize = 1000;

/// Parsed severity level of a formatted `tracing` log line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LogLevel {
    Error,
    Warn,
    Info,
    Debug,
}

impl LogLevel {
    fn parse(line: &str) -> Self {
        if line.contains(" ERROR ") {
            Self::Error
        } else if line.contains(" WARN ") {
            Self::Warn
        } else if line.contains(" INFO ") {
            Self::Info
        } else if line.contains(" DEBUG ") || line.contains(" TRACE ") {
            Self::Debug
        } else {
            Self::Info
        }
    }

    pub(crate) const fn is_warn_or_error(self) -> bool {
        matches!(self, Self::Error | Self::Warn)
    }
}

/// Single captured server log line with monotonic sequence number.
#[derive(Debug, Clone)]
pub(crate) struct LogEntry {
    pub seq: u64,
    pub level: LogLevel,
    pub text: String,
}

struct LogRing {
    entries: VecDeque<LogEntry>,
    latest_seq: u64,
    warn_err_count: usize,
}

/// Cloneable thread-safe ring buffer implementing `tracing_subscriber::fmt::MakeWriter`.
#[derive(Clone)]
pub(crate) struct LogBuffer {
    inner: Arc<Mutex<LogRing>>,
}

impl LogBuffer {
    pub(crate) fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(LogRing {
                entries: VecDeque::with_capacity(MAX_LOG_LINES),
                latest_seq: 0,
                warn_err_count: 0,
            })),
        }
    }

    pub(crate) fn push_line(&self, raw: &str) {
        let trimmed = raw.trim_end_matches('\r');
        if trimmed.is_empty() {
            return;
        }
        let Ok(mut ring) = self.inner.lock() else {
            return;
        };
        if ring.entries.len() >= MAX_LOG_LINES
            && let Some(evicted) = ring.entries.pop_front()
            && evicted.level.is_warn_or_error()
        {
            ring.warn_err_count = ring.warn_err_count.saturating_sub(1);
        }
        let level = LogLevel::parse(trimmed);
        if level.is_warn_or_error() {
            ring.warn_err_count = ring.warn_err_count.saturating_add(1);
        }
        ring.latest_seq = ring.latest_seq.wrapping_add(1);
        let seq = ring.latest_seq;
        ring.entries.push_back(LogEntry {
            seq,
            level,
            text: trimmed.to_owned(),
        });
    }

    /// Returns `(latest_seq, entries, warn_or_error_count)` only if new logs or a clear occurred.
    pub(crate) fn snapshot_if_changed(&self, last_seq: u64) -> Option<(u64, Vec<LogEntry>, usize)> {
        let ring = self.inner.lock().ok()?;
        if ring.latest_seq == last_seq {
            return None;
        }
        Some((
            ring.latest_seq,
            ring.entries.iter().cloned().collect(),
            ring.warn_err_count,
        ))
    }

    pub(crate) fn clear(&self) {
        if let Ok(mut ring) = self.inner.lock() {
            ring.entries.clear();
            ring.warn_err_count = 0;
            ring.latest_seq = ring.latest_seq.wrapping_add(1);
        }
    }
}

/// Per-event `std::io::Write` adapter splitting output on `\n` into [`LogBuffer`].
pub(crate) struct LogWriter {
    buffer: LogBuffer,
    pending: Vec<u8>,
}

impl Write for LogWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.pending.extend_from_slice(buf);
        while let Some(pos) = self.pending.iter().position(|&b| b == b'\n') {
            let line_bytes: Vec<u8> = self.pending.drain(..=pos).collect();
            let line = String::from_utf8_lossy(&line_bytes[..line_bytes.len().saturating_sub(1)]);
            self.buffer.push_line(&line);
        }
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        if !self.pending.is_empty() {
            let leftover = std::mem::take(&mut self.pending);
            let text = String::from_utf8_lossy(&leftover);
            for line in text.split('\n') {
                self.buffer.push_line(line);
            }
        }
        Ok(())
    }
}

impl Drop for LogWriter {
    fn drop(&mut self) {
        let _ = self.flush();
    }
}

impl<'a> MakeWriter<'a> for LogBuffer {
    type Writer = LogWriter;

    fn make_writer(&'a self) -> Self::Writer {
        LogWriter {
            buffer: self.clone(),
            pending: Vec::new(),
        }
    }
}
