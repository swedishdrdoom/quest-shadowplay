// VideoTrimmer.swift
// Video trimming functionality using AVFoundation
//
// Provides:
// - Video metadata extraction (duration, resolution, codec)
// - Timeline thumbnail generation
// - Lossless video trimming
//
// All operations are designed to be fast and non-blocking where possible.

import Foundation
import AVFoundation
import CoreMedia
import CoreGraphics
import AppKit

// MARK: - Video Info Structure

/// Information about a video clip, passed back to Rust
@frozen
public struct VideoInfo {
    public var durationSecs: Float      // Total duration in seconds
    public var width: UInt32            // Video width in pixels
    public var height: UInt32           // Video height in pixels
    public var frameRate: Float         // Frames per second
    public var fileSizeBytes: UInt64    // File size in bytes
    public var hasAudio: Bool           // Whether video has audio track
    public var codec: UInt32            // 0 = unknown, 1 = H.264, 2 = HEVC
}

/// Result of a trim operation
@frozen
public struct TrimResult {
    public var success: Bool
    public var errorMessage: UnsafePointer<CChar>?
    public var outputPath: UnsafePointer<CChar>?
    public var outputSizeBytes: UInt64
}

// MARK: - Video Trimmer Class

/// Handles video analysis and trimming operations
public class VideoTrimmer {
    
    /// Get information about a video file
    public static func getVideoInfo(path: String) -> VideoInfo {
        var info = VideoInfo(
            durationSecs: 0,
            width: 0,
            height: 0,
            frameRate: 0,
            fileSizeBytes: 0,
            hasAudio: false,
            codec: 0
        )
        
        let url = URL(fileURLWithPath: path)
        
        // Get file size
        if let attrs = try? FileManager.default.attributesOfItem(atPath: path),
           let size = attrs[.size] as? UInt64 {
            info.fileSizeBytes = size
        }
        
        let asset = AVAsset(url: url)
        
        // Get duration
        let duration = asset.duration
        if duration.isValid && !duration.isIndefinite {
            info.durationSecs = Float(CMTimeGetSeconds(duration))
        }
        
        // Get video track info
        let videoTracks = asset.tracks(withMediaType: .video)
        if let videoTrack = videoTracks.first {
            let size = videoTrack.naturalSize
            let transform = videoTrack.preferredTransform
            
            // Apply transform to get actual dimensions
            let transformedSize = size.applying(transform)
            info.width = UInt32(abs(transformedSize.width))
            info.height = UInt32(abs(transformedSize.height))
            
            // Frame rate
            info.frameRate = videoTrack.nominalFrameRate
            
            // Codec detection
            let formatDescriptions = videoTrack.formatDescriptions as? [CMFormatDescription] ?? []
            for format in formatDescriptions {
                let codecType = CMFormatDescriptionGetMediaSubType(format)
                switch codecType {
                case kCMVideoCodecType_H264:
                    info.codec = 1
                case kCMVideoCodecType_HEVC:
                    info.codec = 2
                default:
                    info.codec = 0
                }
            }
        }
        
        // Check for audio
        info.hasAudio = !asset.tracks(withMediaType: .audio).isEmpty
        
        return info
    }
    
    /// Generate thumbnails for the timeline
    /// Returns JPEG data for each thumbnail, concatenated with size prefixes
    public static func generateTimelineThumbnails(
        path: String,
        count: Int,
        thumbnailWidth: Int
    ) -> Data? {
        let url = URL(fileURLWithPath: path)
        let asset = AVAsset(url: url)
        
        let duration = asset.duration
        guard duration.isValid && !duration.isIndefinite else {
            NSLog("VideoTrimmer: Invalid duration for \(path)")
            return nil
        }
        
        let durationSecs = CMTimeGetSeconds(duration)
        guard durationSecs > 0 else {
            NSLog("VideoTrimmer: Zero duration for \(path)")
            return nil
        }
        
        let generator = AVAssetImageGenerator(asset: asset)
        generator.appliesPreferredTrackTransform = true
        generator.maximumSize = CGSize(width: thumbnailWidth, height: 0)  // Height auto-calculated
        generator.requestedTimeToleranceBefore = CMTime(seconds: 0.5, preferredTimescale: 600)
        generator.requestedTimeToleranceAfter = CMTime(seconds: 0.5, preferredTimescale: 600)
        
        var result = Data()
        
        // Calculate time intervals
        let interval = durationSecs / Double(count)
        
        for i in 0..<count {
            let time = CMTime(seconds: interval * Double(i) + interval / 2, preferredTimescale: 600)
            
            do {
                let cgImage = try generator.copyCGImage(at: time, actualTime: nil)
                
                // Convert to JPEG
                let nsImage = NSImage(cgImage: cgImage, size: NSSize(width: cgImage.width, height: cgImage.height))
                if let tiffData = nsImage.tiffRepresentation,
                   let bitmap = NSBitmapImageRep(data: tiffData),
                   let jpegData = bitmap.representation(using: .jpeg, properties: [.compressionFactor: 0.7]) {
                    
                    // Write size as 4-byte little-endian integer
                    var size = UInt32(jpegData.count).littleEndian
                    result.append(Data(bytes: &size, count: 4))
                    result.append(jpegData)
                }
            } catch {
                NSLog("VideoTrimmer: Failed to generate thumbnail at \(time.seconds)s: \(error)")
                // Write zero-size to indicate failed thumbnail
                var size: UInt32 = 0
                result.append(Data(bytes: &size, count: 4))
            }
        }
        
        return result
    }
    
    /// Trim a video to the specified time range
    /// Uses AVAssetExportSession for fast, near-lossless trimming
    public static func trimVideo(
        inputPath: String,
        outputPath: String,
        startTime: Float,
        endTime: Float,
        completion: @escaping (Bool, String?) -> Void
    ) {
        let inputURL = URL(fileURLWithPath: inputPath)
        let outputURL = URL(fileURLWithPath: outputPath)
        
        let asset = AVAsset(url: inputURL)
        
        // Validate times
        let duration = CMTimeGetSeconds(asset.duration)
        let clampedStart = max(0, min(Double(startTime), duration))
        let clampedEnd = max(clampedStart, min(Double(endTime), duration))
        
        if clampedEnd - clampedStart < 0.1 {
            completion(false, "Trim duration too short (minimum 0.1 seconds)")
            return
        }
        
        // Create time range
        let startCMTime = CMTime(seconds: clampedStart, preferredTimescale: 600)
        let endCMTime = CMTime(seconds: clampedEnd, preferredTimescale: 600)
        let timeRange = CMTimeRange(start: startCMTime, end: endCMTime)
        
        // Remove existing file if present
        try? FileManager.default.removeItem(at: outputURL)
        
        // Create export session
        guard let exportSession = AVAssetExportSession(
            asset: asset,
            presetName: AVAssetExportPresetPassthrough  // No re-encoding, very fast
        ) else {
            completion(false, "Failed to create export session")
            return
        }
        
        exportSession.outputURL = outputURL
        exportSession.outputFileType = .mp4
        exportSession.timeRange = timeRange
        
        NSLog("VideoTrimmer: Trimming \(inputPath) from \(clampedStart)s to \(clampedEnd)s")
        
        exportSession.exportAsynchronously {
            DispatchQueue.main.async {
                switch exportSession.status {
                case .completed:
                    NSLog("VideoTrimmer: Trim completed successfully to \(outputPath)")
                    completion(true, nil)
                case .failed:
                    let errorMsg = exportSession.error?.localizedDescription ?? "Unknown error"
                    NSLog("VideoTrimmer: Trim failed: \(errorMsg)")
                    completion(false, errorMsg)
                case .cancelled:
                    NSLog("VideoTrimmer: Trim cancelled")
                    completion(false, "Export cancelled")
                default:
                    completion(false, "Unexpected export status: \(exportSession.status.rawValue)")
                }
            }
        }
    }
    
    /// Synchronous version of trimVideo for FFI
    public static func trimVideoSync(
        inputPath: String,
        outputPath: String,
        startTime: Float,
        endTime: Float
    ) -> (success: Bool, error: String?) {
        let semaphore = DispatchSemaphore(value: 0)
        var result: (Bool, String?) = (false, "Timeout")
        
        trimVideo(inputPath: inputPath, outputPath: outputPath, startTime: startTime, endTime: endTime) { success, error in
            result = (success, error)
            semaphore.signal()
        }
        
        // Wait up to 60 seconds
        let timeout = semaphore.wait(timeout: .now() + 60)
        if timeout == .timedOut {
            return (false, "Trim operation timed out")
        }
        
        return result
    }
}

// MARK: - C-callable Functions for Rust FFI

/// Get video information
/// Writes video metadata to output parameters
/// Returns 1 on success, 0 on failure
@_cdecl("swift_get_video_info")
public func swift_get_video_info(
    _ pathPtr: UnsafePointer<CChar>,
    _ outDuration: UnsafeMutablePointer<Float>,
    _ outWidth: UnsafeMutablePointer<UInt32>,
    _ outHeight: UnsafeMutablePointer<UInt32>,
    _ outFrameRate: UnsafeMutablePointer<Float>,
    _ outFileSize: UnsafeMutablePointer<UInt64>,
    _ outHasAudio: UnsafeMutablePointer<Int32>,
    _ outCodec: UnsafeMutablePointer<UInt32>
) -> Int32 {
    let path = String(cString: pathPtr)
    let info = VideoTrimmer.getVideoInfo(path: path)
    
    // Check if we got valid info
    if info.durationSecs <= 0 {
        return 0
    }
    
    outDuration.pointee = info.durationSecs
    outWidth.pointee = info.width
    outHeight.pointee = info.height
    outFrameRate.pointee = info.frameRate
    outFileSize.pointee = info.fileSizeBytes
    outHasAudio.pointee = info.hasAudio ? 1 : 0
    outCodec.pointee = info.codec
    
    return 1
}

/// Generate timeline thumbnails
/// Returns pointer to thumbnail data, or NULL on failure
/// Caller must free the returned pointer with swift_free_data
@_cdecl("swift_generate_thumbnails")
public func swift_generate_thumbnails(
    _ pathPtr: UnsafePointer<CChar>,
    _ count: UInt32,
    _ thumbnailWidth: UInt32,
    _ outSize: UnsafeMutablePointer<UInt64>
) -> UnsafeMutableRawPointer? {
    let path = String(cString: pathPtr)
    
    guard let data = VideoTrimmer.generateTimelineThumbnails(
        path: path,
        count: Int(count),
        thumbnailWidth: Int(thumbnailWidth)
    ) else {
        outSize.pointee = 0
        return nil
    }
    
    // Allocate memory and copy data
    let ptr = UnsafeMutableRawPointer.allocate(byteCount: data.count, alignment: 1)
    data.copyBytes(to: ptr.assumingMemoryBound(to: UInt8.self), count: data.count)
    outSize.pointee = UInt64(data.count)
    
    return ptr
}

/// Free data allocated by swift_generate_thumbnails
@_cdecl("swift_free_data")
public func swift_free_data(_ ptr: UnsafeMutableRawPointer?, _ size: UInt64) {
    if let ptr = ptr {
        ptr.deallocate()
    }
}

/// Trim video synchronously
/// Returns 1 on success, 0 on failure
/// On failure, errorOut will point to error message (must be freed with swift_free_string)
@_cdecl("swift_trim_video")
public func swift_trim_video(
    _ inputPathPtr: UnsafePointer<CChar>,
    _ outputPathPtr: UnsafePointer<CChar>,
    _ startTime: Float,
    _ endTime: Float,
    _ errorOut: UnsafeMutablePointer<UnsafePointer<CChar>?>
) -> Int32 {
    let inputPath = String(cString: inputPathPtr)
    let outputPath = String(cString: outputPathPtr)
    
    let (success, error) = VideoTrimmer.trimVideoSync(
        inputPath: inputPath,
        outputPath: outputPath,
        startTime: startTime,
        endTime: endTime
    )
    
    if let error = error {
        // Allocate string for error message
        let cString = strdup(error)
        errorOut.pointee = UnsafePointer(cString)
    } else {
        errorOut.pointee = nil
    }
    
    return success ? 1 : 0
}

/// Free string allocated by swift_trim_video error
@_cdecl("swift_free_string")
public func swift_free_string(_ ptr: UnsafePointer<CChar>?) {
    if let ptr = ptr {
        free(UnsafeMutablePointer(mutating: ptr))
    }
}

