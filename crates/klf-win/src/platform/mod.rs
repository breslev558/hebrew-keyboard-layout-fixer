//! The ONLY place with unsafe Win32 calls. Every function returns a value; none panics.
pub mod clipboard;
pub mod console;
pub mod input;
pub mod layouts;
pub mod school;
pub mod sys;
pub mod target;
pub mod uia;
