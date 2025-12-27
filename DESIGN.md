# Quest Shadowplay - Design Document

## Project Overview

**Quest Shadowplay** is a Rust-based application for Meta Quest 3 that continuously captures VR eye buffer frames and allows users to save the last 10 seconds of footage on demand (similar to NVIDIA Shadowplay on PC).

---

## Table of Contents

1. [Goals & Requirements](#goals--requirements)
2. [System Architecture](#system-architecture)
3. [Technical Approach](#technical-approach)
4. [Component Breakdown](#component-breakdown)
5. [Data Flow](#data-flow)
6. [Challenges & Solutions](#challenges--solutions)
7. [Implementation Plan](#implementation-plan)
8. [Glossary for Non-Programmers](#glossary-for-non-programmers)

---

## Goals & Requirements

### Primary Goals
- ✅ Continuously capture eye buffer frames from Meta Quest 3
- ✅ Maintain a rolling 10-second buffer of recent frames
- ✅ Save buffer to disk on button press
- ✅ Package as standalone APK for Quest 3

### Technical Requirements
- **Platform**: Android (Quest 3 runs Android 12+)
- **Language**: Rust (compiled to Android/ARM64)
- **Frame Rate Target**: 72-120 FPS (Quest 3 native refresh rates)
- **Buffer Duration**: 10 seconds
- **Output Format**: MP4/H.264 video file

### Constraints
- Must run efficiently without impacting VR performance
- Memory usage must stay within Quest 3's limits (~6GB RAM shared)
- Storage writes must be fast and non-blocking

---

## System Architecture

```
┌─────────────────────────────────────────────────────────────────────┐
│                        Quest Shadowplay                              │
├─────────────────────────────────────────────────────────────────────┤
│                                                                      │
│  ┌──────────────────┐    ┌──────────────────┐    ┌───────────────┐  │
│  │   Frame Capture  │───▶│  Circular Buffer │───▶│ Video Encoder │  │
│  │   (OpenXR Layer) │    │  (Ring Buffer)   │    │  (H.264/H.265)│  │
│  └──────────────────┘    └──────────────────┘    └───────────────┘  │
│           │                       │                      │          │
│           │                       │                      ▼          │
│           │                       │              ┌───────────────┐  │
│           │                       │              │  File Writer  │  │
│           │                       │              │   (Storage)   │  │
│           │                       │              └───────────────┘  │
│           │                       │                                 │
│           ▼                       ▼                                 │
│  ┌──────────────────────────────────────────────────────────────┐  │
│  │                     Input Handler                              │  │
│  │              (Controller Button Detection)                     │  │
│  └──────────────────────────────────────────────────────────────┘  │
│                                                                      │
└─────────────────────────────────────────────────────────────────────┘
```

---

## Technical Approach

### How We Capture Eye Buffer Frames

**The Challenge**: Meta Quest 3's eye buffer is the final rendered image that gets displayed to each eye. Capturing this requires intercepting the graphics pipeline.

**Our Approach**: We'll use **OpenXR API Layer** injection. OpenXR is the standard API that VR applications use to render content. By creating an "API layer" (think of it as a middleman), we can intercept frames before they're sent to the display.

```
Normal Flow:
VR App ──▶ OpenXR Runtime ──▶ Display

Our Flow:
VR App ──▶ [Our Layer] ──▶ OpenXR Runtime ──▶ Display
              │
              └──▶ Copy frame to our buffer
```

### Alternative Approaches Considered

| Approach | Pros | Cons | Selected? |
|----------|------|------|-----------|
| OpenXR API Layer | Standard, future-proof | Complex setup | ✅ Yes |
| Android Screen Capture | Simple API | Captures 2D, not stereo | ❌ No |
| GPU Memory Interception | Low overhead | Very fragile, version-specific | ❌ No |
| Meta Quest Recording API | Official support | Limited customization | ❌ Backup |

---

## Component Breakdown

### 1. Frame Capture Module (`capture/`)

**What it does**: Intercepts rendered frames from the VR graphics pipeline.

**Plain English Explanation**:
> Imagine a security camera watching a conveyor belt. Every item (frame) that passes by, the camera takes a quick photo. This module is that camera - it watches the stream of images heading to your eyes and makes a copy of each one.

**Technical Details**:
- Implements OpenXR API layer specification
- Hooks `xrEndFrame()` to access submitted frames
- Uses GPU-to-GPU copy for efficiency (no CPU stall)
- Handles both eye views (left + right)

### 2. Circular Buffer Module (`buffer/`)

**What it does**: Stores the last 10 seconds of frames in memory, automatically discarding old ones.

**Plain English Explanation**:
> Think of a circular conveyor belt at a sushi restaurant. There's only room for 10 plates. When plate #11 arrives, it pushes plate #1 off the belt. Our buffer works the same way - it always keeps exactly 10 seconds of footage, with new frames replacing old ones.

**Technical Details**:
- Ring buffer data structure
- Pre-allocated memory to avoid runtime allocations
- Lock-free design for thread safety
- Configurable duration (default: 10 seconds)

**Memory Calculation**:
```
Per frame: 1832 x 1920 pixels × 2 eyes × 4 bytes (RGBA) = ~28 MB
At 90 FPS for 10 seconds = 900 frames
Total: 900 × 28 MB = ~25 GB (uncompressed)

Solution: Compress frames to ~100KB each
Compressed: 900 × 100 KB = ~90 MB ✅
```

### 3. Input Handler Module (`input/`)

**What it does**: Detects when the user presses a specific button to trigger saving.

**Plain English Explanation**:
> This is like a doorbell for saving videos. When you press the right button on your Quest controller (we'll use the "Oculus" button + trigger), it sends a signal that says "Save now!"

**Technical Details**:
- Uses Android NDK input system
- Registers for controller events via OpenXR
- Configurable button mapping
- Debounce logic to prevent double-triggers

### 4. Video Encoder Module (`encoder/`)

**What it does**: Converts raw image frames into a compressed video file.

**Plain English Explanation**:
> Raw photos take up huge amounts of space. This module is like a professional photo compressor - it takes all those individual pictures and squishes them into a single, small video file that you can watch later.

**Technical Details**:
- Uses Android MediaCodec for hardware-accelerated encoding
- H.264 or H.265 codec (Quest 3 has hardware encoders)
- Configurable bitrate (default: 20 Mbps)
- Outputs to MP4 container format

### 5. File Writer Module (`storage/`)

**What it does**: Saves the encoded video to Quest 3's storage.

**Plain English Explanation**:
> Once the video is compressed, this module is the librarian that files it away in the right folder on your Quest, giving it a proper name with the date and time.

**Technical Details**:
- Writes to `/sdcard/QuestShadowplay/` directory
- Async I/O to prevent blocking
- Filename format: `clip_YYYYMMDD_HHMMSS.mp4`
- Manages storage quota (auto-delete old clips)

### 6. APK Packaging (`android/`)

**What it does**: Bundles everything into an installable Quest 3 application.

**Plain English Explanation**:
> An APK is like a shipping box that contains everything needed to run our app on Quest 3. This component wraps up all our code, settings, and permissions into one neat package you can install.

**Technical Details**:
- Uses `cargo-apk` or `xbuild` for Rust-to-Android compilation
- Targets ARM64 architecture
- Requires system-level permissions for frame capture
- May need ADB sideloading (not available on official Quest Store)

---

## Data Flow

### Continuous Recording Flow (Always Running)

```
1. VR App renders frame
       │
       ▼
2. Our OpenXR Layer intercepts xrEndFrame()
       │
       ▼
3. GPU copies frame texture to our staging buffer
       │
       ▼
4. Frame is compressed (JPEG/GPU encoding)
       │
       ▼
5. Compressed frame stored in circular buffer
       │
       ▼
6. If buffer full, oldest frame is overwritten
       │
       ▼
7. Repeat for next frame (every ~11ms at 90fps)
```

### Save Trigger Flow (On Button Press)

```
1. User presses save button
       │
       ▼
2. Input handler detects press
       │
       ▼
3. Circular buffer is "frozen" (snapshot taken)
       │
       ▼
4. Background thread starts encoding
       │
       ▼
5. Frames fed to H.264 encoder in order
       │
       ▼
6. Encoded video written to MP4 file
       │
       ▼
7. User notified (haptic feedback + sound)
       │
       ▼
8. Recording continues normally
```

---

## Challenges & Solutions

### Challenge 1: Memory Constraints

**Problem**: 10 seconds of uncompressed 4K stereo video = ~25 GB
**Solution**: Real-time JPEG compression reduces each frame to ~100 KB

### Challenge 2: Performance Impact

**Problem**: Copying and processing frames could cause VR stuttering
**Solution**: 
- Use GPU-to-GPU copies (no CPU involvement)
- Async encoding on separate thread
- Skip frames if system is under load

### Challenge 3: Eye Buffer Access

**Problem**: Quest 3's eye buffer isn't easily accessible
**Solution**: OpenXR API layer intercepts frames at the API boundary

### Challenge 4: Rust on Android

**Problem**: Rust doesn't natively target Quest 3
**Solution**: Use `cargo-ndk` with Android NDK toolchain, target `aarch64-linux-android`

### Challenge 5: System Permissions

**Problem**: Capturing frames requires elevated permissions
**Solution**: App may need to run as system app or use developer mode

---

## Implementation Plan

### Phase 1: Project Setup (Days 1-2)
- [ ] Set up Rust Android toolchain
- [ ] Create basic APK that runs on Quest 3
- [ ] Verify basic OpenXR layer loads

### Phase 2: Frame Capture (Days 3-5)
- [ ] Implement OpenXR layer skeleton
- [ ] Hook `xrEndFrame()` function
- [ ] Copy frame data to CPU-accessible buffer
- [ ] Verify frames are being captured

### Phase 3: Circular Buffer (Days 6-7)
- [ ] Implement ring buffer data structure
- [ ] Add frame compression (JPEG)
- [ ] Test memory usage and performance

### Phase 4: Input Handling (Day 8)
- [ ] Detect controller button presses
- [ ] Implement save trigger logic
- [ ] Add haptic feedback

### Phase 5: Video Encoding (Days 9-11)
- [ ] Integrate Android MediaCodec
- [ ] Encode frames to H.264
- [ ] Write MP4 container

### Phase 6: Polish & Testing (Days 12-14)
- [ ] Performance optimization
- [ ] Error handling
- [ ] User notifications
- [ ] Storage management

---

## Glossary for Non-Programmers

| Term | Simple Explanation |
|------|-------------------|
| **Eye Buffer** | The final image that gets shown to each of your eyes in VR |
| **OpenXR** | A universal language that VR apps use to talk to VR headsets |
| **API Layer** | A middleman that can listen to and modify communications |
| **Circular Buffer** | A fixed-size storage where new items replace the oldest ones |
| **H.264** | A method of shrinking videos to take up less space |
| **APK** | An installation package for Android devices (like .exe for Windows) |
| **NDK** | Tools for building Android apps in languages other than Java |
| **GPU** | Graphics Processing Unit - the chip that draws images |
| **Async** | Doing something in the background without stopping other work |
| **Haptic Feedback** | Vibrations that tell you something happened |

---

## File Structure

```
quest-shadowplay/
├── Cargo.toml                 # Rust project configuration
├── android/
│   ├── AndroidManifest.xml    # App permissions and metadata
│   └── build.gradle           # Android build settings
├── src/
│   ├── lib.rs                 # Main entry point
│   ├── capture/
│   │   ├── mod.rs             # Frame capture module
│   │   └── openxr_layer.rs    # OpenXR interception logic
│   ├── buffer/
│   │   ├── mod.rs             # Buffer module
│   │   └── ring_buffer.rs     # Circular buffer implementation
│   ├── input/
│   │   └── mod.rs             # Controller input handling
│   ├── encoder/
│   │   ├── mod.rs             # Video encoder module
│   │   └── h264.rs            # H.264 encoding logic
│   └── storage/
│       └── mod.rs             # File saving logic
└── README.md                  # User documentation
```

---

## Next Steps

1. **Validate Approach**: Test if OpenXR layer injection works on Quest 3
2. **Set Up Development Environment**: Install Rust, Android NDK, Quest developer tools
3. **Build Minimal Prototype**: Create simplest possible frame capture
4. **Iterate**: Add features incrementally

---

*Document Version: 1.0*
*Last Updated: December 27, 2024*

