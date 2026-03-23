// src/config.rs

use serde::Deserialize;

/// Actions supported by the Shuttle window manager.
#[derive(Debug, Deserialize, PartialEq, Clone)]
#[serde(rename_all = "snake_case")]
pub enum Action {
    FocusLeft,
    FocusRight,
    MoveLeft,
    MoveRight,
    CloseWindow,
    Quit,
    SpawnTerminal,
}

/// A single keybinding configuration parsed from the TOML file.
#[derive(Debug, Deserialize, Clone)]
pub struct KeybindConfig {
    pub modifiers: Vec<String>,
    pub key: String,
    pub action: Action,
}

/// The root configuration structure representing the user's config file.
#[derive(Debug, Deserialize, Clone)]
pub struct Config {
    pub bindings: Vec<KeybindConfig>,
}

/// Converts a modifier string to the corresponding bitmask required by the River protocol.
///
/// This mapping is based on the `river_seat_v1.modifiers` enum definition.
pub fn parse_modifier(mod_str: &str) -> u32 {
    match mod_str.to_lowercase().as_str() {
        "shift" => 1,
        "ctrl" | "control" => 4,
        "alt" | "mod1" => 8,
        "mod3" => 32,
        "super" | "mod4" | "logo" => 64,
        "mod5" => 128,
        _ => {
            eprintln!("Warning: Unknown modifier '{}'", mod_str);
            0
        }
    }
}

/// Converts a key string to its corresponding XKB Keysym.
///
/// Note: This is a basic implementation suitable for an MVP. For production,
/// it is highly recommended to integrate the `xkbcommon` crate for robust
/// and comprehensive keyboard mapping.
pub fn parse_keysym(key_str: &str) -> u32 {
    let lower_key = key_str.to_lowercase();

    if lower_key.len() == 1 {
        // For single characters, fallback to their ASCII decimal value as the base Keysym
        lower_key.chars().next().unwrap() as u32
    } else {
        match lower_key.as_str() {
            "enter" | "return" => 0xff0d,
            "escape" | "esc" => 0xff1b,
            "space" => 0x0020,
            "tab" => 0xff09,
            "left" => 0xff51,
            "right" => 0xff53,
            _ => {
                eprintln!("Warning: Unknown key '{}'", key_str);
                0
            }
        }
    }
}
