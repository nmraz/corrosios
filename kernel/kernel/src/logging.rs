use log::{LevelFilter, Log, Metadata, Record};

use crate::bootparse::CommandLine;

pub fn init(cmdline: CommandLine<'_>) {
    log::set_logger(&LOGGER).expect("logging already initialized");

    let level = get_log_level(cmdline).unwrap_or(LevelFilter::Info);
    log::set_max_level(level);
}

static LOGGER: Logger = Logger;

struct Logger;

impl Log for Logger {
    fn enabled(&self, _metadata: &Metadata<'_>) -> bool {
        true
    }

    fn log(&self, record: &Record<'_>) {
        if let Some(module) = record.module_path() {
            println!("[{} {}] {}", record.level(), module, record.args());
        } else {
            println!("[{}] {}", record.level(), record.args());
        }
    }

    fn flush(&self) {}
}

fn get_log_level(cmdline: CommandLine<'_>) -> Option<LevelFilter> {
    let level_str = cmdline.get_arg_str_value("loglevel")?;
    parse_log_level(level_str)
}

fn parse_log_level(level_str: &str) -> Option<LevelFilter> {
    match level_str {
        "trace" => Some(LevelFilter::Trace),
        "debug" => Some(LevelFilter::Debug),
        "info" => Some(LevelFilter::Info),
        "warn" => Some(LevelFilter::Warn),
        "error" => Some(LevelFilter::Error),
        "off" => Some(LevelFilter::Off),
        _ => None,
    }
}
