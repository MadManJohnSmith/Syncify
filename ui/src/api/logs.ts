/**
 * System Logs API (S170)
 *
 * Interacts with Tauri native logging ring buffer and IPC commands.
 * All responses are defensively normalized so missing fields can never
 * crash the LogsView rendering pipeline.
 */

import { invokeCommand } from './tauri';
import { asArray, asString, asBoolean, asNumber, asRecord } from './normalize';

export type SystemLogLevel = 'info' | 'warn' | 'error' | 'debug' | 'trace' | 'success';

export interface SystemLogEntry {
    id: string;
    timestamp: string;
    level: SystemLogLevel;
    target: string;
    module: string;
    message: string;
    fields?: Record<string, unknown>;
}

export interface GetSystemLogsParams {
    limit?: number;
    level_filter?: string;
    module_filter?: string;
    search?: string;
}

const VALID_LOG_LEVELS: readonly SystemLogLevel[] = ['info', 'warn', 'error', 'debug', 'trace', 'success'];

function normalizeLogLevel(rawLevel: unknown): SystemLogLevel {
    const level = typeof rawLevel === 'string' ? rawLevel.toLowerCase() : '';
    // Accept common aliases ('warning' → 'warn') and fall back to 'info'.
    if ((VALID_LOG_LEVELS as readonly string[]).includes(level)) {
        return level as SystemLogLevel;
    }
    if (level === 'warning') return 'warn';
    return 'info';
}

function normalizeLogEntry(rawEntry: unknown, index: number): SystemLogEntry {
    const entry = asRecord(rawEntry);
    return {
        id: asString(entry?.id) || `log-${index}`,
        timestamp: asString(entry?.timestamp),
        level: normalizeLogLevel(entry?.level),
        target: asString(entry?.target),
        module: asString(entry?.module),
        message: asString(entry?.message),
    };
}

/**
 * Fetch buffered system logs from native Rust backend
 */
export async function getSystemLogs(params?: GetSystemLogsParams): Promise<SystemLogEntry[]> {
    const raw = await invokeCommand<unknown>('get_system_logs', {
        limit: params?.limit,
        levelFilter: params?.level_filter,
        moduleFilter: params?.module_filter,
        search: params?.search,
    });
    return asArray<unknown>(raw).map((entry, index) => normalizeLogEntry(entry, index));
}

/**
 * Clear the native backend log buffer
 */
export async function clearSystemLogs(): Promise<void> {
    return invokeCommand<void>('clear_system_logs');
}

/**
 * Export all buffered system logs as sanitized plain text
 */
export async function exportSystemLogs(): Promise<string> {
    const exported = await invokeCommand<unknown>('export_system_logs');
    return typeof exported === 'string' ? exported : '';
}

/**
 * Record a custom log entry in the backend buffer
 */
export async function recordSystemLog(entry: {
    level: string;
    target?: string;
    module?: string;
    message: string;
}): Promise<SystemLogEntry> {
    const raw = await invokeCommand<unknown>('record_system_log', {
        level: entry.level,
        target: entry.target,
        module: entry.module,
        message: entry.message,
    });
    return normalizeLogEntry(raw, 0);
}

export interface LoggingStatus {
    is_development: boolean;
    file_logging_active: boolean;
    active_log_file_path?: string | null;
    log_dir: string;
    log_level: string;
    buffer_count: number;
    retention_days: number;
    max_file_size_mb: number;
}

/**
 * Fetch current system logging configuration & status from native Rust backend
 */
export async function getLoggingStatus(): Promise<LoggingStatus> {
    const raw = await invokeCommand<unknown>('get_logging_status');
    return {
        is_development: asBoolean((raw as Record<string, unknown> | null)?.is_development),
        file_logging_active: asBoolean((raw as Record<string, unknown> | null)?.file_logging_active),
        active_log_file_path:
            ((raw as Record<string, unknown> | null)?.active_log_file_path as string | null | undefined) ?? null,
        log_dir: asString((raw as Record<string, unknown> | null)?.log_dir),
        log_level: asString((raw as Record<string, unknown> | null)?.log_level, 'info'),
        buffer_count: asNumber((raw as Record<string, unknown> | null)?.buffer_count),
        retention_days: asNumber((raw as Record<string, unknown> | null)?.retention_days),
        max_file_size_mb: asNumber((raw as Record<string, unknown> | null)?.max_file_size_mb),
    };
}

export const logsApi = {
    getSystemLogs,
    clearSystemLogs,
    exportSystemLogs,
    recordSystemLog,
    getLoggingStatus,
};
