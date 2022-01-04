use glib::Sender;
use gtk::glib;
use once_cell::sync::Lazy;
use once_cell::sync::OnceCell;
use std::cell::RefCell;
use std::fmt;
use std::sync::{Arc, Mutex};

#[derive(Default)]
struct Logger {
    pub log_sender: OnceCell<Arc<Mutex<RefCell<Sender<String>>>>>,
    pub log_level: OnceCell<LogLevel>,
}

#[derive(Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum LogLevel {
    Error,
    _Warning,
    Info,
    _Log,
    Debug,
}
impl fmt::Display for LogLevel {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}

#[macro_export]
macro_rules! GPS_ERROR (
    () => ($crate::print!("\n"));
    ($($arg:tt)*) => ({
        logger::print_log(logger::LogLevel::Error, format_args!($($arg)*).to_string());
    })
);

#[macro_export]
macro_rules! GPS_WARN (
    () => ($crate::print!("\n"));
    ($($arg:tt)*) => ({
        logger::print_log(logger::LogLevel::Warning, format_args!($($arg)*).to_string());
    })
);

#[macro_export]
macro_rules! GPS_INFO (
    () => ($crate::print!("\n"));
    ($($arg:tt)*) => ({
        logger::print_log(logger::LogLevel::Info, format_args!($($arg)*).to_string());
    })
);

#[macro_export]
macro_rules! GPS_LOG (
    () => ($crate::print!("\n"));
    ($($arg:tt)*) => ({
        logger::print_log(logger::LogLevel::Log, format_args!($($arg)*).to_string());
    })
);

#[macro_export]
macro_rules! GPS_DEBUG (
    () => ($crate::print!("\n"));
    ($($arg:tt)*) => ({
        logger::print_log(logger::LogLevel::Debug, format_args!($($arg)*).to_string());
    })
);

static LOGGER: Lazy<Logger> = Lazy::new(Logger::default);

pub fn init_logger(sender: Sender<String>, log_level: LogLevel) {
    LOGGER
        .log_sender
        .set(Arc::new(Mutex::new(RefCell::new(sender))))
        .expect("init logger should be called once");
    let _ = LOGGER.log_level.set(log_level);
}

pub fn print_log(log_level: LogLevel, msg: String) {
    if log_level
        <= *LOGGER
            .log_level
            .get()
            .expect("Logger should be initialized before calling print_log")
    {
        let mut sender = LOGGER
            .log_sender
            .get()
            .expect("Logger should be initialized before calling print_log")
            .lock()
            .expect("guarded");

        if let Err(e) = sender.get_mut().send(format!("{}:{}", log_level, msg)) {
            println!("Error: {}", e)
        };
    }
}
