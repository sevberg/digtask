use async_process::Command;

use super::run_context::RunContext;

pub fn default_false() -> bool {
    false
}

pub fn contextualize_command(command: &mut Command, context: &RunContext) {
    match &context.env {
        None => (),
        Some(envmap) => {
            command.envs(envmap);
        }
    }
    match &context.dir {
        None => (),
        Some(dir) => {
            command.current_dir(dir);
        }
    }
}
