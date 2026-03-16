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

#[cfg(test)]
mod tests {
    use super::*;

    // Helper function: Initialize a test environment with 3 windows.
    // Screen width: 1920, Window width: 800, Gap size: 20
    fn setup_test_env() -> (Workspace<u32>, HashMap<u32, Window<u32>>) {
        let mut ws = Workspace::new();
        let mut db = HashMap::new();

        for id in 1..=3 {
            db.insert(id, Window::new(id, 800.0, 1000.0));
            ws.windows.push(id);
        }
        ws.focus_stack.push(1); // Default focus on window 1
        ws.gap_size = 20.0;

        (ws, db)
    }

    #[test]
    fn test_world_coordinates_calculation() {
        let (mut ws, mut db) = setup_test_env();
        recalculate(&mut ws, &mut db, 1920.0);

        // Verify absolute physical coordinates (World X)
        // Window 1: 0.0
        // Window 2: 800 + 20 = 820.0
        // Window 3: 820 + 800 + 20 = 1640.0
        assert_eq!(db.get(&1).unwrap().world_x, 0.0);
        assert_eq!(db.get(&2).unwrap().world_x, 820.0);
        assert_eq!(db.get(&3).unwrap().world_x, 1640.0);
    }

    #[test]
    fn test_camera_mode_always_center() {
        let (mut ws, mut db) = setup_test_env();
        ws.centering_mode = FocusCenteringMode::Always;

        // Focus on the middle window 2 (world_x = 820.0)
        ws.focus_stack.push(2);
        recalculate(&mut ws, &mut db, 1920.0);

        // Center of window 2: 820 + (800 / 2) = 1220
        // Center of screen: 1920 / 2 = 960
        // Target camera position: 1220 - 960 = 260
        assert_eq!(ws.camera_x, 260.0);

        // Verify screen projected coordinates (Screen X) = world_x - camera_x
        assert_eq!(db.get(&2).unwrap().screen_x, 820.0 - 260.0);
    }

    #[test]
    fn test_camera_mode_never_and_overflow() {
        let (mut ws, mut db) = setup_test_env();

        // Test Never mode: The camera should remain idle when the window is fully visible on screen.
        ws.centering_mode = FocusCenteringMode::Never;
        ws.camera_x = 0.0;
        ws.focus_stack.push(2); // Window 2 (820.0 -> 1620.0) is fully within 0~1920
        recalculate(&mut ws, &mut db, 1920.0);
        assert_eq!(
            ws.camera_x, 0.0,
            "Camera should not move when fully visible"
        );

        // Test OnOverflow mode: Window overflows the right edge.
        ws.centering_mode = FocusCenteringMode::OnOverflow;
        ws.focus_stack.push(3); // Window 3 (1640.0 -> 2440.0) overflows the right edge (1920)
        recalculate(&mut ws, &mut db, 1920.0);

        // Overflow detected, forcefully center window 3: 1640 + 400 - 960 = 1080
        assert_eq!(
            ws.camera_x, 1080.0,
            "Camera should force center after overflow"
        );
    }
}
