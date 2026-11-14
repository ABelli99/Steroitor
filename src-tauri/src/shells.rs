use portable_pty::CommandBuilder;
use steroitor_core::shells::{detect, Shell};

pub use steroitor_core::shells::resolve;

pub fn command(shell: &Shell) -> CommandBuilder {
    let mut command = CommandBuilder::new(&shell.program);
    command.args(&shell.args);
    command
}

#[tauri::command]
pub fn terminal_shells() -> Vec<Shell> {
    detect()
}
