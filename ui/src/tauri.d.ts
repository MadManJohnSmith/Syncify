/**
 * Tauri v2 Global Type Declarations
 */
declare global {
    interface Window {
        __TAURI__?: Record<string, unknown>;
        __TAURI_INTERNALS__?: Record<string, unknown>;
        isTauri?: boolean;
    }
}

export { };
