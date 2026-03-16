// src/key_repeat.rs

use calloop::{
    LoopHandle, RegistrationToken,
    timer::{TimeoutAction, Timer},
};
use std::fmt::Debug;
use std::time::Duration;

/// Represents the actions that can be triggered by a key repeat event.
#[derive(Debug, Clone, PartialEq)]
pub enum Action {
    FocusLeft,
    FocusRight,
}

/// A generic binding ID used to distinguish which shortcut triggered the event.
pub trait BindingId: Eq + Clone + Debug {}
impl<T: Eq + Clone + Debug> BindingId for T {}

/// Trait implemented by the application state to execute actions triggered by the timer.
///
/// This decouples the timer closure from the specific state implementation.
pub trait ExecuteAction {
    fn execute_action(&mut self, action: Action);
}

/// Manages the timers for key repeat logic.
pub struct KeyRepeatManager<ID: BindingId> {
    /// Records the ID of the currently repeating key and its cancellation token.
    current_timer: Option<(ID, RegistrationToken)>,

    /// Initial delay before repeating begins (default: 200ms).
    delay: Duration,
    /// Repeat rate after the initial delay (default: 30ms, approx. 33 times/sec).
    rate: Duration,
}

impl<ID: BindingId> KeyRepeatManager<ID> {
    /// Creates a new `KeyRepeatManager` with default delay and rate settings.
    pub fn new() -> Self {
        Self {
            current_timer: None,
            delay: Duration::from_millis(200),
            rate: Duration::from_millis(30),
        }
    }

    /// Starts the key repeat timer for a given action.
    ///
    /// `loop_handle`: Used to insert the timer into the event loop.
    /// `id`: The Wayland object ID of the triggered key.
    /// `action`: The action to execute.
    ///
    /// Note: This method only starts the timer. The initial immediate execution
    /// should be handled before calling this method.
    pub fn start_repeat<State: ExecuteAction + 'static>(
        &mut self,
        loop_handle: &LoopHandle<State>,
        id: ID,
        action: Action,
    ) {
        // Handle edge cases: If a previous key is still repeating, stop it immediately (Latest Key Wins).
        self.stop_repeat(loop_handle, None);

        // Create a timer with the initial delay.
        let timer = Timer::from_duration(self.delay);
        let rate = self.rate;

        // Insert the timer into the calloop event loop.
        let token = loop_handle
            .insert_source(timer, move |_event, _metadata, state: &mut State| {
                // Execute the action (calls the application state logic).
                state.execute_action(action.clone());

                // Core trick: Return ToDuration to change the next trigger time.
                // This switches the period from 200ms to 30ms, looping indefinitely until removed.
                TimeoutAction::ToDuration(rate)
            })
            .expect("Failed to insert timer into event loop");

        // Save the current ID and Token for later cancellation.
        self.current_timer = Some((id, token));
    }

    /// Stops the key repeat timer.
    ///
    /// `target_id`: The ID of the released key. If `None`, it unconditionally
    /// cancels the current timer.
    pub fn stop_repeat<State>(&mut self, loop_handle: &LoopHandle<State>, target_id: Option<ID>) {
        if let Some((ref current_id, token)) = self.current_timer {
            if target_id.is_none() || Some(current_id.clone()) == target_id {
                loop_handle.remove(token);
                self.current_timer = None;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use calloop::EventLoop;

    // Mock application state for testing purposes.
    struct MockShuttleState {
        pub execution_count: i32,
        pub repeat_manager: KeyRepeatManager<u32>, // Use u32 as Mock ID
    }

    impl ExecuteAction for MockShuttleState {
        fn execute_action(&mut self, action: Action) {
            if action == Action::FocusLeft {
                self.execution_count += 1;
            }
        }
    }

    #[test]
    fn test_key_repeat_timing() {
        // Initialize the calloop event loop.
        let mut event_loop: EventLoop<MockShuttleState> =
            EventLoop::try_new().expect("Failed to create event loop");

        let handle = event_loop.handle();

        // Initialize mock state.
        let mut state = MockShuttleState {
            execution_count: 0,
            repeat_manager: KeyRepeatManager::new(),
        };

        let mock_key_id = 1001;

        // Phase 1: Key Pressed
        // Simulate the external immediate execution.
        state.execute_action(Action::FocusLeft);
        // Start the repeater.
        state
            .repeat_manager
            .start_repeat(&handle, mock_key_id, Action::FocusLeft);

        assert_eq!(state.execution_count, 1);

        // Phase 2: 100ms passed (should not trigger)
        event_loop
            .dispatch(Duration::from_millis(100), &mut state)
            .unwrap();
        assert_eq!(state.execution_count, 1, "Should not trigger within 100ms");

        // Phase 3: Another 120ms passed, crossing the 200ms threshold
        // Dispatch returns early upon encountering an event, so this finishes the initial 200ms trigger.
        event_loop
            .dispatch(Duration::from_millis(120), &mut state)
            .unwrap();
        assert_eq!(
            state.execution_count, 2,
            "Crossed 200ms, triggered one repeat"
        );

        // Phase 4: Test consecutive 30ms repeats
        // Since dispatch returns on events, we call it multiple times with a generous 50ms window.
        event_loop
            .dispatch(Duration::from_millis(50), &mut state)
            .unwrap();
        assert_eq!(state.execution_count, 3, "First 30ms repeat");

        event_loop
            .dispatch(Duration::from_millis(50), &mut state)
            .unwrap();
        assert_eq!(state.execution_count, 4, "Second 30ms repeat");

        event_loop
            .dispatch(Duration::from_millis(50), &mut state)
            .unwrap();
        assert_eq!(state.execution_count, 5, "Third 30ms repeat");

        // Phase 5: Key Released
        state.repeat_manager.stop_repeat(&handle, Some(mock_key_id));

        // Phase 6: Confirm cancellation
        // Provide a long duration to ensure no further events are triggered.
        event_loop
            .dispatch(Duration::from_millis(200), &mut state)
            .unwrap();
        assert_eq!(state.execution_count, 5, "Should not trigger after release");
    }

    #[test]
    fn test_spurious_release() {
        // Verify: Press A, Press B, Release A -> B's repeat should not be interrupted.
        let event_loop: EventLoop<MockShuttleState> = EventLoop::try_new().unwrap();
        let handle = event_loop.handle();

        let mut state = MockShuttleState {
            execution_count: 0,
            repeat_manager: KeyRepeatManager::new(),
        };

        let key_a = 1;
        let key_b = 2;

        // 1. Press A
        state
            .repeat_manager
            .start_repeat(&handle, key_a, Action::FocusLeft);

        // 2. Press B (overrides A)
        state
            .repeat_manager
            .start_repeat(&handle, key_b, Action::FocusRight);

        // 3. Release A
        state.repeat_manager.stop_repeat(&handle, Some(key_a));

        // Verify B's timer is still active and not interrupted by A's release.
        assert!(state.repeat_manager.current_timer.is_some());
        assert_eq!(state.repeat_manager.current_timer.unwrap().0, key_b);
    }
}
