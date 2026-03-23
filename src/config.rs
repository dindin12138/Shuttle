// src/config.rs

use serde::Deserialize;
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Deserialize, Clone, Copy, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum FocusCenteringMode {
    Always,
    Never,
    OnOverflow,
}

impl Default for FocusCenteringMode {
    fn default() -> Self {
        FocusCenteringMode::Never
    }
}

#[derive(Debug, Deserialize, PartialEq, Clone)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum Action {
    Spawn { args: Vec<String> },
    CloseWindow,
    Quit,
    FocusLeft,
    FocusRight,
    MoveLeft,
    MoveRight,
    FocusWorkspace { target: u32 },
    MoveToWorkspace { target: u32 },
}

#[derive(Debug, Deserialize, Clone)]
pub struct LayoutConfig {
    pub gaps: f32,
    pub default_column_width: f32,

    #[serde(default)]
    pub center_focused_column: FocusCenteringMode,
}

impl Default for LayoutConfig {
    fn default() -> Self {
        Self {
            gaps: 20.0,
            default_column_width: 800.0,
            center_focused_column: FocusCenteringMode::Never,
        }
    }
}

#[derive(Debug, Deserialize, Clone)]
pub struct OutputConfig {
    pub width: f32,
    pub height: f32,
}

impl Default for OutputConfig {
    fn default() -> Self {
        Self {
            width: 1920.0,
            height: 1200.0,
        }
    }
}

#[derive(Debug, Deserialize, Clone)]
pub struct Config {
    pub keybindings: Option<HashMap<String, HashMap<String, Action>>>,

    #[serde(default)]
    pub layout: LayoutConfig,

    #[serde(default)]
    pub output: OutputConfig,
}

impl Config {
    pub fn get_path() -> PathBuf {
        let home = std::env::var("HOME").expect("HOME environment variable not found.");
        PathBuf::from(home)
            .join(".config")
            .join("shuttle")
            .join("config.toml")
    }

    pub fn load() -> Self {
        let path = Self::get_path();

        if let Ok(content) = fs::read_to_string(&path) {
            match toml::from_str::<Config>(&content) {
                Ok(config) => {
                    println!("Configuration file loaded: {:?}", path);
                    return config;
                }
                Err(e) => {
                    eprintln!("Configuration parsing failed: {}", e);
                }
            }
        } else {
            println!(
                "Configuration file not found at {:?}. Using defaults.",
                path
            );
        }

        Self::default_config()
    }

    fn default_config() -> Self {
        let mut alt_bindings = HashMap::new();
        alt_bindings.insert(
            "Return".to_string(),
            Action::Spawn {
                args: vec!["ghostty".to_string()],
            },
        );
        alt_bindings.insert("h".to_string(), Action::FocusLeft);
        alt_bindings.insert("l".to_string(), Action::FocusRight);

        let mut alt_shift_bindings = HashMap::new();
        alt_shift_bindings.insert("q".to_string(), Action::CloseWindow);
        alt_shift_bindings.insert("e".to_string(), Action::Quit);

        let mut keybindings = HashMap::new();
        keybindings.insert("alt".to_string(), alt_bindings);
        keybindings.insert("alt_shift".to_string(), alt_shift_bindings);

        Self {
            keybindings: Some(keybindings),
            layout: LayoutConfig::default(),
            output: OutputConfig::default(),
        }
    }
}

pub fn parse_modifiers(mod_group: &str) -> u32 {
    let mut mask = 0;
    for m in mod_group.split(|c| c == '_' || c == '+') {
        mask |= parse_single_modifier(m);
    }
    mask
}

fn parse_single_modifier(mod_str: &str) -> u32 {
    match mod_str.to_lowercase().as_str() {
        "shift" => 1,
        "ctrl" | "control" => 4,
        "alt" | "mod1" => 8,
        "mod3" => 32,
        "super" | "mod4" | "logo" => 64,
        "mod5" => 128,
        "none" | "" => 0,
        _ => {
            eprintln!("Warning: Unknown modifier '{}'", mod_str);
            0
        }
    }
}

pub fn parse_keysym(key_str: &str) -> u32 {
    let lower_key = key_str.to_lowercase();
    if lower_key.len() == 1 {
        lower_key.chars().next().unwrap() as u32
    } else {
        match lower_key.as_str() {
            "enter" | "return" => 0xff0d,
            "escape" | "esc" => 0xff1b,
            "space" => 0x0020,
            "tab" => 0xff09,
            "left" => 0xff51,
            "right" => 0xff53,
            "up" => 0xff52,
            "down" => 0xff54,
            "xf86audioraisevolume" => 0x1008ff11,
            "xf86audiolowervolume" => 0x1008ff12,
            "xf86audiomute" => 0x1008ff13,
            _ => {
                eprintln!("Warning: Unknown key '{}'", key_str);
                0
            }
        }
    }
}
