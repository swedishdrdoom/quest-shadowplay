/**
 * Quest Shadowplay - Frontend Application
 * 
 * Connects the UI to the Tauri Rust backend.
 * Uses the new replay buffer pipeline for hardware-accelerated capture.
 */

// ============================================
// STATE
// ============================================

let isLegacyRecording = false;  // Legacy JPEG pipeline
let isReplayBufferActive = false;  // New H.264 replay buffer
let statusInterval = null;
let replayStatsInterval = null;

// ============================================
// TAURI BRIDGE
// ============================================

/**
 * Invoke a Tauri command
 */
async function invoke(cmd, args = {}) {
    if (window.__TAURI__) {
        return await window.__TAURI__.core.invoke(cmd, args);
    } else {
        // Mock for browser development
        console.log(`[Mock] invoke: ${cmd}`, args);
        return mockCommand(cmd, args);
    }
}

/**
 * Mock commands for browser development
 */
function mockCommand(cmd, args) {
    switch (cmd) {
        case 'get_status':
            return {
                is_recording: isLegacyRecording,
                buffer_fill_percent: isLegacyRecording ? Math.random() * 100 : 0,
                frame_count: isLegacyRecording ? Math.floor(Math.random() * 900) : 0,
                buffer_capacity: 900,
                clips_count: 0
            };
        case 'get_replay_stats':
            return {
                is_active: isReplayBufferActive,
                frames_captured: isReplayBufferActive ? 1800 : 0,
                frames_dropped: 0,
                frames_encoded: isReplayBufferActive ? 1800 : 0,
                buffer_fill_percent: isReplayBufferActive ? 85.0 : 0,
                buffer_memory_mb: isReplayBufferActive ? 8.5 : 0,
            };
        case 'start_replay_buffer':
            isReplayBufferActive = true;
            return { success: true, message: 'Replay buffer started' };
        case 'stop_replay_buffer':
            isReplayBufferActive = false;
            return { success: true, message: 'Replay buffer stopped' };
        case 'save_replay':
            return { success: true, message: 'Replay saved!', clip_path: '/tmp/replay.mp4' };
        case 'list_clips':
            return [];
        case 'delete_clip':
            return true;
        default:
            return null;
    }
}

// ============================================
// UI UPDATES
// ============================================

/**
 * Updates the legacy status display
 */
async function updateLegacyStatus() {
    try {
        const status = await invoke('get_status');
        
        // Update buffer display
        document.getElementById('buffer-percent').textContent = 
            `${status.buffer_fill_percent.toFixed(1)}%`;
        document.getElementById('buffer-fill').style.width = 
            `${status.buffer_fill_percent}%`;
        document.getElementById('frame-count').textContent = status.frame_count;
        document.getElementById('frame-capacity').textContent = status.buffer_capacity;
        
        // Update recording state
        isLegacyRecording = status.is_recording;
        updateLegacyRecordingUI();
        
        // Update save button
        document.getElementById('btn-save').disabled = status.frame_count === 0;
        
        // Update clip count
        document.getElementById('clip-count').textContent = 
            `${status.clips_count} clip${status.clips_count !== 1 ? 's' : ''}`;
            
    } catch (error) {
        console.error('Failed to get status:', error);
    }
}

/**
 * Updates the UI to reflect legacy recording state
 */
function updateLegacyRecordingUI() {
    const badge = document.getElementById('status-badge');
    const btn = document.getElementById('btn-record');
    
    if (isLegacyRecording) {
        badge.classList.add('recording');
        badge.querySelector('.status-text').textContent = 'Recording';
        btn.classList.add('recording');
        btn.querySelector('.btn-icon').textContent = '⏹';
        btn.querySelector('.btn-text').textContent = 'Stop Recording';
    } else {
        badge.classList.remove('recording');
        badge.querySelector('.status-text').textContent = 'Idle';
        btn.classList.remove('recording');
        btn.querySelector('.btn-icon').textContent = '▶';
        btn.querySelector('.btn-text').textContent = 'Start Recording';
    }
}

/**
 * Loads and displays saved clips
 */
async function loadClips() {
    try {
        const clips = await invoke('list_clips');
        const grid = document.getElementById('clips-grid');
        const emptyState = document.getElementById('empty-state');
        
        // Clear existing clips (except empty state)
        grid.querySelectorAll('.clip-card').forEach(el => el.remove());
        
        if (clips.length === 0) {
            emptyState.style.display = 'flex';
            return;
        }
        
        emptyState.style.display = 'none';
        
        // Add clip cards
        for (const clip of clips) {
            const card = createClipCard(clip);
            grid.appendChild(card);
            
            // Load thumbnail for all clip types
            loadThumbnail(clip.id, card);
        }
        
    } catch (error) {
        console.error('Failed to load clips:', error);
    }
}

/**
 * Creates a clip card element
 * Event handling is done via event delegation in setupClipsGridEventDelegation()
 */
function createClipCard(clip) {
    const card = document.createElement('div');
    card.className = 'clip-card';
    card.dataset.clipId = clip.id;
    
    const sizeKB = (clip.size_bytes / 1024).toFixed(1);
    const sizeMB = (clip.size_bytes / (1024 * 1024)).toFixed(1);
    const sizeDisplay = clip.size_bytes > 1024 * 1024 ? `${sizeMB} MB` : `${sizeKB} KB`;
    
    const isMP4 = clip.id.endsWith('.mp4');
    const icon = isMP4 ? '🎬' : '📹';
    
    card.innerHTML = `
        <div class="clip-thumbnail">
            ${icon}
        </div>
        <div class="clip-info">
            <div class="clip-time">${clip.timestamp || 'Unknown time'}</div>
            <div class="clip-size">${sizeDisplay}</div>
        </div>
        <div class="clip-actions">
            ${isMP4 ? `
                <button class="clip-action-btn open" data-action="open">
                    ▶️ Play
                </button>
                <button class="clip-action-btn trim" data-action="trim">
                    ✂️ Trim
                </button>
                <button class="clip-action-btn reveal" data-action="reveal">
                    📁
                </button>
            ` : `
                <button class="clip-action-btn export" data-action="export">
                    🎬 MP4
                </button>
            `}
            <button class="clip-action-btn delete" data-action="delete">
                🗑️
            </button>
        </div>
    `;
    
    return card;
}

/**
 * Loads a thumbnail for a clip
 */
async function loadThumbnail(clipId, card) {
    try {
        const thumbnail = await invoke('get_clip_thumbnail', { id: clipId });
        
        if (thumbnail) {
            const thumbEl = card.querySelector('.clip-thumbnail');
            thumbEl.innerHTML = `<img src="${thumbnail}" alt="Clip thumbnail">`;
        }
    } catch (error) {
        console.warn('Failed to load thumbnail:', error);
    }
}

// ============================================
// LEGACY ACTIONS (JPEG pipeline)
// ============================================

/**
 * Toggles legacy recording on/off
 */
async function toggleRecording() {
    const btn = document.getElementById('btn-record');
    btn.disabled = true;
    
    try {
        if (isLegacyRecording) {
            await invoke('stop_recording');
            showToast('Recording stopped', 'info');
        } else {
            const success = await invoke('start_recording');
            if (success) {
                showToast('Recording started', 'success');
            } else {
                showToast('Failed to start recording', 'error');
            }
        }
        
        await updateLegacyStatus();
        
    } catch (error) {
        console.error('Toggle recording failed:', error);
        showToast(`Error: ${error}`, 'error');
    } finally {
        btn.disabled = false;
    }
}

/**
 * Saves the current buffer as a clip (legacy)
 */
async function saveClip() {
    const btn = document.getElementById('btn-save');
    btn.disabled = true;
    btn.querySelector('.btn-text').textContent = 'Saving...';
    
    try {
        const result = await invoke('save_clip');
        
        if (result.success) {
            showToast(result.message, 'success');
            await loadClips();
        } else {
            showToast(result.message, 'error');
        }
        
    } catch (error) {
        console.error('Save failed:', error);
        showToast(`Save failed: ${error}`, 'error');
    } finally {
        btn.disabled = false;
        btn.querySelector('.btn-text').textContent = 'Save Clip';
    }
}

/**
 * Opens a clip in the default video player
 */
async function openClip(clipId) {
    console.log('Opening clip:', clipId);
    try {
        await invoke('open_clip', { id: clipId });
    } catch (error) {
        console.error('Open failed:', error);
        showToast(`Failed to open clip: ${error}`, 'error');
    }
}

/**
 * Reveals a clip in the file manager
 */
async function revealClip(clipId) {
    console.log('Revealing clip:', clipId);
    try {
        await invoke('reveal_clip', { id: clipId });
    } catch (error) {
        console.error('Reveal failed:', error);
        showToast(`Failed to reveal clip: ${error}`, 'error');
    }
}

/**
 * Exports a clip to MP4 (legacy)
 */
async function exportToMp4(clipId) {
    console.log('Exporting clip to MP4:', clipId);
    
    // Find the button for this clip to update its state
    const card = document.querySelector(`[data-clip-id="${clipId}"]`);
    const btn = card ? card.querySelector('[data-action="export"]') : null;
    const originalText = btn ? btn.textContent : '';
    
    if (btn) {
        btn.disabled = true;
        btn.textContent = '⏳ Exporting...';
    }
    
    try {
        showToast('Exporting to MP4...', 'info');
        const result = await invoke('export_to_mp4', { id: clipId });
        
        if (result.success) {
            showToast(`Exported! ${result.message}`, 'success');
            await loadClips();
        } else {
            showToast(`Export failed: ${result.message}`, 'error');
        }
    } catch (error) {
        console.error('Export failed:', error);
        showToast(`Export failed: ${error}`, 'error');
    } finally {
        if (btn) {
            btn.disabled = false;
            btn.textContent = originalText;
        }
    }
}

// ============================================
// REPLAY BUFFER (New H.264 Pipeline)
// ============================================

/**
 * Toggles replay buffer on/off
 */
async function toggleReplayBuffer() {
    const btn = document.getElementById('btn-replay-toggle');
    btn.disabled = true;
    
    try {
        if (isReplayBufferActive) {
            const result = await invoke('stop_replay_buffer');
            if (result.success) {
                showToast(result.message, 'success');
                isReplayBufferActive = false;
                stopReplayStatsPolling();
            } else {
                showToast(result.message, 'error');
            }
        } else {
            const result = await invoke('start_replay_buffer');
            if (result.success) {
                showToast(result.message, 'success');
                isReplayBufferActive = true;
                startReplayStatsPolling();
            } else {
                showToast(result.message, 'error');
            }
        }
        updateReplayBufferUI();
    } catch (error) {
        console.error('Replay buffer toggle failed:', error);
        showToast(`Error: ${error}`, 'error');
    } finally {
        btn.disabled = false;
    }
}

/**
 * Saves the current replay buffer to MP4
 */
async function saveReplay() {
    const btn = document.getElementById('btn-replay-save');
    btn.disabled = true;
    const originalText = btn.querySelector('.btn-text').textContent;
    btn.querySelector('.btn-text').textContent = 'Saving...';
    
    try {
        const result = await invoke('save_replay');
        
        if (result.success) {
            showToast(result.message, 'success');
            if (result.clip_path) {
                console.log('Replay saved to:', result.clip_path);
            }
            await loadClips();
        } else {
            showToast(result.message, 'error');
        }
    } catch (error) {
        console.error('Save replay failed:', error);
        showToast(`Save failed: ${error}`, 'error');
    } finally {
        btn.disabled = false;
        btn.querySelector('.btn-text').textContent = originalText;
    }
}

/**
 * Updates the replay buffer UI
 */
function updateReplayBufferUI() {
    const btn = document.getElementById('btn-replay-toggle');
    const saveBtn = document.getElementById('btn-replay-save');
    const indicator = document.getElementById('replay-indicator');
    
    if (isReplayBufferActive) {
        btn.classList.add('recording');
        btn.querySelector('.btn-icon').textContent = '⏹';
        btn.querySelector('.btn-text').textContent = 'Stop Replay Buffer';
        saveBtn.disabled = false;
        indicator.classList.add('active');
        indicator.querySelector('.status-text').textContent = 'Buffering';
    } else {
        btn.classList.remove('recording');
        btn.querySelector('.btn-icon').textContent = '⏺';
        btn.querySelector('.btn-text').textContent = 'Start Replay Buffer';
        saveBtn.disabled = true;
        indicator.classList.remove('active');
        indicator.querySelector('.status-text').textContent = 'Inactive';
    }
}

/**
 * Updates replay buffer stats display
 */
async function updateReplayStats() {
    try {
        const stats = await invoke('get_replay_stats');
        
        document.getElementById('replay-fill').style.width = `${stats.buffer_fill_percent}%`;
        document.getElementById('replay-fill-percent').textContent = `${stats.buffer_fill_percent.toFixed(1)}%`;
        document.getElementById('replay-frames').textContent = stats.frames_encoded;
        document.getElementById('replay-dropped').textContent = stats.frames_dropped;
        document.getElementById('replay-memory').textContent = `${stats.buffer_memory_mb.toFixed(1)} MB`;
        
        isReplayBufferActive = stats.is_active;
        updateReplayBufferUI();
    } catch (error) {
        console.error('Failed to get replay stats:', error);
    }
}

/**
 * Starts polling for replay stats
 */
function startReplayStatsPolling() {
    if (replayStatsInterval) return;
    replayStatsInterval = setInterval(updateReplayStats, 200); // 5Hz updates
}

/**
 * Stops polling for replay stats
 */
function stopReplayStatsPolling() {
    if (replayStatsInterval) {
        clearInterval(replayStatsInterval);
        replayStatsInterval = null;
    }
}

// ============================================
// LEGACY NATIVE RECORDING (redirects to replay buffer)
// ============================================

async function toggleNativeRecording() {
    return toggleReplayBuffer();
}

async function updateNativeStats() {
    return updateReplayStats();
}

function startNativeStatsPolling() {
    return startReplayStatsPolling();
}

function stopNativeStatsPolling() {
    return stopReplayStatsPolling();
}

// ============================================
// TOAST NOTIFICATIONS
// ============================================

/**
 * Shows a toast notification
 */
function showToast(message, type = 'info') {
    const container = document.getElementById('toast-container');
    
    const toast = document.createElement('div');
    toast.className = `toast ${type}`;
    toast.textContent = message;
    
    container.appendChild(toast);
    
    // Remove after 3 seconds
    setTimeout(() => {
        toast.style.opacity = '0';
        toast.style.transform = 'translateX(100%)';
        setTimeout(() => toast.remove(), 300);
    }, 3000);
}

// ============================================
// INITIALIZATION
// ============================================

/**
 * Initialize the application
 */
async function init() {
    console.log('Initializing Quest Shadowplay UI...');
    
    // Set up event delegation for clips grid
    setupClipsGridEventDelegation();
    
    // Initial status update
    await updateLegacyStatus();
    await updateReplayStats();
    
    // Load clips
    await loadClips();
    
    // Start status polling
    statusInterval = setInterval(updateLegacyStatus, 2000);
    
    console.log('UI initialized');
}

/**
 * Sets up event delegation for the clips grid
 * This handles all button clicks within clip cards
 */
function setupClipsGridEventDelegation() {
    const grid = document.getElementById('clips-grid');
    if (!grid) {
        console.error('Clips grid not found!');
        return;
    }
    
    grid.addEventListener('click', async (e) => {
        // Find the clip card this click belongs to
        const card = e.target.closest('.clip-card');
        if (!card) return;
        
        const clipId = card.dataset.clipId;
        if (!clipId) {
            console.error('No clip ID found on card');
            return;
        }
        
        // Check which action was clicked
        const actionBtn = e.target.closest('[data-action]');
        if (actionBtn) {
            const action = actionBtn.dataset.action;
            console.log('Action clicked:', action, 'for clip:', clipId);
            
            e.preventDefault();
            e.stopPropagation();
            
            switch (action) {
                case 'delete':
                    await handleDeleteClick(clipId);
                    break;
                case 'open':
                    await openClip(clipId);
                    break;
                case 'reveal':
                    await revealClip(clipId);
                    break;
                case 'export':
                    await exportToMp4(clipId);
                    break;
                case 'trim':
                    await openTrimModal(clipId);
                    break;
            }
            return;
        }
        
        // Click on thumbnail opens the clip
        const thumbnail = e.target.closest('.clip-thumbnail');
        if (thumbnail) {
            console.log('Thumbnail clicked for clip:', clipId);
            await openClip(clipId);
        }
    });
    
    console.log('Clips grid event delegation set up');
}

/**
 * Handles delete button click with confirmation
 */
async function handleDeleteClick(clipId) {
    console.log('Delete requested for clip:', clipId);
    
    // Use Tauri's dialog API if available, otherwise skip confirmation
    let confirmed = true;
    if (window.__TAURI__ && window.__TAURI__.dialog) {
        try {
            confirmed = await window.__TAURI__.dialog.confirm(
                'Are you sure you want to delete this clip?',
                { title: 'Delete Clip', kind: 'warning' }
            );
        } catch (e) {
            console.log('Dialog not available, proceeding with delete');
            confirmed = true;
        }
    }
    
    if (!confirmed) {
        console.log('Delete cancelled by user');
        return;
    }
    
    try {
        console.log('Invoking delete_clip command...');
        await invoke('delete_clip', { id: clipId });
        console.log('Delete successful');
        showToast('Clip deleted', 'info');
        await loadClips();
        await updateLegacyStatus();
    } catch (error) {
        console.error('Delete failed:', error);
        showToast(`Delete failed: ${error}`, 'error');
    }
}

// ============================================
// VIDEO TRIMMING
// ============================================

// Trim modal state
let trimState = {
    clipId: null,
    clipPath: null,
    duration: 0,
    startTime: 0,
    endTime: 0,
    isPlaying: false,
};

/**
 * Opens the trim modal for a clip
 */
async function openTrimModal(clipId) {
    console.log('Opening trim modal for:', clipId);
    
    const modal = document.getElementById('trim-modal');
    const video = document.getElementById('trim-video');
    
    try {
        // Get clip details
        showToast('Loading clip info...', 'info');
        const clipInfo = await invoke('get_clip_details', { id: clipId });
        console.log('Clip info:', clipInfo);
        
        // Update state
        trimState.clipId = clipId;
        trimState.clipPath = clipInfo.path;
        trimState.duration = clipInfo.duration_secs;
        trimState.startTime = 0;
        trimState.endTime = clipInfo.duration_secs;
        
        // Update info display
        document.getElementById('trim-duration').textContent = clipInfo.duration_formatted;
        document.getElementById('trim-resolution').textContent = `${clipInfo.width}×${clipInfo.height}`;
        document.getElementById('trim-size').textContent = clipInfo.file_size_formatted;
        document.getElementById('trim-total-time').textContent = formatTime(clipInfo.duration_secs);
        
        // Set up video source
        video.src = convertFileSrc(clipInfo.path);
        video.currentTime = 0;
        
        // Set up inputs
        document.getElementById('trim-start-input').value = 0;
        document.getElementById('trim-start-input').max = clipInfo.duration_secs;
        document.getElementById('trim-end-input').value = clipInfo.duration_secs.toFixed(1);
        document.getElementById('trim-end-input').max = clipInfo.duration_secs;
        
        // Load thumbnails for timeline
        loadTimelineThumbnails(clipId, 10);
        
        // Update timeline handles
        updateTimelineSelection();
        updateTimeLabels();
        
        // Show modal
        modal.style.display = 'flex';
        
        // Set up video event listeners
        video.onloadedmetadata = () => {
            console.log('Video loaded, duration:', video.duration);
        };
        
        video.ontimeupdate = () => {
            updatePlayhead();
            document.getElementById('trim-current-time').textContent = formatTime(video.currentTime);
        };
        
        video.onended = () => {
            trimState.isPlaying = false;
            document.getElementById('btn-play-pause').textContent = '▶';
        };
        
    } catch (error) {
        console.error('Failed to open trim modal:', error);
        showToast(`Failed to load clip: ${error}`, 'error');
    }
}

/**
 * Converts a file path to a Tauri asset URL
 */
function convertFileSrc(path) {
    if (window.__TAURI__ && window.__TAURI__.core.convertFileSrc) {
        return window.__TAURI__.core.convertFileSrc(path);
    }
    // Fallback for development
    return 'file://' + path;
}

/**
 * Closes the trim modal
 */
function closeTrimModal() {
    const modal = document.getElementById('trim-modal');
    const video = document.getElementById('trim-video');
    
    video.pause();
    video.src = '';
    trimState.isPlaying = false;
    
    modal.style.display = 'none';
}

/**
 * Toggles video preview playback
 */
function toggleTrimPreview() {
    const video = document.getElementById('trim-video');
    const btn = document.getElementById('btn-play-pause');
    
    if (trimState.isPlaying) {
        video.pause();
        trimState.isPlaying = false;
        btn.textContent = '▶';
    } else {
        // Start from selection start if at beginning or past selection end
        if (video.currentTime < trimState.startTime || video.currentTime >= trimState.endTime) {
            video.currentTime = trimState.startTime;
        }
        video.play();
        trimState.isPlaying = true;
        btn.textContent = '⏸';
        
        // Stop at selection end
        const checkEnd = () => {
            if (video.currentTime >= trimState.endTime) {
                video.pause();
                trimState.isPlaying = false;
                btn.textContent = '▶';
            } else if (trimState.isPlaying) {
                requestAnimationFrame(checkEnd);
            }
        };
        requestAnimationFrame(checkEnd);
    }
}

/**
 * Loads timeline thumbnails
 */
async function loadTimelineThumbnails(clipId, count) {
    const container = document.getElementById('timeline-thumbnails');
    container.innerHTML = '';
    
    try {
        const result = await invoke('get_timeline_thumbnails', { id: clipId, count: count });
        console.log('Thumbnails loaded:', result.count);
        
        for (const thumbUrl of result.thumbnails) {
            const div = document.createElement('div');
            div.className = 'timeline-thumbnail';
            if (thumbUrl) {
                div.style.backgroundImage = `url(${thumbUrl})`;
            }
            container.appendChild(div);
        }
    } catch (error) {
        console.error('Failed to load thumbnails:', error);
        // Create empty placeholders
        for (let i = 0; i < count; i++) {
            const div = document.createElement('div');
            div.className = 'timeline-thumbnail';
            container.appendChild(div);
        }
    }
}

/**
 * Updates the timeline selection overlay and handles
 */
function updateTimelineSelection() {
    const timeline = document.getElementById('trim-timeline');
    const selection = document.getElementById('timeline-selection');
    const handleStart = document.getElementById('handle-start');
    const handleEnd = document.getElementById('handle-end');
    
    const timelineWidth = timeline.offsetWidth;
    const startPercent = (trimState.startTime / trimState.duration) * 100;
    const endPercent = (trimState.endTime / trimState.duration) * 100;
    
    // Position selection
    selection.style.left = `${startPercent}%`;
    selection.style.width = `${endPercent - startPercent}%`;
    
    // Position handles
    handleStart.style.left = `${startPercent}%`;
    handleEnd.style.left = `${endPercent}%`;
    handleEnd.style.transform = 'translateX(-100%)';
}

/**
 * Updates the playhead position
 */
function updatePlayhead() {
    const video = document.getElementById('trim-video');
    const playhead = document.getElementById('timeline-playhead');
    
    const percent = (video.currentTime / trimState.duration) * 100;
    playhead.style.left = `${percent}%`;
}

/**
 * Updates the time labels
 */
function updateTimeLabels() {
    document.getElementById('label-start').textContent = formatTime(trimState.startTime);
    document.getElementById('label-end').textContent = formatTime(trimState.endTime);
    
    const selectionDuration = trimState.endTime - trimState.startTime;
    document.getElementById('label-selection').textContent = `Selected: ${formatTime(selectionDuration)}`;
}

/**
 * Formats time in M:SS.s format
 */
function formatTime(seconds) {
    const mins = Math.floor(seconds / 60);
    const secs = seconds % 60;
    return `${mins}:${secs.toFixed(1).padStart(4, '0')}`;
}

/**
 * Sets up timeline interaction
 */
function setupTimelineInteraction() {
    const timeline = document.getElementById('trim-timeline');
    const handleStart = document.getElementById('handle-start');
    const handleEnd = document.getElementById('handle-end');
    const startInput = document.getElementById('trim-start-input');
    const endInput = document.getElementById('trim-end-input');
    
    let dragging = null;
    
    // Handle dragging
    function onMouseDown(e, handle) {
        e.preventDefault();
        dragging = handle;
        document.addEventListener('mousemove', onMouseMove);
        document.addEventListener('mouseup', onMouseUp);
    }
    
    function onMouseMove(e) {
        if (!dragging) return;
        
        const rect = timeline.getBoundingClientRect();
        let percent = (e.clientX - rect.left) / rect.width;
        percent = Math.max(0, Math.min(1, percent));
        const time = percent * trimState.duration;
        
        if (dragging === 'start') {
            trimState.startTime = Math.min(time, trimState.endTime - 0.1);
            startInput.value = trimState.startTime.toFixed(1);
        } else if (dragging === 'end') {
            trimState.endTime = Math.max(time, trimState.startTime + 0.1);
            endInput.value = trimState.endTime.toFixed(1);
        }
        
        updateTimelineSelection();
        updateTimeLabels();
    }
    
    function onMouseUp() {
        dragging = null;
        document.removeEventListener('mousemove', onMouseMove);
        document.removeEventListener('mouseup', onMouseUp);
    }
    
    handleStart.addEventListener('mousedown', (e) => onMouseDown(e, 'start'));
    handleEnd.addEventListener('mousedown', (e) => onMouseDown(e, 'end'));
    
    // Click on timeline to seek
    timeline.addEventListener('click', (e) => {
        if (e.target.classList.contains('timeline-handle')) return;
        
        const rect = timeline.getBoundingClientRect();
        const percent = (e.clientX - rect.left) / rect.width;
        const time = percent * trimState.duration;
        
        const video = document.getElementById('trim-video');
        video.currentTime = time;
    });
    
    // Input changes
    startInput.addEventListener('change', () => {
        let value = parseFloat(startInput.value) || 0;
        value = Math.max(0, Math.min(value, trimState.endTime - 0.1));
        trimState.startTime = value;
        startInput.value = value.toFixed(1);
        updateTimelineSelection();
        updateTimeLabels();
    });
    
    endInput.addEventListener('change', () => {
        let value = parseFloat(endInput.value) || trimState.duration;
        value = Math.max(trimState.startTime + 0.1, Math.min(value, trimState.duration));
        trimState.endTime = value;
        endInput.value = value.toFixed(1);
        updateTimelineSelection();
        updateTimeLabels();
    });
}

/**
 * Executes the trim operation
 */
async function executeTrim() {
    const btn = document.getElementById('btn-trim');
    btn.disabled = true;
    btn.querySelector('.btn-text').textContent = 'Trimming...';
    
    try {
        console.log(`Trimming ${trimState.clipId} from ${trimState.startTime}s to ${trimState.endTime}s`);
        
        const result = await invoke('trim_clip', {
            id: trimState.clipId,
            startTime: trimState.startTime,
            endTime: trimState.endTime,
        });
        
        if (result.success) {
            showToast(`${result.message} (${result.new_clip_size})`, 'success');
            closeTrimModal();
            await loadClips();
        } else {
            showToast(`Trim failed: ${result.message}`, 'error');
        }
    } catch (error) {
        console.error('Trim failed:', error);
        showToast(`Trim failed: ${error}`, 'error');
    } finally {
        btn.disabled = false;
        btn.querySelector('.btn-text').textContent = 'Trim Clip';
    }
}

// Set up timeline interaction when DOM is ready
document.addEventListener('DOMContentLoaded', () => {
    setupTimelineInteraction();
});

// Start when DOM is ready
if (document.readyState === 'loading') {
    document.addEventListener('DOMContentLoaded', init);
} else {
    init();
}
