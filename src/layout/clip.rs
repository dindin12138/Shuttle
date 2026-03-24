// src/layout/clip.rs

use crate::config::Config;
use crate::protocol::river_window_manager::{
    river_node_v1::RiverNodeV1, river_window_v1::RiverWindowV1,
};
use crate::state::Shuttle;
use std::collections::HashMap;
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

        let screen_width = config.output.width;
        let gap = config.layout.gaps;

        let target_h = (config.output.height - (gap * 2.0)) as i32;
        let viewport_left = gap;
        let viewport_right = screen_width - gap;

        for (id, node) in node_proxies {
            if active_windows.contains(id) {
                if let Some(window) = shuttle.window_db.get(id) {
                    let win_x = window.screen_x;
                    let win_w = window.width;

                    if win_x + win_w <= viewport_left || win_x >= viewport_right {
                        node.set_position(-10000, -10000);
                    } else {
                        node.set_position(win_x as i32, gap as i32);
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

                    let mut clip_x = 0;
                    let mut clip_w = win_w as i32;

                    if win_x < viewport_left {
                        let overlap = viewport_left - win_x;
                        clip_x = overlap as i32;
                        clip_w = (win_w - overlap).max(0.0) as i32;
                    }

                    if win_x + win_w > viewport_right {
                        let overflow = (win_x + win_w) - viewport_right;
                        clip_w = (win_w - (clip_x as f32) - overflow).max(0.0) as i32;
                    }

                    if clip_w > 0 {
                        proxy.set_clip_box(clip_x, 0, clip_w, target_h);
                        proxy.set_content_clip_box(clip_x, 0, clip_w, target_h);
                    } else {
                        proxy.set_clip_box(0, 0, 0, 0);
                        proxy.set_content_clip_box(0, 0, 0, 0);
                    }
                }
            }
        }
    }
}
