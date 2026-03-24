// src/state/window.rs
use std::fmt::Debug;
use std::hash::Hash;

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

    pub custom_proportion: Option<f32>,
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
            custom_proportion: None,
        }
    }
}
