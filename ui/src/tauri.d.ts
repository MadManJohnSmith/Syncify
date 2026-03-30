/**
 * Tauri Global Type Declarations
 * 
 * Declares the window.__TAURI__ global object that Tauri injects at runtime.
 */

interface TauriInvoke {
    <T>(cmd: string, args?: Record<string, unknown>): Promise<T>;
}

interface TauriEvent {
    listen<T>(event: string, handler: (event: { payload: T }) => void): Promise<() => void>;
    emit(event: string, payload?: unknown): Promise<void>;
}

interface TauriNotification {
    sendNotification(options: { title: string; body: string; icon?: string }): Promise<void>;
    requestPermission(): Promise<'granted' | 'denied' | 'default'>;
    isPermissionGranted(): Promise<boolean>;
}

interface TauriDialog {
    save(options?: { defaultPath?: string; filters?: Array<{ name: string; extensions: string[] }> }): Promise<string | null>;
    open(options?: { multiple?: boolean; directory?: boolean; filters?: Array<{ name: string; extensions: string[] }> }): Promise<string | string[] | null>;
}

interface TauriWindow {
    appWindow: {
        close(): Promise<void>;
        minimize(): Promise<void>;
        maximize(): Promise<void>;
        unmaximize(): Promise<void>;
        toggleMaximize(): Promise<void>;
        setFullscreen(fullscreen: boolean): Promise<void>;
        startDragging(): Promise<void>;
        isMaximized(): Promise<boolean>;
    };
}

interface TauriAPI {
    invoke: TauriInvoke;
    event: TauriEvent;
    notification: TauriNotification;
    dialog: TauriDialog;
    window: TauriWindow;
}

declare global {
    interface Window {
        __TAURI__?: TauriAPI;
    }
}

export { };
