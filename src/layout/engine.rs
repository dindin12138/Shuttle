// src/layout/engine.rs

use crate::config::{Config, FocusCenteringMode};
use crate::state::{Shuttle, WindowId};

pub fn update_layout<ID: WindowId>(shuttle: &mut Shuttle<ID>, output_id: u32, config: &Config) {
    let gap = config.layout.gaps;
    let screen_width = config.output.width;
    let screen_height = config.output.height;
    let available_width = screen_width - (gap * 2.0);
    let default_prop = config.layout.default_column_width.proportion;

    if let Some(output) = shuttle.outputs.get_mut(&output_id) {
        if let Some(workspace) = output.workspaces.get_mut(&output.active_workspace_id) {
            let mut current_x = gap;

            for id in &workspace.windows {
                if let Some(window) = shuttle.window_db.get_mut(id) {
                    let prop = window.custom_proportion.unwrap_or(default_prop);

                    window.width = available_width * prop;
                    window.height = screen_height - (gap * 2.0);
                    window.world_x = current_x;
                    current_x += window.width + gap;
                }
            }

            if let Some(focused_id) = workspace.focused_window() {
                if let Some(window) = shuttle.window_db.get(&focused_id) {
                    let center_mode = config.layout.center_focused_column;
                    let available_width = screen_width - (gap * 2.0);

                    match center_mode {
                        FocusCenteringMode::Always => {
                            workspace.camera_x =
                                window.world_x + (window.width / 2.0) - (screen_width / 2.0);
                        }
                        FocusCenteringMode::Never => {
                            if window.world_x < workspace.camera_x + gap {
                                workspace.camera_x = window.world_x - gap;
                            } else if window.world_x + window.width
                                > workspace.camera_x + screen_width - gap
                            {
                                workspace.camera_x =
                                    window.world_x + window.width - screen_width + gap;
                            }
                        }
                        FocusCenteringMode::OnOverflow => {
                            let is_fully_visible = window.world_x >= workspace.camera_x + gap
                                && (window.world_x + window.width)
                                    <= (workspace.camera_x + screen_width - gap);

                            if window.width > available_width || !is_fully_visible {
                                workspace.camera_x =
                                    window.world_x + (window.width / 2.0) - (screen_width / 2.0);
                            }
                        }
                    }
                }
            }

            for id in &workspace.windows {
                if let Some(window) = shuttle.window_db.get_mut(id) {
                    window.screen_x = window.world_x - workspace.camera_x;
                }
            }
        }
    }
}
