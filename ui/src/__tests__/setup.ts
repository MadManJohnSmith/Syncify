/**
 * Vitest global setup file
 * Mocks Tauri APIs for component testing
 */
import { vi } from 'vitest';

// Type definitions for mock utilities
export interface MockInvokeHandler {
    (command: string, args?: Record<string, unknown>): unknown;
}

export interface MockListenHandler {
    (event: string, handler: (event: { payload: unknown }) => void): () => void;
}

// Default mock implementations
let invokeHandler: MockInvokeHandler = () => Promise.resolve([]);
let listeners: Map<string, Set<(event: { payload: unknown }) => void>> = new Map();

/**
 * Configure mock return values for invoke calls
 */
export function mockInvoke(handler: MockInvokeHandler): void {
    invokeHandler = handler;
}

/**
 * Emit a mock event to all registered listeners
 */
export function emitMockEvent(eventName: string, payload: unknown): void {
    const eventListeners = listeners.get(eventName);
    if (eventListeners) {
        eventListeners.forEach((handler) => {
            handler({ payload });
        });
    }
}

/**
 * Reset all mocks between tests
 */
export function resetMocks(): void {
    invokeHandler = () => Promise.resolve([]);
    listeners.clear();
}

// Mock @tauri-apps/api/core
vi.mock('@tauri-apps/api/core', () => ({
    invoke: vi.fn((command: string, args?: Record<string, unknown>) => {
        return Promise.resolve(invokeHandler(command, args));
    }),
}));

// Mock @tauri-apps/api/event
vi.mock('@tauri-apps/api/event', () => ({
    listen: vi.fn((eventName: string, handler: (event: { payload: unknown }) => void) => {
        if (!listeners.has(eventName)) {
            listeners.set(eventName, new Set());
        }
        listeners.get(eventName)!.add(handler);

        // Return unlisten function
        return Promise.resolve(() => {
            listeners.get(eventName)?.delete(handler);
        });
    }),
}));
