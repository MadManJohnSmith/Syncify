/**
 * S194 residual — local playback composable.
 *
 * Singleton HTML5 Audio driven through the `syncify-media://` protocol:
 * the backend command `resolve_playback_source` verifies the track has a
 * downloaded file, grants exactly that file (in-memory allowlist), and the
 * frontend converts the path with convertFileSrc. Provider streaming is out

 */
import { ref } from 'vue'
import { convertFileSrc } from '@tauri-apps/api/core'
import { invokeCommand } from '../api/tauri'

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
    });
    audio.addEventListener('error', () => {
        if (!audio.src) return; // ignore teardown noise
        error.value = 'No se pudo reproducir el archivo de audio';
        isPlaying.value = false;
    });
}

export function usePlayer() {
    async function play(track: PlayerTrack): Promise<void> {
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
        current, isPlaying, positionSec, durationSec, playbackRate, error, isLoadingSource,
        play, toggle, stop, seek, setRate,
    };
}
