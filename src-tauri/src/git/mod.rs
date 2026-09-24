pub mod commands;
mod history;
mod ops;
mod parse;
mod partial;
mod runner;
mod setup;
#[cfg(test)]
mod testing;

pub use runner::GitConfig;
