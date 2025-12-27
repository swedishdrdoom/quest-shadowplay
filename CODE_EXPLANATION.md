# Quest Shadowplay - Code Explanation for Non-Programmers

This document explains what each part of the Quest Shadowplay code does in plain, simple language. No programming knowledge required!

---

## 🎯 The Big Picture

Quest Shadowplay is like a **security camera for your VR experience** that:
1. Always records the last 10 seconds
2. Throws away old footage automatically
3. Saves a clip when you press a button

Think of it like a TiVo for VR - you can always "rewind" and save what just happened.

---

## 📁 Project Files Explained

### The Main Control Room: `lib.rs`

**What it is**: The "brain" of the application - coordinates everything.

**Everyday analogy**: Like an air traffic control tower that manages all the planes (components) and makes sure they work together.

**What it does**:
1. **Starts everything up** when you launch the app
2. **Receives frames** from the capture system
3. **Listens for button presses** to save clips
4. **Coordinates saving** when you trigger it
5. **Shuts down cleanly** when you exit

**Key concept - "on_frame_captured"**:
> This is like a factory worker on an assembly line. Every time a new picture arrives (~90 times per second), this function:
> 1. Takes the picture
> 2. Puts it in our storage system
> 3. Checks if you pressed the save button
> 4. If yes, starts saving in the background

---

### The Frame Grabber: `capture/mod.rs` and `openxr_layer.rs`

**What it is**: The system that "photographs" what you see in VR.

**Everyday analogy**: Like a photocopier attached to a movie projector - it copies every frame of the movie without slowing it down.

**What it does**:
1. **Intercepts images** before they reach your eyes
2. **Makes copies** of those images
3. **Compresses copies** to save memory (like zipping a file)
4. **Passes originals along** so VR continues normally

**Key concept - "OpenXR Layer"**:
> OpenXR is like a translator between VR games and your headset. Our "layer" is a spy that sits in the middle of this conversation, listening to everything and making copies.

**Key concept - "Compression"**:
> A raw VR image is HUGE (about 14 MB each). We compress each one to about 100 KB (140x smaller!) so we can store thousands of them. It's like vacuum-sealing clothes for storage.

---

### The Rolling Memory: `buffer/mod.rs` and `ring_buffer.rs`

**What it is**: A special storage system that holds exactly 10 seconds of images.

**Everyday analogy**: Like a circular sushi conveyor belt with exactly 900 spots. When spot #901 needs to be added, spot #1 gets removed to make room.

**What it does**:
1. **Stores frames** as they arrive
2. **Automatically removes** the oldest frame when full
3. **Provides all frames** when you want to save
4. **Never runs out of space** or grows too large

**Key concept - "Ring Buffer"**:
> Imagine a circular track with numbered parking spots. A parking attendant walks around adding cars. When all spots are full, the oldest car is towed away before parking a new one. This way, you always have the most recent cars (frames).

**Visual representation**:
```
After 5 frames (capacity 8):
┌───┬───┬───┬───┬───┬───┬───┬───┐
│ A │ B │ C │ D │ E │   │   │   │
└───┴───┴───┴───┴───┴───┴───┴───┘
                      ↑
                   next spot

After 10 frames (buffer full, 2 removed):
┌───┬───┬───┬───┬───┬───┬───┬───┐
│ I │ J │ C │ D │ E │ F │ G │ H │  ← A, B were removed
└───┴───┴───┴───┴───┴───┴───┴───┘
          ↑
       oldest (next to remove)
```

---

### The Button Watcher: `input/mod.rs`

**What it is**: Detects when you press the save button.

**Everyday analogy**: Like a doorbell that only rings when you press TWO buttons at once (so it doesn't ring accidentally).

**What it does**:
1. **Monitors controller buttons** constantly
2. **Detects the save combo** (Left Grip + Left Trigger)
3. **Ignores accidental presses** (debouncing)
4. **Triggers the save** when everything checks out

**Key concept - "Debouncing"**:
> When you press a button, electronics can "bounce" and register multiple presses. Debouncing is like saying "I'll only accept one press per half-second" to ignore these false presses.

**Key concept - "Button Combo"**:
> We don't use a single button because you might press it accidentally while gaming. Instead, we require TWO buttons at once - this almost never happens by accident.

---

### The Video Maker: `encoder/mod.rs`

**What it is**: Turns individual photos into a video file.

**Everyday analogy**: Like a flipbook maker that takes your stack of drawings and binds them into an animated flipbook, then shrinks it to fit in your pocket.

**What it does**:
1. **Takes all stored frames** from the buffer
2. **Uncompresses them** (from JPEG back to full images)
3. **Converts colors** to video format (RGBA to YUV)
4. **Compresses into H.264** video (using Quest's hardware)
5. **Wraps in MP4 container** (the standard video format)

**Key concept - "Hardware Encoding"**:
> Quest 3 has a special chip designed just for making videos. It's like having a professional print shop versus using your home printer - much faster and better quality without slowing down your main computer.

**Key concept - "H.264"**:
> H.264 is a way to compress video. Instead of storing every pixel of every frame, it stores the differences between frames. A 10-second video that would be 25 GB uncompressed becomes just 25 MB.

**Key concept - "MP4 Container"**:
> Raw video data is like loose pages from a book. MP4 is the binding that turns it into a proper book with a table of contents, so any video player knows how to read it.

---

### The File Saver: `storage/mod.rs`

**What it is**: Saves video files and manages storage.

**Everyday analogy**: Like a librarian who:
- Creates folders for organizing clips
- Names files with date and time
- Throws away old clips when storage is full

**What it does**:
1. **Creates the output folder** if it doesn't exist
2. **Generates unique filenames** (clip_20241227_143052.mp4)
3. **Writes videos to storage**
4. **Tracks space used**
5. **Deletes old clips** if needed to make room

**Key concept - "Filename with Timestamp"**:
> Each clip is named with when it was saved: `clip_20241227_143052.mp4` means December 27, 2024 at 2:30:52 PM. This ensures every clip has a unique name.

---

### The Settings Panel: `config.rs`

**What it is**: All the adjustable settings for the app.

**Everyday analogy**: Like a car's settings panel where you can adjust seat position, mirror angles, and radio presets.

**What it does**:
1. **Defines all settings** (buffer length, video quality, etc.)
2. **Provides sensible defaults** (10 seconds, 90 FPS, etc.)
3. **Validates settings** (catches mistakes like "buffer -5 seconds")
4. **Calculates memory usage** so you know what to expect

**Available settings**:
| Setting | What it controls | Default |
|---------|------------------|---------|
| `buffer_duration_seconds` | How many seconds to keep | 10 |
| `target_fps` | How smooth the video is | 90 |
| `video_bitrate` | How detailed the video is | 20 Mbps |
| `jpeg_quality` | Frame compression quality | 80% |
| `trigger_button` | Which buttons save | Left Grip + Trigger |

---

### The Error Reporter: `error.rs`

**What it is**: A system for describing what went wrong when things fail.

**Everyday analogy**: Like error codes on a washing machine display - instead of just "ERROR", it tells you "E04: Water inlet blocked".

**What it does**:
1. **Categorizes problems** (capture, encoding, storage, etc.)
2. **Provides details** (not just "failed" but "failed because disk is full")
3. **Helps with debugging** (developers can find and fix issues)
4. **Enables recovery** (some errors can be retried)

**Types of errors**:
- **Capture errors**: Problems getting frames from VR
- **Encoder errors**: Problems making video
- **Storage errors**: Problems saving files (disk full, no permission)
- **Android errors**: Problems with the operating system

---

## 🔄 How Everything Works Together

Here's the complete flow when you save a clip:

```
YOU'RE PLAYING VR
      │
      │  (constantly happening, 90x per second)
      ▼
┌─────────────────┐
│  Frame arrives  │
│  from VR game   │
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│ OpenXR Layer    │ ← Our "spy" in the middle
│ copies frame    │
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│ Frame compressed│ ← JPEG: 14MB → 100KB
│ to save memory  │
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│ Ring Buffer     │ ← Circular conveyor belt
│ stores frame    │
└────────┬────────┘
         │
         │  (oldest frame automatically removed if full)
         │
         ▼
┌─────────────────┐
│ Check: save     │ ← Looking at your controllers
│ button pressed? │
└────────┬────────┘
         │
         │ NO → continue recording
         │
         │ YES ↓
         ▼
┌─────────────────┐
│ Haptic feedback │ ← "Buzz!" - you know it's saving
│ (short vibrate) │
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│ Snapshot buffer │ ← Copy all 900 frames
│ (don't stop!)   │
└────────┬────────┘
         │
         │  (this happens in background - VR continues!)
         ▼
┌─────────────────┐
│ Decompress each │ ← JPEG → raw pixels
│ frame           │
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│ Hardware encode │ ← Quest's video chip
│ to H.264        │
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│ Wrap in MP4     │ ← Standard video format
│ container       │
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│ Save to storage │ ← /QuestShadowplay/clip_123.mp4
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│ Haptic feedback │ ← "Buzz buzz!" - done!
│ (success)       │
└─────────────────┘
```

---

## 🎓 Key Concepts Summary

| Concept | Plain English |
|---------|---------------|
| **Ring Buffer** | A circular storage that automatically removes old items |
| **OpenXR Layer** | A "spy" that intercepts VR images |
| **JPEG Compression** | Shrinks images 140x to save memory |
| **H.264 Encoding** | Shrinks video 1000x to save storage |
| **Hardware Encoder** | A dedicated chip that's fast at making videos |
| **MP4 Container** | A "wrapper" that any video player can read |
| **Debouncing** | Ignoring accidental button presses |
| **Haptic Feedback** | Controller vibrations that tell you something happened |
| **Async/Background** | Doing work without interrupting VR |

---

## 📊 By the Numbers

| Metric | Value | Plain English |
|--------|-------|---------------|
| Frames per second | 90 | 90 pictures every second |
| Buffer duration | 10 seconds | Last 900 frames stored |
| Uncompressed frame | ~14 MB | Without compression |
| Compressed frame | ~100 KB | After JPEG compression |
| Buffer memory | ~90 MB | Total memory for buffer |
| Save time | ~3 seconds | Time to create video |
| Output file size | ~25 MB | For 10 seconds |

---

## 🤔 Frequently Asked Questions

**Q: Why compress frames twice (JPEG then H.264)?**
> JPEG is fast and keeps frames individually accessible. H.264 is better for final video but needs all frames at once. We use JPEG for the live buffer, then H.264 for saving.

**Q: Why use a ring buffer instead of just recording?**
> Regular recording would fill up memory quickly and needs you to decide BEFORE something cool happens. A ring buffer is always ready!

**Q: What's a "thread" that keeps getting mentioned?**
> Think of threads as workers in a factory. The main worker handles VR, while other workers handle saving videos in the background. They work at the same time without getting in each other's way.

**Q: Why Rust instead of other languages?**
> Rust is fast (important for VR) and prevents common programming mistakes. It's harder to write but more reliable once it works.

---

*This document was created to help non-programmers understand the Quest Shadowplay codebase. Each code file contains additional comments explaining what each piece does in simple terms.*

