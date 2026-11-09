## Puentes Python

**Puente**:
Proceso Python que la app lanza para hablar con un proveedor o una herramienta
externa (`scripts/*.py`). Todos comparten el mismo contrato de envelope JSON por
stdout, el mismo intérprete resuelto y el mismo presupuesto de timeout declarado en
cada llamada (`docs/adr/0021-contrato-unico-de-puentes-python.md`).
_Evitar_: «script» para referirse a un puente cuando se habla de su contrato.

**Envelope de puente**:
Sobre JSON `{success, data, error}` que todo puente escribe por stdout. `success` es
obligatorio; `data` y `error` llegan según el desenlace. Un fallo de dominio viaja
como envelope con `success: false` y exit 1; un bug del puente es traceback en stderr
sin envelope.
_Evitar_: llamar «resultado» al envelope cuando se habla del shape del contrato.

**BridgeClient**:
Única vía Rust para lanzar un puente e interpretar su envelope
(`src-tauri/src/bridges/`). `capture` devuelve la salida cruda; `call<T>` deserializa
el envelope tipado.
_Evitar_: construir `Command` para un puente fuera de este módulo.
## Letras

**Fuente de letras**:
Uno de los orígenes independientes de los que Syncify resuelve una letra (LRCLIB,
Apple Music TTML, Kugou, …). No es un proveedor de música: sirve letras para
cualquier pista, venga o no de su biblioteca.
_Evitar_: 'proveedor' a secas, reservado a los servicios de música.

**Nivel de sincronía**:
Granularidad temporal que garantiza una letra: por sílaba, por palabra, por línea o
sin marcas (plain). Ordena la calidad entre versiones: más fino, mejor.
_Evitar_: 'calidad' a secas; 'tipo de letra' para el nivel.

**Enhanced LRC (ELRC)**:
LRC con marcas de tiempo por palabra o sílaba dentro de la línea. Es el formato de
preservación de la sincronía fina; degradarlo a LRC de línea es pérdida.
_Evitar_: 'LRC mejorado', 'karaoke LRC'.

**Sidecar**:
Fichero `.lrc` hermano del audio que entrega la letra a reproductores externos. Es
una proyección de la letra guardada, no su origen.
_Evitar_: 'fichero de letra' para el almacenamiento canónico.

**Versión de letra**:
Una letra concreta que una **Fuente de letras** produjo para un idioma y formato
dados. Varias versiones pueden convivir para la misma pista; el formato es una
propiedad del contenido, no su identidad.
_Evitar_: 'formato' para la versión.

**Versión activa**:
La versión de letra que la reproducción y la UI muestran de una pista. La fija un
**Pin de versión** del usuario o, en su ausencia, la regla de selección (nivel de
sincronía, idioma preferido, prioridad de fuente).
_Evitar_: 'letra actual' sin más; el badge de UI no persiste la elección.

**Pin de versión**:
Elección explícita del usuario que fija la **Versión activa**. La resolución
automática, los reintentos y el re-fetch karaoke no la degradan: solo la acción del
usuario la cambia.
_Evitar_: 'favorito', 'seleccionada' como estado efímero.

**Marca de borrado**:
Intención persistente del usuario de que una pista no tenga letra, por formato o
completa. La resolución automática la respeta; solo guardar o restaurar
explícitamente ese formato la anula.
_Evitar_: tratarla como caché de la resolución: es voluntad del usuario, no un
resultado.

**Cosecha**:
Barrido del disco local que incorpora letras ya presentes en ficheros (tags
embebidos o sidecars) a la biblioteca. No resuelve en red.
_Evitar_: 'escaneo' genérico; 'descarga', que no toca.

**Letra instrumental**:
Resultado válido de resolución que declara que la pista no tiene letra cantada. No
es un fallo ni ausencia: se muestra como tal.
_Evitar_: 'sin letra', que sugiere que falta resolverla.

**Sonda**:
Comprobación puntual de que una fuente responde con su endpoint, sin resolver una
pista concreta. Distingue falta de credencial de fuente caída.
_Evitar_: 'test de proveedor'; 'ping'.

## Ajustes y backup

**Ajuste**:
Una preferencia del usuario con nombre, valor y superficie de persistencia (tabla
tipada o KV genérica). Persistido no significa consumido: un ajuste sin lector no
es una preferencia, es una promesa falsa.
_Evitar_: 'opción' para el ajuste sin lector; 'config' a secas.

**Ajuste huérfano**:
Ajuste visible o persistido que ningún código lee para decidir comportamiento. No
es deuda menor: miente en la UI. Su destino es cablearse u ocultarse, nunca
quedarse.
_Evitar_: 'ajuste decorativo'; 'dead setting'.

**Superficie de ajustes**:
El almacén atómico de un grupo de ajustes: una tabla tipada (singleton o por fila)
o la KV genérica. Cada valor vive en una sola superficie; la duplicación entre
superficies es un defecto, no un patrón.
_Evitar_: 'tabla de ajustes' para la KV genérica.

**Consumidor**:
El código que lee un ajuste para decidir comportamiento. Es lo que acredita que el
ajuste existe de verdad: escribirlo no basta.
_Evitar_: 'persistido' como sinónimo de consumido.

**Defaults**:
Valores de fábrica de una superficie. Viven en un solo sitio y de ahí comen
arranque, reset y seed; las copias de defaults en la UI están prohibidas.
_Evitar_: 'seed' para el valor leído en runtime.

**Reset**:
Restaurar una o todas las superficies a sus defaults. Es una operación del backend
sobre los almacenes, no una compensación que la UI hace en cliente.
_Evitar_: confundirlo con el borrado de biblioteca; 'factory reset' si solo toca
ajustes.

**Manifest de backup**:
El JSON de exportación con checksum y versionado de esquema que lleva biblioteca y
ajustes, nunca secretos. Restaurarlo en otra máquina reproduce biblioteca y
ajustes; las cuentas piden reautenticar.
_Evitar_: llamar 'backup' a la copia pre-mutación interna.

**Copia pre-mutación**:
Snapshot interno de la BD que una operación peligrosa deja antes de escribir. No es
un backup de usuario: no sale del equipo, no se programa y no se lista en la UI.
_Evitar_: 'backup automático'.

**Navegador de pistas**:
La maquinaria común de las listas de pistas de la UI: una ventana virtual compartida
y, para las listas basadas en biblioteca, un solo composable de mapeo, paginación y
selección. Cada vista declara su fuente de filas; el comportamiento de lista no se
copia entre vistas.
_Evitar_: llamar 'navegador' al explorador de ficheros del sistema; decir 'vista'
al composable.

**Ventana virtual**:
Técnica de render que solo pinta las filas visibles de una lista larga dentro de un
spacer con la altura total. Una sola implementación sirve a todas las vistas; alturas
y holgura son configuración por vista, no copias.
_Evitar_: confundirla con la paginación (que es server-side, de datos; la ventana es
de render).

**Selección por ids**:
La única convención de selección de pistas de la UI: los ids seleccionados viven
fuera de las filas y las acciones masivas los consumen. Las filas no llevan estado
de selección propio.
_Evitar_: 'seleccionado' como flag dentro de la fila; contadores paralelos al array
de ids.
