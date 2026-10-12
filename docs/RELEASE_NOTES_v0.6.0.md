## Novedades

### 📁 Biblioteca local
- Escaneo e importación de carpetas de audio, con opción recursiva y detección de duplicados por ISRC y hash SHA-256 del archivo.
- Nuevo diálogo de sincronización de biblioteca local con selección de carpeta y resumen de resultados.

### 🎵 Reproductor y letras
- Vista del reproductor a pantalla completa con cola, controles y letras LRC sincronizadas por línea; integrada con las estadísticas de reproducción local.
- La vista de letras conserva las marcas de tiempo por palabra de los LRC mejorados, las oculta durante la escucha y permite mostrar u ocultar las marcas de tiempo de las líneas.

### 🎨 Metadatos
- Nuevo control en Metadatos para barrer portadas animadas de álbumes descargados y guardar sus archivos auxiliares junto al audio, sin modificar la carátula incrustada del FLAC.

### 🎧 Calidad, MusicBrainz y cuentas
- Cascada de calidad estricta: para solicitudes sin pérdida, Qobuz intenta conservar esa calidad y puede buscar una edición equivalente en Tidal antes de degradar; con el descenso permitido, se rechaza audio con pérdida por debajo de 320 kbps.
- Caché persistente de consultas a MusicBrainz e índice de grabaciones de la biblioteca por ISRC.
- La vista multi-cuenta muestra la identidad real de usuario por fila; las preferencias de incrustación de letras y carátulas se leen de la base de datos en lugar de fijarse en el trabajador de descargas.

### 🛠️ Correcciones y comentarios
- Los comandos de procesos auxiliares comparten la configuración para evitar ventanas de consola al lanzarlos en Windows.
- La recuperación de operaciones clasifica por separado las muestras de audio de Qobuz y las cargas de audio inválidas.
- La migración de preferencias de importación conserva las filas existentes y aplica los nuevos valores predeterminados solo a las filas futuras.
- Nuevo formulario de errores y comentarios que guarda los reportes localmente como JSON; no los envía automáticamente.

## Limitaciones de esta versión
- La reproducción en una sesión gráfica real no se ha verificado.
- No se ha ejercitado la red en vivo.
- El target Windows/MSVC no se ha comprobado.
- MPRIS no se ha verificado.
- Las pruebas que requieren red o credenciales siguen marcadas como ignoradas.

**Instaladores**: AppImage y DEB (Linux), instalador y portable (Windows).
