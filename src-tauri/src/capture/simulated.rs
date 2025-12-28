//! # Simulated Frame Capture
//!
//! Generates test pattern frames for testing the pipeline.
//! Used when no native capture is available or for unit tests.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use quest_shadowplay::capture::FrameCompressor;
use quest_shadowplay::CapturedFrame;

use super::{CaptureError, FrameCapture};

/// Simulated capture that generates test pattern frames.
pub struct SimulatedCapture {
    is_active: Arc<AtomicBool>,
    fps: u32,
    width: u32,
    height: u32,
}

impl SimulatedCapture {
    /// Creates a new simulated capture source.
    pub fn new() -> Self {
        Self {
            is_active: Arc::new(AtomicBool::new(false)),
            fps: 30, // Lower FPS for simulation
            width: 256,
            height: 256,
        }
    }

    /// Creates with custom parameters.
    #[allow(dead_code)]
    pub fn with_params(fps: u32, width: u32, height: u32) -> Self {
        Self {
            is_active: Arc::new(AtomicBool::new(false)),
            fps,
            width,
            height,
        }
    }
}

impl FrameCapture for SimulatedCapture {
    fn start(&self, on_frame: Arc<dyn Fn(CapturedFrame) + Send + Sync>) -> Result<(), CaptureError> {
        if self.is_active.swap(true, Ordering::SeqCst) {
            return Err(CaptureError::AlreadyRunning);
        }

        let is_active = Arc::clone(&self.is_active);
        let fps = self.fps;
        let width = self.width;
        let height = self.height;

        thread::spawn(move || {
            log::info!("Simulated capture started: {}x{} @ {} FPS", width, height, fps);

            let compressor = FrameCompressor::new(80);
            let frame_duration = Duration::from_micros(1_000_000 / fps as u64);
            let mut frame_number = 0u32;
            let start_time = Instant::now();

            while is_active.load(Ordering::SeqCst) {
                // Generate frame
                if let Some(frame) = generate_test_frame(&compressor, frame_number, width, height) {
                    on_frame(frame);
                }

                frame_number = frame_number.wrapping_add(1);

                // Maintain frame rate
                let elapsed = start_time.elapsed();
                let expected = frame_duration * frame_number;
                if expected > elapsed {
                    thread::sleep(expected - elapsed);
                }

                // Log progress periodically
                if frame_number % (fps * 5) == 0 {
                    log::debug!("Simulated: {} frames captured", frame_number);
                }
            }

            log::info!("Simulated capture stopped after {} frames", frame_number);
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
        "Simulated"
    }
}

impl Default for SimulatedCapture {
    fn default() -> Self {
        Self::new()
    }
}

/// Generates a test frame with a colorful moving pattern.
fn generate_test_frame(
    compressor: &FrameCompressor,
    frame_number: u32,
    width: u32,
    height: u32,
) -> Option<CapturedFrame> {
    let mut rgba = vec![0u8; (width * height * 4) as usize];

    // Create a moving gradient pattern
    let t = frame_number as f32 * 0.05;

    for y in 0..height {
        for x in 0..width {
            let idx = ((y * width + x) * 4) as usize;

            let fx = x as f32 / width as f32;
            let fy = y as f32 / height as f32;

            // Create RGB gradient that moves over time
            let r = ((fx * 255.0 + t * 50.0).sin().abs() * 255.0) as u8;
            let g = ((fy * 255.0 + t * 30.0).cos().abs() * 255.0) as u8;
            let b = (((fx + fy) * 127.5 + t * 40.0).sin().abs() * 255.0) as u8;

            rgba[idx] = r;
            rgba[idx + 1] = g;
            rgba[idx + 2] = b;
            rgba[idx + 3] = 255;
        }
    }

    // Compress to JPEG
    match compressor.compress(&rgba, width, height) {
        Ok(data) => Some(CapturedFrame::new(data, 0, width, height)),
        Err(e) => {
            log::warn!("Frame compression failed: {}", e);
            None
        }
    }
}
