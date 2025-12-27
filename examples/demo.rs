//! # Quest Shadowplay Demo
//!
//! This example demonstrates how Quest Shadowplay works.
//! It simulates VR frame capture and the save workflow.
//!
//! Run with: `cargo run --example demo`

use std::thread;
use std::time::Duration;

use quest_shadowplay::{CapturedFrame, QuestShadowplay};

fn main() {
    // Initialize logging so we can see what's happening
    quest_shadowplay::init_logging();

    println!("╔════════════════════════════════════════════════════════════╗");
    println!("║           Quest Shadowplay Demo                            ║");
    println!("╠════════════════════════════════════════════════════════════╣");
    println!("║ This demo simulates VR frame capture for 5 seconds,        ║");
    println!("║ then saves the buffer to a file.                           ║");
    println!("╚════════════════════════════════════════════════════════════╝");
    println!();

    // Create the application with default config
    let app = QuestShadowplay::new().expect("Failed to create app");

    println!("📹 Configuration:");
    println!("   Buffer duration: {} seconds", app.config().buffer_duration_seconds);
    println!("   Target FPS: {}", app.config().target_fps);
    println!("   Expected frames: {}", app.config().buffer_frame_count());
    println!("   Estimated memory: {:.1} MB", app.config().estimated_memory_mb());
    println!();

    // Simulate VR frame capture
    println!("🎮 Simulating VR frame capture...");
    println!();

    let total_frames = 450; // 5 seconds at 90fps
    let frame_duration = Duration::from_micros(11111); // ~90fps

    for i in 0..total_frames {
        // Create a simulated frame (in reality, this would come from OpenXR)
        let frame = create_simulated_frame(i);
        
        // Feed it to the app
        app.on_frame_captured(frame);

        // Progress indicator every second
        if i > 0 && i % 90 == 0 {
            let seconds = i / 90;
            let fill = app.buffer_fill() * 100.0;
            println!(
                "   ⏱️  {}s elapsed | Buffer: {:.1}% full ({} frames)",
                seconds,
                fill,
                app.buffer_frame_count()
            );
        }

        // Simulate frame timing (but faster for demo)
        thread::sleep(frame_duration / 10);
    }

    println!();
    println!("✅ Capture complete!");
    println!("   Total frames captured: {}", app.stats().frames_received);
    println!("   Buffer fill: {:.1}%", app.buffer_fill() * 100.0);
    println!();

    // Trigger a save
    println!("💾 Triggering save...");
    app.trigger_save();

    // Wait for save to complete
    while app.is_saving() {
        print!(".");
        thread::sleep(Duration::from_millis(100));
    }
    println!();
    println!();

    // Show results
    println!("📊 Final Statistics:");
    let stats = app.stats();
    println!("   Frames received: {}", stats.frames_received);
    println!("   Clips saved: {}", stats.clips_saved);
    println!("   Save errors: {}", stats.save_errors);
    println!();

    // Shutdown gracefully
    app.shutdown();

    println!("👋 Demo complete!");
}

/// Creates a simulated VR frame.
///
/// In a real implementation, this would be actual pixel data
/// captured from the OpenXR eye buffer.
fn create_simulated_frame(frame_number: u32) -> CapturedFrame {
    // Simulate a 100x100 image with varying colors
    // (Real Quest 3 frames are ~1832x1920 per eye)
    let width = 100;
    let height = 100;
    
    let mut rgba = vec![0u8; (width * height * 4) as usize];
    
    // Create a simple pattern that changes each frame
    for y in 0..height {
        for x in 0..width {
            let idx = ((y * width + x) * 4) as usize;
            rgba[idx] = ((x + frame_number) % 256) as u8;     // R
            rgba[idx + 1] = ((y + frame_number) % 256) as u8; // G
            rgba[idx + 2] = (frame_number % 256) as u8;       // B
            rgba[idx + 3] = 255;                              // A
        }
    }

    // Compress and create frame
    let compressor = quest_shadowplay::capture::FrameCompressor::new(80);
    let compressed = compressor.compress(&rgba, width, height)
        .expect("Compression failed");

    CapturedFrame::new(compressed, 0, width, height)
}

