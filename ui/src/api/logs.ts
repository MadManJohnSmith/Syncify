/**
 * System Logs API (S170)
 * 
 * Interacts with Tauri native logging ring buffer and IPC commands.
 */

import { invokeCommand } from './tauri';

export interface SystemLogEntry {
    id: string;
    timestamp: string;
    level: 'info' | 'warn' | 'error' | 'debug' | 'trace' | 'success';
    target: string;
    module: string;
    message: string;
    fields?: Record<string, any>;
}

export interface GetSystemLogsParams {
    limit?: number;
    level_filter?: string;
    module_filter?: string;
    search?: string;
}

/**
 * Fetch buffered system logs from native Rust backend
 */
export async function getSystemLogs(params?: GetSystemLogsParams): Promise<SystemLogEntry[]> {
    return invokeCommand<SystemLogEntry[]>('get_system_logs', {
        limit: params?.limit,
        levelFilter: params?.level_filter,
        moduleFilter: params?.module_filter,
        search: params?.search,
    });
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
    return invokeCommand<string>('export_system_logs');
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
    return invokeCommand<SystemLogEntry>('record_system_log', {
        level: entry.level,
        target: entry.target,
        module: entry.module,
        message: entry.message,
    });
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
    return invokeCommand<LoggingStatus>('get_logging_status');
}

export const logsApi = {
    getSystemLogs,
    clearSystemLogs,
    exportSystemLogs,
    recordSystemLog,
    getLoggingStatus,
};
