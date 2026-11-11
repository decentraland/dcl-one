> Crea escenas de Decentraland con asistentes de IA usando Creator Hub y las SDK Skills oficiales de Decentraland (npx skills add decentraland/sdk-skills).

# Vibe Coding con IA

Crea escenas de Decentraland describiendo lo que quieres. Un asistente de IA se encarga del código de SDK7, la arquitectura ECS y la estructura del proyecto por ti.

Tanto si eres un creador primerizo como un desarrollador experimentado, el "vibe coding" asistido por IA te permite pasar de una idea a una escena en ejecución en minutos en lugar de horas.


> [!NOTE]
> **💡 Consejo**: No necesitas saber TypeScript para empezar. Los asistentes de IA pueden generar código de escena funcional a partir de descripciones en lenguaje natural.


## ¿Qué es el Vibe Coding?

El vibe coding consiste en construir escenas teniendo una conversación con un asistente de IA en lugar de escribir cada línea de código a mano. Describe lo que quieres — "una taberna medieval con puertas clicables y música de fondo" — y la IA escribe código correcto y desplegable de SDK7.

Este enfoque funciona en cualquier nivel de habilidad:

* **Principiantes y no desarrolladores** — Pasa de cero a una escena funcional sin escribir código manualmente.
* **Desarrolladores experimentados** — Omite el boilerplate. Deja que la IA se encargue de la sincronización multijugador, la estructura de la UI y la configuración de despliegue mientras tú te concentras en las decisiones creativas.
* **Equipos y estudios** — Prototipa rápidamente conceptos de escenas antes de comprometer recursos completos de desarrollo.

## Usa el asistente de IA en Creator Hub

La forma más rápida de empezar es el integrado de Creator Hub [Asistente de IA](/docs/creator/content-creator-es/scene-editor/extender-con-codigo/ai-assistant/). Ve tu escena abierta, edita entities, components, smart items y código por ti, y puede ejecutar la preview para comprobar su propio trabajo. Instala automáticamente en tu proyecto las habilidades de abajo, así que no hay nada que configurar más allá de iniciar sesión en una CLI de código.

Es una función experimental, desactivada de forma predeterminada.

## Combina un editor de código con IA

Usa un editor de código de uso general con IA como [Cursor](https://www.cursor.com/) o VS Code con GitHub Copilot o Claude AI. Decentraland proporciona una carpeta de contexto para que estas herramientas entiendan el SDK.

1. Abre Creator Hub y crea o abre una escena.
2. Haz clic en el botón **< > CODE** para abrir tu editor de código.
3. Usa el asistente de IA integrado del editor (el chat de Cursor, Copilot, etc.) para generar o modificar código.

## Instala Skills para cualquier agente de IA

Las Skills son conjuntos de instrucciones listos para usar que enseñan a tu agente de IA cómo trabajar con el SDK de Decentraland. Cada skill cubre un tema específico, como crear escenas, añadir modelos 3D o configurar multijugador, así que la IA ya conoce los patrones, APIs y restricciones correctos sin que tengas que explicárselos. Instalar skills significa menos errores y mejores resultados desde el primer prompt.

```bash
# Elige qué skills de Decentraland instalar desde un selector interactivo
npx skills add decentraland/sdk-skills

# O instala todas las skills de Decentraland
npx skills add decentraland/sdk-skills --all

# O elige skills específicos
npx skills add decentraland/sdk-skills --skill create-scene

# Instalar globalmente (disponible en todos los proyectos)
npx skills add decentraland/sdk-skills -g
```

Esto copia los archivos de skill en la configuración de tu agente para que conozca los patrones y las restricciones de Decentraland.

## Actualizando Skills

Con el tiempo se añaden nuevas skills y las existentes se mejoran. Para obtener las últimas versiones, vuelve a ejecutar el comando de instalación con `--all`:

```bash
# Actualiza cada skill instalada y descarga cualquier nueva
npx skills add decentraland/sdk-skills --all
```

Ejecutar `add` de nuevo vuelve a obtener el repositorio, así que actualiza las skills que ya tienes e instala las que se hayan añadido desde tu primera instalación. Si instalaste skills globalmente, añade `-g` a este comando también.


> [!WARNING]
> **📔 Nota**: No uses `npx skills update` para esto. Solo actualiza las skills que ya están en tu máquina, así que cualquier skill añadida al repositorio después de tu última instalación se omite sin avisar. Usa siempre `npx skills add decentraland/sdk-skills --all` en su lugar.


## Depurar una escena multijugador desplegada

La skill actualizada `authoritative-server` puede buscar en los registros persistentes del servidor, seguir los logs durante un período acotado y recuperar CPU, memoria y otras métricas cuando le pides que investigue una escena desplegada. Actualiza tus skills y primero el SDK de la escena `auth-server` ; consulta [Scene Insights](/docs/creator/content-creator-es/scenes-sdk7/depuracion/scene-insights/) para la configuración, los permisos y los límites.

Por ejemplo, pregunta:

> Investiga errores y crecimiento de CPU o memoria en mi escena multijugador desplegada durante las últimas dos horas. Usa Scene Insights y correlaciona los logs y las métricas con el código fuente.

Tú apruebas el inicio de sesión de la wallet en el navegador. Después, el asistente puede reutilizar la sesión de solo lectura; no necesita tu clave privada ni el archivo de sesión.

## Skills de IA disponibles

Cuando instalas skills en tu agente, están disponibles las siguientes capacidades:

| Skill                  | Qué hace                                                                               |
| ---------------------- | -------------------------------------------------------------------------------------- |
| `sdk-scenes`           | Punto de entrada con las directrices del agente e índice de todas las skills temáticas |
| `create-scene`         | Estructura un nuevo proyecto de escena de SDK7 desde cero                              |
| `migrate-sdk6-to-sdk7` | Convierte una escena heredada de SDK6 a SDK7                                           |
| `add-3d-models`        | Añade modelos 3D (`.glb`/`.gltf`) con posicionamiento, escalado y colliders            |
| `add-interactivity`    | Pointer events, triggers, raycasts                                                     |
| `build-ui`             | UI en espacio de pantalla 2D con React-ECS — HUDs, menús, diálogos                     |
| `animations-tweens`    | Animaciones de modelos GLTF con Animator, tweens de SDK                                |
| `multiplayer-sync`     | Multijugador peer-to-peer usando red CRDT                                              |
| `authoritative-server` | Servidor multijugador sin interfaz para escenas validadas por servidor (BETA)          |
| `audio-video`          | Efectos de sonido, música, streaming de audio y reproductores de video                 |
| `audio-analysis`       | Datos de amplitud y frecuencia en tiempo real para escenas reactivas al audio          |
| `deploy-scene`         | Despliega escenas en Genesis City (basado en LAND)                                     |
| `deploy-worlds`        | Despliega escenas en Worlds (espacios 3D personales)                                   |
| `optimize-scene`       | Optimización del rendimiento, límites de escena, mejores prácticas                     |
| `camera-control`       | Detección de modo de cámara, cámara cinematográfica, cámaras virtuales                 |
| `composites`           | Referencia del formato de archivo composite para contenido estático de la escena       |
| `lighting-environment` | Iluminación dinámica, sombras, skybox, niebla, ajustes del entorno                     |
| `particle-system`      | Efectos de partículas — fuego, humo, chispas, nieve, fuegos artificiales               |
| `npcs`                 | Personajes no jugadores — biblioteca NPC Toolkit y enfoques manuales                   |
| `player-avatar`        | Posición del jugador, perfil, personalización del avatar, attachments                  |
| `player-physics`       | Fuerzas físicas — impulsos, retroceso, fuerzas continuas                               |
| `nft-blockchain`       | Visualización de NFT e interacciones con blockchain/crypto                             |
| `advanced-rendering`   | Billboard, TextShape, materiales PBR, materiales de video                              |
| `advanced-input`       | Sondeo de entrada a nivel de System y control del movimiento del jugador               |
| `scene-runtime`        | APIs de runtime transversales — trabajo asíncrono, HTTP, mensajería                    |
| `script-components`    | Clases de script component para Creator Hub                                            |
| `game-design`          | Patrones de diseño de juegos, límites de escena, presupuestos de rendimiento           |
| `unity-explorer-mcp`   | Controla un Explorer en ejecución para probar y verificar una escena en el mundo       |

Nota: Algunas de estas skills implican obtener modelos 3D u otros assets de catálogos de assets gratuitos. El agente de IA siempre debe obtener confirmación del usuario antes de descargar cualquier asset nuevo a un proyecto de escena.

## Deja que la IA vea tu escena en el mundo

Normalmente la IA escribe código y *tú* ejecutas la preview, la miras y vuelves para decir qué está mal. El cliente de escritorio de Decentraland puede cerrar ese ciclo: incluye un **servidor MCP** opcional que permite a un agente de IA mirar y controlar directamente el Explorer en ejecución. El agente toma sus propias capturas, lee la salida de la consola de la escena, mueve al jugador por el entorno, hace clic en objetos y comprueba si la escena realmente hace lo que se le pidió construir; después corrige lo que encuentra y vuelve a mirar.

Esto convierte el vibe coding de "describe, espera, revisa" en un ciclo que el agente puede ejecutar en gran parte por su cuenta.


> [!NOTE]
> **💡 Consejo**: Instala la `unity-explorer-mcp` skill antes de probar esto. Le enseña al agente todo el flujo de trabajo — cómo iniciar el cliente, cómo encuadrar capturas útiles, cómo comparar lo que ve con el estado real de la escena y cómo recuperarse cuando la escena deja de cargar.
> 
> ```bash
> npx skills add decentraland/sdk-skills --skill unity-explorer-mcp
> ```
> 


### Lo que necesitas

* El **cliente de escritorio de Decentraland** instalado (el mismo que Creator Hub lanza para las previews).
* Un agente de IA que pueda conectarse a servidores MCP por HTTP — Claude Code, Cursor, Cline, VS Code con una extensión compatible con MCP y otros.
* Un SDK actualizado en tu escena: ejecuta `npm i @dcl/sdk@latest` si la bandera `--mcp` de abajo se rechaza como una opción desconocida.

### 1. Inicia la escena con el servidor MCP habilitado

Desde la carpeta de tu escena:

```bash
npm run start -- --mcp
```

Esto hace lo que `npm run start` siempre hace — sirve tu escena en `http://127.0.0.1:8000` y la recarga en caliente cada vez que guardas un archivo, y además inicia el cliente de escritorio con el servidor MCP escuchando en `http://127.0.0.1:8123/unity-explorer-mcp`.

Inicia sesión cuando se abra el cliente. El agente solo puede empezar a trabajar una vez que hayas pasado la pantalla de inicio de sesión y el mundo se haya cargado.

### 2. Conecta tu agente de IA al servidor

En **Claude Code**, regístralo una vez:

```bash
claude mcp add --transport http --scope user explorer http://127.0.0.1:8123/unity-explorer-mcp
```

En **cualquier otro cliente MCP**, añade el servidor de la forma en que lo documente ese cliente, usando estos detalles:

| Configuración | Valor                                                                                                                |
| ------------- | -------------------------------------------------------------------------------------------------------------------- |
| Transporte    | HTTP con streamable (no stdio: no hay ningún comando que ejecutar, el servidor vive dentro del cliente en ejecución) |
| URL           | `http://127.0.0.1:8123/unity-explorer-mcp`                                                                           |
| Autenticación | Ninguna                                                                                                              |
| Nombre        | `explorer`                                                                                                           |

Muchos clientes usan un archivo de configuración JSON para esto (`.cursor/mcp.json`, `mcp.json`y similares — revisa la documentación de tu cliente para conocer los nombres exactos de las claves):

```json
{
  "mcpServers": {
    "explorer": {
      "type": "http",
      "url": "http://127.0.0.1:8123/unity-explorer-mcp"
    }
  }
}
```

Reinicia o recarga tu cliente de IA después de registrar el servidor, para que detecte la conexión. Si el agente dice que las herramientas de Explorer no están disponibles, la causa habitual es que el cliente no estaba en ejecución cuando el agente arrancó — vuelve a conectar el servidor (en Claude Code, ejecuta `/mcp`) con el Explorer abierto.


> [!NOTE]
> **¿Es seguro esto?** El servidor solo se ejecuta mientras inicias el cliente con `--mcp`, solo acepta conexiones desde tu propia máquina (`127.0.0.1`), y rechaza solicitudes procedentes de páginas web. Nada queda expuesto a internet, y está completamente desactivado en un inicio normal del cliente.


### 3. Pide lo que quieres y deja que lo verifique

Con el servidor conectado, pide el trabajo como lo harías normalmente; la diferencia es que ahora el agente puede comprobar sus propios resultados:

> "Añade un cofre del tesoro en el centro de la parcela que se abra al hacer clic, luego acércate y haz clic en él para confirmar que la tapa se anima."

> "El letrero de neón se ve demasiado tenue. Haz una captura, ajusta la intensidad emisiva y muéstrame un antes/después."

> "Algo falla con el ascensor. Sube a la plataforma, observa los logs y dime por qué no se mueve."

Entre bastidores, el agente puede:

* **Ver** — hacer capturas, leer la `console.log` salida y los errores, comprobar si la escena cargó o se bloqueó, listar las entities de la escena e inspeccionar sus components, y leer la posición del jugador y de la cámara.
* **Controlar** — mover y teletransportar al jugador, caminar en una dirección mediante colisiones reales, apuntar la cámara, colocar una cámara libre para una toma específica, cambiar los modos de cámara, hacer clic en objetos de la escena, enviar mensajes de chat y `/commands`, activar emotes y recargar la escena.
* **Medir** — leer recuentos en vivo del contenido de la escena (triángulos, entities, texturas, materials y más) frente a los límites de Decentraland, clasificar qué modelos 3D contribuyen más a esa carga desde cualquier punto de vista, y muestrear la tasa de fotogramas real que experimenta el jugador en cualquier lugar.

### Úsalo para optimizar tu escena

Como el agente puede medir además de ver, puedes encargarle directamente trabajo de rendimiento. Puede comprobar tu escena frente a los [límites de la escena](/docs/creator/content-creator-es/scenes-sdk7/optimizacion/scene-limitations/), identificar qué modelos 3D son los más pesados, medir la tasa de fotogramas en puntos específicos y, lo más importante, volver a medir después de hacer cambios para demostrar que ayudaron:

> "Comprueba si mi escena está dentro de los límites de contenido de Decentraland y dime qué presupuesto está más cerca del tope."

> "La escena se siente entrecortada cerca de la fuente. Ponte ahí, mide la tasa de fotogramas y dime qué modelos cuestan más desde ese punto."

> "Optimiza la escena: encuentra qué está reduciendo la tasa de fotogramas, arréglalo y muéstrame las mediciones antes y después."

Para obtener los mejores resultados, instala la `optimize-scene` skill junto con `unity-explorer-mcp`, para que el agente también conozca las mejores prácticas de optimización de Decentraland. Consulta [Optimización del rendimiento](/docs/creator/content-creator-es/scenes-sdk7/optimizacion/performance-optimization/#optimize-with-ai) para más información sobre este flujo de trabajo, incluido cómo combinarlo con el servidor MCP de Blender para automatizar correcciones en los propios modelos 3D.

### Consejos

* **Pide pruebas, no afirmaciones.** "Verifica con una captura" o "confírmalo desde los logs" es lo que hace que este flujo de trabajo valga la pena. Un buen agente comprueba ambos: los píxeles pueden verse bien mientras el estado subyacente está roto, y viceversa.
* **Las capturas consumen tokens.** Cada captura que el agente mira consume parte de su contexto. Si quieres un recorrido visual largo — una animación en el tiempo, un recorrido por muchos puntos — pídele que capture fotogramas en archivos y lea solo los que importan. La `unity-explorer-mcp` skill incluye un script que hace exactamente esto.
* **Guarda una vez, no cinco veces seguidas.** Los guardados rápidos y sucesivos pueden hacer que el cliente cargue un bundle a medio escribir y haga caer la escena por completo, lo que requiere reiniciar el cliente para recuperarla. Pídele al agente que agrupe sus ediciones en un único guardado.
* **Mantén el cliente abierto.** Si lo cierras, la conexión se pierde y el agente se queda sin ojos. Volver a iniciarlo con el mismo comando lo recupera.
* **Los teleports se comportan de manera diferente en el desarrollo local de escenas.** Moverse entre parcelas con `/goto` no está permitido allí, así que el agente debe reposicionar al jugador dentro de la escena en lugar de teletransportarlo.

## Consejos para hacer prompts efectivos

Obtener los mejores resultados de la IA consiste en dar prompts claros y específicos. Aquí tienes algunos consejos:

### Sé específico sobre lo que quieres

En lugar de:

> "Haz mejor mi escena"

Prueba:

> "Añade una puerta en la posición (8, 0, 8) que se abra con una animación de rotación al hacer clic y reproduzca un efecto de sonido de chirrido"

### Haz referencia a elementos existentes

> "Haz que el botón rojo de la mesa active que el ascensor suba"

### Pide una cosa a la vez

Desglosa las solicitudes complejas en pasos:

1. "Añade una UI de marcador en la esquina superior derecha"
2. "Añade un contador que aumente cuando el jugador haga clic en el objetivo"
3. "Muestra el valor del contador en el marcador"

### Itera y refina

Después de cada cambio:

1. Previsualiza la escena (haz clic en **Play** en Creator Hub, o `npm run start` en la línea de comandos)
2. Comprueba qué funciona y qué no
3. Dile a la IA qué ajustar: "Mueve el NPC 2 metros a la izquierda y haz que mire al jugador"

### Prompts de ejemplo

Para ejecutarse sobre una clonación limpia de [sdk7-scene-template](https://github.com/decentraland/sdk7-scene-template/).

#### Ejemplo de prompt 1

```
Quiero que descartes el código actual de la escena y hagas un pequeño juego de laberinto simple, en 1 parcela, las paredes pueden ser cubos y deben tener colisión.

El juego debe empezar cuando el jugador entre realmente en el laberinto por el punto A y debe terminar cuando salga por el punto B (la única salida del laberinto).

Quiero que añadas 3 puertas diferentes en el laberinto, que tengan que interactuar con la entrada de puntero para abrirlas y pasar.

Debes verificar que el jugador realmente puede jugar y ganar
```

#### Ejemplo de prompt 2

```
Quiero que descartes el código actual de la escena y construyas un juego de plataformas simple.

Tienes que verificar que el juego se puede ganar antes de considerarlo terminado.
```

## En qué puede ayudar la IA

* Estructurar nuevas escenas a partir de una descripción
* Añadir y posicionar modelos 3D
* Escribir controladores de clic e interactividad
* Construir UI (HUDs, menús, diálogos)
* Configurar la sincronización multijugador
* Configurar el Servidor Multijugador para anti-cheat
* Añadir audio, video y streaming
* Crear animaciones y tweens
* Optimizar el rendimiento de la escena
* Preparar escenas para el despliegue
* Depurar problemas en el código existente
* Probar y verificar visualmente una escena en un Explorer en ejecución (consulta [Deja que la IA vea tu escena en el mundo](#let-the-ai-see-your-scene-in-world))

## Limitaciones

Aunque las herramientas de IA son potentes, ten en cuenta lo siguiente:

* **Previsualiza siempre** — el código generado por IA puede no verse exactamente como esperas. Ejecuta una vista previa para verificarlo.
* **Los límites de la escena siguen aplicando** — la IA no puede saltarse los límites de Decentraland [límites de la escena](/docs/creator/content-creator-es/scenes-sdk7/optimizacion/scene-limitations/) (conteo de triángulos, tamaños de archivos, límites de parcelas).
* **Lógica de juego compleja** — Para mecánicas de juego intrincadas, quizá tengas que guiar a la IA paso a paso o refinar su resultado manualmente.
* **Modelos 3D personalizados** — La IA puede usar assets gratuitos existentes o cargar modelos que proporciones, pero no puede crear modelos 3D desde cero (a menos que uses otras herramientas como el servidor MCP oficial de Blender al mismo tiempo).

## Siguientes pasos

* [Inicio rápido de SDK](/docs/creator/content-creator-es/scenes-sdk7/primeros-pasos/sdk-101/) — Aprende los fundamentos de SDK7
* [Combina con código](/docs/creator/content-creator-es/scene-editor/extender-con-codigo/overview/) — Mezcla la edición visual con código
* [Servidor multijugador](/docs/creator/content-creator-es/scenes-sdk7/networking/authoritative-servers/) — Multijugador con servidor autoritativo
* [Ejemplos de escenas](/docs/creator/content-creator-es/tutoriales-y-ejemplos/examples/) — Explora mecánicas de escena y repositorios reutilizables
* [Recursos útiles](/docs/creator/content-creator-es/scenes-sdk7/primeros-pasos/useful-resources/) — Más herramientas de IA, bibliotecas de assets y add-ons para acelerar tu flujo de trabajo


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-es/scenes-sdk7/primeros-pasos/vibe-coding.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
