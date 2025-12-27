# Quest Shadowplay - Implementation Plan

## Executive Summary

This document provides a step-by-step implementation guide for building Quest Shadowplay, a "replay buffer" application for Meta Quest 3. Each section includes technical details AND plain-English explanations for non-programmers.

---

## Phase 1: Development Environment Setup

### What We're Doing (Plain English)
> Before we can build anything, we need to set up our "workshop" with all the right tools. This is like gathering your ingredients and preheating the oven before cooking.

### Technical Setup Steps

#### 1.1 Install Rust Toolchain
```bash
# Install Rust (if not already installed)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Add Android target for Quest 3 (ARM64 processor)
rustup target add aarch64-linux-android
```

**Plain English**: Rust is our programming language. The Quest 3 uses a different type of computer chip (ARM) than your laptop (usually Intel/AMD), so we need to teach Rust how to build for Quest's chip.

#### 1.2 Install Android NDK
```bash
# Download Android NDK (Native Development Kit)
# Required version: NDK r25 or later

# On macOS with Homebrew:
brew install android-ndk

# Set environment variable
export ANDROID_NDK_HOME=/path/to/android-ndk
```

**Plain English**: The NDK is a translator. It helps convert our Rust code into something the Quest 3 (which runs Android) can understand and run.

#### 1.3 Install Build Tools
```bash
# Install cargo-ndk for easier Android builds
cargo install cargo-ndk

# Install xbuild for APK packaging (alternative)
cargo install xbuild
```

#### 1.4 Quest 3 Developer Setup
1. Create a Meta Developer account at developer.oculus.com
2. Enable Developer Mode on your Quest 3
3. Install ADB (Android Debug Bridge) for sideloading

**Plain English**: Meta requires you to register as a developer to install custom apps. "Sideloading" means installing apps outside the official store - like installing a program from the internet instead of the app store.

---

## Phase 2: Project Skeleton

### What We're Doing (Plain English)
> Now we're creating the basic structure of our app - like drawing the floor plan before building a house. We'll make empty "rooms" (folders and files) that we'll fill in later.

### 2.1 Create Project Structure

```bash
cargo new quest-shadowplay --lib
cd quest-shadowplay
```

### 2.2 Configure Cargo.toml

```toml
[package]
name = "quest-shadowplay"
version = "0.1.0"
edition = "2021"

[lib]
crate-type = ["cdylib"]  # Creates a shared library for Android

[dependencies]
# OpenXR bindings for VR API access
openxr = "0.18"

# Android-specific functionality
ndk = "0.8"
ndk-glue = "0.7"

# Logging
log = "0.4"
android_logger = "0.13"

# Ring buffer implementation
ringbuf = "0.3"

# Image compression
image = "0.24"
turbojpeg = "0.5"  # Fast JPEG encoding

# Threading
crossbeam = "0.8"
parking_lot = "0.12"

# Time handling
chrono = "0.4"

[target.'cfg(target_os = "android")'.dependencies]
jni = "0.21"

[profile.release]
opt-level = 3
lto = true  # Link-time optimization for smaller binary
```

**Plain English Explanation of Dependencies**:
| Dependency | What It Does | Analogy |
|------------|--------------|---------|
| `openxr` | Talks to VR system | A translator for VR language |
| `ndk` | Android system access | Keys to Android's rooms |
| `ringbuf` | Circular buffer | That sushi conveyor belt |
| `turbojpeg` | Fast image compression | A photo-shrinking machine |
| `crossbeam` | Safe multitasking | A traffic controller for tasks |
| `chrono` | Date/time handling | A calendar and clock |

---

## Phase 3: Frame Capture Implementation

### What We're Doing (Plain English)
> This is the core of our app. We're building a "tap" on the video stream going to your eyes. Every single image that your Quest shows you, we'll grab a copy of it. It's like having a photocopier attached to a TV that copies every frame.

### 3.1 OpenXR Layer Concept

OpenXR layers work by intercepting function calls. When a VR app calls `xrEndFrame()` to display a frame, our layer sees it first.

```
┌─────────────┐      ┌─────────────┐      ┌─────────────┐
│   VR Game   │ ──▶  │  Our Layer  │ ──▶  │   Runtime   │
└─────────────┘      └─────────────┘      └─────────────┘
                           │
                           ▼
                    ┌─────────────┐
                    │ Frame Copy  │
                    └─────────────┘
```

### 3.2 Layer Implementation Structure

```rust
// src/capture/openxr_layer.rs

/// This structure holds all the original OpenXR functions
/// We call these after doing our frame capture
struct OpenXRFunctions {
    // The real xrEndFrame function from the runtime
    real_end_frame: PFN_xrEndFrame,
    // ... other functions we might need
}

/// Called by OpenXR when the app submits a frame
/// This is our "tap" on the video stream
pub extern "C" fn hooked_xr_end_frame(
    session: XrSession,
    frame_end_info: *const XrFrameEndInfo,
) -> XrResult {
    // 1. Access the frame data from frame_end_info
    // 2. Copy the frame textures to our buffer
    // 3. Call the real xrEndFrame to continue normal operation
}
```

**Plain English**: 
> Imagine you're a mail sorter. Every letter (frame) that comes through, you quickly photocopy it, then put the original back in the mail stream. The sender and receiver never know you made a copy. That's exactly what our "hooked" function does.

### 3.3 Frame Data Access

```rust
// Accessing the actual pixel data from GPU textures
pub struct CapturedFrame {
    /// Raw pixel data (compressed JPEG)
    pub data: Vec<u8>,
    /// Timestamp when frame was captured
    pub timestamp: u64,
    /// Which eye this is for (0 = left, 1 = right)
    pub eye_index: u32,
    /// Frame dimensions
    pub width: u32,
    pub height: u32,
}

impl CapturedFrame {
    /// Captures frame from GPU texture
    /// This is the tricky part - getting pixels off the GPU
    pub fn capture_from_texture(texture: &GpuTexture) -> Self {
        // 1. Create a staging buffer (CPU-accessible copy of GPU memory)
        // 2. Issue GPU copy command
        // 3. Wait for copy to complete
        // 4. Read pixels from staging buffer
        // 5. Compress to JPEG
        // 6. Return CapturedFrame
    }
}
```

**Plain English**:
> The GPU (graphics chip) keeps images in its own special memory that's super fast but hard to access. Getting the image is like asking someone in a secure building to make you a copy - you can't just walk in and grab it. We create a "staging area" where the GPU can drop off copies for us to pick up.

---

## Phase 4: Circular Buffer Implementation

### What We're Doing (Plain English)
> We need to store 10 seconds of video, but we can't keep recording forever - we'd run out of memory! So we use a "circular buffer" - imagine a circular track with a train of 900 cars (10 seconds × 90 frames per second). When a new frame arrives, the oldest car gets emptied and refilled with the new frame. The train never grows, but it always has the most recent 10 seconds.

### 4.1 Ring Buffer Design

```rust
// src/buffer/ring_buffer.rs

use parking_lot::RwLock;

/// A fixed-size buffer that overwrites old data
/// when new data arrives and the buffer is full
pub struct FrameRingBuffer {
    /// The actual storage for frames
    frames: Vec<Option<CapturedFrame>>,
    /// Index where the next frame will be written
    write_index: usize,
    /// Total number of frames stored
    count: usize,
    /// Maximum frames to store
    capacity: usize,
}

impl FrameRingBuffer {
    /// Creates a new buffer that holds `duration_seconds` of frames
    pub fn new(duration_seconds: f32, fps: u32) -> Self {
        let capacity = (duration_seconds * fps as f32) as usize;
        Self {
            frames: vec![None; capacity],
            write_index: 0,
            count: 0,
            capacity,
        }
    }

    /// Adds a frame, potentially overwriting the oldest one
    pub fn push(&mut self, frame: CapturedFrame) {
        self.frames[self.write_index] = Some(frame);
        self.write_index = (self.write_index + 1) % self.capacity;
        if self.count < self.capacity {
            self.count += 1;
        }
    }

    /// Gets all frames in chronological order (oldest first)
    pub fn get_all_frames(&self) -> Vec<&CapturedFrame> {
        let mut result = Vec::with_capacity(self.count);
        
        // Start from the oldest frame
        let start = if self.count < self.capacity {
            0
        } else {
            self.write_index
        };
        
        for i in 0..self.count {
            let index = (start + i) % self.capacity;
            if let Some(ref frame) = self.frames[index] {
                result.push(frame);
            }
        }
        
        result
    }
}
```

**Visual Representation**:
```
Initial state (empty):
┌───┬───┬───┬───┬───┬───┬───┬───┐
│   │   │   │   │   │   │   │   │
└───┴───┴───┴───┴───┴───┴───┴───┘
  ▲
  └── write_index

After 3 frames:
┌───┬───┬───┬───┬───┬───┬───┬───┐
│ A │ B │ C │   │   │   │   │   │
└───┴───┴───┴───┴───┴───┴───┴───┘
              ▲
              └── write_index

After buffer is full + 2 more frames:
┌───┬───┬───┬───┬───┬───┬───┬───┐
│ I │ J │ C │ D │ E │ F │ G │ H │  ← A and B were overwritten!
└───┴───┴───┴───┴───┴───┴───┴───┘
          ▲
          └── write_index (and oldest frame)
```

### 4.2 Thread-Safe Wrapper

```rust
// src/buffer/mod.rs

use parking_lot::RwLock;
use std::sync::Arc;

/// Thread-safe wrapper around our ring buffer
/// Multiple threads can read, but only one can write
pub struct SharedFrameBuffer {
    inner: Arc<RwLock<FrameRingBuffer>>,
}

impl SharedFrameBuffer {
    pub fn new(duration_seconds: f32, fps: u32) -> Self {
        Self {
            inner: Arc::new(RwLock::new(
                FrameRingBuffer::new(duration_seconds, fps)
            )),
        }
    }

    /// Called by capture thread to add frames
    pub fn push_frame(&self, frame: CapturedFrame) {
        self.inner.write().push(frame);
    }

    /// Called by save thread to get frames for encoding
    pub fn snapshot(&self) -> Vec<CapturedFrame> {
        self.inner.read()
            .get_all_frames()
            .into_iter()
            .cloned()
            .collect()
    }
}
```

**Plain English**:
> Imagine our buffer is a shared notebook. The "capture" thread is constantly writing new entries (frames) while the "save" thread occasionally needs to photocopy the whole notebook. We use a "lock" system - like those bathroom door indicators showing "occupied" or "vacant" - to make sure they don't interfere with each other.

---

## Phase 5: Input Handling

### What We're Doing (Plain English)
> We need to know when you want to save a clip. We'll listen for a specific button press on your Quest controller. It's like setting up a doorbell - when you press it, something happens!

### 5.1 Controller Input Detection

```rust
// src/input/mod.rs

use openxr as xr;

/// Possible buttons that can trigger a save
#[derive(Clone, Copy)]
pub enum TriggerButton {
    /// Left grip + left trigger together (safe combo)
    LeftGripTrigger,
    /// The "Oculus" / Meta button (requires special permission)
    MenuButton,
    /// Custom binding
    Custom { left: xr::ActionState, right: xr::ActionState },
}

/// Handles input detection
pub struct InputHandler {
    /// Which button combo triggers save
    trigger: TriggerButton,
    /// Prevents multiple triggers from one press
    debounce_time: std::time::Duration,
    /// Last time we triggered
    last_trigger: std::time::Instant,
}

impl InputHandler {
    pub fn new() -> Self {
        Self {
            trigger: TriggerButton::LeftGripTrigger,
            debounce_time: std::time::Duration::from_millis(500),
            last_trigger: std::time::Instant::now() - std::time::Duration::from_secs(10),
        }
    }

    /// Check if the save button is pressed
    /// Returns true only once per press (debounced)
    pub fn check_save_triggered(&mut self, input_state: &InputState) -> bool {
        let is_pressed = match self.trigger {
            TriggerButton::LeftGripTrigger => {
                input_state.left_grip > 0.9 && input_state.left_trigger > 0.9
            }
            TriggerButton::MenuButton => {
                input_state.menu_button
            }
            _ => false,
        };

        if is_pressed && self.last_trigger.elapsed() > self.debounce_time {
            self.last_trigger = std::time::Instant::now();
            true
        } else {
            false
        }
    }
}
```

**Plain English**:
> We don't want to accidentally save clips, so we use a button combo (like holding two buttons at once). We also have "debounce" - if you press the buttons and they bounce a little, we won't think you pressed them 10 times. We wait half a second before accepting another save command.

### 5.2 Haptic Feedback

```rust
/// Vibrates controllers to confirm save started/completed
pub fn send_haptic_feedback(session: &xr::Session, success: bool) {
    let vibration = xr::HapticVibration {
        duration: if success { 
            xr::Duration::from_nanos(200_000_000)  // 200ms for success
        } else { 
            xr::Duration::from_nanos(100_000_000)  // 100ms for "working"
        },
        frequency: if success { 200.0 } else { 100.0 },  // Hz
        amplitude: 0.8,  // 80% strength
    };
    
    // Apply to both controllers
    session.apply_haptic_feedback(left_hand, &vibration);
    session.apply_haptic_feedback(right_hand, &vibration);
}
```

**Plain English**:
> When you press save, the controllers vibrate to tell you "got it!" A short buzz means "I'm saving now", and a longer, higher-pitched buzz means "done!" It's like how your phone vibrates when you type.

---

## Phase 6: Video Encoding

### What We're Doing (Plain English)
> We have 900 photos (frames). Now we need to stitch them together into a video file. This is like making a flipbook animation, but instead of leaving them as separate pages, we bind them into a single book (video file) that takes up way less space.

### 6.1 MediaCodec Integration

Quest 3 has a hardware video encoder - a special chip designed just for making videos. It's much faster and more power-efficient than doing it in software.

```rust
// src/encoder/h264.rs

use jni::JNIEnv;

/// Wrapper around Android's MediaCodec hardware encoder
pub struct H264Encoder {
    /// JNI reference to MediaCodec object
    codec: jni::objects::GlobalRef,
    /// Output width
    width: u32,
    /// Output height
    height: u32,
    /// Bitrate in bits per second
    bitrate: u32,
    /// Target frame rate
    fps: u32,
}

impl H264Encoder {
    /// Creates a new hardware H.264 encoder
    pub fn new(env: &JNIEnv, width: u32, height: u32) -> Result<Self, EncoderError> {
        // Find a hardware encoder for video/avc (H.264)
        let codec_name = "video/avc";
        
        // Configure encoder parameters
        let format = MediaFormat::new();
        format.set_string("mime", codec_name);
        format.set_integer("width", width as i32);
        format.set_integer("height", height as i32);
        format.set_integer("color-format", COLOR_FormatYUV420Flexible);
        format.set_integer("bitrate", 20_000_000);  // 20 Mbps
        format.set_integer("frame-rate", 90);
        format.set_integer("i-frame-interval", 1);  // Keyframe every second

        // Create and configure the codec
        let codec = MediaCodec::createEncoderByType(codec_name)?;
        codec.configure(format, null, null, CONFIGURE_FLAG_ENCODE)?;
        codec.start()?;

        Ok(Self { codec, width, height, bitrate: 20_000_000, fps: 90 })
    }

    /// Encodes a single frame and returns encoded data
    pub fn encode_frame(&mut self, frame: &CapturedFrame) -> Result<Vec<u8>, EncoderError> {
        // 1. Get input buffer from codec
        // 2. Copy frame data into buffer (converting RGB to YUV)
        // 3. Queue the buffer for encoding
        // 4. Get output buffer with encoded data
        // 5. Return encoded bytes
    }

    /// Finalizes the encoding (flushes remaining frames)
    pub fn finish(&mut self) -> Result<Vec<u8>, EncoderError> {
        // Signal end of stream
        // Drain remaining output buffers
        // Return final encoded data
    }
}
```

**Plain English**:
> Quest 3 has a dedicated "video making" chip (hardware encoder). Instead of our main processor sweating to compress video, we hand off the job to this specialist. It's like using a professional print shop instead of your home printer - faster, better quality, and your computer doesn't slow down.

### 6.2 MP4 Container Writing

```rust
// src/encoder/mp4_muxer.rs

/// Wraps encoded video data in an MP4 container
pub struct Mp4Muxer {
    /// Output file path
    output_path: String,
    /// Video track ID
    video_track: u32,
    /// Frames written
    frame_count: u64,
}

impl Mp4Muxer {
    pub fn new(output_path: &str, width: u32, height: u32, fps: u32) -> Result<Self, IoError> {
        // Create MP4 file with video track
        // Configure timing based on fps
    }

    /// Adds an encoded frame to the video
    pub fn add_frame(&mut self, encoded_data: &[u8], timestamp: u64) -> Result<(), IoError> {
        // Write to video track with correct timing
    }

    /// Finalizes the MP4 file (writes headers, indexes)
    pub fn finalize(self) -> Result<(), IoError> {
        // Write moov atom (table of contents)
        // Close file
    }
}
```

**Plain English**:
> MP4 is like a shipping container format. The compressed video is our cargo, and MP4 is the standardized box that any video player knows how to open. We also add a "table of contents" so players can skip around in the video.

---

## Phase 7: Putting It All Together

### Main Application Flow

```rust
// src/lib.rs

use std::sync::Arc;
use std::thread;

/// Main application state
pub struct QuestShadowplay {
    buffer: Arc<SharedFrameBuffer>,
    input: InputHandler,
    is_saving: Arc<AtomicBool>,
}

impl QuestShadowplay {
    /// Called once when app starts
    pub fn initialize() -> Result<Self, AppError> {
        // 1. Initialize OpenXR layer
        // 2. Create frame buffer (10 seconds at 90fps)
        // 3. Set up input handling
        
        let buffer = Arc::new(SharedFrameBuffer::new(10.0, 90));
        let input = InputHandler::new();
        
        log::info!("Quest Shadowplay initialized!");
        
        Ok(Self {
            buffer,
            input,
            is_saving: Arc::new(AtomicBool::new(false)),
        })
    }

    /// Called every frame by our OpenXR layer hook
    pub fn on_frame(&mut self, frame: CapturedFrame) {
        // Add frame to buffer
        self.buffer.push_frame(frame);
        
        // Check for save trigger
        if self.input.check_save_triggered() && !self.is_saving.load() {
            self.trigger_save();
        }
    }

    /// Starts background save operation
    fn trigger_save(&self) {
        let buffer = Arc::clone(&self.buffer);
        let is_saving = Arc::clone(&self.is_saving);
        
        is_saving.store(true);
        
        // Spawn background thread for encoding
        thread::spawn(move || {
            // Short haptic = "save started"
            send_haptic_feedback(false);
            
            // 1. Snapshot the buffer
            let frames = buffer.snapshot();
            
            // 2. Create encoder
            let mut encoder = H264Encoder::new(1920, 1832).unwrap();
            let mut muxer = Mp4Muxer::new(&generate_filename(), 1920, 1832, 90).unwrap();
            
            // 3. Encode all frames
            for frame in frames {
                let encoded = encoder.encode_frame(&frame).unwrap();
                muxer.add_frame(&encoded, frame.timestamp).unwrap();
            }
            
            // 4. Finalize
            encoder.finish();
            muxer.finalize().unwrap();
            
            // Long haptic = "save complete!"
            send_haptic_feedback(true);
            
            is_saving.store(false);
        });
    }
}

/// Generates output filename with timestamp
fn generate_filename() -> String {
    let now = chrono::Local::now();
    format!(
        "/sdcard/QuestShadowplay/clip_{}.mp4",
        now.format("%Y%m%d_%H%M%S")
    )
}
```

**Plain English - The Complete Picture**:
> Here's how everything works together:
> 1. **Every frame** (90 times per second): We intercept the image, compress it, and add it to our circular buffer
> 2. **Every frame** we also check: "Did the user press the save button?"
> 3. **If save is pressed**: We make a copy of all buffered frames, then start a background job to:
>    - Vibrate the controllers ("got it!")
>    - Compress all frames into video
>    - Save to a file with today's date
>    - Vibrate again ("done!")
> 4. **While saving**: Recording continues! You don't miss any frames

---

## Phase 8: APK Packaging

### What We're Doing (Plain English)
> All our code is useless if we can't install it on your Quest. This phase packages everything into an APK (Android Package) file - the format Quest 3 understands. It's like putting your software into a box with an instruction manual so the Quest knows how to install and run it.

### 8.1 Android Manifest

```xml
<!-- android/AndroidManifest.xml -->
<?xml version="1.0" encoding="utf-8"?>
<manifest xmlns:android="http://schemas.android.com/apk/res/android"
    package="com.questshadowplay.app">

    <!-- Permissions we need -->
    <uses-permission android:name="android.permission.WRITE_EXTERNAL_STORAGE"/>
    <uses-permission android:name="android.permission.READ_EXTERNAL_STORAGE"/>
    
    <!-- VR features -->
    <uses-feature android:name="android.hardware.vr.headtracking" 
                  android:required="true"/>
    
    <application
        android:label="Quest Shadowplay"
        android:hasCode="true"
        android:debuggable="true">
        
        <!-- This is an OpenXR API layer, not a regular app -->
        <meta-data
            android:name="com.oculus.supportedDevices"
            android:value="quest3"/>
            
    </application>
</manifest>
```

### 8.2 Build Commands

```bash
# Build the Rust library for Android
cargo ndk -t arm64-v8a -o ./jniLibs build --release

# Package as APK (using xbuild)
x build --release --platform android

# Or using Gradle
./gradlew assembleRelease

# Install on Quest (connected via USB)
adb install -r target/release/quest-shadowplay.apk
```

---

## Testing Plan

### Unit Tests
- Ring buffer correctly overwrites old frames
- Frame compression produces valid JPEG data
- Input debouncing works correctly

### Integration Tests
- Full capture → buffer → save pipeline
- File is written to correct location
- MP4 file is playable

### Performance Tests
- Memory usage stays under 200MB
- No noticeable FPS drop in VR apps
- Save completes in under 5 seconds

---

## Risk Assessment

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| OpenXR layer doesn't work on Quest | Medium | High | Have backup using screen capture API |
| Performance impact too high | Medium | High | Implement frame skipping, reduce resolution |
| Memory overflow | Low | High | Strict buffer limits, monitoring |
| Quest OS update breaks layer | Medium | Medium | Version checking, graceful degradation |

---

## Success Criteria

- [ ] APK installs successfully on Quest 3
- [ ] Frames are captured while VR apps run
- [ ] 10-second buffer maintains expected frame count
- [ ] Save completes in under 5 seconds
- [ ] Output video is playable and correctly represents captured frames
- [ ] No noticeable performance impact on VR experience
- [ ] Haptic feedback confirms save operations

---

*Implementation Plan Version: 1.0*
*Last Updated: December 27, 2024*

