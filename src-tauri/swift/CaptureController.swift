// CaptureController.swift
// Hardware-accelerated screen capture pipeline for macOS
//
// Pipeline: ScreenCaptureKit → CVPixelBuffer → VideoToolbox → AVAssetWriter
//
// Uses separate dispatch queues for:
// - Capture (receiving frames from SCStream)
// - Encode (VideoToolbox H.264 encoding)
// - IO (AVAssetWriter file output)

import Foundation
import ScreenCaptureKit
import CoreMedia
import CoreVideo
import VideoToolbox
import AVFoundation

// MARK: - Configuration (matches Rust struct)

@frozen
public struct CaptureConfig {
    public var width: UInt32
    public var height: UInt32
    public var fps: UInt32
    public var bitrate: UInt32
    public var keyframeInterval: Float
}

// MARK: - Capture Controller

@objc public class CaptureController: NSObject {
    
    // Configuration
    private let config: CaptureConfig
    
    // ScreenCaptureKit
    private var stream: SCStream?
    private var streamOutput: StreamOutput?
    
    // Encoding
    private var assetWriter: AVAssetWriter?
    private var videoInput: AVAssetWriterInput?
    private var pixelBufferAdaptor: AVAssetWriterInputPixelBufferAdaptor?
    
    // Queues (separate for each pipeline stage)
    private let captureQueue = DispatchQueue(label: "com.questshadowplay.capture", qos: .userInteractive)
    private let encodeQueue = DispatchQueue(label: "com.questshadowplay.encode", qos: .userInteractive)
    private let ioQueue = DispatchQueue(label: "com.questshadowplay.io", qos: .utility)
    
    // State
    private var isCapturing = false
    private var startTime: CMTime?
    
    // Statistics
    private var framesCapture: UInt64 = 0
    private var framesDropped: UInt64 = 0
    private var framesEncoded: UInt64 = 0
    
    // MARK: - Initialization
    
    public init(config: CaptureConfig) {
        self.config = config
        super.init()
    }
    
    // MARK: - Public API
    
    public func start(outputPath: String) -> Bool {
        guard !isCapturing else {
            print("[CaptureController] Already capturing")
            return false
        }
        
        // Reset stats
        framesCapture = 0
        framesDropped = 0
        framesEncoded = 0
        startTime = nil
        
        // Setup asset writer
        guard setupAssetWriter(outputPath: outputPath) else {
            print("[CaptureController] Failed to setup asset writer")
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
            print("[CaptureController] Started capture: \(config.width)x\(config.height) @ \(config.fps)fps")
        }
        
        return success
    }
    
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
        
        // Finalize asset writer
        encodeQueue.sync {
            self.videoInput?.markAsFinished()
        }
        
        let semaphore = DispatchSemaphore(value: 0)
        assetWriter?.finishWriting {
            semaphore.signal()
        }
        semaphore.wait()
        
        print("[CaptureController] Stopped. Captured: \(framesCapture), Dropped: \(framesDropped), Encoded: \(framesEncoded)")
    }
    
    public var isActive: Bool { isCapturing }
    public var framesCaptured: UInt64 { framesCapture }
    public var getFramesDropped: UInt64 { framesDropped }
    public var getFramesEncoded: UInt64 { framesEncoded }
    
    // MARK: - Asset Writer Setup
    
    private func setupAssetWriter(outputPath: String) -> Bool {
        let url = URL(fileURLWithPath: outputPath)
        
        // Remove existing file
        try? FileManager.default.removeItem(at: url)
        
        do {
            assetWriter = try AVAssetWriter(outputURL: url, fileType: .mp4)
        } catch {
            print("[CaptureController] Failed to create asset writer: \(error)")
            return false
        }
        
        // Video settings - H.264 hardware encoding
        let videoSettings: [String: Any] = [
            AVVideoCodecKey: AVVideoCodecType.h264,
            AVVideoWidthKey: config.width,
            AVVideoHeightKey: config.height,
            AVVideoCompressionPropertiesKey: [
                AVVideoAverageBitRateKey: config.bitrate,
                AVVideoMaxKeyFrameIntervalKey: Int(Float(config.fps) * config.keyframeInterval),
                AVVideoProfileLevelKey: AVVideoProfileLevelH264HighAutoLevel,
                AVVideoExpectedSourceFrameRateKey: config.fps,
                AVVideoAllowFrameReorderingKey: false, // Lower latency
            ] as [String: Any]
        ]
        
        videoInput = AVAssetWriterInput(mediaType: .video, outputSettings: videoSettings)
        videoInput?.expectsMediaDataInRealTime = true
        
        // Pixel buffer adaptor for efficient buffer handling
        let sourcePixelBufferAttributes: [String: Any] = [
            kCVPixelBufferPixelFormatTypeKey as String: kCVPixelFormatType_32BGRA,
            kCVPixelBufferWidthKey as String: config.width,
            kCVPixelBufferHeightKey as String: config.height,
        ]
        
        pixelBufferAdaptor = AVAssetWriterInputPixelBufferAdaptor(
            assetWriterInput: videoInput!,
            sourcePixelBufferAttributes: sourcePixelBufferAttributes
        )
        
        if assetWriter!.canAdd(videoInput!) {
            assetWriter!.add(videoInput!)
        } else {
            print("[CaptureController] Cannot add video input")
            return false
        }
        
        guard assetWriter!.startWriting() else {
            print("[CaptureController] Failed to start writing: \(assetWriter!.error?.localizedDescription ?? "unknown")")
            return false
        }
        
        assetWriter!.startSession(atSourceTime: .zero)
        
        return true
    }
    
    // MARK: - Screen Capture Setup
    
    private func setupScreenCapture() async -> Bool {
        // Check permission
        do {
            // This will prompt for permission if needed
            let content = try await SCShareableContent.excludingDesktopWindows(false, onScreenWindowsOnly: true)
            
            guard let display = content.displays.first else {
                print("[CaptureController] No displays found")
                return false
            }
            
            print("[CaptureController] Capturing display: \(display.width)x\(display.height)")
            
            // Configure stream
            let filter = SCContentFilter(display: display, excludingWindows: [])
            
            let streamConfig = SCStreamConfiguration()
            streamConfig.width = Int(config.width)
            streamConfig.height = Int(config.height)
            streamConfig.minimumFrameInterval = CMTime(value: 1, timescale: CMTimeScale(config.fps))
            streamConfig.pixelFormat = kCVPixelFormatType_32BGRA
            streamConfig.queueDepth = 3 // Small queue to avoid latency
            streamConfig.showsCursor = true
            
            // Create stream
            stream = SCStream(filter: filter, configuration: streamConfig, delegate: nil)
            
            // Create output handler
            streamOutput = StreamOutput { [weak self] sampleBuffer in
                self?.handleFrame(sampleBuffer)
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
    
    private func handleFrame(_ sampleBuffer: CMSampleBuffer) {
        framesCapture += 1
        
        guard isCapturing,
              let videoInput = videoInput,
              videoInput.isReadyForMoreMediaData else {
            framesDropped += 1
            return
        }
        
        // Get pixel buffer
        guard let pixelBuffer = CMSampleBufferGetImageBuffer(sampleBuffer) else {
            framesDropped += 1
            return
        }
        
        // Get presentation time
        let presentationTime = CMSampleBufferGetPresentationTimeStamp(sampleBuffer)
        
        // Set start time on first frame
        if startTime == nil {
            startTime = presentationTime
        }
        
        // Calculate relative time
        let relativeTime = CMTimeSubtract(presentationTime, startTime!)
        
        // Encode on encode queue
        encodeQueue.async { [weak self] in
            guard let self = self,
                  let adaptor = self.pixelBufferAdaptor,
                  self.videoInput?.isReadyForMoreMediaData == true else {
                return
            }
            
            if adaptor.append(pixelBuffer, withPresentationTime: relativeTime) {
                self.framesEncoded += 1
            } else {
                self.framesDropped += 1
            }
        }
    }
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

@_cdecl("swift_capture_create")
public func swift_capture_create(
    _ width: UInt32,
    _ height: UInt32,
    _ fps: UInt32,
    _ bitrate: UInt32,
    _ keyframeInterval: Float
) -> UnsafeMutableRawPointer? {
    let config = CaptureConfig(
        width: width,
        height: height,
        fps: fps,
        bitrate: bitrate,
        keyframeInterval: keyframeInterval
    )
    let controller = CaptureController(config: config)
    return Unmanaged.passRetained(controller).toOpaque()
}

@_cdecl("swift_capture_start")
public func swift_capture_start(_ handle: UnsafeMutableRawPointer, _ outputPath: UnsafePointer<CChar>) -> Bool {
    let controller = Unmanaged<CaptureController>.fromOpaque(handle).takeUnretainedValue()
    let path = String(cString: outputPath)
    return controller.start(outputPath: path)
}

@_cdecl("swift_capture_stop")
public func swift_capture_stop(_ handle: UnsafeMutableRawPointer) {
    let controller = Unmanaged<CaptureController>.fromOpaque(handle).takeUnretainedValue()
    controller.stop()
}

@_cdecl("swift_capture_destroy")
public func swift_capture_destroy(_ handle: UnsafeMutableRawPointer) {
    let controller = Unmanaged<CaptureController>.fromOpaque(handle).takeRetainedValue()
    controller.stop()
    // Controller will be deallocated when this function returns
}

@_cdecl("swift_capture_get_frames_captured")
public func swift_capture_get_frames_captured(_ handle: UnsafeMutableRawPointer) -> UInt64 {
    let controller = Unmanaged<CaptureController>.fromOpaque(handle).takeUnretainedValue()
    return controller.framesCaptured
}

@_cdecl("swift_capture_get_frames_dropped")
public func swift_capture_get_frames_dropped(_ handle: UnsafeMutableRawPointer) -> UInt64 {
    let controller = Unmanaged<CaptureController>.fromOpaque(handle).takeUnretainedValue()
    return controller.getFramesDropped
}

@_cdecl("swift_capture_get_frames_encoded")
public func swift_capture_get_frames_encoded(_ handle: UnsafeMutableRawPointer) -> UInt64 {
    let controller = Unmanaged<CaptureController>.fromOpaque(handle).takeUnretainedValue()
    return controller.getFramesEncoded
}

@_cdecl("swift_capture_is_active")
public func swift_capture_is_active(_ handle: UnsafeMutableRawPointer) -> Bool {
    let controller = Unmanaged<CaptureController>.fromOpaque(handle).takeUnretainedValue()
    return controller.isActive
}

