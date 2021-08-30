//! Logging module.
use std::io;

use colored::*;
use log::{Level, Log, Metadata, Record, SetLoggerError};

struct Logger {
    level: Level,
    target: &'static str,
}

impl Log for Logger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        metadata.level() <= self.level
    }

    fn log(&self, record: &Record) {
        let metadata = record.metadata();
        let is_native = self.target == record.target();

        // When using the "info" level, ignore all logs from other targets.
        if metadata.level() == Level::Info && !is_native {
            return;
        }

        if self.enabled(metadata) {
            if record.level() == Level::Error {
                write(record, record.target(), is_native, io::stderr());
            } else {
                write(record, record.target(), is_native, io::stdout());
            }

            fn write(
                record: &log::Record,
                target: &str,
                is_native: bool,
                mut stream: impl io::Write,
            ) {
                let msg = record.args().to_string();

                let message = if is_native {
                    match record.level() {
                        Level::Error => format!("== {}", msg).red().bold(),
                        Level::Warn => format!("{} {}", "=>".blue(), msg).yellow(),
                        Level::Info => format!("{} {}", "=>".blue(), msg.normal().bold()).normal(),
                        Level::Debug => format!("{} {}", "=>".blue(), msg).dimmed(),
                        Level::Trace => format!("=> {}", msg).white().dimmed(),
                    }
                } else {
                    let msg = format!("** {} ({})", msg, target);

                    match record.level() {
                        Level::Error => msg.red(),
                        Level::Warn => msg.yellow(),
                        Level::Info => msg.normal(),
                        Level::Debug => msg.dimmed(),
                        Level::Trace => msg.white().dimmed(),
                    }
                };
                writeln!(stream, "{}", message).ok();
            }
        }
    }

    fn flush(&self) {}
}

/// Initialize a new logger.
pub fn init(target: &'static str) -> Result<(), SetLoggerError> {
    let level = log::Level::Debug;
    let logger = Logger { level, target };

    log::set_boxed_logger(Box::new(logger))?;

    Ok(())
}

/// Set the maximum log level.
pub fn set_level(level: log::Level) {
    log::set_max_level(level.to_level_filter());
}
