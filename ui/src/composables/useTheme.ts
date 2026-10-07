import { ref } from "vue";

export type ThemeId = "camaleon" | "tinta" | "consola" | "riso";

export interface ThemeMeta {
  id: ThemeId;
  name: string;
  description: string;
}

export const THEMES: ThemeMeta[] = [
  { id: "camaleon", name: "Camaleón", description: "La interfaz se tiñe con el color del disco en reproducción." },
  { id: "tinta", name: "Tinta", description: "Monocromo: el único color llega de la carátula." },
  { id: "consola", name: "Consola", description: "Ámbar sobre carbón neutro." },
  { id: "riso", name: "Riso", description: "Papel frío con dos tintas: azul y rosa." },
];

const STORAGE_KEY = "syncify.theme";
const VALID: ThemeId[] = ["camaleon", "tinta", "consola", "riso"];

function readInitial(): ThemeId {
  const fromDom = document.documentElement.dataset.theme as ThemeId | undefined;
  if (fromDom && VALID.includes(fromDom)) return fromDom;
  return "camaleon";
}

const current = ref<ThemeId>(readInitial());

export function useTheme() {
  function setTheme(id: ThemeId) {
    current.value = id;
    document.documentElement.dataset.theme = id;
    try {
      localStorage.setItem(STORAGE_KEY, id);
    } catch {
      /* almacenamiento no disponible: el tema vive solo en esta sesión */
    }
  }
  return { themes: THEMES, current, setTheme };
}
