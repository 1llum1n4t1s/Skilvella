# Skilvella

[日本語](README.ja.md) | [English](README.md) | [简体中文](README.zh-CN.md) | [繁體中文](README.zh-TW.md) | [한국어](README.ko.md) | **Español** | [Français](README.fr.md) | [Deutsch](README.de.md) | [Português (Brasil)](README.pt-BR.md)

Una aplicación local que organiza en el navegador grandes colecciones de archivos `SKILL.md` y carga mediante MCP solo las habilidades que necesitas. Sin importar cuántas habilidades tengas, expone únicamente tres herramientas MCP.

- Búsqueda por palabras clave en nombres, descripciones, contenido y etiquetas, incluida la coincidencia de subcadenas en japonés.
- Registro de carpetas, creación de habilidades, edición de contenido, etiquetas y categorías, operaciones en lote y activación de la publicación MCP habilidad por habilidad.
- Historial guardado antes de cada edición con restauración, detección de conflictos con ediciones externas y comprobaciones de nombres duplicados, errores de sintaxis y archivos ausentes.
- Windows / macOS / Linux. Se usa desde el navegador del mismo PC.

## Primeros pasos

En Windows, ejecuta [SkilvellaApp-win-Setup.exe](https://skilvella.kagayoi.com/SkilvellaApp-win-Setup.exe) e inicia "Skilvella" desde el menú Inicio. La versión con instalador (Setup) se registra durante la instalación para iniciarse cuando el usuario actual inicia sesión. Si cambias desde una versión independiente (ZIP) existente, cierra primero la versión antigua desde su icono de la bandeja de tareas (si la versión antigua sigue en ejecución, el puerto predeterminado (7465) no está disponible y la versión con instalador (Setup) muestra un mensaje al respecto y no se inicia). El registro de inicio automático de la versión ZIP se traspasa automáticamente a la versión con instalador (Setup) durante la instalación, y el acceso directo "Skilvella" del menú Inicio pasa a apuntar también a la versión con instalador (Setup). Si habías desactivado el inicio automático, sigue desactivado. Al desinstalar también se elimina el registro de inicio automático que apunta a la versión con instalador (Setup). La carpeta de la versión antigua no se elimina automáticamente, así que bórrala a mano si ya no la necesitas. Si usas una versión antigua con instalador (Setup) sin mecanismo de actualización automática (de la 1.1.4 a la 1.1.6), reinstala una vez con el nuevo Setup. La carpeta de datos estándar `%LOCALAPPDATA%\Skilvella` se comparte y se conserva tras desinstalar. La versión con instalador se actualiza automáticamente ([Actualizaciones de la aplicación](#app-updates)).

Para usar la versión ZIP o la versión para macOS, descarga el paquete correspondiente ([Windows x64](https://skilvella.kagayoi.com/portable/Skilvella-windows-x86_64.zip) o [Mac Apple Silicon](https://skilvella.kagayoi.com/portable/Skilvella-macos-aarch64.tar.gz), alojados en skilvella.kagayoi.com) y extráelo. Las versiones ZIP y macOS hasta la 1.1.8 buscaban actualizaciones en GitHub, algo que ya no se usa, así que si usas una de ellas, descarga manualmente el nuevo paquete una sola vez.

- Windows x64: extrae `Skilvella-windows-x86_64.zip` en la carpeta que vayas a seguir usando y ejecuta `skilvella.exe`. Se ejecuta en la bandeja de tareas sin abrir un terminal, y el primer inicio normal añade "Skilvella" al menú Inicio del usuario actual.
- Mac Apple Silicon: extrae `Skilvella-macos-aarch64.tar.gz` y ejecuta `./skilvella` en un terminal.
- Antes de extraer, puedes verificar el SHA-256 del paquete con [`SHA256SUMS.txt`](https://skilvella.kagayoi.com/portable/SHA256SUMS.txt).

Para Linux o para compilar desde el código fuente, consulta la [guía de desarrollo](docs/DEVELOPMENT.md).

Skilvella muestra su icono en la bandeja de tareas en Windows y en la barra de menús en Mac. Haz clic izquierdo en él para abrir el gestor en tu navegador predeterminado. Haz clic derecho (o Control-clic en Mac) para elegir "Abrir administrador", "Buscar actualizaciones", "Reiniciar servicio" o "Salir". En Windows, si no ves el icono, comprueba también el desbordamiento de iconos ocultos de la bandeja.

"Reiniciar servicio" reinicia el servidor HTTP/MCP y la vigilancia de archivos en el mismo proceso y en la misma URL. Las habilidades y la configuración guardadas se conservan. Las solicitudes duplicadas durante un reinicio se combinan, y el reinicio espera a que terminen las operaciones HTTP en curso. Las conexiones MCP se cierran, así que vuelve a conectar desde tu cliente. Recarga cualquier página del gestor que esté abierta antes de usarla. "Salir" detiene el servidor y el icono.

También puedes abrir el gestor directamente en [http://127.0.0.1:7465/](http://127.0.0.1:7465/). El endpoint MCP es `http://127.0.0.1:7465/mcp`. En un entorno sin pantalla, pasa `--no-tray`. En Linux no muestra ningún icono y se ejecuta como servidor HTTP/MCP.

En la disposición independiente de Windows, iniciar sin argumentos intenta registrar la aplicación para que se inicie cuando el usuario actual inicia sesión. Si ya está registrada, iniciar el ejecutable desde otra ubicación no cambia la ruta registrada. Puedes activar o desactivar el inicio automático en "Configuración" → "Aplicaciones" → "Inicio" de Windows. Para omitir el registro en este inicio, pasa `--no-autostart`. Para el inicio automático gestionado por Toolport, configúralo en Toolport.

No se necesita Rust, Node.js, Python ni un servidor de base de datos externo en tiempo de ejecución. Si lo iniciaste desde un terminal, también puedes salir con `Ctrl+C` en la consola asociada.

## Gestión de tu biblioteca

En el primer inicio, Skilvella detecta automáticamente la carpeta gestionada. Comprueba en este orden las carpetas existentes `~/.agents/skills`, `$CODEX_HOME/skills` y `~/.codex/skills`, y si no encuentra ninguna, crea `<data-folder-name>-skills` junto a la carpeta de datos. Se omite un candidato que contenga únicamente entradas de invocación de Codex generadas por Skilvella, y las propias entradas no se cuentan como habilidades gestionadas. Esto solo se cumple cuando la carpeta de datos tiene un registro de propiedad de esas entradas. Si vuelves a crear la carpeta de datos o apuntas a otra con `--data-dir`, las entradas generadas anteriormente que sigan existiendo se registrarán como habilidades normales. Si antes se habían registrado orígenes, esa configuración tiene prioridad y se traspasa. Si un problema al leer el registro de propiedad de las entradas o los candidatos impide completar la detección automática, se conserva el destino de registro automático guardado. Comprueba el estado de la detección en la lista de carpetas a la izquierda de la biblioteca.

La biblioteca es una sola pantalla: la lista de carpetas gestionadas a la izquierda y la lista de habilidades a la derecha. Pulsa "+ Añadir carpeta" para abrir el selector de carpetas estándar del sistema operativo; la carpeta elegida se registra tal cual y se escanea. Los archivos no se mueven. No necesitas introducir un nombre: se usa el nombre de la carpeta como nombre visible (puedes cambiarlo después con "Cambiar nombre visible"). En entornos donde no se puede abrir el cuadro de diálogo (como las sesiones SSH), introduce una ruta absoluta en "Introducir una ruta".

Al seleccionar una carpeta a la izquierda se muestran su ubicación, su recuento y la advertencia de ruta de búsqueda estándar, junto con "Cambiar nombre visible" y "Dejar de gestionar". Para la carpeta predeterminada, "Cambiar carpeta de referencia" permite cambiar la carpeta de referencia y "Examinar…" permite elegirla desde el cuadro de diálogo. El ajuste persiste tras los reinicios, y "Dejar de gestionar" no elimina archivos. Si eliges una carpeta que ya está gestionada, o una que tiene relación de carpeta padre/hija con una carpeta gestionada, la adición se detiene y se muestra el nombre de la carpeta que se solapa. Si una carpeta se mueve o se elimina fuera de Skilvella, aparece una advertencia en la barra lateral y en la página de la carpeta, y la sincronización se detiene hasta resolverlo. Si la moviste, abre "Cambiar la ubicación de la carpeta" e introduce la nueva ubicación (o elígela con "Examinar…"). Solo se actualiza el registro; no se mueven archivos y se conservan los ID de las habilidades, el estado de publicación, el uso y el historial. Si ya no necesitas la carpeta, elige "Dejar de gestionar". Si la nueva ubicación ya se gestiona como otra carpeta, deja de gestionar primero esa carpeta.

Las habilidades nuevas encontradas al añadir, por detección automática o por ediciones externas se registran con la **publicación MCP activada** si se pueden leer. Las habilidades con errores de lectura o con frontmatter no válido permanecen sin publicar, y no se publican automáticamente ni siquiera después de corregirlas. El estado de publicación de las habilidades existentes no cambia al volver a escanear ni al iniciar (las habilidades que dejaste de publicar nunca se publican automáticamente). En Windows y macOS, una carpeta cuyo nombre solo difiere en mayúsculas y minúsculas ASCII se trata como la misma habilidad, y su estado de publicación no cambia. Si no quieres que una habilidad se publique, elige "No publicada" en la columna "Estado" de la lista después de añadir la carpeta. Elegir "Publicada" o "No publicada" en la columna "Estado" de la lista se guarda de inmediato. También puedes cambiarlo en la página individual o con las operaciones en lote de la lista. Al cambiar a publicada, los archivos se comprueban de nuevo, y las habilidades ausentes, con errores de lectura o con frontmatter no válido no se publican; la selección se revierte y se muestra el motivo. Las etiquetas y las categorías no cambian.

La biblioteca muestra desde el principio las habilidades de todas las carpetas y permite buscarlas ahí mismo. Una carpeta que tiene directamente dentro un directorio o archivo `.git` se muestra anidada bajo la carpeta como un origen (repositorio), y todo lo demás se agrupa en "Habilidades personales". Si una carpeta sin ningún repositorio de origen directamente dentro es ella misma un repositorio Git, la carpeta entera pasa a ser un origen. La lista normal muestra las habilidades que existen actualmente. Los registros antiguos, como los anteriores a un traslado, se conservan y se pueden revisar con el estado "No encontrada" o "Todos" y en "Limpiar registros de habilidades ausentes" a la izquierda. En la lista "No encontrada", "Eliminar registros ausentes" elimina los datos de registro, la configuración y el historial de todas las páginas que coinciden con las condiciones de búsqueda. Esto no se puede deshacer. No modifica los archivos reales, y cancela la eliminación de todas las entradas si no se puede acceder al origen o si existe una ruta. Las categorías se pueden editar desde los metadatos de cada habilidad.

Skilvella busca los archivos `SKILL.md` de las carpetas registradas **de forma recursiva y sin límite de profundidad**. Sigue escaneando subdirectorios más profundos incluso después de encontrar una habilidad. Las habilidades padre e hija se gestionan con ID y ajustes de publicación independientes, y los archivos de una hija no se incluyen en el ámbito de publicación MCP de la padre. Se excluyen los enlaces simbólicos, las uniones (junctions) de Windows, `.git`, `node_modules`, `target` y `.work`. Anular el registro de una carpeta no elimina los archivos originales. Si vuelves a añadir una carpeta que habías dejado de gestionar, se restauran sus datos de gestión anteriores (ajustes de publicación, etiquetas, historial). Las carpetas con el mismo nombre no entran en conflicto entre sí.

Al guardar el contenido se modifica el archivo real en la ubicación registrada. La vista previa desactiva el HTML, la carga de imágenes y las acciones de los enlaces, y no ejecuta scripts. Los cambios hechos en editores externos se vigilan automáticamente. Si no se reflejan, usa "Volver a escanear".

Cuando hay errores de lectura, abre la lista de errores de todas las carpetas desde "Revisar y corregir errores de lectura" en la biblioteca o desde "Errores de lectura" a la izquierda. Abre el nombre o la ruta de la habilidad para comprobar la causa y, en caso de errores de sintaxis, corrige el contenido y guarda. Si no se puede leer un archivo, comprueba su ubicación, sus permisos y su codificación de caracteres y vuelve a escanear. El aviso desaparece cuando se resuelve el error.

<a id="app-updates"></a>

## Actualizaciones de la aplicación

**Versión con instalador (Setup)**: las actualizaciones automáticas están activadas por defecto. Busca una versión nueva al iniciarse y cada 24 horas mientras sigue en ejecución, y la descarga si hay una disponible. La actualización se aplica automáticamente en el siguiente inicio, reiniciándose con la misma configuración. No se conecta dentro de las 24 horas posteriores a la comprobación anterior. Si falla una comprobación o una descarga, sigue ejecutando la versión actual. Puedes desactivarlo con "Actualizar automáticamente al iniciar" en "Actualizaciones de la aplicación" dentro de "Configuración" (el ajuste se guarda en `app-update.json` en la carpeta de datos, como `%LOCALAPPDATA%\Skilvella`, y no modifica la biblioteca de habilidades). "Buscar actualizaciones" comprueba sin tener en cuenta el intervalo de 24 horas, e "Instalar actualización" descarga la actualización y reinicia de inmediato. La información de actualización se obtiene de `skilvella.kagayoi.com`.

Lo siguiente se aplica a la versión independiente (ZIP) y a Mac.

Puedes comprobar la última versión estable en skilvella.kagayoi.com desde la sección de actualizaciones de "Configuración" o desde el elemento "Buscar actualizaciones" del menú de la bandeja. Al elegir "Instalar actualización", se descarga por HTTPS el paquete para Windows x64 / Mac Apple Silicon y `SHA256SUMS.txt`, se verifica el SHA-256 y después se aplica. Esto requiere el `curl` incluido en el sistema operativo y permiso de escritura en la ubicación del ejecutable. La actualización desde la propia aplicación no es compatible en Linux ni en otras CPU.

La actualización espera a que terminen las operaciones HTTP en curso antes de salir, y luego se reinicia con los mismos argumentos de inicio y la misma carpeta de trabajo. Vuelve a conectar tus clientes MCP. Si falla la verificación del paquete, se conserva la versión actual. Si no se puede iniciar el nuevo proceso después de aplicar la actualización, intenta restaurar el ejecutable antiguo o el destino de referencia de Toolport. La página de configuración que inició la actualización se recarga automáticamente cuando la nueva versión responde. Si realizas una operación de actualización en una página de configuración antigua que ya estaba abierta antes de que la aplicación se reiniciara, se te pide que recargues la página.

Se crea una carpeta `.skilvella-update-*` en la misma ubicación que el ejecutable previo a la actualización, y conserva `update.log`. En la disposición normal, también almacena `previous-binary` para la recuperación. Skilvella elimina esta carpeta automáticamente al iniciarse cuando tiene más de 24 horas, no hay ninguna actualización en curso y solo contiene archivos creados por el actualizador. No necesitas eliminarla tú mismo; si lo haces, espera a que el asistente de actualización haya terminado.

Con la gestión de `Skilvella-Latest.ps1` de Toolport, se conserva la versión existente, se añaden la carpeta y la información de verificación de la nueva versión, y se cambia el destino de referencia del lanzador. El asistente de actualización se ejecuta hasta que sale la aplicación reiniciada, así que cierra Skilvella antes de eliminar la carpeta de actualización.

Cuando Skilvella lo inicia un monitor de procesos externo, puede que debas reiniciarlo desde esa herramienta. Si no se puede obtener el bloqueo de inicio de Toolport, la actualización no se aplica. Si no se inicia, vuelve a ejecutar el lanzador. Una ubicación gestionada por Toolport que contenga caracteres no ASCII, como japonés, no admite la actualización desde la propia aplicación. La disposición independiente normal no está sujeta a esta limitación.

<a id="mcp-connection"></a>

## Conexión MCP

Registra la siguiente URL en un cliente compatible con Streamable HTTP.

```text
http://127.0.0.1:7465/mcp
```

En Toolport, regístrala como servidor HTTP y especifica la misma URL. Mantén Skilvella en ejecución.

| Herramienta | Finalidad |
| --- | --- |
| `search_skills` | Buscar habilidades publicadas y obtener resúmenes e ID |
| `load_skill` | Obtener el contenido de `SKILL.md` y la lista de archivos de referencia indicando un ID |
| `read_skill_file` | Obtener, por líneas, texto UTF-8 dentro de una habilidad |

Con `load_skill`, sigue pasando los argumentos de continuación hasta que `next` sea nulo, concatena el contenido y después sigue las instrucciones. La lista es normalmente de 100 entradas por página y hasta 8 KiB de rutas en total, pero si una sola ruta supera los 8 KiB, esa ruta se devuelve sola, por lo que la respuesta se vuelve grande. Obtén las habilidades y las referencias que necesitan de una en una. Para las referencias, usa `read_skill_file` para obtener `next_text_offset` del rango de líneas necesario hasta el final, y usa `next_start_line` solo cuando se necesiten más líneas. Las páginas de bytes del mismo rango de líneas se concatenan tal cual, y al concatenar entre rangos de líneas se insertan saltos de línea. Al continuar con `next_text_offset`, conserva `start_line` y `max_lines`; para el siguiente rango de líneas, establece `start_line` en `next_start_line` y restablece `text_offset` a 0. Los saltos de línea se normalizan como antes, y se elimina el salto de línea final. Para las continuaciones, especifica el `revision` devuelto como `expected_revision`. Si el archivo cambió mientras tanto, vuelve a obtenerlo desde el principio. `id` es el ID de skill devuelto por `search_skills`; también se acepta un nombre publicado exacto si es único, y un nombre ambiguo devuelve los ID candidatos. La primera llamada a `read_skill_file` solo necesita `id` y `path`; si el resultado se trunca, vuelve a llamar con el objeto `next`, que ya incluye `expected_revision`.

MCP es de solo lectura. No puede devolver el contenido de habilidades no publicadas, de habilidades con frontmatter no válido ni de habilidades ausentes. Tampoco hay ninguna función para ejecutar archivos de referencia. El contenido está limitado a 2 MiB y el texto de referencia a 1 MiB.

La publicación MCP por sí sola no cambia el hecho de que los agentes descubren y cargan de forma estándar las mismas habilidades. Para evitar la carga doble, coloca las habilidades fuera de las rutas de búsqueda estándar del agente y registra esa ubicación con "+ Añadir carpeta" de la biblioteca. Skilvella nunca mueve archivos. Si hay que mover archivos existentes, hazlo tú e indica la nueva ubicación con "Cambiar la ubicación de la carpeta" de esa carpeta. Los archivos de configuración de Codex no se editan. Reinicia Codex después de cambiar las rutas de búsqueda estándar. El contenido ya cargado en una conversación no se elimina. Comprueba por separado la configuración de búsqueda personalizada y otros clientes.

## Selección automática y sugerencias de comandos

Primero registra y verifica Skilvella con un cliente que use la [Conexión MCP](#mcp-connection). En "Configuración", en "Sincronización de puntos de invocación", marca los destinos de sincronización (Codex, Claude Code, Grok, Cursor, GitHub Copilot, Devin; se pueden elegir varios) y guarda, y la propia Skilvella sincroniza automáticamente los puntos de invocación de cada cliente. Los clientes cuya carpeta de configuración no se encuentra en este equipo (como `~/.cursor`) se pliegan en "Otros clientes" dentro del mismo formulario; los clientes activados o con un error de sincronización se muestran siempre. La tabla de estado solo muestra los clientes activados y los que tienen un error de sincronización. Si habías activado "integración con Codex" en una versión anterior, se traspasa con Codex seleccionado. Funciona igual en Windows, macOS y Linux y no usa el Programador de tareas ni ningún otro proceso residente aparte. La sincronización se ejecuta al iniciar, después de las operaciones de gestión y durante las comprobaciones periódicas, y el ajuste se conserva tras reiniciar el servicio.

| Destino de sincronización | Ubicación del punto de invocación | Cómo se expone a la búsqueda | Control de selección automática |
| --- | --- | --- | --- |
| Codex | `~/.agents/skills/skilvella/<name>` | Archivos reales (Codex busca en carpetas anidadas) | `agents/openai.yaml` |
| Claude Code | `~/.claude/skilvella/skills/<name>` | Un enlace a `~/.claude/skills/<name>` (una unión (junction) en Windows, un enlace simbólico en macOS y Linux) | `disable-model-invocation` |
| Grok | `~/.grok/skills/skilvella/<name>` | Archivos reales (Grok busca en carpetas anidadas) | `disable-model-invocation` |
| Cursor | `~/.cursor/skills/skilvella/<name>` | Archivos reales (Cursor busca en carpetas anidadas, pero puede no seguir enlaces) | `disable-model-invocation` |
| GitHub Copilot | `~/.copilot/skilvella/skills/<name>` | Un enlace a `~/.copilot/skills/<name>` | `disable-model-invocation` |
| Devin | `<config de Devin>/skilvella/skills/<name>` | Un enlace a `<config de Devin>/skills/<name>` | `triggers: [user]` |

`<config de Devin>` es `%APPDATA%\devin` en Windows y `~/.config/devin` en macOS y Linux (no se usa `XDG_CONFIG_HOME`). GitHub Copilot siempre usa `~/.copilot` y no sigue `COPILOT_HOME`.

Claude Code busca solo un nivel directamente bajo `~/.claude/skills`, así que los puntos de invocación se colocan fuera de la ruta de búsqueda y se crea un enlace por cada habilidad. Si en `~/.claude/skills` ya hay una habilidad o un enlace con el mismo nombre, no se sobrescribe y el nombre se muestra como error de sincronización. Un enlace que creaste a mano desde `~/.claude/skills/<name>` hacia `~/.agents/skills/skilvella/<name>` siguiendo las indicaciones de una versión antigua se redirige al punto de invocación de Claude Code y pasa a gestionarse, pero solo para los nombres que son destinos de sincronización. Los enlaces de nombres que no son destinos de sincronización se dejan intactos (si desactivas Codex, su destino de referencia desaparece). Al desactivar la sincronización de Claude Code solo se eliminan los enlaces que Skilvella creó o asumió. Si has registrado el propio `~/.claude/skills` como carpeta gestionada, no se realiza la sincronización.

Solo la entrada de Claude Code también conserva algunas claves del frontmatter del `SKILL.md` original, de modo que una skill que define `context: fork` (y `agent`, como `agent: Explore`) también puede ejecutarse como subagente bifurcado al invocarla mediante la entrada. Las claves que se conservan son `context` (solo `fork`), `agent` y `background` (solo junto con `context: fork`), `model`, `effort` (`low`, `medium`, `high`, `xhigh`, `max`), `argument-hint` y `paths`. Los valores se comprueban por tipo y solo se descarta la clave no válida. Las claves que cambian los permisos o toda la sesión (`allowed-tools`, `disallowed-tools`, `hooks`) no se conservan, y tampoco `shell`, `arguments`, `when_to_use`, `license` y similares; `disable-model-invocation` y `user-invocable` siguen el ajuste "Uso" de Skilvella. Las entradas de los demás clientes no cambian. Si la invocación lleva argumentos, la entrada indica al agente que los use como entrada de la skill cargada.

GitHub Copilot (la CLI de Copilot solo encuentra `~/.copilot/skills/<name>/SKILL.md`) y Devin usan el mismo método de enlaces y las mismas reglas: no se sobrescriben entradas existentes con el mismo nombre, solo se eliminan los enlaces que creó Skilvella y no se sincroniza si la carpeta de enlaces coincide con una carpeta gestionada. Solo Claude Code asume los enlaces a puntos de Codex creados a mano. No se ha confirmado si la CLI de Copilot respeta `disable-model-invocation` ni con qué profundidad busca Devin.

Cursor también lee `~/.agents/skills` y `~/.claude/skills`, GitHub Copilot lee `~/.agents/skills` y Devin lee `~/.agents/skills` y `~/.copilot/skills`, así que un mismo punto puede aparecer repetido si activas varios clientes.

Grok también lee las rutas de búsqueda de Codex (`~/.agents/skills`) y de Claude Code (`~/.claude/skills`), y para habilidades con el mismo nombre da prioridad en el orden `~/.grok/skills`, `~/.agents/skills`, `~/.claude/skills`. Las entradas de Codex detienen la selección automática con `agents/openai.yaml`, así que en Grok incluso las habilidades en "Solo sugerencias de comandos" se seleccionan automáticamente. Si usas Grok, selecciona Grok como destino de sincronización.

Elige el modo en la página de detalle de cada habilidad o con "Uso" en la lista. En la lista puedes comprobar el modo actual y cambiarlo ahí mismo. MCP no devuelve el contenido de las habilidades no publicadas, y tras una sincronización correcta también elimina sus entradas de sugerencia.

Si pulsas el botón Atrás del navegador o recargas mientras hay un guardado en curso en la lista, Skilvella comprueba el resultado del guardado antes de mostrar la configuración más reciente. Una solicitud que no se aceptó porque la conexión se interrumpió se cancela, y el resultado de la comprobación se muestra en pantalla.

| Modo | Nombre y descripción pasados para la selección automática | Sugerencias de comandos |
| --- | --- | --- |
| Solo búsqueda (predeterminado) | No se pasan | No se muestran |
| Solo sugerencias de comandos | No se pasan | Se muestran |
| Selección automática + sugerencias de comandos | Se pasan | Se muestran |

Para las habilidades que muestran sugerencias de comandos, el comando de invocación de cada destino de sincronización (`$name` para Codex, `/name` para los demás clientes) se muestra bajo el nombre de la habilidad en la lista y en "Uso" en la página de detalle. Si el nombre contiene algo distinto de letras minúsculas, dígitos y `-`, pasa a ser `skill-<first 8 characters of the id>`, y si existe una habilidad publicada con el mismo nombre, pasa a ser `<name>-<first 8 characters of the id>`, así que usa el comando que se muestre. Cuando no hay comando, la página de detalle muestra el motivo (solo búsqueda, ningún destino de sincronización seleccionado, no publicada o error de lectura). El comando de una habilidad que cambias de no publicada a publicada en la lista aparece tras recargar.

En la lista, marca habilidades, elige "Uso" en las operaciones en lote y pulsa "Aplicar a la selección" para cambiar a la vez el modo de varias habilidades. "Sin cambios" conserva el modo actual. Puedes cambiar al mismo tiempo el estado de publicación y las etiquetas.

Al abrir "Configuración en bloque por origen" en "Todas las habilidades" puedes seleccionar varios orígenes y cambiar a la vez el estado de publicación y el uso con "Aplicar a los orígenes seleccionados". El ámbito son las habilidades de todas las páginas registradas en el momento de la ejecución y no se traspasa a las habilidades añadidas después. El recuento previo se basa en el índice de escaneo y excluye las entradas ausentes y con errores. El resultado muestra el número de actualizadas y el número de excluidas. Al publicar, los archivos se comprueban de nuevo, y si no se pueden volver a validar porque faltan, no se pueden leer, no son válidos o algo similar, se cancela el guardado de la configuración de todas las entradas. La configuración se guarda en una sola transacción, y las habilidades originales no se modifican.

Codex no tiene un ajuste independiente para "activar la selección automática pero ocultarla de las sugerencias", así que Skilvella ofrece los tres modos correspondientes. Incluso con selección automática, no mantiene en todo momento una copia completa del contenido original; el contenido se obtiene de MCP cuando hace falta. Las habilidades que no se muestran en las sugerencias se pueden buscar con `$skilvella-search`, que es solo para invocación explícita. También puedes usar directamente `search_skills` de MCP.

El modo de uso es común a todos los destinos de sincronización. Las ubicaciones de los puntos de invocación son las de la tabla anterior. Cuando dejas de publicar una habilidad o cambias su modo, los artefactos que ya no hacen falta se mueven fuera de las rutas de búsqueda (`skilvella-staging` / `skilvella-retired` directamente bajo `~/.agents`, `~/.claude`, `~/.grok`, `~/.cursor`, `~/.copilot` y `<config de Devin>`). Si se detectan archivos desconocidos o ediciones manuales, no se sobrescriben y se muestra un error de sincronización en la página de configuración. Si quedan archivos temporales incompletos por falta de espacio en disco o una salida forzada, se detiene protegiendo el índice existente que está en buen estado. Comprueba el contenido de la ruta mostrada y vuelve a intentarlo. Al desactivar la integración se eliminan de las rutas de búsqueda los puntos de invocación que le pertenecen. Mientras la sincronización esté detenida, pueden permanecer sugerencias y ajustes de selección automática anteriores. Los archivos originales, los estados de publicación y el historial de ediciones no cambian. Si la visualización de sugerencias de Codex no se actualiza, vuelve a abrir el menú de sugerencias o reinicia Codex. Claude Code refleja los cambios en `~/.claude/skills` incluso en una sesión en ejecución, pero si creaste el propio `~/.claude/skills` después de iniciar la sesión, ejecuta `/reload-skills`. El contenido ya cargado en una conversación existente no se puede eliminar.

<a id="display-settings"></a>

## Ajustes de visualización

En la biblioteca, selecciona una carpeta a la izquierda y abre "Cambiar nombre visible" para cambiar el nombre de la carpeta que se muestra en pantalla. La entrada debe tener como máximo 100 caracteres incluidos los espacios iniciales y finales, y no se permiten caracteres de control como los saltos de línea. Los espacios iniciales y finales se recortan al guardar, y un valor vacío vuelve al identificador original. El nombre visible se guarda en la aplicación, y el nombre real de la carpeta, los ID de las habilidades, los ajustes de publicación y el historial de ediciones no cambian. Se permiten nombres visibles duplicados, pero un nombre que hayas cambiado se muestra junto con su identificador original.

En "Configuración" del gestor puedes elegir el idioma como automático o uno de nueve idiomas (japonés, inglés, chino simplificado, chino tradicional, coreano, español, francés, alemán, portugués de Brasil). El valor predeterminado "Automático" usa el idioma configurado en el navegador. El idioma se guarda al seleccionarlo, y si JavaScript está desactivado, pulsa "Guardar". La selección se guarda por puerto de conexión en el mismo navegador, y las demás pestañas que ya estén abiertas la aplican a partir de la siguiente página que carguen. Un ajuste de idioma de una versión anterior se traspasa al abrir el gestor en la misma dirección en el mismo navegador. El traspaso requiere JavaScript. Los nombres de las habilidades, las descripciones, el contenido, las rutas y los detalles técnicos de los errores conservan su texto original. Los menús de la bandeja y de la barra de menús, la salida de la línea de comandos como `--help`, los mensajes de inicio y los errores, y los cuadros de mensaje de la versión con instalador (Setup) también se muestran en los mismos nueve idiomas. Estos idiomas se determinan en el orden de la variable de entorno `SKILVELLA_LANGUAGE` (por ejemplo `ja`, `pt-BR`) → `LC_ALL` → `LC_MESSAGES` → `LANG` → el idioma de visualización del sistema operativo, y si no se puede determinar, se usa inglés. Los nombres y las descripciones de las herramientas MCP están en inglés.

Puedes elegir el tema entre Automático, Claro y Oscuro cuando JavaScript está activado. La selección se guarda en el navegador, y Automático sigue el ajuste de esquema de color del navegador.

## Datos y copias de seguridad

| SO | Ubicación de datos predeterminada |
| --- | --- |
| Windows | `%LOCALAPPDATA%\Skilvella` |
| macOS | `~/Library/Application Support/Skilvella` |
| Linux | `$XDG_DATA_HOME/skilvella` o `~/.local/share/skilvella` |

La fuente de verdad de `SKILL.md` y de los archivos de referencia son las carpetas registradas. Además del índice de búsqueda, el archivo `library.sqlite3` de la carpeta de datos también almacena etiquetas, categorías, ajustes de publicación, ubicaciones registradas, nombres visibles de las carpetas e historial de ediciones. **Si eliminas toda la base de datos, estos datos de gestión se pierden.**

Para hacer una copia de seguridad, cierra primero Skilvella y después copia tanto la carpeta de datos como las carpetas registradas. La base de datos por sí sola no puede restaurar los archivos de habilidades actuales. En otro PC las rutas absolutas de los orígenes registrados son distintas, así que no se admite compartir la base de datos tal cual para un uso simultáneo.

## Opciones y solución de problemas

```sh
skilvella --data-dir /absolute/path/to/data --port 7465 --no-watch
skilvella --help
```

- Si el puerto está en uso, cierra el proceso existente o cámbialo con `--port`. El gestor y MCP usan el mismo puerto.
- Si el gestor no vuelve tras actualizar desde una versión antigua de Windows, inicia una vez el `skilvella.exe` actualizado en la disposición normal, o el lanzador una vez con la gestión de Toolport.
- Si un traslado de carpeta interrumpido de una versión antigua no se puede recuperar al iniciar, el inicio se aborta. Se conservan los datos originales y el registro de migración.
- La ubicación de los datos y las ubicaciones de registro de habilidades no pueden solaparse. Especifica carpetas separadas en un disco local.
- Las ediciones hechas en otra pestaña también se detectan como conflictos al guardar. Copia el borrador conservado, recarga y combínalo con el contenido más reciente.
- La búsqueda usa AND separado por espacios y coincidencia de subcadenas. No se incluyen la búsqueda por significado de habilidades en inglés desde una consulta en japonés, los modelos de embeddings ni la sincronización en la nube.
- Aunque la carpeta de origen quede sin conexión, se conservan los datos de registro y el historial. Vuelve a escanear después de que se reconecte.

Las conexiones están limitadas a `127.0.0.1`. Está pensada para usarse donde confíes en los usuarios y procesos del mismo PC.

La licencia es [MIT](LICENSE).

Si quieres contribuir al desarrollo, consulta la [guía de desarrollo](docs/DEVELOPMENT.md), el [diseño](DESIGN.md) y las [pautas para colaboradores](AGENTS.md).
