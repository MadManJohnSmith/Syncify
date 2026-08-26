import { invoke } from '@tauri-apps/api/core'

export interface BridgeResult {
    success: boolean
    data?: unknown
    error?: string
}

/**
 * Normalize a raw bridge payload so `success` is always a real boolean and
 * `error` a real string, regardless of what the Python bridge returned.
 */
function normalizeBridgeResult(raw: unknown): BridgeResult {
    const rec = raw !== null && typeof raw === 'object' ? (raw as Record<string, unknown>) : {}
    return {
        success: rec.success === true,
        data: rec.data ?? undefined,
        error: typeof rec.error === 'string' ? rec.error : undefined,
    }
}

export const toolsApi = {
    checkFfmpeg: async (): Promise<BridgeResult> => {
        return normalizeBridgeResult(await invoke<unknown>('check_ffmpeg_available'))
    },

    checkFingerprint: async (): Promise<BridgeResult> => {
        return normalizeBridgeResult(await invoke<unknown>('check_fingerprint_available'))
    },

    /**
     * Persist UTF-8 text to a user-resolved path (dialog plugin on the caller).
     * Returns the number of bytes written by the backend command.
     */
    writeTextFile: async (path: string, contents: string): Promise<number> => {
        const written = await invoke<unknown>('write_text_file', { path, contents })
        return typeof written === 'number' ? written : 0
    }
}
