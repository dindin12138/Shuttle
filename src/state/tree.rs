// src/state/tree.rs

use std::collections::HashMap;
use std::fmt::Debug;
use tracing::{debug, info, warn};

use crate::config::FocusCenteringMode;
use crate::state::window::{Window, WindowId};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct UsableArea {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

/// Workspace (The Canvas / Scroll).
///
/// This is the core concept of Shuttle. A Workspace acts as an infinitely long
/// horizontal scroll. It corresponds to Niri's concept of a workspace or
/// River's concept of a Tag.
#[derive(Debug, Clone)]
pub struct Workspace<ID: WindowId> {
    /// The sequential order of windows on the canvas.
    /// This determines the left-to-right layout arrangement.
    pub windows: Vec<ID>,

    /// The Most Recently Used (MRU) focus history stack.
    /// The top element is always the currently focused window.
    /// Used for smart focus restoration after a window is closed.
    pub focus_stack: Vec<ID>,

    /// The physical position of the camera's left edge.
    pub camera_x: f32,
    /// The target position the camera should pan to.
    pub target_camera_x: f32,
    /// The active tracking strategy for the camera.
    pub centering_mode: FocusCenteringMode,
    /// The gap size between windows in pixels.
    pub gap_size: f32,
}

impl<ID: WindowId> Workspace<ID> {
    /// Creates a new, empty workspace with default layout configurations.
    pub fn new() -> Self {
        Self {
            windows: Vec::new(),
            focus_stack: Vec::new(),
            camera_x: 0.0,
            target_camera_x: 0.0,
            centering_mode: FocusCenteringMode::OnOverflow,
            gap_size: 20.0,
        }
    }

    /// Retrieves the ID of the currently focused window.
    pub fn focused_window(&self) -> Option<ID> {
        self.focus_stack.last().cloned()
    }

    /// Inserts a new window into the workspace.
    pub fn insert_window(&mut self, id: ID, focus_immediately: bool) {
        if let Some(focus_id) = self.focused_window() {
            if let Some(idx) = self.windows.iter().position(|w| w == &focus_id) {
                // Insert immediately to the right of the currently focused window
                self.windows.insert(idx + 1, id.clone());
            } else {
                self.windows.push(id.clone());
            }
        } else {
            self.windows.push(id.clone());
        }

        debug!("Window inserted into workspace: {:?}", id);

        if focus_immediately {
            self.focus_window(id);
        } else {
            // If not focused, push it to the bottom of the MRU stack
            self.focus_stack.insert(0, id);
        }
    }

    /// Removes a window from both the layout list and the MRU focus stack.
    pub fn remove_window(&mut self, id: ID) {
        if let Some(idx) = self.windows.iter().position(|w| w == &id) {
            self.windows.remove(idx);
            debug!("Window removed from workspace layout: {:?}", id);
        }

        self.focus_stack.retain(|w| w != &id);
    }

    /// Focuses a specific window and updates the MRU stack.
    pub fn focus_window(&mut self, id: ID) {
        if !self.windows.contains(&id) {
            warn!(
                "Attempted to focus a window that does not exist in the current Workspace: {:?}",
                id
            );
            return;
        }

        self.focus_stack.retain(|w| w != &id);
        info!(
            "Workspace focus updated: {:?}, stack: {:?}",
            id, self.focus_stack
        );
        self.focus_stack.push(id);
    }

    /// Shifts the focus sequentially (left or right) through the window list.
    pub fn cycle_focus(&mut self, direction: i32) {
        if self.windows.is_empty() {
            return;
        }

        let current_focus = match self.focused_window() {
            Some(id) => id,
            None => return,
        };

        let current_idx = self
            .windows
            .iter()
            .position(|w| w == &current_focus)
            .unwrap();

        let len = self.windows.len() as i32;
        let new_idx = (current_idx as i32 + direction + len) % len;

        let new_focus_id = self.windows[new_idx as usize].clone();
        self.focus_window(new_focus_id);
    }

    pub fn move_focused_window(&mut self, direction: i32) {
        let focused_id = match self.focused_window() {
            Some(id) => id,
            None => return,
        };

        let current_idx = self.windows.iter().position(|w| w == &focused_id).unwrap();
        let new_idx = current_idx as i32 + direction;

        if new_idx >= 0 && (new_idx as usize) < self.windows.len() {
            self.windows.swap(current_idx, new_idx as usize);
            info!("Moved window {:?} to index {}", focused_id, new_idx);
        }
    }
}

/// Output (Monitor / Screen).
///
/// Represents a physical screen. Each screen manages its own set of Workspaces.
#[derive(Debug)]
pub struct Output<ID: WindowId> {
    /// The mapped workspaces for this output.
    /// Key: Workspace ID (1, 2, 3...) which corresponds to River's Tag.
    pub workspaces: HashMap<u32, Workspace<ID>>,

    /// The ID of the currently active workspace displayed on this output.
    pub active_workspace_id: u32,

    pub usable_area: Option<UsableArea>,
}

impl<ID: WindowId> Output<ID> {
    /// Creates a new output with a default workspace initialized.
    pub fn new() -> Self {
        let mut workspaces = HashMap::new();
        workspaces.insert(1, Workspace::new());

        Self {
            workspaces,
            active_workspace_id: 1,
            usable_area: None,
        }
    }

    /// Retrieves a mutable reference to the currently active workspace.
    /// Automatically creates it if it does not exist.
    pub fn current_workspace_mut(&mut self) -> &mut Workspace<ID> {
        self.workspaces
            .entry(self.active_workspace_id)
            .or_insert_with(Workspace::new)
    }

    pub fn switch_workspace(&mut self, target: u32) {
        if self.active_workspace_id != target {
            info!("Switched to workspace: {}", target);
            self.active_workspace_id = target;
            self.workspaces
                .entry(target)
                .or_insert_with(|| Workspace::new());
        }
    }

    pub fn move_focused_to_workspace(&mut self, target: u32) {
        if self.active_workspace_id == target {
            return;
        }

        let focused_id = self
            .workspaces
            .get(&self.active_workspace_id)
            .and_then(|ws| ws.focused_window());

        if let Some(id) = focused_id {
            if let Some(ws) = self.workspaces.get_mut(&self.active_workspace_id) {
                ws.windows.retain(|x| x != &id);
                ws.focus_stack.retain(|x| x != &id);
            }
            let target_ws = self
                .workspaces
                .entry(target)
                .or_insert_with(|| Workspace::new());

            info!("Moved window {:?} to workspace {}", id, target);
            target_ws.insert_window(id, true);
        }
    }
}

/// Shuttle (Global Application State).
///
/// Acts as the central state manager and the "brain" of the window manager.
#[derive(Debug)]
pub struct Shuttle<ID: WindowId> {
    /// The core Window Database.
    /// The single source of truth holding Window ownership. Provides O(1) lookups.
    pub window_db: HashMap<ID, Window<ID>>,

    /// Screen management mapped by the Output's unique identifier.
    pub outputs: HashMap<u32, Output<ID>>,
}

impl<ID: WindowId> Shuttle<ID> {
    /// Initializes a new, empty global state.
    pub fn new() -> Self {
        Self {
            window_db: HashMap::new(),
            outputs: HashMap::new(),
        }
    }

    /// Centrally cleans up all windows marked as closed across all workspaces.
    pub fn cleanup_closed_windows(&mut self) {
        let dead_ids: Vec<ID> = self
            .window_db
            .iter()
            .filter(|(_, w)| w.is_closed)
            .map(|(id, _)| id.clone())
            .collect();

        if dead_ids.is_empty() {
            return;
        }

        for id in &dead_ids {
            // Remove the window from all workspace views safely
            for output in self.outputs.values_mut() {
                output.current_workspace_mut().remove_window(id.clone());
            }

            // Remove the entity from the primary database
            self.window_db.remove(id);
            info!("Successfully cleaned up closed window: {:?}", id);
        }
    }
}
