// src/layout/engine.rs

use crate::config::{Config, FocusCenteringMode};
use crate::state::{Shuttle, WindowId};
use tracing::trace;

pub fn update_layout<ID: WindowId + std::fmt::Debug>(
    shuttle: &mut Shuttle<ID>,
    output_id: u32,
    config: &Config,
) {
    let gap = config.layout.gaps;
    let default_prop = config.layout.default_column_width.proportion;

    trace!(
        "--- Layout Engine Update Started | Output: {} ---",
        output_id
    );

    if let Some(output) = shuttle.outputs.get_mut(&output_id) {
        let screen_width = output
            .usable_area
            .map(|a| a.width as f32)
            .unwrap_or(config.output.width);
        let screen_height = output
            .usable_area
            .map(|a| a.height as f32)
            .unwrap_or(config.output.height);
        let screen_x_offset = output.usable_area.map(|a| a.x as f32).unwrap_or(0.0);

        let available_width = screen_width - (gap * 2.0);

        if let Some(workspace) = output.workspaces.get_mut(&output.active_workspace_id) {
            let mut current_x = gap;

            for id in &workspace.windows {
                if let Some(window) = shuttle.window_db.get_mut(id) {
                    let prop = window.custom_proportion.unwrap_or(default_prop);
                    let calculated_width = (prop * (available_width + gap)) - gap;
                    window.width = calculated_width.max(1.0);
                    window.height = screen_height - (gap * 2.0);
                    window.world_x = current_x;

                    trace!(
                        "Window {:?} layout -> width: {}, world_x: {}",
                        id, window.width, window.world_x
                    );

                    current_x += window.width + gap;
                }
            }

            if let Some(focused_id) = workspace.focused_window() {
                if let Some(window) = shuttle.window_db.get(&focused_id) {
                    let center_mode = config.layout.center_focused_column;

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

                            if window.width > available_width {
                                workspace.camera_x =
                                    window.world_x + (window.width / 2.0) - (screen_width / 2.0);
                            } else if !is_fully_visible {
                                if window.world_x < workspace.camera_x + gap {
                                    workspace.camera_x = window.world_x - gap;
                                } else if window.world_x + window.width
                                    > workspace.camera_x + screen_width - gap
                                {
                                    workspace.camera_x =
                                        window.world_x + window.width - screen_width + gap;
                                }
                            }
                        }
                    }

                    trace!(
                        "Camera panning resolved -> mode: {:?}, camera_x: {}",
                        center_mode, workspace.camera_x
                    );
                }
            }

            for id in &workspace.windows {
                if let Some(window) = shuttle.window_db.get_mut(id) {
                    window.screen_x = window.world_x - workspace.camera_x + screen_x_offset;
                }
            }

            trace!("--- Layout Engine Update Finished ---");
        }
    }
}
