use log::{LevelFilter, Log, Metadata, Record};

pub fn init() {
    log::set_logger(&LOGGER).expect("logging already initialized");
    log::set_max_level(LevelFilter::Debug);
}

static LOGGER: Logger = Logger;

struct Logger;

impl Log for Logger {
    fn enabled(&self, _metadata: &Metadata<'_>) -> bool {
        true
    }

    fn log(&self, record: &Record<'_>) {
        if let Some(file) = record.module_path() {
            println!("[{} {}] {}", record.level(), file, record.args());
        } else {
            println!("[{}] {}", record.level(), record.args());
        }
    }

    fn flush(&self) {}
}
