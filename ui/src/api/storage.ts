import { invoke } from "@tauri-apps/api/core";

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

export const getStorageStats = () => invoke<StorageStats>("get_storage_stats");
