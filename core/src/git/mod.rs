//! Integrazione Git: esegue l'eseguibile git reale e ne interpreta l'output. Nessuna dipendenza dalla UI.
pub mod changes;
pub mod graph;
pub mod history;
pub mod ops;
pub mod parse;
pub mod partial;
pub mod reword;
pub mod runner;
pub mod setup;
#[cfg(test)]
mod testing;
