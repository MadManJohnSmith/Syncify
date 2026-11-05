# Fuentes self-hosted

Archivos `.woff2` servidos por la propia app (Vite copia `ui/public/` tal cual al
bundle). El CSS que los declara vive en `ui/src/styles/fonts.css`, generado a
partir de los dos documentos css2 de Google Fonts que antes se cargaban con
`<link>` en `ui/index.html`:

- `Inter`, `Geist`, `Geist Mono`, `Space Grotesk` (subset latin/cyrillic/…)
- `Material Symbols Outlined` (fuente variable de iconos)

Motivo: sin red (o con Google Fonts bloqueado) los iconos se renderizaban como
texto y cambiaba la tipografía. Con los archivos locales la UI es idéntica en
cualquier red.

Licencias (se conservan las de cada familia; este directorio solo redistribuye
los archivos servidos públicamente por Google Fonts):

- Inter — SIL Open Font License 1.1 (https://github.com/rsms/inter)
- Geist / Geist Mono — SIL Open Font License 1.1 (https://github.com/vercel/geist-font)
- Space Grotesk — SIL Open Font License 1.1 (https://github.com/floriankarsten/space-grotesk)
- Material Symbols — Apache License 2.0 (https://github.com/google/material-design-icons)

Regenerar (si se cambian pesos o familias): volver a descargar el CSS de
`fonts.googleapis.com/css2?family=...` con un User-Agent de navegador,
reescribir cada `url(https://fonts.gstatic.com/...)` a `url(/fonts/<archivo>)`
y guardar los `.woff2` aquí.
