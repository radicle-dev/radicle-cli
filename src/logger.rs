//! Logging module.
use std::io;

use colored::*;
use log::{Level, Log, Metadata, Record, SetLoggerError};

struct Logger {
    level: Level,
    targets: Vec<&'static str>,
}

impl Log for Logger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        metadata.level() <= self.level
    }

    fn log(&self, record: &Record) {
        if !self.targets.contains(&record.target()) {
            return;
        }

        if self.enabled(record.metadata()) {
            let module = record.module_path().unwrap_or_default();

            if record.level() == Level::Error {
                write(record, module, io::stderr());
            } else {
                write(record, module, io::stdout());
            }

            fn write(record: &log::Record, _module: &str, mut stream: impl io::Write) {
                let msg = record.args().to_string();
                let message = match record.level() {
                    Level::Error => format!("== {}", msg).red().bold(),
                    Level::Warn => format!("{} {}", "=>".blue(), msg).yellow(),
                    Level::Info => format!("{} {}", "=>".blue(), msg.normal().bold()).normal(),
                    Level::Debug => format!("{} {}", "=>".blue(), msg).dimmed(),
                    Level::Trace => format!("=> {}", msg).white().dimmed(),
                };
                writeln!(stream, "{}", message).ok();
            }
        }
    }

    fn flush(&self) {}
}

/// Initialize a new logger.
pub fn init(level: Level, targets: Vec<&'static str>) -> Result<(), SetLoggerError> {
    let logger = Logger { level, targets };

    log::set_boxed_logger(Box::new(logger))?;
    log::set_max_level(level.to_level_filter());

    Ok(())
}
