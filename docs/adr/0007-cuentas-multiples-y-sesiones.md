# ADR-0007 — Cuentas múltiples, sesiones aisladas y ciclo de vida

**Estado:** propuesto · **Fecha:** 2026-11-04
**Origen:** ronda de grilling del flujo de cuentas e inicio de sesión, con la instalación
real de 14 cuentas.
**Spec:** [#62](https://github.com/MadManJohnSmith/Syncify/issues/62) · **Sonda:**
[#63](https://github.com/MadManJohnSmith/Syncify/issues/63).
**Spec de cuentas:** [#83](https://github.com/MadManJohnSmith/Syncify/issues/83) · **Tickets:** [#84](https://github.com/MadManJohnSmith/Syncify/issues/84)–[#95](https://github.com/MadManJohnSmith/Syncify/issues/95).

## Contexto

La app ya soporta varias cuentas por servicio en la base (14 en la instalación medida),
pero la interfaz sigue asumiendo una cuenta activa por servicio: `is_active` decide qué
biblioteca y qué contadores se muestran, y la consulta de estado filtra por ella. La
spec #62 resolvió el login dentro del webview con un perfil por proveedor; con varias
cuentas del mismo servicio, un perfil compartido mezcla cookies y puede leer el
artefacto de la cuenta equivocada. Quitar una cuenta hoy es una operación destructiva
sin diálogo de consecuencias, y una credencial caducada no tiene un camino claro de
reautenticación: la cuenta queda marcada, pero el usuario no siempre sabe qué hacer.

## Decisión

1. **Varias cuentas por servicio, todas visibles.** No hay cuenta activa. La biblioteca
   es una vista agregada con una fila por pista canónica y filtros explícitos por
   servicio y cuenta. Cada cuenta conserva sus contadores; la cabecera añade un total de
   pistas canónicas distintas, nunca la suma de entradas.
2. **Un perfil de webview por cuenta.** El directorio de datos se identifica por la
   cuenta, o por un id de intento de login antes de que la cuenta exista. Añadir una
   segunda cuenta no comparte el cookie jar con la primera y el artefacto leído
   pertenece siempre a la cuenta correcta. El perfil se conserva cifrado en reposo con
   la misma clave que las credenciales, se descifra solo mientras la ventana de login
   está abierta y se borra al cerrar sesión o al eliminar la cuenta.
3. **Ciclo de vida explícito.** Una cuenta puede estar habilitada, desactivada,
   archivada o eliminada. Desactivar detiene sync y descargas y conserva todo; archivar
   la oculta y conserva todo; eliminar quita la cuenta y sus entradas de biblioteca,
   pero conserva las pistas canónicas y los archivos descargados. Borrar audio es una
   casilla aparte, nunca implícita. El diálogo de quitar cuenta presenta las opciones con
   sus consecuencias.
4. **Reautenticación con aviso y un clic.** Cuando el proveedor rechaza la credencial, la
   cuenta se marca, su sync y sus descargas se detienen y la ficha ofrece el mismo
   webview para reautenticar sin perder la biblioteca. No hay reintentos silenciosos en
   bucle.
5. **Login dentro de la app, sin capturar contraseñas.** Se mantiene lo decidido en #62:
   webview con descriptor por proveedor, lectura solo del artefacto necesario (cookie o
   clave de almacenamiento), cancelación limpia si se cierra la ventana y sesión
   recordada mediante el artefacto cifrado. La app no guarda la contraseña ni el correo
   del formulario.
6. **Re-login reutiliza la cuenta.** Un nuevo login del mismo usuario actualiza la cuenta
   existente: primero por `external_user_id`, después por email normalizado. Nunca crea
   una cuenta duplicada.

7. **La etiqueta distingue cuentas y el correo se enmascara.** La ficha muestra el nombre
   del proveedor y una etiqueta editable; el correo se muestra enmascarado y se usa solo
   para identidad, nunca en logs.
8. **Suscripción y vigencia visibles.** La ficha muestra si la cuenta puede transmitir y
   descargar y hasta cuándo; se refresca en cada sync (24 h por defecto) y al reautenticar.
   Si caduca, avisa y deja de enrutar descargas a esa cuenta.
9. **Cerrar sesión es una acción de seguridad distinta de desactivar.** Borra credenciales
   y perfil, conserva cuenta y biblioteca, deja la cuenta «sin sesión» y detiene sync y
   descargas hasta reautenticar.


10. **Si la clave no está o la base viene de otra máquina, se pide reautenticar.** Las
    credenciales indescifrables se detectan, la cuenta y la biblioteca se conservan, el
    estado pasa a «sin sesión» y la ficha ofrece reautenticar. Nunca se inventan
    credenciales ni se falla en silencio.
11. **El backup no contiene secretos.** Incluye biblioteca, ajustes y cuentas, pero no
    credenciales ni perfiles de webview; tras restaurar se reautentica.
12. **Una caída del proveedor no es una credencial inválida.** Un 5xx o un fallo de red se
    muestra como «proveedor no disponible», la cuenta sigue habilitada y se reintenta con
    backoff; solo un 401/403 marca reautenticación.
13. **Reconectar una cuenta existente la actualiza, no la duplica.** El login detecta la
    cuenta ya conectada, avisa y ofrece reautenticar o abrir su ficha; si el artefacto es
    de otra cuenta, se añade como nueva.

### Reglas derivadas

- El toggle de participación del ADR-0006 y el estado «desactivada» del ciclo de vida son
  el mismo estado; no hay dos interruptores.
- Archivar implica deshabilitar y ocultar; eliminar borra el perfil de webview, desactivar
  y archivar lo conservan.
- Una cuenta con sesión cerrada no sincroniza ni descarga hasta reautenticar.


14. **Refresco proactivo cuando hay vencimiento y reactivo al 401.** Si el proveedor
    publica la expiración, se refresca antes; si no, se reacciona al primer 401/403. Un
    fallo de refresco marca la cuenta para reautenticar.
15. **Cero cuentas es un estado vacío, no un wizard.** La pantalla de cuentas muestra las
    seis tarjetas con «conectar» y el estado de cada servicio; no hay flujo de bienvenida
    separado.
16. **Las cuentas se ordenan por servicio y etiqueta.** Agrupadas por servicio y
    alfabéticas dentro, con la etiqueta editable como criterio; el orden es estable y no
    cambia con el uso.


## Consecuencias

- `is_active` deja de significar «cuenta visible para la interfaz»; se sustituye por el
  estado del ciclo de vida y por el toggle de participación en sync y descargas. La
  migración debe preservar la cuenta que hoy está activa como habilitada.
- La biblioteca agregada necesita consultas e índices por pista canónica y filtros por
  cuenta; los contadores por cuenta no se suman.
- Eliminar una cuenta debe borrar su directorio de webview; archivar y desactivar lo
  conservan.
- La spec #62 se amplía con el perfil por cuenta; la sonda #63 sigue siendo el caso de
  Deezer.

## Preguntas abiertas

No quedan decisiones de contrato. Nombres de comandos, migraciones, textos y
disposición exacta pertenecen a la spec y a los tickets.
