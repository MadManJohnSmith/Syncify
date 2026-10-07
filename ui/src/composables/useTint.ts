// Tinte Camaleón: extrae el matiz dominante de la carátula y lo publica en
// --tint-h (matiz) y --tint-k (intensidad) sobre <html>.
import { ref } from "vue";

const hueCache = new Map<string, number>();
const DEFAULT_HUE = 340;

export const tintIntensity = ref(1);

function loadIntensity() {
  try {
    const v = localStorage.getItem("syncify.tint");
    if (v !== null) tintIntensity.value = Math.min(1, Math.max(0, Number(v) || 0));
    // Aplica la intensidad persistida al DOM: sin esto, tras reiniciar la app
    // los tokens usarían --tint-k: 1 hasta que una carátula repusiera el tinte.
    document.documentElement.style.setProperty("--tint-k", String(tintIntensity.value));
  } catch {
    /* sin almacenamiento: intensidad por defecto */
  }
}
loadIntensity();

export function setTintIntensity(k: number) {
  tintIntensity.value = Math.min(1, Math.max(0, k));
  document.documentElement.style.setProperty("--tint-k", String(tintIntensity.value));
  try {
    localStorage.setItem("syncify.tint", String(tintIntensity.value));
  } catch {
    /* noop */
  }
}

function hueFromRgb(r: number, g: number, b: number): number {
  const max = Math.max(r, g, b);
  const min = Math.min(r, g, b);
  const d = max - min;
  if (d === 0) return DEFAULT_HUE;
  let h: number;
  if (max === r) h = ((g - b) / d) % 6;
  else if (max === g) h = (b - r) / d + 2;
  else h = (r - g) / d + 4;
  h *= 60;
  if (h < 0) h += 360;
  return h;
}

function extractHue(url: string): Promise<number> {
  return new Promise((resolve) => {
    const img = new Image();
    img.crossOrigin = "anonymous";
    img.onload = () => {
      try {
        const c = document.createElement("canvas");
        c.width = 24;
        c.height = 24;
        const ctx = c.getContext("2d", { willReadFrequently: true });
        if (!ctx) {
          resolve(DEFAULT_HUE);
          return;
        }
        ctx.drawImage(img, 0, 0, 24, 24);
        const data = ctx.getImageData(0, 0, 24, 24).data;
        let sumSin = 0;
        let sumCos = 0;
        let weight = 0;
        for (let i = 0; i < data.length; i += 4) {
          const r = data[i] / 255;
          const g = data[i + 1] / 255;
          const b = data[i + 2] / 255;
          const max = Math.max(r, g, b);
          const min = Math.min(r, g, b);
          const sat = max === 0 ? 0 : (max - min) / max;
          const w = sat * sat;
          if (w < 0.02) continue;
          const h = (hueFromRgb(r, g, b) * Math.PI) / 180;
          sumSin += Math.sin(h) * w;
          sumCos += Math.cos(h) * w;
          weight += w;
        }
        if (weight === 0) {
          resolve(DEFAULT_HUE);
          return;
        }
        let h = (Math.atan2(sumSin, sumCos) * 180) / Math.PI;
        if (h < 0) h += 360;
        resolve(h);
      } catch {
        resolve(DEFAULT_HUE);
      }
    };
    img.onerror = () => resolve(DEFAULT_HUE);
    img.src = url;
  });
}

let lastUrl: string | null = null;

export async function applyTintFromCover(url: string | null | undefined) {
  if (!url) {
    resetTint();
    return;
  }
  if (url === lastUrl) return;
  lastUrl = url;
  let hue = hueCache.get(url);
  if (hue === undefined) {
    hue = await extractHue(url);
    hueCache.set(url, hue);
  }
  document.documentElement.style.setProperty("--tint-h", hue.toFixed(1));
  document.documentElement.style.setProperty("--tint-k", String(tintIntensity.value));
}

export function resetTint() {
  lastUrl = null;
  document.documentElement.style.removeProperty("--tint-h");
  document.documentElement.style.removeProperty("--tint-k");
}
