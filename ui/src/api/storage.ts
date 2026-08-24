import { invoke } from "@tauri-apps/api/core";
import { asArray, asNumber, asRecord, asString } from "./normalize";

export interface StorageFormatCount {
    format: string;
    size_bytes: number;
}

export interface StorageStats {
    used_bytes: number;
    total_bytes: number;
    available_bytes: number;
    path: string;
    breakdown: StorageFormatCount[];
}

/**
 * Get storage statistics with defensive defaults so a partial backend payload
 * can never crash dashboard rendering.
 */
export async function getStorageStats(): Promise<StorageStats> {
    const raw = await invoke<unknown>("get_storage_stats");
    const rec = asRecord(raw);
    return {
        used_bytes: asNumber(rec?.used_bytes),
        total_bytes: asNumber(rec?.total_bytes),
        available_bytes: asNumber(rec?.available_bytes),
        path: asString(rec?.path),
        breakdown: asArray<StorageFormatCount>(rec?.breakdown)
            .filter((item) => asRecord(item) !== null)
            .map((item) => ({
                format: asString(asRecord(item)?.format),
                size_bytes: asNumber(asRecord(item)?.size_bytes),
            })),
    };
}
