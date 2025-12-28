//! # Android Screen Capture using MediaProjection
//!
//! Uses Android's MediaProjection API for screen capture.
//! Requires user permission granted via system dialog.
//!
//! **Note**: This is a stub that uses simulated capture.
//! Real MediaProjection integration requires JNI calls to Android APIs.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use quest_shadowplay::capture::FrameCompressor;
use quest_shadowplay::CapturedFrame;

use super::{CaptureError, FrameCapture};

/// Android screen capture using MediaProjection.
///
/// Currently uses simulated frames while the real JNI
/// implementation is developed.
pub struct AndroidCapture {
    is_active: Arc<AtomicBool>,
    fps: u32,
}

impl AndroidCapture {
    /// Creates a new Android capture source.
    pub fn new() -> Self {
        Self {
            is_active: Arc::new(AtomicBool::new(false)),
            fps: 60, // Quest 3 target FPS
        }
    }
}

impl Default for AndroidCapture {
    fn default() -> Self {
        Self::new()
    }
}

impl FrameCapture for AndroidCapture {
    fn start(&self, on_frame: Arc<dyn Fn(CapturedFrame) + Send + Sync>) -> Result<(), CaptureError> {
        if self.is_active.swap(true, Ordering::SeqCst) {
            return Err(CaptureError::AlreadyRunning);
        }

        // TODO: Real implementation would:
        // 1. Request MediaProjection permission via Activity
        // 2. Create VirtualDisplay
        // 3. Get Surface and set up ImageReader
        // 4. Process frames in native code

        let is_active = Arc::clone(&self.is_active);
        let fps = self.fps;

        thread::spawn(move || {
            log::info!("Android capture started at {} FPS (simulated)", fps);

            let compressor = FrameCompressor::new(80);
            let frame_duration = Duration::from_micros(1_000_000 / fps as u64);
            let mut frame_number = 0u32;

            // Simulated Quest 3 resolution
            let width = 1832u32;  // Quest 3 eye resolution
            let height = 1920u32;

            while is_active.load(Ordering::SeqCst) {
                let frame_start = std::time::Instant::now();

                // Generate test frame (replace with real MediaProjection)
                if let Some(frame) = generate_test_frame(&compressor, frame_number, width, height) {
                    on_frame(frame);
                }

                frame_number = frame_number.wrapping_add(1);

                // Maintain frame rate
                let elapsed = frame_start.elapsed();
                if elapsed < frame_duration {
                    thread::sleep(frame_duration - elapsed);
                }

                if frame_number % (fps * 10) == 0 {
                    log::info!("Android capture: {} frames", frame_number);
                }
            }

            log::info!("Android capture stopped after {} frames", frame_number);
        });

        Ok(())
    }

    fn stop(&self) {
        self.is_active.store(false, Ordering::SeqCst);
    }

    fn is_active(&self) -> bool {
        self.is_active.load(Ordering::SeqCst)
    }

    fn source_name(&self) -> &'static str {
        "Android MediaProjection"
    }
}

/// Generates a test frame (placeholder for real MediaProjection).
fn generate_test_frame(
    compressor: &FrameCompressor,
    frame_number: u32,
    width: u32,
    height: u32,
) -> Option<CapturedFrame> {
    // For Quest, generate a more interesting VR-like pattern
    let mut rgba = vec![0u8; (width * height * 4) as usize];

    let t = frame_number as f32 * 0.02;
    let center_x = width as f32 / 2.0;
    let center_y = height as f32 / 2.0;

    for y in 0..height {
        for x in 0..width {
            let idx = ((y * width + x) * 4) as usize;

            // Create radial pattern simulating VR content
            let dx = x as f32 - center_x;
            let dy = y as f32 - center_y;
            let dist = (dx * dx + dy * dy).sqrt();
            let angle = dy.atan2(dx);

            let r = ((dist * 0.01 + angle + t).sin() * 127.0 + 128.0) as u8;
            let g = ((dist * 0.02 + angle * 2.0 + t * 1.5).cos() * 127.0 + 128.0) as u8;
            let b = ((angle * 3.0 + t * 2.0).sin() * 127.0 + 128.0) as u8;

            rgba[idx] = r;
            rgba[idx + 1] = g;
            rgba[idx + 2] = b;
            rgba[idx + 3] = 255;
        }
    }

    match compressor.compress(&rgba, width, height) {
        Ok(data) => Some(CapturedFrame::new(data, 0, width, height)),
        Err(e) => {
            if frame_number % 100 == 0 {
                log::warn!("Frame compression failed: {}", e);
            }
            None
        }
    }
}

