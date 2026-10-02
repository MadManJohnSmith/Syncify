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
    },

    /**
     * Persist a user-submitted report (bug report / feedback / help article
     * rating) as a JSON file in the app log directory (FE-8). Returns the
     * written file path so the UI can confirm the save to the user.
     */
    saveUserReport: async (report: UserReportInput): Promise<UserReportSaved> => {
        const raw = await invoke<unknown>('save_user_report', { report })
        const rec = raw !== null && typeof raw === 'object' ? (raw as Record<string, unknown>) : {}
        return {
            filePath: typeof rec.file_path === 'string' ? rec.file_path : '',
            createdAt: typeof rec.created_at === 'string' ? rec.created_at : '',
        }
    }
}

export interface UserReportInput {
    /** 'bug_report' | 'feedback' | 'article_feedback' */
    kind: string
    /** Main free text (bug description / feedback message) */
    message: string
    /** Bug reports only: reproduction steps */
    steps_to_reproduce?: string
    /** Feedback only: 'Bug Report' | 'Feature Request' | 'General Feedback' */
    feedback_type?: string
    /** Article feedback only: title of the rated article */
    article_title?: string
    /** Article feedback only: thumbs up / thumbs down */
    helpful?: boolean | null
    /** Bug reports only: attach the sanitized system log dump */
    attach_logs?: boolean
}

export interface UserReportSaved {
    filePath: string
    createdAt: string
}
