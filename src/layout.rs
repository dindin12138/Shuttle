// src/layout.rs

use crate::state::{Window, WindowId, Workspace};
use std::collections::HashMap;

/// Defines the camera tracking behavior when focusing on a window.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FocusCenteringMode {
    /// Always keep the focused window in the absolute center of the screen.
    Always,
    /// Only move the camera the minimum amount necessary to keep the focused window visible.
    Never,
    /// Do not move the camera if the window is fully visible. Center it only if it overflows the screen.
    OnOverflow,
}

/// Core layout engine.
///
/// Calculates the world coordinates of all windows in the workspace and
/// determines the appropriate camera position based on the centering mode.
pub fn recalculate<ID: WindowId>(
    workspace: &mut Workspace<ID>,
    window_db: &mut HashMap<ID, Window<ID>>,
    screen_width: f32,
) {
    if workspace.windows.is_empty() {
        return;
    }

    let gap = workspace.gap_size;

    // Step 1: Calculate physical world coordinates for all windows sequentially
    let mut current_x = 0.0;
    for id in &workspace.windows {
        if let Some(window) = window_db.get_mut(id) {
            window.world_x = current_x;
            current_x += window.width + gap;
        }
    }

    // Step 2: Smart camera tracking based on the active centering mode
    let focused_id = match workspace.focused_window() {
        Some(id) => id,
        None => return,
    };

    // Extract the absolute position and width of the currently focused window
    let (fw_x, fw_w) = if let Some(fw) = window_db.get(&focused_id) {
        (fw.world_x, fw.width)
    } else {
        return;
    };

    let cur_cam = workspace.camera_x;

    // Determine the target camera position based on the selected layout strategy
    let target_cam = match workspace.centering_mode {
        FocusCenteringMode::Always => fw_x + (fw_w / 2.0) - (screen_width / 2.0),

        FocusCenteringMode::Never => {
            if fw_x < cur_cam {
                fw_x // Align to the left edge of the screen
            } else if fw_x + fw_w > cur_cam + screen_width {
                fw_x + fw_w - screen_width // Align to the right edge of the screen
            } else {
                cur_cam // Window is fully visible, no camera movement needed
            }
        }

        FocusCenteringMode::OnOverflow => {
            // If the window is fully within the screen boundaries, do nothing
            if fw_x >= cur_cam && fw_x + fw_w <= cur_cam + screen_width {
                cur_cam
            } else {
                // Window is partially or fully outside the screen, bring it to the absolute center
                fw_x + (fw_w / 2.0) - (screen_width / 2.0)
            }
        }
    };

    workspace.target_camera_x = target_cam;

    // Instantly snap the camera to the target position
    // Note: Smooth spring animation logic will replace this instant snap in later phases
    workspace.camera_x = target_cam;

    // Step 3: Screen projection
    // Map the absolute world coordinates to the final screen coordinates relative to the camera
    for id in &workspace.windows {
        if let Some(window) = window_db.get_mut(id) {
            let screen_x = window.world_x - workspace.camera_x;
            window.screen_x = screen_x;
        }
    }
}
