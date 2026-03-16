// src/handlers.rs

use crate::{AppData, TimerCommand};
use crate::{config, key_repeat, state};

use wayland_client::protocol::{wl_registry, wl_seat};
use wayland_client::{Connection, Dispatch, Proxy, QueueHandle};

use crate::protocol::river_window_manager::{
    river_node_v1, river_seat_v1, river_window_manager_v1, river_window_v1,
};
use crate::protocol::river_xkb_bindings::{river_xkb_binding_v1, river_xkb_bindings_v1};

impl Dispatch<wl_registry::WlRegistry, ()> for AppData {
    fn event(
        state: &mut Self,
        registry: &wl_registry::WlRegistry,
        event: wl_registry::Event,
        _: &(),
        _: &Connection,
        qh: &QueueHandle<AppData>,
    ) {
        if let wl_registry::Event::Global {
            name, interface, ..
        } = event
        {
            match interface.as_str() {
                "wl_seat" => {
                    state.wl_seat = Some(registry.bind::<wl_seat::WlSeat, _, _>(name, 1, qh, ()));
                }
                "river_window_manager_v1" => {
                    state.window_manager = Some(
                        registry.bind::<river_window_manager_v1::RiverWindowManagerV1, _, _>(
                            name,
                            3,
                            qh,
                            (),
                        ),
                    );
                }
                "river_xkb_bindings_v1" => {
                    state.xkb_bindings_manager = Some(
                        registry.bind::<river_xkb_bindings_v1::RiverXkbBindingsV1, _, _>(
                            name,
                            2,
                            qh,
                            (),
                        ),
                    );
                }
                _ => {}
            }
        }
    }
}

impl Dispatch<river_window_manager_v1::RiverWindowManagerV1, ()> for AppData {
    fn event(
        state: &mut Self,
        _: &river_window_manager_v1::RiverWindowManagerV1,
        event: river_window_manager_v1::Event,
        _: &(),
        _: &Connection,
        _qh: &QueueHandle<AppData>,
    ) {
        match event {
            river_window_manager_v1::Event::Seat { id } => {
                state.river_seat = Some(id);
            }

            // 1. New window creation
            river_window_manager_v1::Event::Window { id } => {
                let object_id = id.id();

                // Initialize the window with temporary dimensions.
                // Actual dimensions will be provided via the Dimensions event.
                let new_window = state::Window::new(object_id.clone(), 800.0, 1000.0);
                state
                    .shuttle
                    .window_db
                    .insert(object_id.clone(), new_window);

                // Acquire the node proxy for physical positioning
                let node = id.get_node(_qh, ());
                state.node_proxies.insert(object_id.clone(), node);
                state.window_proxies.insert(object_id.clone(), id.clone());

                let workspace = state
                    .shuttle
                    .outputs
                    .entry(1)
                    .or_insert_with(state::Output::new)
                    .current_workspace_mut();
                workspace.insert_window(object_id, true);

                // Mark management state as dirty to trigger a new manage sequence
                state.window_manager.as_ref().unwrap().manage_dirty();
            }

            // 2. Manage Sequence: Propose logical properties (e.g., dimensions)
            river_window_manager_v1::Event::ManageStart => {
                state.shuttle.cleanup_closed_windows();

                let output_id = 1;
                if let Some(output) = state.shuttle.outputs.get(&output_id) {
                    if let Some(workspace) = output.workspaces.get(&output.active_workspace_id) {
                        for id in &workspace.windows {
                            if let Some(window) = state.shuttle.window_db.get(id) {
                                if let Some(proxy) = state.window_proxies.get(id) {
                                    proxy.propose_dimensions(
                                        window.width as i32,
                                        window.height as i32,
                                    );
                                }
                            }
                        }
                    }
                }

                // Commit the manage sequence
                state.window_manager.as_ref().unwrap().manage_finish();
            }

            // 3. Render Sequence: Apply physical coordinates to nodes
            river_window_manager_v1::Event::RenderStart => {
                let output_id = 1;
                let screen_width = 1920.0; // Assuming a 1080p screen width

                if let Some(output) = state.shuttle.outputs.get(&output_id) {
                    if let Some(workspace) = output.workspaces.get(&output.active_workspace_id) {
                        for id in &workspace.windows {
                            if let Some(window) = state.shuttle.window_db.get(id) {
                                let win_x = window.screen_x;
                                let win_w = window.width;
                                let win_h = window.height;

                                // High-Performance Culling
                                // If the window is completely off-screen (left or right bounds)
                                if win_x + win_w <= 0.0 || win_x >= screen_width {
                                    if let Some(node) = state.node_proxies.get(id) {
                                        // Dispatch to an extreme off-screen coordinate to ensure the GPU skips rendering
                                        node.set_position(-10000, -10000);
                                    }
                                    // Crucial: continue immediately to bypass further protocol requests and save IPC overhead
                                    continue;
                                }
                                // Standard Rendering: Set Position
                                if let Some(node) = state.node_proxies.get(id) {
                                    // Y coordinate is fixed at 20 to leave a top margin
                                    node.set_position(win_x as i32, 20);
                                }
                                // Visual Perfection: Edge Clipping
                                if let Some(proxy) = state.window_proxies.get(id) {
                                    let mut clip_x = 0;
                                    let mut clip_w = win_w as i32;

                                    if win_x < 0.0 {
                                        // Window is clipped on the left edge
                                        clip_x = (-win_x) as i32;
                                        clip_w = (win_w + win_x) as i32;
                                    } else if win_x + win_w > screen_width {
                                        // Window is clipped on the right edge
                                        clip_w = (screen_width - win_x) as i32;
                                    }

                                    // Check if clipping is actually required
                                    if clip_w < win_w as i32 {
                                        // Enable clipping: clips both window content and decorations (shadows/borders)
                                        // to prevent visual bleeding into adjacent monitors.
                                        proxy.set_clip_box(clip_x, 0, clip_w, win_h as i32);
                                        proxy.set_content_clip_box(clip_x, 0, clip_w, win_h as i32);
                                    } else {
                                        // Window is fully visible on screen.
                                        // Per River protocol: Setting a clip box with 0 width/height disables clipping.
                                        proxy.set_clip_box(0, 0, 0, 0);
                                        proxy.set_content_clip_box(0, 0, 0, 0);
                                    }
                                }
                            }
                        }
                    }
                }
                // Commit the render sequence to apply changes to the screen
                state.window_manager.as_ref().unwrap().render_finish();
            }
            _ => {}
        }
    }
}

impl Dispatch<river_window_v1::RiverWindowV1, ()> for AppData {
    fn event(
        state: &mut Self,
        proxy: &river_window_v1::RiverWindowV1,
        event: river_window_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<AppData>,
    ) {
        match event {
            // Listen for actual dimensions requested by the client application
            river_window_v1::Event::Dimensions { width, height } => {
                let object_id = proxy.id();
                if let Some(window) = state.shuttle.window_db.get_mut(&object_id) {
                    window.width = width as f32;
                    window.height = height as f32;
                }
                // Recalculate layout upon dimension changes
                state.shuttle.update_layout(1, 1920.0);
            }

            // Gracefully handle window destruction
            river_window_v1::Event::Closed => {
                let object_id = proxy.id();
                if let Some(window) = state.shuttle.window_db.get_mut(&object_id) {
                    window.is_closed = true;
                }

                // Destroy associated proxies to free up resources
                if let Some(node) = state.node_proxies.remove(&object_id) {
                    node.destroy();
                }
                state.window_proxies.remove(&object_id);
                proxy.destroy();

                state.window_manager.as_ref().unwrap().manage_dirty();
            }
            _ => {}
        }
    }
}

impl Dispatch<river_xkb_binding_v1::RiverXkbBindingV1, ()> for AppData {
    fn event(
        state: &mut Self,
        proxy: &river_xkb_binding_v1::RiverXkbBindingV1,
        event: river_xkb_binding_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<AppData>,
    ) {
        let object_id = proxy.id();

        match event {
            river_xkb_binding_v1::Event::Pressed => {
                if let Some(action) = state.input_manager.handle_pressed(object_id.clone()) {
                    let repeat_action = match action {
                        config::Action::FocusLeft => key_repeat::Action::FocusLeft,
                        config::Action::FocusRight => key_repeat::Action::FocusRight,
                        _ => return,
                    };

                    use key_repeat::ExecuteAction;
                    state.execute_action(repeat_action.clone());

                    // Send a timer command through the channel to handle key repeat asynchronously
                    let _ = state
                        .timer_tx
                        .send(TimerCommand::StartRepeat(object_id, repeat_action));
                }
            }
            river_xkb_binding_v1::Event::Released => {
                let _ = state
                    .timer_tx
                    .send(TimerCommand::StopRepeat(Some(object_id)));
            }
            _ => {}
        }
    }
}

// ==========================================
// Required empty implementations
// ==========================================

impl Dispatch<river_node_v1::RiverNodeV1, ()> for AppData {
    fn event(
        _: &mut Self,
        _: &river_node_v1::RiverNodeV1,
        _: river_node_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<AppData>,
    ) {
    }
}

impl Dispatch<wl_seat::WlSeat, ()> for AppData {
    fn event(
        _: &mut Self,
        _: &wl_seat::WlSeat,
        _: wl_seat::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<AppData>,
    ) {
    }
}

impl Dispatch<river_xkb_bindings_v1::RiverXkbBindingsV1, ()> for AppData {
    fn event(
        _: &mut Self,
        _: &river_xkb_bindings_v1::RiverXkbBindingsV1,
        _: river_xkb_bindings_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<AppData>,
    ) {
    }
}

impl Dispatch<river_seat_v1::RiverSeatV1, ()> for AppData {
    fn event(
        _: &mut Self,
        _: &river_seat_v1::RiverSeatV1,
        _: river_seat_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<AppData>,
    ) {
    }
}
