// CaptureController.swift
// Hardware-accelerated replay buffer for macOS
//
// Pipeline: ScreenCaptureKit → CVPixelBuffer → VTCompressionSession (H.264) → Ring Buffer
// On save: Ring Buffer → AVAssetWriter → MP4
//
// Uses separate dispatch queues for:
// - Capture (receiving frames from SCStream)
// - Encode (VTCompressionSession H.264 encoding)
// - IO (AVAssetWriter file output on save)

import Foundation
import ScreenCaptureKit
import CoreMedia
import CoreVideo
import VideoToolbox
import AVFoundation

// MARK: - Configuration

@frozen
public struct CaptureConfig {
    public var width: UInt32
    public var height: UInt32
    public var fps: UInt32
    public var bitrate: UInt32
    public var keyframeInterval: Float
    public var bufferDurationSecs: Float  // How many seconds to keep in replay buffer
}

// MARK: - Encoded Frame (stored in ring buffer)

/// Represents a single H.264-encoded frame with timing information
private struct EncodedFrame {
    let data: Data                    // H.264 NAL units
    let presentationTime: CMTime      // When this frame should be displayed
    let duration: CMTime              // Frame duration
    let isKeyFrame: Bool              // Is this an I-frame?
    
    var sizeBytes: Int { data.count }
}

// MARK: - Encoded Frame Ring Buffer

/// Thread-safe ring buffer that stores the last N seconds of H.264 frames
private class EncodedFrameBuffer {
    private var frames: [EncodedFrame] = []
    private let maxDuration: CMTime
    private let lock = NSLock()
    
    init(maxDurationSecs: Float) {
        self.maxDuration = CMTime(seconds: Double(maxDurationSecs), preferredTimescale: 600)
        print("[EncodedFrameBuffer] Created with max duration: \(maxDurationSecs)s")
    }
    
    /// Appends a frame to the buffer, trimming old frames if needed
    func append(_ frame: EncodedFrame) {
        lock.lock()
        defer { lock.unlock() }
        
        frames.append(frame)
        trimToMaxDuration()
    }
    
    /// Returns all frames currently in the buffer (thread-safe copy)
    func getFrames() -> [EncodedFrame] {
        lock.lock()
        defer { lock.unlock() }
        return frames
    }
    
    /// Returns the current buffer fill percentage (0.0 - 1.0)
    func fillPercentage() -> Float {
        lock.lock()
        defer { lock.unlock() }
        
        guard frames.count >= 2,
              let first = frames.first,
              let last = frames.last else {
            return 0.0
        }
        
        let currentDuration = CMTimeSubtract(last.presentationTime, first.presentationTime)
        let ratio = CMTimeGetSeconds(currentDuration) / CMTimeGetSeconds(maxDuration)
        return Float(min(1.0, max(0.0, ratio)))
    }
    
    /// Returns approximate memory usage in bytes
    func memorySizeBytes() -> Int {
        lock.lock()
        defer { lock.unlock() }
        return frames.reduce(0) { $0 + $1.sizeBytes }
    }
    
    /// Clears all frames from the buffer
    func clear() {
        lock.lock()
        defer { lock.unlock() }
        frames.removeAll()
    }
    
    /// Trims frames older than maxDuration, always starting from a keyframe
    private func trimToMaxDuration() {
        guard frames.count > 1 else { return }
        
        let lastTime = frames.last!.presentationTime
        let cutoffTime = CMTimeSubtract(lastTime, maxDuration)
        
        // Find the first keyframe that's after the cutoff
        var trimIndex = 0
        for (index, frame) in frames.enumerated() {
            if CMTimeCompare(frame.presentationTime, cutoffTime) >= 0 {
                // This frame is after cutoff - find the nearest keyframe at or before this
                break
            }
            if frame.isKeyFrame {
                trimIndex = index
            }
        }
        
        if trimIndex > 0 {
            frames.removeFirst(trimIndex)
        }
    }
}

// MARK: - Replay Capture Controller

@objc public class CaptureController: NSObject {
    
    // Configuration
    private let config: CaptureConfig
    
    // ScreenCaptureKit
    private var stream: SCStream?
    private var streamOutput: StreamOutput?
    
    // VideoToolbox encoder
    private var compressionSession: VTCompressionSession?
    private var formatDescription: CMFormatDescription?
    
    // Replay buffer
    private let replayBuffer: EncodedFrameBuffer
    
    // Queues (separate for each pipeline stage)
    private let captureQueue = DispatchQueue(label: "com.questshadowplay.capture", qos: .userInteractive)
    private let encodeQueue = DispatchQueue(label: "com.questshadowplay.encode", qos: .userInteractive)
    private let ioQueue = DispatchQueue(label: "com.questshadowplay.io", qos: .utility)
    
    // State
    private var isCapturing = false
    private var baseTime: CMTime?  // First frame's timestamp (for relative timing)
    
    // Statistics
    private var framesCapture: UInt64 = 0
    private var framesDropped: UInt64 = 0
    private var framesEncoded: UInt64 = 0
    
    // MARK: - Initialization
    
    public init(config: CaptureConfig) {
        self.config = config
        self.replayBuffer = EncodedFrameBuffer(maxDurationSecs: config.bufferDurationSecs)
        super.init()
    }
    
    deinit {
        stop()
        teardownEncoder()
    }
    
    // MARK: - Public API
    
    /// Starts capturing to the replay buffer (no file output yet)
    public func start() -> Bool {
        guard !isCapturing else {
            print("[CaptureController] Already capturing")
            return false
        }
        
        // Reset state
        framesCapture = 0
        framesDropped = 0
        framesEncoded = 0
        baseTime = nil
        replayBuffer.clear()
        
        // Setup encoder
        guard setupEncoder() else {
            print("[CaptureController] Failed to setup encoder")
            return false
        }
        
        // Setup screen capture
        let semaphore = DispatchSemaphore(value: 0)
        var success = false
        
        Task {
            success = await self.setupScreenCapture()
            semaphore.signal()
        }
        
        semaphore.wait()
        
        if success {
            isCapturing = true
            print("[CaptureController] Started replay buffer: \(config.width)x\(config.height) @ \(config.fps)fps, buffer: \(config.bufferDurationSecs)s")
        }
        
        return success
    }
    
    /// Stops capturing
    public func stop() {
        guard isCapturing else { return }
        isCapturing = false
        
        // Stop stream
        stream?.stopCapture { error in
            if let error = error {
                print("[CaptureController] Error stopping stream: \(error)")
            }
        }
        stream = nil
        streamOutput = nil
        
        // Flush encoder
        if let session = compressionSession {
            VTCompressionSessionCompleteFrames(session, untilPresentationTimeStamp: .invalid)
        }
        
        print("[CaptureController] Stopped. Captured: \(framesCapture), Dropped: \(framesDropped), Encoded: \(framesEncoded)")
    }
    
    /// Saves the current replay buffer to an MP4 file
    /// Returns true on success
    public func saveReplay(outputPath: String) -> Bool {
        let frames = replayBuffer.getFrames()
        
        guard !frames.isEmpty else {
            print("[CaptureController] No frames in replay buffer")
            return false
        }
        
        guard let formatDesc = self.formatDescription else {
            print("[CaptureController] No format description available")
            return false
        }
        
        print("[CaptureController] Saving \(frames.count) frames to: \(outputPath)")
        
        let url = URL(fileURLWithPath: outputPath)
        
        // Remove existing file
        try? FileManager.default.removeItem(at: url)
        
        // Create asset writer
        guard let assetWriter = try? AVAssetWriter(outputURL: url, fileType: .mp4) else {
            print("[CaptureController] Failed to create asset writer")
            return false
        }
        
        // Create video input with passthrough (no re-encoding)
        let videoInput = AVAssetWriterInput(mediaType: .video, outputSettings: nil, sourceFormatHint: formatDesc)
        videoInput.expectsMediaDataInRealTime = false
        
        guard assetWriter.canAdd(videoInput) else {
            print("[CaptureController] Cannot add video input to asset writer")
            return false
        }
        assetWriter.add(videoInput)
        
        guard assetWriter.startWriting() else {
            print("[CaptureController] Failed to start writing: \(assetWriter.error?.localizedDescription ?? "unknown")")
            return false
        }
        
        // Calculate time offset so video starts at 0
        guard let firstFrame = frames.first else { return false }
        let timeOffset = firstFrame.presentationTime
        
        assetWriter.startSession(atSourceTime: .zero)
        
        // Write all frames
        var framesWritten = 0
        for frame in frames {
            // Wait for input to be ready
            while !videoInput.isReadyForMoreMediaData {
                Thread.sleep(forTimeInterval: 0.001)
            }
            
            // Adjust presentation time to start from 0
            let adjustedTime = CMTimeSubtract(frame.presentationTime, timeOffset)
            
            // Create sample buffer from encoded data
            if let sampleBuffer = createSampleBuffer(from: frame, adjustedTime: adjustedTime, formatDescription: formatDesc) {
                if videoInput.append(sampleBuffer) {
                    framesWritten += 1
                } else {
                    print("[CaptureController] Failed to append frame \(framesWritten)")
                }
            }
        }
        
        videoInput.markAsFinished()
        
        // Wait for writing to complete
        let semaphore = DispatchSemaphore(value: 0)
        assetWriter.finishWriting {
            semaphore.signal()
        }
        semaphore.wait()
        
        if assetWriter.status == .completed {
            print("[CaptureController] Successfully saved \(framesWritten) frames")
            return true
        } else {
            print("[CaptureController] Save failed: \(assetWriter.error?.localizedDescription ?? "unknown")")
            return false
        }
    }
    
    // MARK: - Encoder Setup
    
    private func setupEncoder() -> Bool {
        var session: VTCompressionSession?
        
        let status = VTCompressionSessionCreate(
            allocator: kCFAllocatorDefault,
            width: Int32(config.width),
            height: Int32(config.height),
            codecType: kCMVideoCodecType_H264,
            encoderSpecification: nil,
            imageBufferAttributes: [
                kCVPixelBufferPixelFormatTypeKey: kCVPixelFormatType_32BGRA,
                kCVPixelBufferWidthKey: config.width,
                kCVPixelBufferHeightKey: config.height,
            ] as CFDictionary,
            compressedDataAllocator: nil,
            outputCallback: nil,  // We'll use VTCompressionSessionEncodeFrameWithOutputHandler
            refcon: nil,
            compressionSessionOut: &session
        )
        
        guard status == noErr, let session = session else {
            print("[CaptureController] Failed to create compression session: \(status)")
            return false
        }
        
        // Configure encoder for real-time encoding
        VTSessionSetProperty(session, key: kVTCompressionPropertyKey_RealTime, value: kCFBooleanTrue)
        VTSessionSetProperty(session, key: kVTCompressionPropertyKey_ProfileLevel, value: kVTProfileLevel_H264_High_AutoLevel)
        VTSessionSetProperty(session, key: kVTCompressionPropertyKey_AverageBitRate, value: config.bitrate as CFNumber)
        VTSessionSetProperty(session, key: kVTCompressionPropertyKey_MaxKeyFrameInterval, value: config.fps as CFNumber)  // Keyframe every second
        VTSessionSetProperty(session, key: kVTCompressionPropertyKey_MaxKeyFrameIntervalDuration, value: config.keyframeInterval as CFNumber)
        VTSessionSetProperty(session, key: kVTCompressionPropertyKey_AllowFrameReordering, value: kCFBooleanFalse)  // No B-frames for lower latency
        VTSessionSetProperty(session, key: kVTCompressionPropertyKey_ExpectedFrameRate, value: config.fps as CFNumber)
        
        VTCompressionSessionPrepareToEncodeFrames(session)
        
        self.compressionSession = session
        print("[CaptureController] Encoder configured: H.264, \(config.bitrate/1_000_000)Mbps, keyframe every \(config.keyframeInterval)s")
        
        return true
    }
    
    private func teardownEncoder() {
        if let session = compressionSession {
            VTCompressionSessionInvalidate(session)
            compressionSession = nil
        }
    }
    
    // MARK: - Screen Capture Setup
    
    private func setupScreenCapture() async -> Bool {
        do {
            let content = try await SCShareableContent.excludingDesktopWindows(false, onScreenWindowsOnly: true)
            
            guard let display = content.displays.first else {
                print("[CaptureController] No displays found")
                return false
            }
            
            print("[CaptureController] Capturing display: \(display.width)x\(display.height)")
            
            let filter = SCContentFilter(display: display, excludingWindows: [])
            
            let streamConfig = SCStreamConfiguration()
            streamConfig.width = Int(config.width)
            streamConfig.height = Int(config.height)
            streamConfig.minimumFrameInterval = CMTime(value: 1, timescale: CMTimeScale(config.fps))
            streamConfig.pixelFormat = kCVPixelFormatType_32BGRA
            streamConfig.queueDepth = 3
            streamConfig.showsCursor = true
            
            stream = SCStream(filter: filter, configuration: streamConfig, delegate: nil)
            
            streamOutput = StreamOutput { [weak self] sampleBuffer in
                self?.handleCapturedFrame(sampleBuffer)
            }
            
            try stream?.addStreamOutput(streamOutput!, type: .screen, sampleHandlerQueue: captureQueue)
            try await stream?.startCapture()
            
            return true
            
        } catch {
            print("[CaptureController] Screen capture setup failed: \(error)")
            return false
        }
    }
    
    // MARK: - Frame Handling
    
    private func handleCapturedFrame(_ sampleBuffer: CMSampleBuffer) {
        framesCapture += 1
        
        guard isCapturing,
              let session = compressionSession,
              let pixelBuffer = CMSampleBufferGetImageBuffer(sampleBuffer) else {
            framesDropped += 1
            return
        }
        
        let presentationTime = CMSampleBufferGetPresentationTimeStamp(sampleBuffer)
        let duration = CMSampleBufferGetDuration(sampleBuffer)
        
        // Set base time on first frame
        if baseTime == nil {
            baseTime = presentationTime
        }
        
        // Encode frame
        encodeQueue.async { [weak self] in
            self?.encodeFrame(pixelBuffer: pixelBuffer, presentationTime: presentationTime, duration: duration)
        }
    }
    
    private func encodeFrame(pixelBuffer: CVPixelBuffer, presentationTime: CMTime, duration: CMTime) {
        guard let session = compressionSession else { return }
        
        // Encode with output handler (receives encoded data synchronously)
        let status = VTCompressionSessionEncodeFrame(
            session,
            imageBuffer: pixelBuffer,
            presentationTimeStamp: presentationTime,
            duration: duration,
            frameProperties: nil,
            infoFlagsOut: nil
        ) { [weak self] status, infoFlags, sampleBuffer in
            guard status == noErr, let sampleBuffer = sampleBuffer else {
                self?.framesDropped += 1
                return
            }
            
            self?.handleEncodedFrame(sampleBuffer, presentationTime: presentationTime, duration: duration)
        }
        
        if status != noErr {
            framesDropped += 1
        }
    }
    
    private func handleEncodedFrame(_ sampleBuffer: CMSampleBuffer, presentationTime: CMTime, duration: CMTime) {
        // Store format description for later muxing
        if formatDescription == nil {
            formatDescription = CMSampleBufferGetFormatDescription(sampleBuffer)
        }
        
        // Check if this is a keyframe
        let attachments = CMSampleBufferGetSampleAttachmentsArray(sampleBuffer, createIfNecessary: false)
        var isKeyFrame = true
        if let attachments = attachments, CFArrayGetCount(attachments) > 0 {
            let dict = unsafeBitCast(CFArrayGetValueAtIndex(attachments, 0), to: CFDictionary.self)
            if let notSync = CFDictionaryGetValue(dict, Unmanaged.passUnretained(kCMSampleAttachmentKey_NotSync).toOpaque()) {
                isKeyFrame = !(unsafeBitCast(notSync, to: CFBoolean.self) == kCFBooleanTrue)
            }
        }
        
        // Extract the encoded H.264 data
        guard let dataBuffer = CMSampleBufferGetDataBuffer(sampleBuffer) else {
            framesDropped += 1
            return
        }
        
        var length: Int = 0
        var dataPointer: UnsafeMutablePointer<Int8>?
        CMBlockBufferGetDataPointer(dataBuffer, atOffset: 0, lengthAtOffsetOut: nil, totalLengthOut: &length, dataPointerOut: &dataPointer)
        
        guard let dataPointer = dataPointer, length > 0 else {
            framesDropped += 1
            return
        }
        
        let data = Data(bytes: dataPointer, count: length)
        
        let frame = EncodedFrame(
            data: data,
            presentationTime: presentationTime,
            duration: duration,
            isKeyFrame: isKeyFrame
        )
        
        replayBuffer.append(frame)
        framesEncoded += 1
    }
    
    // MARK: - Sample Buffer Creation (for saving)
    
    private func createSampleBuffer(from frame: EncodedFrame, adjustedTime: CMTime, formatDescription: CMFormatDescription) -> CMSampleBuffer? {
        var blockBuffer: CMBlockBuffer?
        
        let data = frame.data
        let status = data.withUnsafeBytes { (bytes: UnsafeRawBufferPointer) -> OSStatus in
            guard let baseAddress = bytes.baseAddress else { return -1 }
            
            return CMBlockBufferCreateWithMemoryBlock(
                allocator: kCFAllocatorDefault,
                memoryBlock: nil,
                blockLength: data.count,
                blockAllocator: kCFAllocatorDefault,
                customBlockSource: nil,
                offsetToData: 0,
                dataLength: data.count,
                flags: 0,
                blockBufferOut: &blockBuffer
            )
        }
        
        guard status == noErr, let blockBuffer = blockBuffer else {
            return nil
        }
        
        // Copy data to block buffer
        let copyStatus = data.withUnsafeBytes { (bytes: UnsafeRawBufferPointer) -> OSStatus in
            guard let baseAddress = bytes.baseAddress else { return -1 }
            return CMBlockBufferReplaceDataBytes(
                with: baseAddress,
                blockBuffer: blockBuffer,
                offsetIntoDestination: 0,
                dataLength: data.count
            )
        }
        
        guard copyStatus == noErr else { return nil }
        
        var sampleBuffer: CMSampleBuffer?
        var timingInfo = CMSampleTimingInfo(
            duration: frame.duration,
            presentationTimeStamp: adjustedTime,
            decodeTimeStamp: .invalid
        )
        
        var sampleSize = data.count
        
        let sampleStatus = CMSampleBufferCreateReady(
            allocator: kCFAllocatorDefault,
            dataBuffer: blockBuffer,
            formatDescription: formatDescription,
            sampleCount: 1,
            sampleTimingEntryCount: 1,
            sampleTimingArray: &timingInfo,
            sampleSizeEntryCount: 1,
            sampleSizeArray: &sampleSize,
            sampleBufferOut: &sampleBuffer
        )
        
        guard sampleStatus == noErr else { return nil }
        
        // Set keyframe flag
        if frame.isKeyFrame {
            if let attachments = CMSampleBufferGetSampleAttachmentsArray(sampleBuffer!, createIfNecessary: true) {
                let dict = unsafeBitCast(CFArrayGetValueAtIndex(attachments, 0), to: CFMutableDictionary.self)
                CFDictionarySetValue(dict, Unmanaged.passUnretained(kCMSampleAttachmentKey_NotSync).toOpaque(), 
                                    Unmanaged.passUnretained(kCFBooleanFalse).toOpaque())
            }
        }
        
        return sampleBuffer
    }
    
    // MARK: - Public Properties
    
    public var isActive: Bool { isCapturing }
    public var framesCaptured: UInt64 { framesCapture }
    public var getFramesDropped: UInt64 { framesDropped }
    public var getFramesEncoded: UInt64 { framesEncoded }
    public var bufferFillPercent: Float { replayBuffer.fillPercentage() * 100.0 }
    public var bufferMemoryBytes: Int { replayBuffer.memorySizeBytes() }
}

// MARK: - Stream Output Handler

private class StreamOutput: NSObject, SCStreamOutput {
    private let handler: (CMSampleBuffer) -> Void
    
    init(handler: @escaping (CMSampleBuffer) -> Void) {
        self.handler = handler
        super.init()
    }
    
    func stream(_ stream: SCStream, didOutputSampleBuffer sampleBuffer: CMSampleBuffer, of type: SCStreamOutputType) {
        guard type == .screen else { return }
        handler(sampleBuffer)
    }
}

// MARK: - C Interface for Rust FFI

/// Creates a new capture controller
/// bufferDurationSecs: how many seconds of footage to keep in replay buffer
@_cdecl("swift_replay_create")
public func swift_replay_create(
    _ width: UInt32,
    _ height: UInt32,
    _ fps: UInt32,
    _ bitrate: UInt32,
    _ keyframeInterval: Float,
    _ bufferDurationSecs: Float
) -> UnsafeMutableRawPointer? {
    let config = CaptureConfig(
        width: width,
        height: height,
        fps: fps,
        bitrate: bitrate,
        keyframeInterval: keyframeInterval,
        bufferDurationSecs: bufferDurationSecs
    )
    let controller = CaptureController(config: config)
    return Unmanaged.passRetained(controller).toOpaque()
}

/// Starts capturing to the replay buffer
@_cdecl("swift_replay_start")
public func swift_replay_start(_ handle: UnsafeMutableRawPointer) -> Bool {
    let controller = Unmanaged<CaptureController>.fromOpaque(handle).takeUnretainedValue()
    return controller.start()
}

/// Stops capturing
@_cdecl("swift_replay_stop")
public func swift_replay_stop(_ handle: UnsafeMutableRawPointer) {
    let controller = Unmanaged<CaptureController>.fromOpaque(handle).takeUnretainedValue()
    controller.stop()
}

/// Saves the current replay buffer to an MP4 file
@_cdecl("swift_replay_save")
public func swift_replay_save(_ handle: UnsafeMutableRawPointer, _ outputPath: UnsafePointer<CChar>) -> Bool {
    let controller = Unmanaged<CaptureController>.fromOpaque(handle).takeUnretainedValue()
    let path = String(cString: outputPath)
    return controller.saveReplay(outputPath: path)
}

/// Destroys the capture controller
@_cdecl("swift_replay_destroy")
public func swift_replay_destroy(_ handle: UnsafeMutableRawPointer) {
    let _ = Unmanaged<CaptureController>.fromOpaque(handle).takeRetainedValue()
    // Controller will be deallocated when this function returns
}

/// Returns whether capture is active
@_cdecl("swift_replay_is_active")
public func swift_replay_is_active(_ handle: UnsafeMutableRawPointer) -> Bool {
    let controller = Unmanaged<CaptureController>.fromOpaque(handle).takeUnretainedValue()
    return controller.isActive
}

/// Returns number of frames captured
@_cdecl("swift_replay_get_frames_captured")
public func swift_replay_get_frames_captured(_ handle: UnsafeMutableRawPointer) -> UInt64 {
    let controller = Unmanaged<CaptureController>.fromOpaque(handle).takeUnretainedValue()
    return controller.framesCaptured
}

/// Returns number of frames dropped
@_cdecl("swift_replay_get_frames_dropped")
public func swift_replay_get_frames_dropped(_ handle: UnsafeMutableRawPointer) -> UInt64 {
    let controller = Unmanaged<CaptureController>.fromOpaque(handle).takeUnretainedValue()
    return controller.getFramesDropped
}

/// Returns number of frames encoded
@_cdecl("swift_replay_get_frames_encoded")
public func swift_replay_get_frames_encoded(_ handle: UnsafeMutableRawPointer) -> UInt64 {
    let controller = Unmanaged<CaptureController>.fromOpaque(handle).takeUnretainedValue()
    return controller.getFramesEncoded
}

/// Returns buffer fill percentage (0-100)
@_cdecl("swift_replay_get_buffer_fill")
public func swift_replay_get_buffer_fill(_ handle: UnsafeMutableRawPointer) -> Float {
    let controller = Unmanaged<CaptureController>.fromOpaque(handle).takeUnretainedValue()
    return controller.bufferFillPercent
}

/// Returns buffer memory usage in bytes
@_cdecl("swift_replay_get_buffer_memory")
public func swift_replay_get_buffer_memory(_ handle: UnsafeMutableRawPointer) -> UInt64 {
    let controller = Unmanaged<CaptureController>.fromOpaque(handle).takeUnretainedValue()
    return UInt64(controller.bufferMemoryBytes)
}
