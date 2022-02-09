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

#[derive(Debug, Eq, Ord, PartialEq, PartialOrd)]

pub enum LogLevel {
    Error,
    Warning,
    Info,
    Debug,
    Trace,
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
macro_rules! GPS_DEBUG (
    () => ($crate::print!("\n"));
    ($($arg:tt)*) => ({
        logger::print_log(logger::LogLevel::Debug, format_args!($($arg)*).to_string());
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
    sender: Sender<String>,
    buffer: String,
}

impl io::Write for WriteAdapter {
    // On write we forward each u8 of the buffer to the sender and return the length of the buffer
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.buffer
            .push_str(&String::from_utf8(buf.to_vec()).unwrap());
        if self.buffer.ends_with('\n') {
            self.buffer.pop();
            self.sender.send(self.buffer.clone()).unwrap();
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
        LogLevel::Error => LevelFilter::Error,
        LogLevel::Warning => LevelFilter::Warn,
        LogLevel::Info => LevelFilter::Info,
        LogLevel::Debug => LevelFilter::Debug,
        LogLevel::Trace => LevelFilter::Trace,
    }
}

pub fn init_logger(sender: Sender<String>, log_file: &str) {
    simplelog::CombinedLogger::init(vec![
        WriteLogger::new(
            translate_to_simple_logger(LogLevel::Trace),
            Config::default(),
            File::create(log_file).unwrap(),
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
    };
}
