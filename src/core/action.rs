// src/core/action.rs

use crate::config;
use crate::core::repeat;
use crate::state::{AppData, TimerCommand};
use wayland_client::backend::ObjectId;

pub fn execute_config_action(state: &mut AppData, object_id: ObjectId, action: config::Action) {
    match action {
        config::Action::Spawn { args } => {
            if !args.is_empty() {
                let mut command = std::process::Command::new(&args[0]);
                if args.len() > 1 {
                    command.args(&args[1..]);
                }
                if let Err(e) = command.spawn() {
                    eprintln!("Failed to spawn {:?}: {}", args, e);
                }
            }
        }
        config::Action::CloseWindow => {
            if let Some(output) = state.shuttle.outputs.get(&1) {
                if let Some(ws) = output.workspaces.get(&output.active_workspace_id) {
                    if let Some(focused) = ws.focused_window() {
                        if let Some(proxy) = state.window_proxies.get(&focused) {
                            proxy.close();
                        }
                    }
                }
            }
        }
        config::Action::FocusLeft
        | config::Action::FocusRight
        | config::Action::MoveLeft
        | config::Action::MoveRight => {
            let repeat_action = match action {
                config::Action::FocusLeft => repeat::Action::FocusLeft,
                config::Action::FocusRight => repeat::Action::FocusRight,
                config::Action::MoveLeft => repeat::Action::MoveLeft,
                config::Action::MoveRight => repeat::Action::MoveRight,
                _ => unreachable!(),
            };

            execute_repeat_action(state, repeat_action.clone());
            let _ = state
                .timer_tx
                .send(TimerCommand::StartRepeat(object_id, repeat_action));
        }
        config::Action::FocusWorkspace { target } => {
            if let Some(output) = state.shuttle.outputs.get_mut(&1) {
                output.switch_workspace(target);
                state.request_manage();
            }
        }
        config::Action::MoveToWorkspace { target } => {
            if let Some(output) = state.shuttle.outputs.get_mut(&1) {
                output.move_focused_to_workspace(target);
                output.switch_workspace(target);
                state.request_manage();
            }
        }
        config::Action::Quit => {
            println!("Initiating graceful shutdown...");
            std::process::Command::new("riverctl")
                .arg("exit")
                .spawn()
                .ok();
            state.loop_signal.stop();
        }
    }
}

pub fn execute_repeat_action(state: &mut AppData, action: repeat::Action) {
    let output_id = 1;
    let needs_layout = match action {
        repeat::Action::FocusLeft => {
            state
                .shuttle
                .outputs
                .get_mut(&output_id)
                .unwrap()
                .current_workspace_mut()
                .cycle_focus(-1);
            true
        }
        repeat::Action::FocusRight => {
            state
                .shuttle
                .outputs
                .get_mut(&output_id)
                .unwrap()
                .current_workspace_mut()
                .cycle_focus(1);
            true
        }
        repeat::Action::MoveLeft => {
            state
                .shuttle
                .outputs
                .get_mut(&output_id)
                .unwrap()
                .current_workspace_mut()
                .move_focused_window(-1);
            true
        }
        repeat::Action::MoveRight => {
            state
                .shuttle
                .outputs
                .get_mut(&output_id)
                .unwrap()
                .current_workspace_mut()
                .move_focused_window(1);
            true
        }
    };

    if needs_layout {
        crate::layout::engine::update_layout(&mut state.shuttle, output_id, &state.config);
        state.request_manage();
    }
}

impl repeat::ExecuteAction for AppData {
    fn execute_action(&mut self, action: repeat::Action) {
        execute_repeat_action(self, action);
    }
}
