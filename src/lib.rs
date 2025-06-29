pub mod ast;
pub mod cli;
pub mod config;
pub mod languages;
pub mod processor;
pub mod rules;

pub use processor::{ProcessingOptions, Processor};
pub use rules::preservation::PreservationRule;

use std::error::Error;
use std::fmt;

#[derive(Debug)]
pub enum UncommentError {
    Io(std::io::Error),
    ParseError(String),
    LanguageNotSupported(String),
    TreeSitterError(String),
}

impl fmt::Display for UncommentError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            UncommentError::Io(err) => write!(f, "IO error: {err}"),
            UncommentError::ParseError(msg) => write!(f, "Parse error: {msg}"),
            UncommentError::LanguageNotSupported(lang) => {
                write!(f, "Language not supported: {lang}")
            }
            UncommentError::TreeSitterError(msg) => write!(f, "Tree-sitter error: {msg}"),
        }
    }
}

impl Error for UncommentError {}

impl From<std::io::Error> for UncommentError {
    fn from(err: std::io::Error) -> Self {
        UncommentError::Io(err)
    }
}

pub type Result<T> = std::result::Result<T, UncommentError>;
