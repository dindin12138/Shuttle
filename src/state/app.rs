// src/state/app.rs

use calloop::LoopSignal;
use calloop::channel::Sender;
use wayland_client::backend::ObjectId;
use wayland_client::protocol::wl_seat;

use crate::config::Config;
use crate::core::input::InputManager;
use crate::core::repeat::{Action as RepeatAction, KeyRepeatManager};
use crate::protocol::river_window_manager::{
    river_node_v1, river_seat_v1, river_window_manager_v1, river_window_v1,
};
use crate::protocol::river_xkb_bindings::{river_xkb_binding_v1, river_xkb_bindings_v1};
use crate::state::Shuttle;

/// Commands sent to the timer channel to handle key repeat logic across lifetimes.
pub enum TimerCommand {
    StartRepeat(ObjectId, RepeatAction),
    StopRepeat(Option<ObjectId>),
}

#[derive(Debug, PartialEq, Default)]
pub enum RiverState {
    #[default]
    Idle,
    ManageRequested,
    Managing,
    WaitingForRender,
    Rendering,
}

/// The global application state (The God Object).
pub struct AppData {
    pub shuttle: Shuttle<ObjectId>,
    pub input_manager: InputManager<ObjectId>,
    pub repeat_manager: KeyRepeatManager<ObjectId>,

    pub config: Config,

    pub timer_tx: Sender<TimerCommand>,
    pub loop_signal: LoopSignal,

    pub wl_seat: Option<wl_seat::WlSeat>,
    pub window_manager: Option<river_window_manager_v1::RiverWindowManagerV1>,
    pub xkb_bindings_manager: Option<river_xkb_bindings_v1::RiverXkbBindingsV1>,
    pub river_seat: Option<river_seat_v1::RiverSeatV1>,

    pub window_proxies: std::collections::HashMap<ObjectId, river_window_v1::RiverWindowV1>,
    pub node_proxies: std::collections::HashMap<ObjectId, river_node_v1::RiverNodeV1>,

    pub pending_bindings: Vec<river_xkb_binding_v1::RiverXkbBindingV1>,
    pub active_bindings: Vec<river_xkb_binding_v1::RiverXkbBindingV1>,

    pub river_state: RiverState,
    pub needs_manage: bool,
}

impl AppData {
    pub fn request_manage(&mut self) {
        self.needs_manage = true;
        self.try_send_manage_dirty();
    }

    pub fn try_send_manage_dirty(&mut self) {
        if self.needs_manage && self.river_state == RiverState::Idle {
            if let Some(wm) = &self.window_manager {
                wm.manage_dirty();
                self.river_state = RiverState::ManageRequested;
                self.needs_manage = false;
            }
        }
    }
}
