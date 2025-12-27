//! # Input Handler Module
//!
//! Controller input detection for triggering clip saves.
//!
//! ## Plain English
//!
//! Watches controller buttons and detects when you want to save.
//! Uses button COMBINATIONS (not single buttons) to prevent accidents.

use std::time::{Duration, Instant};

use crate::config::TriggerButton;

// ============================================
// INPUT STATE
// ============================================

/// Current state of all controller inputs.
///
/// Updated every frame with the latest controller data.
#[derive(Clone, Debug, Default)]
pub struct InputState {
    // Left controller
    /// Left trigger value (0.0 to 1.0)
    pub left_trigger: f32,
    /// Left grip value (0.0 to 1.0)
    pub left_grip: f32,
    /// Left thumbstick X (-1.0 to 1.0)
    pub left_stick_x: f32,
    /// Left thumbstick Y (-1.0 to 1.0)
    pub left_stick_y: f32,
    /// Left X button
    pub left_x: bool,
    /// Left Y button
    pub left_y: bool,
    /// Left menu button
    pub left_menu: bool,

    // Right controller
    /// Right trigger value (0.0 to 1.0)
    pub right_trigger: f32,
    /// Right grip value (0.0 to 1.0)
    pub right_grip: f32,
    /// Right thumbstick X (-1.0 to 1.0)
    pub right_stick_x: f32,
    /// Right thumbstick Y (-1.0 to 1.0)
    pub right_stick_y: f32,
    /// Right A button
    pub right_a: bool,
    /// Right B button
    pub right_b: bool,
}

impl InputState {
    /// Creates a new input state with all values at rest.
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns true if left trigger is fully pressed (>90%).
    pub fn left_trigger_pressed(&self) -> bool {
        self.left_trigger > 0.9
    }

    /// Returns true if left grip is fully pressed (>90%).
    pub fn left_grip_pressed(&self) -> bool {
        self.left_grip > 0.9
    }

    /// Returns true if right trigger is fully pressed (>90%).
    pub fn right_trigger_pressed(&self) -> bool {
        self.right_trigger > 0.9
    }

    /// Returns true if right grip is fully pressed (>90%).
    pub fn right_grip_pressed(&self) -> bool {
        self.right_grip > 0.9
    }
}

// ============================================
// INPUT HANDLER
// ============================================

/// Handles input detection and save triggering.
///
/// ## Features
/// - Watches for configured button combination
/// - Debouncing prevents accidental double-triggers
/// - Edge detection catches the moment of press
pub struct InputHandler {
    /// Which buttons trigger a save
    trigger_button: TriggerButton,

    /// Minimum time between saves
    debounce_duration: Duration,

    /// When we last triggered
    last_trigger_time: Option<Instant>,

    /// Was pressed last frame?
    was_pressed: bool,

    /// Current input state
    current_state: InputState,
}

impl InputHandler {
    /// Creates a new input handler.
    pub fn new(trigger_button: TriggerButton) -> Self {
        Self {
            trigger_button,
            debounce_duration: Duration::from_millis(500),
            last_trigger_time: None,
            was_pressed: false,
            current_state: InputState::new(),
        }
    }

    /// Updates the input state.
    pub fn update(&mut self, state: InputState) {
        self.current_state = state;
    }

    /// Checks if a save should be triggered.
    ///
    /// Returns `true` only on the rising edge of the button press
    /// (the moment it's first pressed) and respects debouncing.
    pub fn check_save_triggered(&mut self) -> bool {
        let is_pressed = self.is_combo_pressed();

        // Edge detection: only trigger when first pressed
        let just_pressed = is_pressed && !self.was_pressed;
        self.was_pressed = is_pressed;

        if !just_pressed {
            return false;
        }

        // Debounce check
        if let Some(last) = self.last_trigger_time {
            if last.elapsed() < self.debounce_duration {
                log::debug!("Save trigger debounced");
                return false;
            }
        }

        self.last_trigger_time = Some(Instant::now());
        log::info!("Save triggered!");
        true
    }

    /// Checks if the trigger combo is currently pressed.
    fn is_combo_pressed(&self) -> bool {
        match &self.trigger_button {
            TriggerButton::LeftGripAndTrigger => {
                self.current_state.left_grip_pressed()
                    && self.current_state.left_trigger_pressed()
            }
            TriggerButton::RightGripAndTrigger => {
                self.current_state.right_grip_pressed()
                    && self.current_state.right_trigger_pressed()
            }
            TriggerButton::BothGrips => {
                self.current_state.left_grip_pressed()
                    && self.current_state.right_grip_pressed()
            }
        }
    }

    /// Changes the trigger button.
    pub fn set_trigger_button(&mut self, button: TriggerButton) {
        self.trigger_button = button;
    }

    /// Changes the debounce duration.
    pub fn set_debounce_ms(&mut self, ms: u64) {
        self.debounce_duration = Duration::from_millis(ms);
    }

    /// Returns the current input state.
    pub fn current_state(&self) -> &InputState {
        &self.current_state
    }

    /// Returns whether the combo is currently held.
    pub fn is_combo_held(&self) -> bool {
        self.is_combo_pressed()
    }
}

impl Default for InputHandler {
    fn default() -> Self {
        Self::new(TriggerButton::default())
    }
}

// ============================================
// HAPTIC FEEDBACK
// ============================================

/// Parameters for haptic (vibration) feedback.
#[derive(Clone, Debug)]
pub struct HapticParams {
    /// Duration in milliseconds
    pub duration_ms: u32,
    /// Amplitude (0.0 to 1.0)
    pub amplitude: f32,
    /// Frequency in Hz (optional)
    pub frequency: Option<f32>,
}

impl HapticParams {
    /// Short click feedback.
    pub fn click() -> Self {
        Self {
            duration_ms: 50,
            amplitude: 0.7,
            frequency: Some(200.0),
        }
    }

    /// Success feedback.
    pub fn success() -> Self {
        Self {
            duration_ms: 200,
            amplitude: 0.8,
            frequency: Some(250.0),
        }
    }

    /// Error feedback.
    pub fn error() -> Self {
        Self {
            duration_ms: 300,
            amplitude: 1.0,
            frequency: Some(100.0),
        }
    }
}

// ============================================
// TESTS
// ============================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_input_state_default() {
        let state = InputState::new();
        assert_eq!(state.left_trigger, 0.0);
        assert!(!state.left_trigger_pressed());
    }

    #[test]
    fn test_input_pressed() {
        let mut state = InputState::new();
        state.left_trigger = 1.0;
        state.left_grip = 0.95;

        assert!(state.left_trigger_pressed());
        assert!(state.left_grip_pressed());
    }

    #[test]
    fn test_trigger_detection() {
        let mut handler = InputHandler::new(TriggerButton::LeftGripAndTrigger);

        // Not pressed
        handler.update(InputState::new());
        assert!(!handler.check_save_triggered());

        // Full press
        let mut state = InputState::new();
        state.left_trigger = 1.0;
        state.left_grip = 1.0;
        handler.update(state);
        assert!(handler.check_save_triggered());

        // Still held - should not trigger again
        assert!(!handler.check_save_triggered());
    }

    #[test]
    fn test_debounce() {
        let mut handler = InputHandler::new(TriggerButton::LeftGripAndTrigger);
        handler.set_debounce_ms(100);

        let mut pressed = InputState::new();
        pressed.left_trigger = 1.0;
        pressed.left_grip = 1.0;

        // First press
        handler.update(pressed.clone());
        assert!(handler.check_save_triggered());

        // Release
        handler.update(InputState::new());

        // Press again immediately - should be debounced
        handler.update(pressed);
        assert!(!handler.check_save_triggered());
    }
}
