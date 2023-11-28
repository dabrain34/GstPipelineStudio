// logger.rs
//
// Copyright 2022 Stéphane Cerveau <scerveau@collabora.com>
//
// This file is part of GstPipelineStudio
//
// SPDX-License-Identifier: GPL-3.0-only

use gtk::glib::Sender;
use log::{debug, error, info, trace, warn};
use simplelog::*;
use std::fmt;
use std::io;

use std::fs::File;

use chrono::Local;
use std::sync::Mutex;

lazy_static::lazy_static! {
    static ref MSG_LOGGER: Mutex<Option<MessageLogger>> = Mutex::new(None);
}

#[derive(Debug, Eq, Ord, PartialEq, PartialOrd)]

pub enum LogLevel {
    Off,
    Error,
    Warning,
    Info,
    Debug,
    Trace,
}

#[derive(Debug, Clone, PartialEq)]
pub enum LogType {
    App,
    Gst,
    Message,
}

impl LogLevel {
    pub fn from_u32(value: u32) -> LogLevel {
        match value {
            0 => LogLevel::Off,
            1 => LogLevel::Error,
            2 => LogLevel::Warning,
            3 => LogLevel::Info,
            4 => LogLevel::Debug,
            5 => LogLevel::Trace,
            _ => panic!("Unknown value: {}", value),
        }
    }
}

impl fmt::Display for LogLevel {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{self:?}")
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
macro_rules! GPS_DEBUG (
    () => ($crate::print!("\n"));
    ($($arg:tt)*) => ({
        logger::print_log(logger::LogLevel::Debug, format_args!($($arg)*).to_string());
    })
);

#[macro_export]
macro_rules! GPS_MSG_LOG (
    () => ($crate::print!("\n"));
    ($($arg:tt)*) => ({
        logger::pring_msg_logger(logger::LogType::Message, format_args!($($arg)*).to_string());
    })
);

#[macro_export]
macro_rules! GPS_GST_LOG (
    () => ($crate::print!("\n"));
    ($($arg:tt)*) => ({
        logger::pring_msg_logger(logger::LogType::Gst, format_args!($($arg)*).to_string());
    })
);

#[macro_export]
macro_rules! GPS_TRACE (
    () => ($crate::print!("\n"));
    ($($arg:tt)*) => ({
        logger::print_log(logger::LogLevel::Trace, format_args!($($arg)*).to_string());
    })
);

struct WriteAdapter {
    sender: Sender<(LogType, String)>,
    buffer: String,
}

impl io::Write for WriteAdapter {
    // On write we forward each u8 of the buffer to the sender and return the length of the buffer
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.buffer
            .push_str(&String::from_utf8(buf.to_vec()).unwrap());
        if self.buffer.ends_with('\n') {
            self.buffer.pop();
            self.sender
                .send((LogType::App, self.buffer.clone()))
                .unwrap();
            self.buffer = String::from("");
        }

        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

fn translate_to_simple_logger(log_level: LogLevel) -> LevelFilter {
    match log_level {
        LogLevel::Off => LevelFilter::Off,
        LogLevel::Error => LevelFilter::Error,
        LogLevel::Warning => LevelFilter::Warn,
        LogLevel::Info => LevelFilter::Info,
        LogLevel::Debug => LevelFilter::Debug,
        LogLevel::Trace => LevelFilter::Trace,
    }
}

pub fn init_logger(sender: Sender<(LogType, String)>, log_file: &str) {
    simplelog::CombinedLogger::init(vec![
        WriteLogger::new(
            translate_to_simple_logger(LogLevel::Trace),
            Config::default(),
            File::create(log_file).unwrap_or_else(|_| panic!("Unable to create log {}", log_file)),
        ),
        WriteLogger::new(
            translate_to_simple_logger(LogLevel::Debug),
            Config::default(),
            WriteAdapter {
                sender,
                buffer: String::from(""),
            },
        ),
        TermLogger::new(
            LevelFilter::Info,
            Config::default(),
            TerminalMode::Mixed,
            ColorChoice::Auto,
        ),
    ])
    .unwrap();
}

pub fn set_log_level(level: LogLevel) {
    log::set_max_level(translate_to_simple_logger(level));
}

pub fn print_log(log_level: LogLevel, msg: String) {
    match log_level {
        LogLevel::Error => {
            error!("{}", msg);
        }
        LogLevel::Warning => {
            warn!("{}", msg);
        }
        LogLevel::Info => {
            info!("{}", msg);
        }
        LogLevel::Debug => {
            debug!("{}", msg);
        }
        LogLevel::Trace => {
            trace!("{}", msg);
        }
        _ => {}
    };
}

#[derive(Debug, Clone)]
pub struct MessageLogger {
    sender: Sender<(LogType, String)>,
}

impl MessageLogger {
    pub fn new(sender: Sender<(LogType, String)>) -> Self {
        Self { sender }
    }

    pub fn print_log(&self, log_type: LogType, msg: String) {
        let to_send = format!("{}\t{}", Local::now().format("%H:%M:%S"), msg);
        self.sender.send((log_type.clone(), to_send)).unwrap();
    }
}

pub fn init_msg_logger(sender: Sender<(LogType, String)>) {
    let mut msg_logger = MSG_LOGGER.lock().unwrap();
    if msg_logger.is_none() {
        // Initialize the variable
        *msg_logger = Some(MessageLogger::new(sender));
    }
}

pub fn pring_msg_logger(log_type: LogType, msg: String) {
    let msg_logger = MSG_LOGGER.lock().unwrap();
    if let Some(logger) = msg_logger.as_ref() {
        logger.print_log(log_type, msg);
    }
}
