# Syncify — sistema de temas

La UI tiene 10 temas conmutables **en runtime**: 4 estándar y 6 de servicio.
El mecanismo: un atributo `data-theme` en `<html>` que re-tematiza toda la app
al vuelo, porque las utilidades de Tailwind referencian variables CSS
(`@theme inline`), no valores.

- Temas estándar: `tinta` (default/fallback) · `camaleon` · `consola` · `riso`
- Temas de servicio: `spotify` · `apple_music` · `tidal` · `qobuz` · `deezer` ·
  `soundcloud` (paletas de marca, seleccionables en Ajustes → Apariencia)
- Definición de tokens: `ui/src/styles/main.css` (un bloque `[data-theme]` por
  cada uno de los 10)
- Registro: `ui/src/composables/useTheme.ts` — `VALID_STANDARD`,
  `VALID_SERVICE` y `THEMES` (clave `syncify.theme`; el fallback si no hay
  tema guardado es `tinta`, también en el bootstrap de `ui/index.html`)
- Anti-FOUC: script inline en `ui/index.html` que fija el tema guardado antes
  del primer paint; su lista `validos` contiene los 10 ids
- Variante `dark:`: `@custom-variant dark` en `main.css` se define sobre los
  10 skins (incluido `riso`, que es light), no sigue al SO
- Tinte Camaleón: `ui/src/composables/useTint.ts` publica `--tint-h` (matiz
  dominante de la carátula) y `--tint-k` (intensidad, clave `syncify.tint`)

## Temas estándar

| Tema | Idea | color-scheme |
|---|---|---|
| `tinta` | Monocromo: el único color llega de la carátula | dark |
| `camaleon` | Se tiñe con el color del disco en reproducción | dark |
| `consola` | Ámbar sobre carbón neutro, plano, sin glow | dark |
| `riso` | Papel frío con dos tintas: azul (interactivo) y rosa (suena) | light |

## Temas de servicio

Los 6 ids de `VALID_SERVICE` en `useTheme.ts` llevan la paleta de marca de su
servicio. Se definen igual que los estándar (bloque `[data-theme]` en
`main.css`) y aparecen en la misma selector de Ajustes → Apariencia.

## Tokens semánticos y sus utilidades

| Token CSS | Utilidad Tailwind | Papel |
|---|---|---|
| `--bg-base` | `bg-base` | Fondo de la app |
| `--bg-shell` | `bg-shell` | Chrome fijo: sidebar, header, barras |
| `--surface-1` | `bg-surface` | Tarjetas, paneles |
| `--surface-2` | `bg-elevated` | Elementos elevados, nav activa |
| `--border` | `border-line` | Bordes estándar |
| `--border-strong` | `border-line-strong` | Bordes con más presencia |
| `--text-1` / `--text-2` / `--text-3` | `text-ink` / `text-ink-2` / `text-ink-3` | Texto primario / secundario / terciario |
| `--accent` / `--accent-hover` | `bg-accent`, `text-accent`, `bg-accent-hover` | Color interactivo |
| `--accent-soft` | `bg-accent-soft` | Selección / fondo sutil de acento |
| `--on-accent` | `text-on-accent` | Texto sobre acento |
| `--ok` / `--warn` / `--error` / `--info` | `text-ok` `text-warn` `text-error` `text-info` (y `bg-*`) | Semántica de estado |
| `--heart` | `text-heart`, `bg-heart` | Favoritos / «me gusta» |
| `--aura` | `background: var(--aura)` | Línea-aura superior del player |
| `--glow` | `box-shadow: var(--glow)` | Resplandor del player |
| `--prog` | `background: var(--prog)` | Relleno de barras de progreso |
| `--shadow-card` / `--shadow-pop` | `box-shadow: var(--shadow-card)` | Elevaciones |

Los alias legacy (`bg-primary`, `bg-surface-dark`, `bg-background-dark`,
`border-border-dark`, `text-text-secondary`, `text-success`…) siguen
funcionando: resuelven contra estos mismos tokens vía `@theme inline` en
`main.css`. Son ruta de migración, no destino.

## Cómo añadir un tema nuevo

1. **Tokens**: añade un bloque `[data-theme="mi-tema"] { ... }` en
   `ui/src/styles/main.css` con TODAS las variables de un tema existente
   (`color-scheme` incluido) y añádelo a la `@custom-variant dark` del mismo
   fichero si participa en la variante `dark:`.
2. **Registro**: añade su `id` a `VALID_STANDARD` o `VALID_SERVICE` (según su
   tipo) y su metadato a `THEMES` en `ui/src/composables/useTheme.ts`.
3. **Anti-FOUC**: añade el `id` a la lista `validos` del script inline de
   `ui/index.html` (y al bootstrap por defecto si corresponde).
4. Nada más: los alias y las utilidades semánticas lo recogen solas, y la
   pestaña Ajustes → Apariencia lo lista automáticamente (sus tarjetas de
   preview llevan su propio `data-theme`).

## Contrato del proyecto

- **Prohibido hardcodear hex** en componentes y vistas. Los únicos hex viven en
  `main.css` (definición de temas) y en los valores de marca de los servicios
  (Spotify, SoundCloud…) y `quality-gold/silver/gray`, que no teman.
- Usa utilidades semánticas (`bg-surface`, `text-ink-2`, `border-line`…) o
  `var(--token)` en estilos inline/`<style>` cuando no haya utilidad.
- Un elemento que deba verse con OTRO tema (p. ej. las tarjetas-preview de
  Ajustes → Apariencia) lleva su propio `data-theme`: los tokens caen en
  cascada, sin duplicar valores.
- Las reglas base globales (select, scrollbar, `.glass-panel`,
  `.material-symbols-filled`) van en `@layer base` de `main.css`, tokenizadas.
