// Barrel split from api/types.ts; tipos espejo de los comandos Rust.
// ==============================================
// BRIDGE RESULT (Generic)
// ==============================================

export interface BridgeResult {
    success: boolean;
    data: Record<string, unknown> | null;
    error: string | null;
}

// ==============================================
// HEALTH CHECK TYPES
// ==============================================

export interface HealthCheck {
    database_ok: boolean;
    python_ok: boolean;
    ffmpeg_available: boolean;
    chromaprint_available: boolean;
    services_configured: string[];
    errors: string[];
}

// ==============================================
// PROGRESS EVENT TYPES
// ==============================================

export interface ProgressEvent {
    operation: string;
    id: string;
    status: 'started' | 'progress' | 'completed' | 'failed';
    current: number;
    total: number;
    percentage: number;
    message: string | null;
    data: Record<string, unknown> | null;
}

// ==============================================
// DEPENDENCY TYPES
// ==============================================

export interface DependencyStatus {
    name: string;
    available: boolean;
    version: string | null;
    required: boolean;
}

export interface DependencyCheckResult {
    all_available: boolean;
    dependencies: DependencyStatus[];
}
