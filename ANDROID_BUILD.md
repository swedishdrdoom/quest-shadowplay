# Building Quest Shadowplay APK for Meta Quest 3

This guide explains how to build the Android APK for testing on Quest 3.

## Prerequisites

### 1. Install Android Studio

Download and install Android Studio:
- https://developer.android.com/studio

During installation, make sure to install:
- Android SDK (API 32 or higher)
- Android NDK (r25 or later)
- Android SDK Command-line Tools

### 2. Set Environment Variables

Add these to your shell profile (`~/.zshrc` or `~/.bashrc`):

```bash
export ANDROID_HOME="$HOME/Library/Android/sdk"
export NDK_HOME="$ANDROID_HOME/ndk/25.2.9519653"  # Adjust version as needed
export PATH="$PATH:$ANDROID_HOME/platform-tools:$ANDROID_HOME/tools"
```

Then run:
```bash
source ~/.zshrc  # or source ~/.bashrc
```

### 3. Install Rust Android Targets

```bash
rustup target add aarch64-linux-android
```

### 4. Accept Android Licenses

```bash
$ANDROID_HOME/cmdline-tools/latest/bin/sdkmanager --licenses
```

## Building the APK

### Step 1: Initialize Android Project

```bash
cd /Users/drdoomvr/Code/quest-shadowplay/src-tauri
cargo tauri android init
```

### Step 2: Build Debug APK

```bash
cargo tauri android build --debug
```

The APK will be at:
```
src-tauri/gen/android/app/build/outputs/apk/universal/debug/app-universal-debug.apk
```

### Step 3: Build Release APK

```bash
cargo tauri android build --release
```

## Installing on Quest 3

### 1. Enable Developer Mode

1. Go to [developer.oculus.com](https://developer.oculus.com)
2. Create a developer account
3. In the Meta Quest app on your phone, enable Developer Mode

### 2. Connect Quest via ADB

```bash
adb devices
```

### 3. Install the APK

```bash
adb install path/to/app-universal-debug.apk
```

### 4. Launch the App

The app will appear in your Quest's app library under "Unknown Sources".

## Quest-Specific Configuration

For optimal Quest 3 experience, the AndroidManifest.xml includes:

- VR headtracking feature requirement
- 2D app mode (runs alongside VR apps)
- Storage permissions for saving clips
- MediaProjection for screen capture

## Troubleshooting

### "Android SDK not found"

Make sure `ANDROID_HOME` is set correctly:
```bash
echo $ANDROID_HOME
ls $ANDROID_HOME
```

### "NDK not found"

Install NDK via Android Studio SDK Manager or:
```bash
sdkmanager "ndk;25.2.9519653"
```

### ADB device not recognized

1. Restart ADB server: `adb kill-server && adb start-server`
2. Check USB connection
3. Allow USB debugging on Quest when prompted

### App crashes on startup

Check logs with:
```bash
adb logcat | grep -i shadowplay
```

## Testing Flow

1. Install APK on Quest
2. Launch Quest Shadowplay from Unknown Sources
3. App opens as a 2D panel
4. Press "Start Recording" to begin capture
5. Launch another VR app
6. Return to Shadowplay
7. Press "Save Clip" to save last 10 seconds

## Notes

- The app runs as a 2D app alongside immersive VR apps
- MediaProjection captures the screen content
- Clips are saved to `/sdcard/QuestShadowplay/`
- Files can be accessed when Quest is connected to PC via USB


