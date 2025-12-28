//! # macOS Screen Capture using Core Graphics
//!
//! Uses Core Graphics for efficient screen capture on macOS.
//! This provides reliable capture that works across macOS versions.
//!
//! **Privacy Note**: User must grant Screen Recording permission in
//! System Preferences > Privacy & Security > Screen Recording.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use core_graphics::display::{CGDisplay, CGRect};
use quest_shadowplay::capture::FrameCompressor;
use quest_shadowplay::CapturedFrame;

use super::{CaptureError, FrameCapture};

/// Target width for captured frames (downscaled for performance)
const TARGET_WIDTH: u32 = 1280;

/// macOS screen capture using Core Graphics.
///
/// Captures the main display at the specified frame rate.
/// Falls back gracefully if capture permissions are denied.
pub struct MacOSCapture {
    is_active: Arc<AtomicBool>,
    fps: u32,
}

impl MacOSCapture {
    /// Creates a new macOS capture source.
    pub fn new() -> Self {
        Self {
            is_active: Arc::new(AtomicBool::new(false)),
            fps: 30, // 30 FPS for Mac testing
        }
    }
}

impl Default for MacOSCapture {
    fn default() -> Self {
        Self::new()
    }
}

impl FrameCapture for MacOSCapture {
    fn start(&self, on_frame: Arc<dyn Fn(CapturedFrame) + Send + Sync>) -> Result<(), CaptureError> {
        if self.is_active.swap(true, Ordering::SeqCst) {
            return Err(CaptureError::AlreadyRunning);
        }

        let is_active = Arc::clone(&self.is_active);
        let fps = self.fps;

        thread::spawn(move || {
            log::info!("macOS capture started at {} FPS (downscaled to {}px wide)", fps, TARGET_WIDTH);

            let compressor = FrameCompressor::new(70); // Lower quality for speed
            let frame_duration = Duration::from_micros(1_000_000 / fps as u64);
            let mut frame_number = 0u32;

            while is_active.load(Ordering::SeqCst) {
                let frame_start = std::time::Instant::now();

                // Capture screen
                match capture_main_display(&compressor) {
                    Ok(frame) => {
                        on_frame(frame);
                    }
                    Err(e) => {
                        // Don't spam logs - only log occasionally
                        if frame_number % 100 == 0 {
                            log::warn!("Screen capture failed: {}", e);
                        }
                    }
                }

                frame_number = frame_number.wrapping_add(1);

                // Maintain frame rate
                let elapsed = frame_start.elapsed();
                if elapsed < frame_duration {
                    thread::sleep(frame_duration - elapsed);
                }

                // Log progress periodically
                if frame_number % (fps * 10) == 0 {
                    let fps_actual = frame_number as f32 / frame_start.elapsed().as_secs_f32().max(0.001);
                    log::info!("macOS capture: {} frames (~{:.1} FPS)", frame_number, fps_actual);
                }
            }

            log::info!("macOS capture stopped after {} frames", frame_number);
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
        "macOS Screen Capture"
    }
}

/// Captures the main display using Core Graphics, with downscaling.
fn capture_main_display(compressor: &FrameCompressor) -> Result<CapturedFrame, String> {
    // Get main display bounds
    let display = CGDisplay::main();
    let bounds = display.bounds();

    // Take screenshot of main display
    let image = CGDisplay::screenshot(
        CGRect::new(
            &core_graphics::geometry::CGPoint::new(0.0, 0.0),
            &bounds.size,
        ),
        core_graphics::display::kCGWindowListOptionOnScreenOnly,
        core_graphics::window::kCGNullWindowID,
        core_graphics::display::kCGWindowImageDefault,
    )
    .ok_or_else(|| "Failed to capture screen - check Screen Recording permissions".to_string())?;

    // Get image properties
    let src_width = image.width() as u32;
    let src_height = image.height() as u32;
    let bytes_per_row = image.bytes_per_row();

    // Calculate target dimensions (maintain aspect ratio)
    let scale = TARGET_WIDTH as f32 / src_width as f32;
    let dst_width = TARGET_WIDTH;
    let dst_height = (src_height as f32 * scale) as u32;

    // Get raw pixel data
    let data = image.data();
    let pixel_data = data.bytes();

    // Downscale using nearest-neighbor (fast) while converting BGRA→RGBA
    let mut rgba = Vec::with_capacity((dst_width * dst_height * 4) as usize);
    
    for dst_y in 0..dst_height {
        let src_y = (dst_y as f32 / scale) as u32;
        for dst_x in 0..dst_width {
            let src_x = (dst_x as f32 / scale) as u32;
            let idx = (src_y as usize * bytes_per_row) + (src_x as usize * 4);
            
            if idx + 3 < pixel_data.len() {
                rgba.push(pixel_data[idx + 2]); // R (from B position)
                rgba.push(pixel_data[idx + 1]); // G
                rgba.push(pixel_data[idx]);     // B (from R position)
                rgba.push(255);                 // A
            } else {
                // Fallback for edge cases
                rgba.extend_from_slice(&[0, 0, 0, 255]);
            }
        }
    }

    // Compress to JPEG
    let compressed = compressor
        .compress(&rgba, dst_width, dst_height)
        .map_err(|e| format!("Compression failed: {}", e))?;

    Ok(CapturedFrame::new(compressed, 0, dst_width, dst_height))
}
