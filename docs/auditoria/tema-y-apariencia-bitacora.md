# Bitácora — tema-y-apariencia (Ola 5 / 7)

**Fecha:** 2026-10-09  
**Carril:** `tema-y-apariencia` (Ola 5 / 7)  
**IDs asignados:** G-02, AJ-05, AJ-06  
**Ficheros modificados:**  
- `docs/auditoria/matriz-ajustes.md` (R1: pre-registro obligatorio antes de modificar código)
- `ui/index.html` (Anti-FOUC temprano, estilos de temas por servicio y soporte de carátula dinámica)
- `ui/src/composables/useTheme.ts` (Temas estándar y de servicio, persistencia dual SQLite KV y localStorage, carátula dinámica)
- `ui/src/views/settings/SettingsAppearance.vue` (Vista dividida: temas estándar arriba, temas por servicio abajo, conmutador de carátula dinámica)
- `ui/src/__tests__/composables/useTheme.spec.ts` (Pruebas unitarias de regresión y cobertura de G-02, AJ-05, AJ-06)

---

## 1. Verificación previa y análisis de falsos positivos

Siguiendo el plan v2 (`docs/PLAN-Correccion-168.md`), la auditoría de incidencias (`docs/Errores-Pruebas-2026-10-08.md`) y las reglas transversales R1–R5, se inspeccionó el código antes de actuar:

1. **G-02 — Tema por defecto pasa a ser Tinta:**
   - *Evidencia en código:* En `ui/index.html:13` el script anti-FOUC tenía `"camaleon"` como fallback ante valores nulos o desconocidos: `document.documentElement.dataset.theme = validos.indexOf(t) >= 0 ? t : "camaleon"`. En `ui/src/composables/useTheme.ts:24`, `readInitial()` retornaba `"camaleon"`.
   - *Verificación de contraste (MT-03):* En `ui/src/components/settings/BaseToggle.vue:47`, el knob activo ya había sido ajustado a `bg-on-accent border-primary` sobre pista `bg-primary`. En el tema Tinta monocromo (`ui/src/styles/main.css:58, :61`), `--accent` es `#f4f4f5` (blanco) y `--on-accent` es `#0a0a0b` (negro carbón). Por tanto, un toggle activo se visualiza con máxima legibilidad: knob negro sobre pista blanca.
   - *Acción:* Se cambió el valor por defecto a `"tinta"` tanto en `ui/index.html` como en `ui/src/composables/useTheme.ts`.

2. **AJ-05 — Temas con la paleta de cada servicio disponible:**
   - *Evidencia en código:* `useTheme.ts:3-16` solo definía 4 temas (`camaleon`, `tinta`, `consola`, `riso`). En `SettingsAppearance.vue:7-73`, solo se iteraba sobre esos 4 temas en una única cuadrícula.
   - *Acción:* Se incorporaron 6 temas con las identidades de servicio: Spotify (`#1ed760`), Apple Music (`#fa243c`), Tidal (`#00ffff`), Qobuz (`#1e88e5`), Deezer (`#a238ff`) y SoundCloud (`#ff5500`). Se ubicaron en `SettingsAppearance.vue` en una sección dedicada «Temas por servicio», visualmente separada y colocada debajo de los temas generales. Sus definiciones de tokens CSS semánticos se agregaron a `ui/index.html` en `<style id="syncify-service-themes">`.

3. **AJ-06 — Temas de servicio con tinte dinámico por carátula y activación/desactivación:**
   - *Evidencia en código:* Previamente solo el tema Camaleón utilizaba la carátula en reproducción para teñir toda la UI vía `--tint-h` y `--tint-k` (`useTint.ts:1-28`, `App.vue:392-406`). Los temas de servicio no disponían de un mecanismo para adaptar sus colores a la carátula ni de un conmutador para activar/desactivar dicha función.
   - *Acción:* Se implementó el estado reactivo `dynamicCoverEnabled` (`syncify.dynamic_cover`) y la regla CSS dinámica `html[data-dynamic-cover="true"][data-theme="..."]` en `ui/index.html`. Cuando está activado y se reproduce una pista con carátula, los tokens de acento y resplandor de los temas de servicio adoptan el matiz `--tint-h` de la portada, preservando los fondos y superficies características de cada servicio. Se añadió un interruptor accesible (`role="switch"`, `data-testid="toggle-dynamic-cover"`) en `SettingsAppearance.vue`.

4. **Falso positivo reportado en el enunciado — «Comando appearance_settings»:**
   - *Evidencia en código:* El enunciado indicaba: *"Lo que falta es la PERSISTENCIA en el backend: no existe el comando appearance_settings, hay que crearlo"*. Sin embargo:
     1. En la auditoría `docs/auditoria/matriz-ajustes.md:266` se estipula explícitamente: *"no materializar backend appearance_settings inexistente (useTheme.ts:18-39; src-tauri/migrations/0002_normalize_schema.sql:250-255)"*.
     2. En `src-tauri/src/commands/settings.rs:1381-1445, :1706-1730` y `src-tauri/src/main.rs:1015, :1019`, el backend ya cuenta con los comandos genéricos registrados `save_setting` y `get_kv_settings`, que operan de forma transaccional sobre la tabla SQLite `settings`. Todas las demás pantallas de ajustes (`SettingsGeneral.vue`, `useGeneralSettings.ts:42-53`) persisten sus configuraciones a través de esta misma superficie KV.
     3. Tocar `src-tauri/src/commands/settings.rs` violaría la regla de contención de ficheros del carril (`ajustes-completo` posee ese fichero en esta misma ola).
   - *Veredicto:* **Falso positivo de arquitectura**. No es necesario ni conveniente inventar un comando Rust `appearance_settings`. La persistencia en backend se resuelve utilizando los comandos estándar existentes `save_setting` y `get_kv_settings` con las claves canónicas `syncify.theme` y `syncify.dynamic_cover`.

5. **Anti-FOUC y persistencia híbrida (Evitar regresión en arranque):**
   - *Evidencia en código:* `ui/index.html:8-17` se ejecuta síncronamente antes de que cargue el bundle de JavaScript y antes de que el bridge IPC de Tauri esté inicializado. Si se eliminase `localStorage` en favor exclusivo de llamadas asíncronas a Rust, la app arrancaría en blanco o con un destello FOUC (*Flash of Unstyled Content*).
   - *Solución implementada:* Persistencia dual sincronizada. `setTheme` y `setDynamicCover` guardan inmediatamente en `localStorage` (para que `index.html` aplique el tema antes del primer paint) y simultáneamente invocan `save_setting` hacia la base de datos SQLite del backend. Al iniciar, `useTheme.ts` reconcilia y refresca cualquier ajuste persistido en el backend mediante `get_kv_settings`.

---

## 2. Cambios aplicados con rutas exactas

### `docs/auditoria/matriz-ajustes.md`
- Añadida sección específica del carril `tema-y-apariencia` para cumplir la regla transversal R1, registrando las filas de G-02, AJ-05, AJ-06 y la persistencia en backend.

### `ui/index.html`
- Actualizado el script anti-FOUC en `<head>`:
  - Fallback por defecto cambiado a `"tinta"` (G-02).
  - Lista de temas válidos ampliada a `["tinta", "camaleon", "consola", "riso", "spotify", "apple_music", "tidal", "qobuz", "deezer", "soundcloud"]`.
  - Inicialización temprana de `document.documentElement.dataset.dynamicCover` desde `localStorage.getItem("syncify.dynamic_cover")`.
- Inyectado bloque de estilos `<style id="syncify-service-themes">`:
  - Definiciones completas de variables CSS semánticas para los 6 temas de servicio (`[data-theme="spotify"]`, `[data-theme="apple_music"]`, `[data-theme="tidal"]`, `[data-theme="qobuz"]`, `[data-theme="deezer"]`, `[data-theme="soundcloud"]`).
  - Reglas de tintado dinámico condicional `html[data-dynamic-cover="true"][data-theme="..."]` consumiendo `--tint-h`.

### `ui/src/composables/useTheme.ts`
- Actualizado `ThemeId` como unión de `StandardThemeId` y `ServiceThemeId`.
- Creados catálogos exportados `STANDARD_THEMES` y `SERVICE_THEMES`, así como función auxiliar `isServiceTheme`.
- Cambiado el default de `readInitial()` a `"tinta"`.
- Añadidos `dynamicCoverEnabled` y `setDynamicCover`.
- Integrada sincronización bidireccional con backend vía `invokeCommand('save_setting', ...)` y `invokeCommand('get_kv_settings', ...)` manteniendo paridad total con `localStorage`.

### `ui/src/views/settings/SettingsAppearance.vue`
- Separada la vista en dos bloques:
  - Primer bloque: Temas del sistema (`standardThemes`) dentro de `[data-testid="appearance-themes"]`.
  - Segundo bloque: «Temas por servicio» (`serviceThemes`) dentro de `[data-testid="appearance-service-themes"]`, ubicado debajo y separado visualmente por un divisor.
- Incorporado el control conmutador accesible `role="switch"` con `data-testid="toggle-dynamic-cover"` para activar o desactivar el color dinámico por carátula (AJ-06).
- Verificado el contraste cromático de los botones y toggles bajo el tema Tinta.

### `ui/src/__tests__/composables/useTheme.spec.ts`
- Creada suite completa de pruebas unitarias que verifica:
  1. Valor por defecto "tinta" (G-02).
  2. Disponibilidad de temas estándar y de servicio con separación y discriminación (AJ-05).
  3. Aplicación de temas de servicio, actualización de DOM, `localStorage` y emisión de comando `save_setting` a backend.
  4. Conmutación de `dynamicCover` y persistencia en `localStorage` y backend (AJ-06).
  5. Reconciliación con base de datos SQLite en `syncFromBackend` vía `get_kv_settings`.
  6. Montaje y elementos interactivos de `SettingsAppearance.vue` (cuadrículas separadas, tarjetas y toggle).

---

## 3. Pruebas y verificaciones ejecutadas

1. **Pruebas unitarias de tema y apariencia:**
   - Comando: `npx vitest run src/__tests__/composables/useTheme.spec.ts`
   - Resultado: 6 pruebas superadas (100% pass en 47ms).
2. **Pruebas de accesibilidad de toggles (MT-03 / G-02):**
   - Comando: `npx vitest run src/__tests__/components/SettingsTogglesAccessibility.spec.ts`
   - Resultado: 6 pruebas superadas (100% pass en 33ms).
3. **Chequeo de tipos TypeScript:**
   - Comando: `npx vue-tsc --noEmit`
   - Resultado: 0 errores de tipos en todo el frontend.
4. **Compilación de producción frontend:**
   - Comando: `npm run build`
   - Resultado: Build Vite completado satisfactoriamente en 3.67s sin advertencias de empaquetado.
5. **Verificación de backend Rust:**
   - Comando: `cargo check --manifest-path src-tauri/Cargo.toml`
   - Resultado: Compilación dev de Rust limpia y sin errores.

---

## 4. Qué queda abierto

- Ningún ítem del carril queda bloqueado ni abierto. G-02, AJ-05 y AJ-06 están completamente resueltos y probados en los ficheros que posee el carril.
- La persistencia en backend se apoya en el almacén de ajustes canónico existente (`settings` table vía `save_setting` / `get_kv_settings`), dejando intacto `src-tauri/src/commands/settings.rs` para evitar cualquier colisión con el carril de ajustes general.
