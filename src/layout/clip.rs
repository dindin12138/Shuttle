// src/layout/clip.rs

use crate::config::Config;
use crate::protocol::river_window_manager::{
    river_node_v1::RiverNodeV1,
    river_window_v1::{self, RiverWindowV1},
};
use crate::state::Shuttle;
use std::collections::HashMap;
use tracing::trace;
use wayland_client::backend::ObjectId;

pub fn apply_viewport_clipping(
    shuttle: &Shuttle<ObjectId>,
    config: &Config,
    node_proxies: &HashMap<ObjectId, RiverNodeV1>,
    window_proxies: &HashMap<ObjectId, RiverWindowV1>,
    output_id: u32,
) {
    if let Some(output) = shuttle.outputs.get(&output_id) {
        let active_windows = output
            .workspaces
            .get(&output.active_workspace_id)
            .map(|ws| ws.windows.clone())
            .unwrap_or_default();

        let focused_id = output
            .workspaces
            .get(&output.active_workspace_id)
            .and_then(|ws| ws.focused_window());

        let screen_width = output
            .usable_area
            .map(|a| a.width as f32)
            .unwrap_or(config.output.width);
        let screen_height = output
            .usable_area
            .map(|a| a.height as f32)
            .unwrap_or(config.output.height);
        let screen_x_offset = output.usable_area.map(|a| a.x as f32).unwrap_or(0.0);
        let screen_y_offset = output.usable_area.map(|a| a.y as f32).unwrap_or(0.0);

        let gap = config.layout.gaps;

        let target_h = (screen_height - (gap * 2.0)) as i32;
        let viewport_left = screen_x_offset + gap;
        let viewport_right = screen_x_offset + screen_width - gap;

        trace!("=== Viewport Clipping Started | Output: {} ===", output_id);

        for (id, node) in node_proxies {
            if active_windows.contains(id) {
                if let Some(window) = shuttle.window_db.get(id) {
                    let win_x = window.screen_x;
                    let win_w = window.width;

                    if win_x + win_w <= viewport_left || win_x >= viewport_right {
                        node.set_position(-10000, -10000);
                        trace!("Node {:?} -> Hidden (Out of Viewport)", id);
                    } else {
                        let win_y = screen_y_offset + gap;
                        node.set_position(win_x as i32, win_y as i32);
                        trace!(
                            "Node {:?} -> Placed at X: {}, Y: {}",
                            id, win_x as i32, win_y as i32
                        );
                    }
                }
            } else {
                node.set_position(-10000, -10000);
            }
        }

        for id in &active_windows {
            if let Some(window) = shuttle.window_db.get(id) {
                if let Some(proxy) = window_proxies.get(id) {
                    let win_x = window.screen_x;
                    let win_w = window.width;

                    let is_focused = Some(id.clone()) == focused_id;
                    let (b_width, (r, g, b, a)) = if config.layout.focus_ring.enable {
                        if is_focused {
                            (
                                config.layout.focus_ring.width as i32,
                                config.layout.focus_ring.get_active_color_u32(),
                            )
                        } else {
                            (
                                config.layout.focus_ring.width as i32,
                                config.layout.focus_ring.get_inactive_color_u32(),
                            )
                        }
                    } else {
                        (0, (0, 0, 0, 0))
                    };

                    proxy.set_borders(river_window_v1::Edges::all(), b_width, r, g, b, a);

                    let mut content_clip_x = 0;
                    let mut content_clip_w = win_w as i32;

                    if win_x < viewport_left {
                        let overlap = viewport_left - win_x;
                        content_clip_x = overlap as i32;
                        content_clip_w = (win_w - overlap).max(0.0) as i32;
                    }
                    if win_x + win_w > viewport_right {
                        let overflow = (win_x + win_w) - viewport_right;
                        content_clip_w =
                            (win_w - (content_clip_x as f32) - overflow).max(0.0) as i32;
                    }

                    let mut outer_clip_x = -b_width;
                    let outer_clip_y = -b_width;
                    let mut outer_clip_w = win_w as i32 + (b_width * 2);
                    let outer_clip_h = target_h + (b_width * 2);

                    if win_x < viewport_left {
                        let overlap = viewport_left - win_x;
                        let cut = overlap - outer_clip_x as f32;
                        if cut > 0.0 {
                            outer_clip_x += cut as i32;
                            outer_clip_w -= cut as i32;
                        }
                    }

                    if win_x + win_w > viewport_right {
                        let right_edge = viewport_right - win_x;
                        let current_right = outer_clip_x as f32 + outer_clip_w as f32;
                        let cut = current_right - right_edge;
                        if cut > 0.0 {
                            outer_clip_w -= cut as i32;
                        }
                    }

                    if content_clip_w > 0 {
                        trace!(
                            "Window {:?} -> Content Clip: (x:{}, w:{}), Outer Clip: (x:{}, w:{})",
                            id, content_clip_x, content_clip_w, outer_clip_x, outer_clip_w
                        );

                        proxy.set_content_clip_box(content_clip_x, 0, content_clip_w, target_h);
                        proxy.set_clip_box(
                            outer_clip_x,
                            outer_clip_y,
                            outer_clip_w.max(0),
                            outer_clip_h.max(0),
                        );
                    } else {
                        trace!("Window {:?} -> Fully Clipped (Invisible)", id);

                        proxy.set_content_clip_box(0, 0, 0, 0);
                        proxy.set_clip_box(0, 0, 0, 0);
                    }
                }
            }
        }
        trace!("=== Viewport Clipping Finished ===");
    }
}
