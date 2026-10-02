/**
 * S194 residual — local playback composable.
 *
 * Singleton HTML5 Audio driven through the `syncify-media://` protocol:
 * the backend command `resolve_playback_source` verifies the track has a
 * downloaded file, grants exactly that file (in-memory allowlist), and the
 * frontend converts the path with convertFileSrc. Provider streaming is out
 * of scope for this sprint (S194 verdict).
 */
import { ref } from 'vue'
import { convertFileSrc } from '@tauri-apps/api/core'
import { invokeCommand } from '../api/tauri'

/**
 * Outcome of queueing a track: 'started' means playback began right away
 * (nothing was loaded), 'queued' means it waits in the up-next queue.
 */
export type QueueOutcome = 'queued' | 'started'

export interface PlayerTrack {
    id: number;
    title: string;
    artist: string;
    album?: string | null;
    coverUrl?: string | null;
}

interface PlaybackSource {
    track_id: number;
    file_path: string;
    format?: string | null;
}

const audio = new Audio();

const current = ref<PlayerTrack | null>(null);
const isPlaying = ref(false);
const positionSec = ref(0);
const durationSec = ref(0);
const playbackRate = ref(1);
const error = ref<string | null>(null);
const isLoadingSource = ref(false);

// FE-9: up-next playback queue. "Play Next" inserts at the front, "add to
// queue" appends; when the current track ends the queue auto-advances.
const upNext = ref<PlayerTrack[]>([]);

let bound = false;
function bindAudioEvents(): void {
    if (bound) return;
    bound = true;
    audio.addEventListener('timeupdate', () => { positionSec.value = audio.currentTime; });
    audio.addEventListener('durationchange', () => {
        durationSec.value = Number.isFinite(audio.duration) ? audio.duration : 0;
    });
    audio.addEventListener('play', () => { isPlaying.value = true; });
    audio.addEventListener('pause', () => { isPlaying.value = false; });
    audio.addEventListener('ended', () => {
        isPlaying.value = false;
        positionSec.value = 0;
        const next = upNext.value.shift();
        if (next) {
            // A failing next track surfaces through `error`; the chain stops there.
            void playTrack(next).catch(() => { /* error ref carries the message */ });
        }
    });
    audio.addEventListener('error', () => {
        if (!audio.src) return; // ignore teardown noise
        error.value = 'No se pudo reproducir el archivo de audio';
        isPlaying.value = false;
    });
}

async function playTrack(track: PlayerTrack): Promise<void> {
    bindAudioEvents();
    error.value = null;
    isLoadingSource.value = true;
    try {
        const src = await invokeCommand<PlaybackSource>('resolve_playback_source', { trackId: track.id });
        // Re-selecting the same file keeps its playback position.
        if (!audio.src.includes(encodeURIComponent(src.file_path))) {
            audio.src = convertFileSrc(src.file_path, 'syncify-media');
            positionSec.value = 0;
        }
        await audio.play();
        current.value = track;
    } catch (err) {
        const msg = err instanceof Error ? err.message : String(err);
        error.value = msg;
        throw err instanceof Error ? err : new Error(msg);
    } finally {
        isLoadingSource.value = false;
    }
}

/**
 * FE-9: queue a track to start right after the current one. With nothing
 * loaded it starts immediately so the action always has an audible effect.
 */
async function playNext(track: PlayerTrack): Promise<QueueOutcome> {
    if (!audio.src) {
        await playTrack(track);
        return 'started';
    }
    upNext.value = [track, ...upNext.value];
    return 'queued';
}

export function usePlayer() {

    function toggle(): void {
        if (!audio.src) return;
        if (audio.paused) void audio.play().catch(() => { isPlaying.value = false; });
        else audio.pause();
    }

    function stop(): void {
        audio.pause();
        audio.removeAttribute('src');
        audio.load();
        current.value = null;
        positionSec.value = 0;
        durationSec.value = 0;
        error.value = null;
        upNext.value = [];
    }

    function seek(sec: number): void {
        if (!Number.isFinite(sec)) return;
        audio.currentTime = Math.max(0, sec);
        positionSec.value = audio.currentTime;
    }

    function setRate(rate: number): void {
        if (Number.isFinite(rate) && rate > 0) {
            audio.playbackRate = rate;
            playbackRate.value = rate;
        }
    }

    return {
        current, isPlaying, positionSec, durationSec, playbackRate, error, isLoadingSource, upNext,
        play: playTrack, playNext, toggle, stop, seek, setRate,
    };
}
