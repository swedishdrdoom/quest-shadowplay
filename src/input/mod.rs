//! # Input Handler Module
//!
//! This module handles controller input detection for triggering clip saves.
//!
//! ## Plain English Explanation
//!
//! Your Quest controllers have buttons, triggers, and joysticks. This module
//! watches those inputs and detects when you want to save a clip.
//!
//! We use a button COMBINATION (like Ctrl+S on a keyboard) instead of a
//! single button, so you don't accidentally save clips while playing games.
//!
//! ```text
//!     Left Controller              Right Controller
//!    ┌─────────────────┐          ┌─────────────────┐
//!    │    [Menu]       │          │       [Menu]    │
//!    │                 │          │                 │
//!    │   ╭─────╮       │          │       ╭─────╮   │
//!    │   │Stick│       │          │       │Stick│   │
//!    │   ╰─────╯       │          │       ╰─────╯   │
//!    │                 │          │                 │
//!    │ [Grip]  [Trig]  │          │  [Trig]  [Grip] │
//!    └─────────────────┘          └─────────────────┘
//!
//!    Default save combo: Left Grip + Left Trigger (both held)
//! ```

use std::time::{Duration, Instant};

use crate::config::TriggerButton;

// ============================================
// INPUT STATE
// Current state of all controller inputs
// ============================================

/// Current state of all controller inputs
///
/// ## Plain English
///
/// This is a "snapshot" of what all the buttons look like right now.
/// It gets updated many times per second as you interact with the controllers.
#[derive(Clone, Debug, Default)]
pub struct InputState {
    // ----------------------------------------
    // LEFT CONTROLLER
    // ----------------------------------------
    
    /// Left trigger (index finger) value (0.0 to 1.0)
    ///
    /// ## Values
    /// - 0.0: Not pressed at all
    /// - 0.5: Pressed halfway
    /// - 1.0: Fully pressed
    pub left_trigger: f32,
    
    /// Left grip (middle finger) value (0.0 to 1.0)
    pub left_grip: f32,
    
    /// Left thumbstick X position (-1.0 to 1.0)
    pub left_stick_x: f32,
    
    /// Left thumbstick Y position (-1.0 to 1.0)
    pub left_stick_y: f32,
    
    /// Left thumbstick button pressed
    pub left_stick_click: bool,
    
    /// Left X button pressed
    pub left_x_button: bool,
    
    /// Left Y button pressed
    pub left_y_button: bool,
    
    /// Left menu button pressed (the hamburger icon)
    pub left_menu_button: bool,
    
    // ----------------------------------------
    // RIGHT CONTROLLER
    // ----------------------------------------
    
    /// Right trigger value (0.0 to 1.0)
    pub right_trigger: f32,
    
    /// Right grip value (0.0 to 1.0)
    pub right_grip: f32,
    
    /// Right thumbstick X position
    pub right_stick_x: f32,
    
    /// Right thumbstick Y position
    pub right_stick_y: f32,
    
    /// Right thumbstick button pressed
    pub right_stick_click: bool,
    
    /// Right A button pressed
    pub right_a_button: bool,
    
    /// Right B button pressed
    pub right_b_button: bool,
    
    /// Right Oculus/Meta button pressed (may not be accessible)
    pub right_system_button: bool,
}

impl InputState {
    /// Creates a new input state with all values at rest
    pub fn new() -> Self {
        Self::default()
    }

    /// Checks if the left trigger is "fully" pressed (>90%)
    pub fn left_trigger_pressed(&self) -> bool {
        self.left_trigger > 0.9
    }

    /// Checks if the left grip is "fully" pressed (>90%)
    pub fn left_grip_pressed(&self) -> bool {
        self.left_grip > 0.9
    }

    /// Checks if the right trigger is "fully" pressed (>90%)
    pub fn right_trigger_pressed(&self) -> bool {
        self.right_trigger > 0.9
    }

    /// Checks if the right grip is "fully" pressed (>90%)
    pub fn right_grip_pressed(&self) -> bool {
        self.right_grip > 0.9
    }
}

// ============================================
// INPUT HANDLER
// Detects save trigger and manages debouncing
// ============================================

/// Handles input detection and save triggering
///
/// ## Plain English
///
/// This is like a doorbell that:
/// 1. Watches for the right button combination
/// 2. Makes sure you really meant to press it (debouncing)
/// 3. Tells the app "save now!" when everything checks out
///
/// ## Debouncing Explained
///
/// When you press a button, it might "bounce" electrically, making it
/// look like multiple quick presses. Debouncing ignores presses that
/// happen too close together (within 500ms by default).
pub struct InputHandler {
    /// Which button(s) trigger a save
    trigger_button: TriggerButton,
    
    /// Minimum time between saves
    debounce_duration: Duration,
    
    /// When we last triggered a save
    last_trigger_time: Option<Instant>,
    
    /// Was the trigger held last frame? (for edge detection)
    was_pressed_last_frame: bool,
    
    /// Current input state
    current_state: InputState,
}

impl InputHandler {
    /// Creates a new input handler with the specified trigger button
    ///
    /// ## Parameters
    /// - `trigger_button`: Which button combination triggers saves
    ///
    /// ## Example
    /// ```
    /// let handler = InputHandler::new(TriggerButton::LeftGripAndTrigger);
    /// ```
    pub fn new(trigger_button: TriggerButton) -> Self {
        Self {
            trigger_button,
            debounce_duration: Duration::from_millis(500),
            last_trigger_time: None,
            was_pressed_last_frame: false,
            current_state: InputState::new(),
        }
    }

    /// Updates the current input state
    ///
    /// ## Plain English
    ///
    /// Call this every frame with the latest controller data.
    /// "Hey, here's what all the buttons look like now."
    pub fn update(&mut self, new_state: InputState) {
        self.current_state = new_state;
    }

    /// Checks if a save should be triggered
    ///
    /// ## Plain English
    ///
    /// Answers the question: "Should we save right now?"
    ///
    /// Returns `true` only when:
    /// 1. The trigger button combo is pressed
    /// 2. It wasn't pressed last frame (edge detection - catches the moment of press)
    /// 3. Enough time has passed since the last save (debouncing)
    ///
    /// ## Returns
    /// `true` if a save should be triggered, `false` otherwise
    pub fn check_save_triggered(&mut self) -> bool {
        let is_pressed = self.is_trigger_pressed();
        
        // Edge detection: only trigger on the rising edge (when first pressed)
        let just_pressed = is_pressed && !self.was_pressed_last_frame;
        self.was_pressed_last_frame = is_pressed;
        
        // If not just pressed, no trigger
        if !just_pressed {
            return false;
        }
        
        // Check debounce
        if let Some(last_time) = self.last_trigger_time {
            if last_time.elapsed() < self.debounce_duration {
                log::debug!("Save trigger ignored (debounce)");
                return false;
            }
        }
        
        // Trigger!
        self.last_trigger_time = Some(Instant::now());
        log::info!("Save triggered!");
        true
    }

    /// Checks if the trigger button combination is currently pressed
    ///
    /// ## Plain English
    ///
    /// "Is the user holding the save button combo right now?"
    fn is_trigger_pressed(&self) -> bool {
        match &self.trigger_button {
            TriggerButton::LeftGripAndTrigger => {
                self.current_state.left_grip_pressed() && 
                self.current_state.left_trigger_pressed()
            }
            TriggerButton::RightGripAndTrigger => {
                self.current_state.right_grip_pressed() && 
                self.current_state.right_trigger_pressed()
            }
            TriggerButton::BothGrips => {
                self.current_state.left_grip_pressed() && 
                self.current_state.right_grip_pressed()
            }
            TriggerButton::Custom { description: _ } => {
                // Custom bindings would need additional implementation
                false
            }
        }
    }

    /// Changes the trigger button configuration
    pub fn set_trigger_button(&mut self, button: TriggerButton) {
        self.trigger_button = button;
        log::info!("Save trigger changed to: {:?}", self.trigger_button);
    }

    /// Changes the debounce duration
    ///
    /// ## Parameters
    /// - `duration_ms`: Minimum milliseconds between saves
    pub fn set_debounce_ms(&mut self, duration_ms: u64) {
        self.debounce_duration = Duration::from_millis(duration_ms);
    }

    /// Returns the current input state
    pub fn current_state(&self) -> &InputState {
        &self.current_state
    }

    /// Returns whether the trigger combo is currently held
    pub fn is_trigger_held(&self) -> bool {
        self.is_trigger_pressed()
    }

    /// Returns time since last save, or None if never saved
    pub fn time_since_last_save(&self) -> Option<Duration> {
        self.last_trigger_time.map(|t| t.elapsed())
    }
}

impl Default for InputHandler {
    fn default() -> Self {
        Self::new(TriggerButton::default())
    }
}

// ============================================
// HAPTIC FEEDBACK
// Controller vibration for user feedback
// ============================================

/// Parameters for haptic (vibration) feedback
///
/// ## Plain English
///
/// This describes how the controller should vibrate.
/// Like choosing a ringtone - you can pick the duration,
/// strength, and pattern.
#[derive(Clone, Debug)]
pub struct HapticFeedback {
    /// How long to vibrate (in milliseconds)
    pub duration_ms: u32,
    
    /// Vibration strength (0.0 to 1.0)
    ///
    /// ## Values
    /// - 0.0: No vibration
    /// - 0.5: Medium vibration
    /// - 1.0: Maximum vibration
    pub amplitude: f32,
    
    /// Vibration frequency in Hz (optional)
    ///
    /// ## Values
    /// - Low frequency (~50 Hz): Deep rumble
    /// - Medium (~150 Hz): Standard vibration
    /// - High (~300 Hz): Sharp buzz
    pub frequency: Option<f32>,
}

impl HapticFeedback {
    /// Creates a short "click" feedback
    ///
    /// ## When to Use
    /// Confirming a button press - "Got it!"
    pub fn click() -> Self {
        Self {
            duration_ms: 50,
            amplitude: 0.7,
            frequency: Some(200.0),
        }
    }

    /// Creates a "success" feedback
    ///
    /// ## When to Use
    /// Operation completed successfully - "Done!"
    pub fn success() -> Self {
        Self {
            duration_ms: 200,
            amplitude: 0.8,
            frequency: Some(250.0),
        }
    }

    /// Creates an "error" feedback
    ///
    /// ## When to Use
    /// Something went wrong - "Oops!"
    pub fn error() -> Self {
        Self {
            duration_ms: 300,
            amplitude: 1.0,
            frequency: Some(100.0),
        }
    }

    /// Creates a "working" feedback
    ///
    /// ## When to Use
    /// Long operation in progress - "Working on it..."
    pub fn working() -> Self {
        Self {
            duration_ms: 100,
            amplitude: 0.4,
            frequency: Some(150.0),
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
        assert_eq!(state.left_grip, 0.0);
        assert!(!state.left_trigger_pressed());
        assert!(!state.left_grip_pressed());
    }

    #[test]
    fn test_input_state_pressed() {
        let mut state = InputState::new();
        state.left_trigger = 1.0;
        state.left_grip = 0.95;
        
        assert!(state.left_trigger_pressed());
        assert!(state.left_grip_pressed());
    }

    #[test]
    fn test_trigger_detection() {
        let mut handler = InputHandler::new(TriggerButton::LeftGripAndTrigger);
        
        // No press
        let mut state = InputState::new();
        handler.update(state.clone());
        assert!(!handler.check_save_triggered());
        
        // Full press
        state.left_trigger = 1.0;
        state.left_grip = 1.0;
        handler.update(state);
        assert!(handler.check_save_triggered());
        
        // Should not trigger again immediately (held)
        assert!(!handler.check_save_triggered());
    }

    #[test]
    fn test_debounce() {
        let mut handler = InputHandler::new(TriggerButton::LeftGripAndTrigger);
        handler.set_debounce_ms(100); // Short debounce for testing
        
        let mut state = InputState::new();
        state.left_trigger = 1.0;
        state.left_grip = 1.0;
        
        // First press
        handler.update(state.clone());
        assert!(handler.check_save_triggered());
        
        // Release and press again quickly
        handler.update(InputState::new()); // Release
        handler.update(state.clone()); // Press again
        assert!(!handler.check_save_triggered()); // Should be debounced
    }

    #[test]
    fn test_haptic_presets() {
        let click = HapticFeedback::click();
        assert!(click.duration_ms < 100);
        
        let success = HapticFeedback::success();
        assert!(success.duration_ms > click.duration_ms);
        
        let error = HapticFeedback::error();
        assert_eq!(error.amplitude, 1.0); // Max amplitude for errors
    }
}

