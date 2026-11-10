//! Keyboard Layout Fixer — pure code, no Windows calls: tested on Linux with `cargo test`.
pub mod bubble;
pub mod convert;
pub mod diag;
pub mod flow;
pub mod i18n;
pub mod indicator;
pub mod layouts;
pub mod ngram;
pub mod policy;
pub mod profile;
pub mod school;
pub mod settings;
pub mod sound;
pub mod replace;
pub mod report;
pub mod typing;
pub mod update;
pub mod words;

pub const VERSION: &str = env!("CARGO_PKG_VERSION");
