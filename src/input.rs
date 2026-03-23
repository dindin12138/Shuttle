// src/input.rs

use std::collections::HashMap;
use std::fmt::Debug;
use std::hash::Hash;

use crate::config::{Action, Config};

/// A generic identifier for a Wayland object used to represent keybindings.
/// In production, this maps to `wayland_client::backend::ObjectId`.
pub trait BindingId: Eq + Hash + Clone + Debug {}
impl<T: Eq + Hash + Clone + Debug> BindingId for T {}

/// Maps Wayland binding object IDs to their corresponding actions.
/// Acts as the central $O(1)$ router for input events.
pub struct InputManager<ID: BindingId> {
    /// The routing table mapping Wayland object IDs to `Action`s.
    pub bindings_map: HashMap<ID, Action>,
}

impl<ID: BindingId> InputManager<ID> {
    /// Creates a new, empty `InputManager`.
    pub fn new() -> Self {
        Self {
            bindings_map: HashMap::new(),
        }
    }

    /// Parses the configuration to extract the required modifier masks and keysyms.
    ///
    /// Note: This function does not interact with Wayland directly. It prepares
    /// the required data for subsequent Wayland registration.
    pub fn prepare_bindings(&self, config: &Config) -> Vec<(u32, u32, Action)> {
        let mut prepared = Vec::new();
        if let Some(bindings_map) = &config.keybindings {
            for (mod_group, keys_map) in bindings_map {
                let mods_mask = crate::config::parse_modifiers(mod_group);
                for (key_str, action) in keys_map {
                    let keysym = crate::config::parse_keysym(key_str);
                    if keysym != 0 {
                        prepared.push((mods_mask, keysym, action.clone()));
                    }
                }
            }
        }
        prepared
    }

    /// Registers the mapping between a Wayland object ID and its action.
    ///
    /// This should be called after successfully registering a binding with the compositor
    /// and receiving the corresponding Wayland object ID.
    pub fn register_wayland_object(&mut self, object_id: ID, action: Action) {
        self.bindings_map.insert(object_id, action);
    }

    /// Retrieves the action associated with the triggered object ID.
    ///
    /// Called when a `river_xkb_binding_v1::Event::Pressed` event is received.
    pub fn handle_pressed(&self, object_id: ID) -> Option<Action> {
        self.bindings_map.get(&object_id).cloned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_input_manager_routing() {
        // 1. Mock user configuration
        let toml_str = r#"
            [[bindings]]
            modifiers = ["Super"]
            key = "h"
            action = "focus_left"

            [[bindings]]
            modifiers = ["Super", "Shift"]
            key = "q"
            action = "quit"
        "#;
        let config: Config = toml::from_str(toml_str).expect("Failed to parse TOML");

        // 2. Initialize InputManager using u32 as a mock Wayland ObjectId
        let mut input_manager: InputManager<u32> = InputManager::new();

        // 3. Prepare registration data (Parse TOML into underlying data types)
        let prepared_data = input_manager.prepare_bindings(&config);
        assert_eq!(prepared_data.len(), 2);

        // Verify the parsed results (Super=64, h=104)
        assert_eq!(prepared_data[0].0, 64);
        assert_eq!(prepared_data[0].1, 104);
        assert_eq!(prepared_data[0].2, Action::FocusLeft);

        // 4. Mock the Wayland interaction process
        // Assume these configurations were sent to River, returning object IDs 1001 and 1002
        let mock_object_id_h = 1001;
        let mock_object_id_q = 1002;

        input_manager.register_wayland_object(mock_object_id_h, prepared_data[0].2.clone());
        input_manager.register_wayland_object(mock_object_id_q, prepared_data[1].2.clone());

        assert_eq!(input_manager.bindings_map.len(), 2);

        // 5. Mock user pressing a key
        // Assume River sends a pressed event for object 1001
        let action_to_take = input_manager.handle_pressed(mock_object_id_h);

        // Accurately retrieve the corresponding action in O(1) time
        assert_eq!(action_to_take, Some(Action::FocusLeft));

        // Assume River sends a pressed event for object 1002
        let action_to_take = input_manager.handle_pressed(mock_object_id_q);
        assert_eq!(action_to_take, Some(Action::Quit));

        // Assume an event from an unknown object ID
        let unknown_action = input_manager.handle_pressed(9999);
        assert_eq!(unknown_action, None);
    }
}
