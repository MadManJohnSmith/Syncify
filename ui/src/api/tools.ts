import { invoke } from '@tauri-apps/api/core'

export interface BridgeResult {
    success: boolean
    data?: any
    error?: string
}

export const toolsApi = {
    checkFfmpeg: async (): Promise<BridgeResult> => {
        return await invoke('check_ffmpeg_available')
    },

    checkFingerprint: async (): Promise<BridgeResult> => {
        return await invoke('check_fingerprint_available')
    }
}
