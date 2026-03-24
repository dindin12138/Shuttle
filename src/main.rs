// src/main.rs

mod config;
pub mod core;
pub mod layout;
mod log;
mod protocol;
pub mod state;

use crate::state::{AppData, RiverState, TimerCommand};
use tracing::{error, info};

use calloop::EventLoop;
use calloop::channel::{Event as ChannelEvent, channel};
use calloop_wayland_source::WaylandSource;
use std::process::exit;
use wayland_client::{Connection, EventQueue, Proxy};

use protocol::river_window_manager::river_seat_v1::Modifiers as RiverModifiers;

fn main() {
    let _guard = log::init();

    info!("Starting Shuttle Window Manager...");

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

    let user_config = config::Config::load();

    let mut app_data = AppData {
        shuttle: state::Shuttle::new(),
        input_manager: core::input::InputManager::new(),
        repeat_manager: core::repeat::KeyRepeatManager::new(),
        config: user_config.clone(),
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
    info!("Querying the compositor for supported protocols...");

    // First roundtrip: fetch global managers
    event_queue.roundtrip(&mut app_data).unwrap();

    // Environment checks
    if app_data.window_manager.is_none()
        || app_data.xkb_bindings_manager.is_none()
        || app_data.wl_seat.is_none()
    {
        error!(
            "Fatal error: The current Wayland environment does not support the required River v0.4.0+ protocols!"
        );
        error!(
            "(This is expected behavior in the current NixOS stable branch. Code compiled successfully, exiting safely.)"
        );
        exit(0);
    }

    info!("Waiting for River to initialize Seat resources...");

    // Second roundtrip: fetch the initial Seat events sent by River
    event_queue.roundtrip(&mut app_data).unwrap();

    let river_seat = match app_data.river_seat.as_ref() {
        Some(s) => s.clone(),
        None => {
            error!("Fatal error: The River compositor did not emit a river_seat event!");
            exit(0);
        }
    };

    info!("Resources acquired successfully. Registering keybindings...");

    // 1. Create a mock configuration

    let prepared = app_data.input_manager.prepare_bindings(&user_config);
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
    info!("Keybindings registered successfully. Entering event loop...");

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
