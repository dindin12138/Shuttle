// src/handlers.rs

use crate::core::action;
use crate::state;
use crate::state::{AppData, TimerCommand, UsableArea};
use tracing::{debug, error, info, trace};

use wayland_client::protocol::{wl_registry, wl_seat};
use wayland_client::{Connection, Dispatch, Proxy, QueueHandle};

use crate::protocol::river_layer_shell::{
    river_layer_shell_output_v1, river_layer_shell_seat_v1, river_layer_shell_v1,
};
use crate::protocol::river_window_manager::{
    river_node_v1, river_output_v1, river_seat_v1, river_window_manager_v1, river_window_v1,
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
                    info!("Bound Wayland global: wl_seat");
                    state.wl_seat = Some(registry.bind::<wl_seat::WlSeat, _, _>(name, 1, qh, ()));
                }
                "river_window_manager_v1" => {
                    info!("Bound River protocol: river_window_manager_v1");
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
                    info!("Bound River protocol: river_xkb_bindings_v1");
                    state.xkb_bindings_manager = Some(
                        registry.bind::<river_xkb_bindings_v1::RiverXkbBindingsV1, _, _>(
                            name,
                            2,
                            qh,
                            (),
                        ),
                    );
                }
                "river_layer_shell_v1" => {
                    info!("Bound River protocol: river_layer_shell_v1");
                    state.layer_shell_manager = Some(
                        registry.bind::<river_layer_shell_v1::RiverLayerShellV1, _, _>(
                            name,
                            1,
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
    fn event_created_child(
        opcode: u16,
        qh: &QueueHandle<Self>,
    ) -> std::sync::Arc<dyn wayland_client::backend::ObjectData> {
        match opcode {
            6 => qh.make_data::<river_window_v1::RiverWindowV1, ()>(()),
            7 => qh.make_data::<river_output_v1::RiverOutputV1, ()>(()),
            8 => qh.make_data::<river_seat_v1::RiverSeatV1, ()>(()),
            _ => {
                error!(
                    "Unexpected event_created_child opcode {} from RiverWindowManager",
                    opcode
                );
                panic!("Unexpected event_created_child opcode");
            }
        }
    }

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
                debug!("Received River Seat: {:?}", id.id());
                state.river_seat = Some(id.clone());
                if let Some(ls_mgr) = &state.layer_shell_manager {
                    let _ = ls_mgr.get_seat(&id, _qh, ());
                }
            }

            river_window_manager_v1::Event::Output { id } => {
                info!("Received River Output: {:?}", id.id());
                if let Some(ls_mgr) = &state.layer_shell_manager {
                    let _ = ls_mgr.get_output(&id, _qh, ());
                }
            }

            river_window_manager_v1::Event::Window { id } => {
                let object_id = id.id();
                info!("New window detected: {:?}", object_id);

                let gap = state.config.layout.gaps;

                let output_id = 1;
                let (screen_width, screen_height) =
                    if let Some(output) = state.shuttle.outputs.get(&output_id) {
                        if let Some(area) = output.usable_area {
                            (area.width as f32, area.height as f32)
                        } else {
                            (state.config.output.width, state.config.output.height)
                        }
                    } else {
                        (state.config.output.width, state.config.output.height)
                    };

                let available_width = screen_width - (gap * 2.0);
                let default_prop = state.config.layout.default_column_width.proportion;

                let initial_width = (default_prop * (available_width + gap)) - gap;
                let initial_width = initial_width.max(1.0);
                let target_h = screen_height - (gap * 2.0);

                let new_window = state::Window::new(object_id.clone(), initial_width, target_h);

                state
                    .shuttle
                    .window_db
                    .insert(object_id.clone(), new_window);

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

                state.request_manage();
            }

            river_window_manager_v1::Event::ManageStart => {
                trace!("ManageStart sequence initiated");

                state.river_state = crate::RiverState::Managing;
                state.shuttle.cleanup_closed_windows();
                crate::layout::engine::update_layout(&mut state.shuttle, 1, &state.config);

                for binding in state.pending_bindings.drain(..) {
                    binding.enable();
                    state.active_bindings.push(binding);
                }

                let output_id = 1;
                if let Some(output) = state.shuttle.outputs.get(&output_id) {
                    if let Some(workspace) = output.workspaces.get(&output.active_workspace_id) {
                        if let Some(focused_id) = workspace.focused_window() {
                            if let Some(window_proxy) = state.window_proxies.get(&focused_id) {
                                if let Some(seat) = &state.river_seat {
                                    seat.focus_window(window_proxy);
                                }
                            }
                        }
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

                state.window_manager.as_ref().unwrap().manage_finish();
                state.river_state = crate::RiverState::WaitingForRender;
            }

            river_window_manager_v1::Event::RenderStart => {
                trace!("RenderStart sequence initiated");

                state.river_state = crate::RiverState::Rendering;

                crate::layout::clip::apply_viewport_clipping(
                    &state.shuttle,
                    &state.config,
                    &state.node_proxies,
                    &state.window_proxies,
                    1,
                );

                state.window_manager.as_ref().unwrap().render_finish();
                state.river_state = crate::RiverState::Idle;
                state.try_send_manage_dirty();
            }
            _ => {}
        }
    }
}

impl Dispatch<river_layer_shell_output_v1::RiverLayerShellOutputV1, ()> for AppData {
    fn event(
        state: &mut Self,
        _: &river_layer_shell_output_v1::RiverLayerShellOutputV1,
        event: river_layer_shell_output_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<AppData>,
    ) {
        let river_layer_shell_output_v1::Event::NonExclusiveArea {
            x,
            y,
            width,
            height,
        } = event;

        info!(
            "Layer Shell NonExclusiveArea updated: x={}, y={}, w={}, h={}",
            x, y, width, height
        );

        let output = state
            .shuttle
            .outputs
            .entry(1)
            .or_insert_with(state::Output::new);

        output.usable_area = Some(UsableArea {
            x,
            y,
            width,
            height,
        });

        state.request_manage();
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
            river_window_v1::Event::Dimensions { width, height } => {
                let object_id = proxy.id();
                trace!(
                    "Window {:?} dimension update: {}x{}",
                    object_id, width, height
                );
                if let Some(window) = state.shuttle.window_db.get_mut(&object_id) {
                    window.width = width as f32;
                    window.height = height as f32;
                }
                state.request_manage();
            }
            river_window_v1::Event::Closed => {
                let object_id = proxy.id();
                info!("Window closed by client: {:?}", object_id);
                if let Some(window) = state.shuttle.window_db.get_mut(&object_id) {
                    window.is_closed = true;
                }
                if let Some(node) = state.node_proxies.remove(&object_id) {
                    node.destroy();
                }
                state.window_proxies.remove(&object_id);
                proxy.destroy();
                state.request_manage();
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
                debug!("Key binding pressed: {:?}", object_id);
                if let Some(action_def) = state.input_manager.handle_pressed(object_id.clone()) {
                    action::execute_config_action(state, object_id, action_def);
                }
            }
            river_xkb_binding_v1::Event::Released => {
                debug!("Key binding released: {:?}", object_id);
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

impl Dispatch<river_layer_shell_v1::RiverLayerShellV1, ()> for AppData {
    fn event(
        _: &mut Self,
        _: &river_layer_shell_v1::RiverLayerShellV1,
        _: river_layer_shell_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<AppData>,
    ) {
    }
}

impl Dispatch<river_layer_shell_seat_v1::RiverLayerShellSeatV1, ()> for AppData {
    fn event(
        _: &mut Self,
        _: &river_layer_shell_seat_v1::RiverLayerShellSeatV1,
        _: river_layer_shell_seat_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<AppData>,
    ) {
    }
}

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

impl Dispatch<river_output_v1::RiverOutputV1, ()> for AppData {
    fn event(
        _: &mut Self,
        _: &river_output_v1::RiverOutputV1,
        _: river_output_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<AppData>,
    ) {
    }
}
