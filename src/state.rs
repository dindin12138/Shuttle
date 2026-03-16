// src/state.rs

use std::collections::HashMap;
use std::fmt::Debug;
use std::hash::Hash;
use tracing::{info, warn};

use crate::layout::FocusCenteringMode;

/// A generic identifier for a window.
/// In a production environment, this maps to `wayland_client::backend::ObjectId`.
pub trait WindowId: Eq + Hash + Clone + Debug {}
impl<T: Eq + Hash + Clone + Debug> WindowId for T {}

/// State machine for physical animations.
///
/// Used to handle temporary states for physical animations like spring physics.
#[derive(Debug, Clone, Copy, Default)]
pub struct AnimationState {
    /// The current visual position being rendered (used for smooth scrolling).
    pub current_offset: f32,
    /// The animation velocity (used for spring/damping physics calculations).
    pub velocity: f32,
}

/// The physical entity of a window.
///
/// Stores the physical properties of a window. Note that it does not store
/// a traditional `x` coordinate initially, as the layout is dynamically
/// calculated by the `Workspace` engine.
#[derive(Debug, Clone)]
pub struct Window<ID: WindowId> {
    pub id: ID,

    /// Geometric width.
    pub width: f32,
    /// Geometric height.
    pub height: f32,
    /// The absolute X coordinate on the infinite scrolling canvas (World Coordinate).
    pub world_x: f32,
    /// The final projected screen X coordinate sent to the River compositor.
    pub screen_x: f32,

    /// The target width used during animations (e.g., opening, closing, resizing).
    pub target_width: f32,

    /// Indicates whether the window is floating (exempt from tiling layout).
    pub is_floating: bool,

    /// Temporary animation state.
    pub anim_state: AnimationState,

    /// A mark used for delayed cleanup (tombstone).
    pub is_closed: bool,
}

impl<ID: WindowId> Window<ID> {
    /// Creates a new window instance with the specified dimensions.
    pub fn new(id: ID, width: f32, height: f32) -> Self {
        Self {
            id,
            width,
            height,
            world_x: 0.0,
            screen_x: 0.0,
            target_width: width,
            is_floating: false,
            anim_state: AnimationState::default(),
            is_closed: false,
        }
    }
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
}

impl<ID: WindowId> Output<ID> {
    /// Creates a new output with a default workspace initialized.
    pub fn new() -> Self {
        let mut workspaces = HashMap::new();
        workspaces.insert(1, Workspace::new());

        Self {
            workspaces,
            active_workspace_id: 1,
        }
    }

    /// Retrieves a mutable reference to the currently active workspace.
    /// Automatically creates it if it does not exist.
    pub fn current_workspace_mut(&mut self) -> &mut Workspace<ID> {
        self.workspaces
            .entry(self.active_workspace_id)
            .or_insert_with(Workspace::new)
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
            tracing::info!("♻️ Successfully cleaned up closed window: {:?}", id);
        }
    }

    /// Core layout trigger.
    /// Delegates the infinite scrolling layout calculations to the layout engine.
    pub fn update_layout(&mut self, output_id: u32, screen_width: f32) {
        let output = match self.outputs.get_mut(&output_id) {
            Some(o) => o,
            None => return,
        };
        let workspace = output.current_workspace_mut();

        crate::layout::recalculate(workspace, &mut self.window_db, screen_width);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Use u32 as a mock ID for testing purposes
    type TestWorkspace = Workspace<u32>;

    #[test]
    fn test_window_insertion_order() {
        let mut ws = TestWorkspace::new();

        // 1. Insert window 100
        ws.insert_window(100, true);
        assert_eq!(ws.windows, vec![100]);
        assert_eq!(ws.focused_window(), Some(100));

        // 2. Insert window 200 (should be placed to the right of 100)
        ws.insert_window(200, true);
        assert_eq!(ws.windows, vec![100, 200]);
        assert_eq!(ws.focused_window(), Some(200));

        // 3. Focus back on 100, then insert 300
        ws.focus_window(100);
        ws.insert_window(300, true);

        // Expectation: 300 should be inserted directly to the right of 100 (Niri behavior)
        assert_eq!(ws.windows, vec![100, 300, 200]);
    }

    #[test]
    fn test_mru_focus_stack() {
        let mut ws = TestWorkspace::new();

        // Sequentially insert A(1), B(2), C(3)
        ws.insert_window(1, true);
        ws.insert_window(2, true);
        ws.insert_window(3, true);

        // The current MRU stack should have 3 at the top
        assert_eq!(ws.focused_window(), Some(3));

        // Manually focus 1
        ws.focus_window(1);

        // Remove 1; the focus should automatically fallback to 3
        // (because 3 was the most recently activated window prior to 1)
        ws.remove_window(1);

        assert_eq!(ws.focused_window(), Some(3));
        assert!(!ws.windows.contains(&1));
    }

    #[test]
    fn test_cycle_focus() {
        let mut ws = TestWorkspace::new();
        ws.insert_window(10, true);
        ws.insert_window(20, true);
        ws.insert_window(30, true);

        // Initial focus is on 30 (last inserted), force focus to 10
        ws.focus_window(10);

        // Cycle right (+1)
        ws.cycle_focus(1);
        assert_eq!(ws.focused_window(), Some(20));

        // Cycle right (+1)
        ws.cycle_focus(1);
        assert_eq!(ws.focused_window(), Some(30));

        // Cycle right again (Should wrap around back to 10)
        ws.cycle_focus(1);
        assert_eq!(ws.focused_window(), Some(10));
    }

    #[test]
    fn test_cleanup_closed_windows() {
        let mut shuttle = Shuttle::<u32>::new();
        let output_id = 1;

        // Initialize the environment
        shuttle.outputs.insert(output_id, Output::new());
        let workspace = shuttle
            .outputs
            .get_mut(&output_id)
            .unwrap()
            .current_workspace_mut();

        // Create and insert windows 1 and 2
        shuttle.window_db.insert(1, Window::new(1, 800.0, 1000.0));
        shuttle.window_db.insert(2, Window::new(2, 800.0, 1000.0));
        workspace.insert_window(1, true);
        workspace.insert_window(2, true);

        // Verify initial state
        assert_eq!(shuttle.window_db.len(), 2);
        assert_eq!(workspace.windows.len(), 2);
        assert_eq!(workspace.focus_stack.len(), 2);

        // Simulate a window close event from Wayland (Tombstone marking)
        shuttle.window_db.get_mut(&1).unwrap().is_closed = true;

        // Execute global cleanup
        shuttle.cleanup_closed_windows();

        // Verify cleanup results: Window 1 must be completely removed from all structures!
        assert_eq!(
            shuttle.window_db.len(),
            1,
            "Database should only contain 1 window"
        );
        assert!(
            shuttle.window_db.get(&1).is_none(),
            "Window 1 entity must be destroyed"
        );

        let cleaned_ws = shuttle
            .outputs
            .get_mut(&output_id)
            .unwrap()
            .current_workspace_mut();
        assert_eq!(
            cleaned_ws.windows,
            vec![2],
            "Window 1 must be removed from the view list"
        );
        assert_eq!(
            cleaned_ws.focus_stack,
            vec![2],
            "No dangling pointers should remain in the focus stack"
        );
    }
}
