/**
 * Quest Shadowplay - Frontend Application
 * 
 * Connects the UI to the Tauri Rust backend.
 */

// ============================================
// STATE
// ============================================

let isRecording = false;
let statusInterval = null;

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
                is_recording: isRecording,
                buffer_fill_percent: isRecording ? Math.random() * 100 : 0,
                frame_count: isRecording ? Math.floor(Math.random() * 900) : 0,
                buffer_capacity: 900,
                clips_count: 0
            };
        case 'start_recording':
            isRecording = true;
            return true;
        case 'stop_recording':
            isRecording = false;
            return true;
        case 'save_clip':
            return { success: true, message: 'Mock saved!', clip_id: 'mock_clip.qsp' };
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
 * Updates the status display
 */
async function updateStatus() {
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
        isRecording = status.is_recording;
        updateRecordingUI();
        
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
 * Updates the UI to reflect recording state
 */
function updateRecordingUI() {
    const badge = document.getElementById('status-badge');
    const btn = document.getElementById('btn-record');
    
    if (isRecording) {
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
            
            // Load thumbnail
            loadThumbnail(clip.id, card);
        }
        
    } catch (error) {
        console.error('Failed to load clips:', error);
    }
}

/**
 * Creates a clip card element
 */
function createClipCard(clip) {
    const card = document.createElement('div');
    card.className = 'clip-card';
    card.dataset.clipId = clip.id;
    
    const sizeKB = (clip.size_bytes / 1024).toFixed(1);
    const sizeMB = (clip.size_bytes / (1024 * 1024)).toFixed(1);
    const sizeDisplay = clip.size_bytes > 1024 * 1024 ? `${sizeMB} MB` : `${sizeKB} KB`;
    
    card.innerHTML = `
        <div class="clip-thumbnail" data-clip-id="${clip.id}">
            📹
        </div>
        <div class="clip-info">
            <div class="clip-time">${clip.timestamp || 'Unknown time'}</div>
            <div class="clip-size">${sizeDisplay}</div>
        </div>
        <div class="clip-actions">
            <button class="clip-action-btn export" onclick="exportToMp4('${clip.id}', event)">
                🎬 MP4
            </button>
            <button class="clip-action-btn delete" onclick="deleteClip('${clip.id}', event)">
                🗑️ Delete
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
// ACTIONS
// ============================================

/**
 * Toggles recording on/off
 */
async function toggleRecording() {
    const btn = document.getElementById('btn-record');
    btn.disabled = true;
    
    try {
        if (isRecording) {
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
        
        await updateStatus();
        
    } catch (error) {
        console.error('Toggle recording failed:', error);
        showToast(`Error: ${error}`, 'error');
    } finally {
        btn.disabled = false;
    }
}

/**
 * Saves the current buffer as a clip
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
 * Deletes a clip
 */
async function deleteClip(clipId, event) {
    event.stopPropagation();
    
    if (!confirm('Delete this clip?')) {
        return;
    }
    
    try {
        await invoke('delete_clip', { id: clipId });
        showToast('Clip deleted', 'info');
        await loadClips();
        await updateStatus();
    } catch (error) {
        console.error('Delete failed:', error);
        showToast(`Delete failed: ${error}`, 'error');
    }
}

/**
 * Exports a clip to MP4
 */
async function exportToMp4(clipId, event) {
    event.stopPropagation();
    
    const btn = event.target;
    const originalText = btn.textContent;
    btn.disabled = true;
    btn.textContent = '⏳ Exporting...';
    
    try {
        showToast('Exporting to MP4...', 'info');
        const result = await invoke('export_to_mp4', { id: clipId });
        
        if (result.success) {
            showToast(`Exported! ${result.message}`, 'success');
            if (result.mp4_path) {
                console.log('MP4 saved to:', result.mp4_path);
                // Try to open the folder containing the MP4
                try {
                    await invoke('open_folder', { path: result.mp4_path });
                } catch (e) {
                    // Ignore if open_folder not available
                }
            }
        } else {
            showToast(`Export failed: ${result.message}`, 'error');
        }
    } catch (error) {
        console.error('Export failed:', error);
        showToast(`Export failed: ${error}`, 'error');
    } finally {
        btn.disabled = false;
        btn.textContent = originalText;
    }
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
    
    // Initial status update
    await updateStatus();
    
    // Load clips
    await loadClips();
    
    // Start status polling (every 500ms when recording, 2s otherwise)
    statusInterval = setInterval(async () => {
        await updateStatus();
    }, isRecording ? 500 : 2000);
    
    console.log('UI initialized');
}

// Start when DOM is ready
if (document.readyState === 'loading') {
    document.addEventListener('DOMContentLoaded', init);
} else {
    init();
}

