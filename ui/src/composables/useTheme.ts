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
  { id: "tinta", name: "Tinta", description: "Monocromo: el único color llega de la carátula." },
  { id: "camaleon", name: "Camaleón", description: "La interfaz se tiñe con el color del disco en reproducción." },
  { id: "consola", name: "Consola", description: "Ámbar sobre carbón neutro." },
  { id: "riso", name: "Riso", description: "Papel frío con dos tintas: azul y rosa." },
];

export const SERVICE_THEMES: ThemeMeta[] = [
  { id: "spotify", name: "Spotify", description: "Verde vibrante sobre fondo negro azabache.", isService: true },
  { id: "apple_music", name: "Apple Music", description: "Rojo carmesí sobre gris medianoche pulido.", isService: true },
  { id: "tidal", name: "Tidal", description: "Cian de alta resolución sobre negro absoluto.", isService: true },
  { id: "qobuz", name: "Qobuz", description: "Azul profundo audiófilo sobre zafiro oscuro.", isService: true },
  { id: "deezer", name: "Deezer", description: "Púrpura neón dinámico sobre antracita.", isService: true },
  { id: "soundcloud", name: "SoundCloud", description: "Naranja cálido sobre carbono mate.", isService: true },
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
    /* localStorage no accesible */
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
    /* localStorage no accesible */
  }
  return false;
}

const current = ref<ThemeId>(readInitial());
const dynamicCoverEnabled = ref<boolean>(readInitialDynamicCover());

// Sincronización inicial con DOM si estamos en entorno navegador
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
    console.debug("[useTheme] No se pudieron sincronizar ajustes desde el backend:", err);
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
      /* almacenamiento no disponible: el tema vive solo en esta sesión */
    }
    void invokeCommand("save_setting", { key: STORAGE_KEY, value: id }).catch((err) => {
      console.debug("[useTheme] Error guardando tema en backend:", err);
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
      /* almacenamiento no disponible */
    }
    void invokeCommand("save_setting", { key: DYNAMIC_COVER_KEY, value: String(enabled) }).catch((err) => {
      console.debug("[useTheme] Error guardando ajuste dynamic cover en backend:", err);
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
