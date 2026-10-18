import { ref } from "vue";
import { invokeCommand } from "@/api/tauri";

export type StandardThemeId = "tinta" | "camaleon" | "consola" | "riso";
export type ServiceThemeId = "spotify" | "apple_music" | "tidal" | "qobuz" | "deezer" | "soundcloud";
export type ThemeId = StandardThemeId | ServiceThemeId;

export interface ThemeMeta {
  id: ThemeId;
  name: string;
  description: string;
  isService?: boolean;
}

export const STANDARD_THEMES: ThemeMeta[] = [
  { id: "tinta", name: "Ink", description: "Monochrome: the only color comes from the cover art." },
  { id: "camaleon", name: "Chameleon", description: "The interface takes on the color of the playing album." },
  { id: "consola", name: "Console", description: "Amber on neutral charcoal." },
  { id: "riso", name: "Riso", description: "Cool paper with two inks: blue and pink." },
];

export const SERVICE_THEMES: ThemeMeta[] = [
  { id: "spotify", name: "Spotify", description: "Vibrant green on jet black.", isService: true },
  { id: "apple_music", name: "Apple Music", description: "Crimson red on polished midnight gray.", isService: true },
  { id: "tidal", name: "Tidal", description: "High-resolution cyan on pure black.", isService: true },
  { id: "qobuz", name: "Qobuz", description: "Deep audiophile blue on dark sapphire.", isService: true },
  { id: "deezer", name: "Deezer", description: "Dynamic neon purple on charcoal.", isService: true },
  { id: "soundcloud", name: "SoundCloud", description: "Warm orange on matte carbon.", isService: true },
];

export const THEMES: ThemeMeta[] = [...STANDARD_THEMES, ...SERVICE_THEMES];

export const STORAGE_KEY = "syncify.theme";
export const DYNAMIC_COVER_KEY = "syncify.dynamic_cover";

export const VALID_STANDARD: StandardThemeId[] = ["tinta", "camaleon", "consola", "riso"];
export const VALID_SERVICE: ServiceThemeId[] = ["spotify", "apple_music", "tidal", "qobuz", "deezer", "soundcloud"];
export const VALID: ThemeId[] = [...VALID_STANDARD, ...VALID_SERVICE];

export function isServiceTheme(id: ThemeId): boolean {
  return VALID_SERVICE.includes(id as ServiceThemeId);
}

function readInitial(): ThemeId {
  if (typeof document !== "undefined" && document.documentElement) {
    const fromDom = document.documentElement.dataset.theme as ThemeId | undefined;
    if (fromDom && VALID.includes(fromDom)) return fromDom;
  }
  try {
    if (typeof localStorage !== "undefined") {
      const fromStorage = localStorage.getItem(STORAGE_KEY) as ThemeId | null;
      if (fromStorage && VALID.includes(fromStorage)) return fromStorage;
    }
  } catch {
    /* localStorage unavailable */
  }
  return "tinta";
}

function readInitialDynamicCover(): boolean {
  if (typeof document !== "undefined" && document.documentElement) {
    const fromDom = document.documentElement.dataset.dynamicCover;
    if (fromDom === "true") return true;
    if (fromDom === "false") return false;
  }
  try {
    if (typeof localStorage !== "undefined") {
      const fromStorage = localStorage.getItem(DYNAMIC_COVER_KEY);
      if (fromStorage !== null) return fromStorage === "true";
    }
  } catch {
    /* localStorage unavailable */
  }
  return false;
}

const current = ref<ThemeId>(readInitial());
const dynamicCoverEnabled = ref<boolean>(readInitialDynamicCover());

// Initial DOM sync when running in a browser environment
if (typeof document !== "undefined" && document.documentElement) {
  document.documentElement.dataset.theme = current.value;
  document.documentElement.dataset.dynamicCover = String(dynamicCoverEnabled.value);
}

async function syncFromBackend() {
  try {
    const res = await invokeCommand<Record<string, string>>("get_kv_settings", {
      keys: [STORAGE_KEY, DYNAMIC_COVER_KEY],
    });
    if (res && typeof res === "object") {
      const backendTheme = res[STORAGE_KEY];
      if (backendTheme && VALID.includes(backendTheme as ThemeId)) {
        if (backendTheme !== current.value) {
          current.value = backendTheme as ThemeId;
          if (typeof document !== "undefined" && document.documentElement) {
            document.documentElement.dataset.theme = backendTheme;
          }
          try {
            if (typeof localStorage !== "undefined") {
              localStorage.setItem(STORAGE_KEY, backendTheme);
            }
          } catch {}
        }
      }
      const backendDyn = res[DYNAMIC_COVER_KEY];
      if (backendDyn !== undefined) {
        const isDyn = backendDyn === "true";
        dynamicCoverEnabled.value = isDyn;
        if (typeof document !== "undefined" && document.documentElement) {
          document.documentElement.dataset.dynamicCover = String(isDyn);
        }
        try {
          if (typeof localStorage !== "undefined") {
            localStorage.setItem(DYNAMIC_COVER_KEY, String(isDyn));
          }
        } catch {}
      }
    }
  } catch (err) {
    console.debug("[useTheme] Could not sync settings from the backend:", err);
  }
}

if (typeof window !== "undefined") {
  void syncFromBackend();
}

export function useTheme() {
  if (typeof document !== "undefined" && document.documentElement) {
    if (!document.documentElement.dataset.theme) {
      document.documentElement.dataset.theme = current.value;
    }
    if (!document.documentElement.dataset.dynamicCover) {
      document.documentElement.dataset.dynamicCover = String(dynamicCoverEnabled.value);
    }
  }

  function setTheme(id: ThemeId) {
    current.value = id;
    if (typeof document !== "undefined" && document.documentElement) {
      document.documentElement.dataset.theme = id;
    }
    try {
      if (typeof localStorage !== "undefined") {
        localStorage.setItem(STORAGE_KEY, id);
      }
    } catch {
      /* storage unavailable: the theme only lives in this session */
    }
    void invokeCommand("save_setting", { key: STORAGE_KEY, value: id }).catch((err) => {
      console.debug("[useTheme] Error saving theme to the backend:", err);
    });
  }

  function setDynamicCover(enabled: boolean) {
    dynamicCoverEnabled.value = enabled;
    if (typeof document !== "undefined" && document.documentElement) {
      document.documentElement.dataset.dynamicCover = String(enabled);
    }
    try {
      if (typeof localStorage !== "undefined") {
        localStorage.setItem(DYNAMIC_COVER_KEY, String(enabled));
      }
    } catch {
      /* storage unavailable */
    }
    void invokeCommand("save_setting", { key: DYNAMIC_COVER_KEY, value: String(enabled) }).catch((err) => {
      console.debug("[useTheme] Error saving dynamic cover setting to the backend:", err);
    });
  }

  return {
    themes: THEMES,
    standardThemes: STANDARD_THEMES,
    serviceThemes: SERVICE_THEMES,
    current,
    setTheme,
    dynamicCoverEnabled,
    setDynamicCover,
    isServiceTheme,
    syncFromBackend,
  };
}
