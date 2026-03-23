// src/main.rs

mod config;
mod handlers;
mod input;
mod key_repeat;
mod layout;
mod protocol;
mod state;

use calloop::channel::{Event as ChannelEvent, Sender, channel};
use calloop::{EventLoop, LoopSignal};
use calloop_wayland_source::WaylandSource;
use std::process::exit;
use wayland_client::backend::ObjectId;
use wayland_client::protocol::wl_seat;
use wayland_client::{Connection, EventQueue, Proxy};

use protocol::river_window_manager::river_node_v1;
use protocol::river_window_manager::river_seat_v1::{self, Modifiers as RiverModifiers};
use protocol::river_window_manager::river_window_manager_v1;
use protocol::river_window_manager::river_window_v1;
use protocol::river_xkb_bindings::river_xkb_bindings_v1;

/// Commands sent to the timer channel to handle key repeat logic across lifetimes.
pub enum TimerCommand {
    StartRepeat(ObjectId, key_repeat::Action),
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
///
/// Holds the window manager state, input managers, and Wayland proxies.
pub struct AppData {
    pub shuttle: state::Shuttle<ObjectId>,
    pub input_manager: input::InputManager<ObjectId>,
    pub repeat_manager: key_repeat::KeyRepeatManager<ObjectId>,

    /// A channel sender used to decouple the timer from the event loop's lifetime.
    pub timer_tx: Sender<TimerCommand>,
    pub loop_signal: LoopSignal,

    pub wl_seat: Option<wl_seat::WlSeat>,
    pub window_manager: Option<river_window_manager_v1::RiverWindowManagerV1>,
    pub xkb_bindings_manager: Option<river_xkb_bindings_v1::RiverXkbBindingsV1>,
    pub river_seat: Option<river_seat_v1::RiverSeatV1>,

    /// Proxies for communicating with physical window entities.
    pub window_proxies: std::collections::HashMap<ObjectId, river_window_v1::RiverWindowV1>,
    pub node_proxies: std::collections::HashMap<ObjectId, river_node_v1::RiverNodeV1>,

    pub pending_bindings:
        Vec<crate::protocol::river_xkb_bindings::river_xkb_binding_v1::RiverXkbBindingV1>,
    pub active_bindings:
        Vec<crate::protocol::river_xkb_bindings::river_xkb_binding_v1::RiverXkbBindingV1>,

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

impl key_repeat::ExecuteAction for AppData {
    fn execute_action(&mut self, action: key_repeat::Action) {
        println!("Executing action: {:?}", action);

        // Temporarily hardcoded to the primary display and 1080p resolution
        let output_id = 1;
        let screen_width = 1920.0;

        // Use a match expression to determine if a layout update is needed
        let needs_layout = match action {
            key_repeat::Action::FocusLeft => {
                self.shuttle
                    .outputs
                    .get_mut(&output_id)
                    .unwrap()
                    .current_workspace_mut()
                    .cycle_focus(-1);
                true
            }
            key_repeat::Action::FocusRight => {
                self.shuttle
                    .outputs
                    .get_mut(&output_id)
                    .unwrap()
                    .current_workspace_mut()
                    .cycle_focus(1);
                true
            }
        };

        // Drive the layout engine if the state has changed
        if needs_layout {
            self.shuttle.update_layout(output_id, screen_width);
            self.request_manage();

            // Mark the management state as dirty to trigger a new River configuration sequence
            // if let Some(wm) = &self.window_manager {
            //     wm.manage_dirty();
            // }
        }
    }
}

fn main() {
    let mut event_loop: EventLoop<AppData> = EventLoop::try_new().unwrap();
    let loop_handle = event_loop.handle();
    let loop_signal = event_loop.get_signal();

    // Create a safe channel that spans across lifetimes
    let (timer_tx, timer_rx) = channel::<TimerCommand>();

    // Listen to the channel: when a timer command is received, insert the timer into the loop
    let loop_handle_for_channel = loop_handle.clone();
    loop_handle
        .insert_source(timer_rx, move |event, _, app_data: &mut AppData| {
            if let ChannelEvent::Msg(cmd) = event {
                match cmd {
                    TimerCommand::StartRepeat(id, action) => {
                        app_data
                            .repeat_manager
                            .start_repeat(&loop_handle_for_channel, id, action);
                    }
                    TimerCommand::StopRepeat(id) => {
                        app_data
                            .repeat_manager
                            .stop_repeat(&loop_handle_for_channel, id);
                    }
                }
            }
        })
        .unwrap();

    // Connect to the Wayland environment
    let conn = Connection::connect_to_env().expect("Failed to connect to the Wayland environment.");
    let display = conn.display();
    let mut event_queue = conn.new_event_queue();
    let qh = event_queue.handle();

    let mut app_data = AppData {
        shuttle: state::Shuttle::new(),
        input_manager: input::InputManager::new(),
        repeat_manager: key_repeat::KeyRepeatManager::new(),
        timer_tx,
        loop_signal,
        wl_seat: None,
        window_manager: None,
        xkb_bindings_manager: None,
        river_seat: None,
        window_proxies: std::collections::HashMap::new(),
        node_proxies: std::collections::HashMap::new(),
        pending_bindings: Vec::new(),
        active_bindings: Vec::new(),
        river_state: RiverState::Idle,
        needs_manage: false,
    };

    let _registry = display.get_registry(&qh, ());
    println!("1. Querying the compositor for supported protocols...");

    // First roundtrip: fetch global managers
    event_queue.roundtrip(&mut app_data).unwrap();

    // Environment checks
    if app_data.window_manager.is_none()
        || app_data.xkb_bindings_manager.is_none()
        || app_data.wl_seat.is_none()
    {
        eprintln!(
            "❌ Fatal error: The current Wayland environment does not support the required River v0.4.0+ protocols!"
        );
        eprintln!(
            "(This is expected behavior in the current NixOS stable branch. Code compiled successfully, exiting safely.)"
        );
        exit(0);
    }

    println!("2. Waiting for River to initialize Seat resources...");

    // Second roundtrip: fetch the initial Seat events sent by River
    event_queue.roundtrip(&mut app_data).unwrap();

    let river_seat = match app_data.river_seat.as_ref() {
        Some(s) => s.clone(),
        None => {
            eprintln!("❌ Fatal error: The River compositor did not emit a river_seat event!");
            exit(0);
        }
    };

    println!("✅ Resources acquired successfully. Registering keybindings...");

    // 1. Create a mock configuration
    let dummy_config = config::Config {
        bindings: vec![
            config::KeybindConfig {
                modifiers: vec!["Super".to_string()],
                key: "a".to_string(),
                action: config::Action::FocusLeft,
            },
            config::KeybindConfig {
                modifiers: vec!["Super".to_string()],
                key: "d".to_string(),
                action: config::Action::FocusRight,
            },
            config::KeybindConfig {
                modifiers: vec!["Super".to_string()],
                key: "Enter".to_string(),
                action: config::Action::SpawnTerminal,
            },
            config::KeybindConfig {
                modifiers: vec!["Super".to_string()],
                key: "q".to_string(),
                action: config::Action::CloseWindow,
            },
        ],
    };

    let prepared = app_data.input_manager.prepare_bindings(&dummy_config);
    let xkb_manager = app_data.xkb_bindings_manager.as_ref().unwrap();

    // 2. Iterate and send requests to River
    for (mods, keysym, action) in prepared {
        let modifiers = RiverModifiers::from_bits_truncate(mods);

        // Register the binding using the appropriate river_seat
        let binding_proxy = xkb_manager.get_xkb_binding(&river_seat, keysym, modifiers, &qh, ());

        // binding_proxy.enable();
        app_data.pending_bindings.push(binding_proxy.clone());

        app_data
            .input_manager
            .register_wayland_object(binding_proxy.id(), action);
    }

    // 3. Commit the Manage Sequence
    // app_data.window_manager.as_ref().unwrap().manage_finish();
    println!("🎉 Keybindings registered successfully. Entering event loop...");

    app_data.request_manage();

    // Hook into the calloop event loop
    let wayland_source = WaylandSource::new(conn, event_queue);
    loop_handle
        .insert_source(
            wayland_source,
            |_, queue: &mut EventQueue<AppData>, app_data| queue.dispatch_pending(app_data),
        )
        .unwrap();

    event_loop.run(None, &mut app_data, |_| {}).unwrap();
}
