> Crea y cambia tu escena chateando con un asistente de IA dentro de Creator Hub.

# Asistente de IA

Creator Hub puede ejecutar un asistente de IA que ve tu escena abierta y la edita por ti. Pídele que coloque objetos, conecte comportamientos, escriba scripts o ejecute la vista previa y compruebe el resultado.


> [!WARNING]
> **📔 Nota**: El asistente ejecuta una CLI de programación que instalas e inicias sesión en ella, con acceso completo a tu máquina, el mismo alcance que tiene en tu terminal. Actívalo solo si confías en lo que le pides que haga.


## Dónde están sus ajustes

El asistente está activado de forma predeterminada. Sus ajustes tienen su propia pestaña:

1. Haz clic en el logotipo de Creator Hub en la esquina superior izquierda y selecciona **Configuración**.
2. Ve a la **AI** pestaña.

Esa pestaña contiene el **Activar asistente de IA** interruptor, la **Pagar esto con una clave de API en su lugar** opción, la **Conectar** lista de CLIs de programación, y las **Exponer el servidor MCP del asistente de IA** ajustes.

Cuando el asistente está activado, un **Asistente de IA** botón (el icono de destellos) aparece en la barra superior del editor, a la izquierda del **Code** botón. Haz clic en él para abrir el panel de chat junto a tu escena.

![La barra superior del editor, con el nombre de la escena a la izquierda y el icono de destellos de Asistente de IA, y los botones Code, Preview y Publish a la derecha.](/docs/_remote/97b2496489d20fd6596ad9b5.webp)

![El panel de Asistente de IA al abrirse por primera vez: un menú desplegable Nueva conversación y un menú desplegable de CLI que muestra Claude en la parte superior, un mensaje de bienvenida, una tarjeta Acerca del uso de IA, una ficha Selección con el nombre de la entity seleccionada y el campo Preguntar al asistente en la parte inferior.](/docs/_remote/dceaca65cd87dab45134934a.webp)

## Conecta una CLI de programación

El asistente no usa una clave de API propia. Ejecuta una CLI de programación que instalas e inicias sesión en ella, por lo que el trabajo se cobra a la suscripción de esa herramienta.

Se admiten cuatro: Claude, Codex, Cursor y Gemini. Instalas cada uno con el instalador de su proveedor, desde una terminal.

### Instalar una CLI

Ejecuta el comando para tu sistema operativo:

| CLI    | macOS y Linux                                           | Windows                                                                                      |
| ------ | ------------------------------------------------------- | -------------------------------------------------------------------------------------------- |
| Claude | `curl -fsSL https://claude.ai/install.sh \| bash`       | `winget install Anthropic.ClaudeCode`                                                        |
| Codex  | `curl -fsSL https://chatgpt.com/codex/install.sh \| sh` | `powershell -ExecutionPolicy ByPass -c "irm https://chatgpt.com/codex/install.ps1 \| iex"`   |
| Cursor | `curl https://cursor.com/install -fsS \| bash`          | `powershell -ExecutionPolicy ByPass -c "irm 'https://cursor.com/install?win32=true' \| iex"` |
| Gemini | `npm install -g @google/gemini-cli`                     | `npm install -g @google/gemini-cli`                                                          |

No tienes que copiarlos a mano. Creator Hub muestra los comandos exactos para tu sistema operativo en dos lugares, junto con un enlace a la guía de instalación de ese proveedor:

* La tarjeta de configuración en el panel de chat, que aparece la primera vez que abres el panel sin ninguna CLI conectada.
* Los ajustes, en la **AI** pestaña debajo de **Conectar**, en la **Vía terminal** sección debajo de cada CLI. Cada comando allí tiene un **Copiar** botón.

Las guías del proveedor:

* [Guía de instalación de Claude Code](https://code.claude.com/docs/en/terminal-guide)
* [Guía de instalación de Codex CLI](https://developers.openai.com/codex/cli)
* [Guía de instalación de Cursor CLI](https://cursor.com/docs/cli/installation)
* [Guía de instalación de Gemini CLI](https://www.geminicli.com/docs/get-started/installation)

Después de instalarlo, haz clic en **Volver a comprobar** en la tarjeta de configuración del panel de chat para que Creator Hub detecte la nueva CLI, sin necesidad de reiniciar la aplicación. La **AI** pestaña vuelve a comprobarlo por sí sola cada vez que la abres.


> [!WARNING]
> **📔 Nota**: Claude Desktop no es lo mismo que Claude Code. El asistente necesita Claude Code, la herramienta de terminal de Anthropic, y iniciar sesión en la aplicación de escritorio no te inicia sesión en él. Si Creator Hub encuentra Claude Desktop sin Claude Code, te lo indica y te muestra el comando de instalación.
> 
> Gemini CLI necesita una clave de API de Gemini de pago, o una licencia Gemini Code Assist Standard o Enterprise.


### Iniciar sesión

| CLI    | Iniciar sesión                            |
| ------ | ----------------------------------------- |
| Claude | `claude auth login`                       |
| Codex  | `codex login`                             |
| Cursor | `cursor-agent login`                      |
| Gemini | ejecuta `gemini` y sigue las indicaciones |

Ejecuta el comando de inicio de sesión en una terminal. Termina en tu navegador.

Codex tiene un atajo: si aún no está instalado, Creator Hub puede instalarlo e iniciar sesión por ti. Haz clic en **Iniciar sesión con Codex** en la tarjeta de configuración del panel de chat, o **Iniciar sesión** junto a Codex, debajo de **Conectar** en la **AI** pestaña. Creator Hub ejecuta el inicio de sesión y abre tu navegador, y aparece un **Cerrar sesión** enlace en el mismo lugar. Si instalaste Codex tú mismo con el comando anterior, no hay botón: inicia sesión desde una terminal como los demás.

Un punto verde y **Conectado** junto a una CLI significa que está listo para usar.


> [!NOTE]
> **💡 Consejo**: Si tienes más de una CLI con sesión iniciada, un menú desplegable en el panel de chat te permite cambiar entre ellas.


### Cuando un agente te pide que inicies sesión de nuevo

Un inicio de sesión guardado puede caducar aunque la CLI siga pareciendo conectada. Cuando una CLI responde a un mensaje con "not logged in", Creator Hub muestra un **Se necesita iniciar sesión** estado con un punto ámbar en lugar de pasar la línea de comandos en bruto al chat.

El panel de chat y la **Vía terminal** sección de la **AI** pestaña muestran los dos comandos que hay que ejecutar, uno a la vez, y terminan en tu navegador:

| CLI    | Cierra sesión y vuelve a iniciarla                          |
| ------ | ----------------------------------------------------------- |
| Claude | `claude auth logout`, luego `claude auth login`             |
| Codex  | `codex logout`, luego `codex login`                         |
| Cursor | `cursor-agent logout`, luego `cursor-agent login`           |
| Gemini | ejecuta `gemini`, luego `/auth logout`, luego `/auth login` |

Si Creator Hub instaló Codex por ti, usa **Cerrar sesión** y luego **Iniciar sesión** en la **AI** la pestaña en su lugar.

Envía tu mensaje de nuevo una vez que hayas iniciado sesión. El agente permanece habilitado todo el tiempo, así que no tienes que configurarlo desde cero.

### Pagar con una clave de API en su lugar

Si prefieres pagar por token en lugar de usar una suscripción de CLI, activa **Pagar esto con una clave de API en su lugar** en la **AI** pestaña de los ajustes. Lee la clave de API de tu proveedor desde tu entorno: `ANTHROPIC_API_KEY`, `OPENAI_API_KEY`, `CURSOR_API_KEY` o `GEMINI_API_KEY`. Está desactivado de forma predeterminada.

Esta también es una forma de evitar un inicio de sesión que no puedes completar. Cuando una CLI está en el **Se necesita iniciar sesión** estado, Creator Hub muestra un **O usa una clave de API** sugerencia con el nombre de variable correspondiente.

## Qué puede hacer

El asistente trabaja en la escena que tienes abierta. Puede:

* **Editar la escena**: crear y eliminar entities, cambiar components, colocar [smart items](/docs/creator/content-creator-es/scene-editor/interactividad/smart-items/), adjuntar [scripts](/docs/creator/content-creator-es/scene-editor/extender-con-codigo/script-component/), y leer y escribir `scene.json`.
* **Escribir código** en los archivos de tu escena.
* **Ejecutar la vista previa** y controlarla: tomar capturas de pantalla, caminar por ella, hacer clic en cosas, leer los logs y comprobar el rendimiento.
* **Leer tu selección**, para que puedas seleccionar una entity y decir "haz que esta sea más alta".

Los cambios se guardan a medida que ocurren y se añaden al historial normal de deshacer. Para revertir todo un intercambio de una vez, usa **Deshacer cambios de IA** en la respuesta del asistente.

![El panel de Asistente de IA acoplado a la derecha del editor tras una solicitud para renombrar una entity y añadir un Mesh Renderer: la respuesta enumera las herramientas que ejecutó, describe el resultado y muestra un botón Deshacer cambios de IA debajo.](/docs/_remote/6a907fb6c14f96a55c037af0.webp)

Mientras trabaja el asistente, sus cambios aparecen en vivo en el entity tree y el Canvas.

![El Scene Editor completo con el panel de Asistente de IA acoplado a la derecha, la entity Stage renombrada visible en el entity tree y la respuesta del asistente terminando con Deshacer cambios de IA.](/docs/_remote/1a8e6565c0eaf9d6cba56d1b.webp)

## Trabajar con el chat

* **Nueva conversación** inicia una conversación nueva.
* **Historial de chat** enumera conversaciones anteriores y cuánto tiempo hace que se iniciaron. Las conversaciones se guardan por proyecto, así que al volver a abrir una escena reaparecen sus chats.
* **Abrir en una ventana aparte** saca al asistente a su propia ventana, lo que resulta útil en un segundo monitor. **Volver a acoplar en el editor** lo devuelve.
* El panel se puede arrastrar para hacerlo más ancho o más estrecho.

Una **Acerca del uso de IA** tarjeta te recuerda que el asistente consume tokens de tu cuenta conectada. Vigila el uso de esa cuenta.

El asistente procesa un mensaje a la vez. Mientras una respuesta está en curso, un **Pensando…** indicador permanece visible hasta que termina el turno, incluso durante llamadas largas a herramientas. Enviar un segundo mensaje no cancela el primero: espera la respuesta o pulsa **Detener** para terminar tú mismo el turno.

Algunas respuestas te hacen una pregunta antes de continuar. Si dejas una sin responder demasiado tiempo, se cierra con "Esta pregunta ya no está activa." y puedes volver a preguntar.

### Adjuntar archivos a un prompt

A veces es más fácil mostrarle algo al asistente que describírselo. Puedes adjuntar archivos a cualquier mensaje de tres maneras:

* Haz clic en el icono de clip a la izquierda del cuadro de mensaje, etiquetado **Adjuntar archivos**, y elige uno o más archivos.
* Arrastra archivos desde tu explorador de archivos al cuadro de mensaje. Aparece una **Suelta archivos para adjuntar** sugerencia mientras arrastras.
* Pega una imagen directamente desde tu portapapeles, por ejemplo una captura de pantalla que acabas de tomar.

Cada archivo adjunto aparece como una ficha encima del cuadro de mensaje, con un icono según su tipo: imagen, modelo 3D (`.glb`, `.gltf`), audio (`.mp3`, `.wav`, `.ogg`, `.m4a`, `.flac`), o un archivo genérico para cualquier otra cosa. Haz clic en el **Eliminar** cruz de una ficha para quitarlo antes de enviar. Puedes adjuntar hasta 8 archivos a un solo mensaje, y puedes enviar adjuntos sin ningún texto.

Los archivos que arrastras o eliges desde el disco se entregan al asistente por su ruta real, así que no se copia nada y no hay límite de tamaño. El asistente puede ver imágenes directamente, y puede referenciar un modelo o archivo de audio por su ruta cuando escribe código de la escena. Las imágenes pegadas no tienen archivo en el disco, así que Creator Hub primero las guarda en un archivo temporal; esas están limitadas a 8 MB cada una.

Si tu CLI de Claude es demasiado antigua, el panel muestra una advertencia. Ejecuta `claude update` en una terminal para solucionarlo.

### Señala un asset en tu prompt

Adjuntar un archivo envía su contenido. Mencionar un asset es más ligero: solo le dice al asistente a qué asset de tu proyecto te refieres, así que puedes decir "usa este" en lugar de escribir una ruta.

Con el asistente abierto, haz doble clic en cualquier asset de la **Local Assets** pestaña. Aparece como una ficha encima del cuadro de mensaje, con el texto **Asset:&#x20;*****nombre de archivo***, o **Animación integrada:&#x20;*****nombre*** de una de las animaciones integradas del avatar.

* Menciona tantos assets como quieras. Cada uno obtiene su propia ficha y su propia **Eliminar** cruz.
* Las menciones se sitúan junto a la **Selección** ficha, así puedes señalar una entity en tu escena y un asset al mismo tiempo: "pon esta animación en ese NPC".
* Las menciones solo se aplican a tu siguiente mensaje. Se borran en cuanto lo envías.


> [!NOTE]
> **💡 Consejo**: Seleccionar la raíz de la Scene cuenta como no seleccionar nada, así que nunca se envía al asistente. Selecciona una entity real cuando quieras que el asistente actúe sobre ella.


## Las Skills de Decentraland se instalan para ti

Cuando se ejecuta el asistente, Creator Hub descarga las oficiales [Decentraland SDK Skills](/docs/creator/content-creator-es/scenes-sdk7/primeros-pasos/vibe-coding/) y las enlaza en tu proyecto, para que el asistente siga patrones verificados de SDK7 en lugar de adivinar.

Se enlazan en `.claude/skills` y `.agents/skills` dentro de tu carpeta de proyecto, y ambas se añaden a un `.gitignore` por ti, así que nunca terminan en tu repo.

## Archivos que el asistente deja en tu carpeta de escena

El asistente puede leer y editar tu escena abierta mediante un servidor MCP local. A cada CLI se le informa de ese servidor de forma diferente, así que, según cuál uses, puedes encontrar archivos nuevos en tu proyecto:

| CLI    | Qué aparece en la carpeta de la escena                                 |
| ------ | ---------------------------------------------------------------------- |
| Cursor | `.cursor/mcp.json`, además de `.cursor/rules/creator-hub.mdc`          |
| Gemini | `.gemini/settings.json`                                                |
| Claude | Nada. La configuración va en un archivo temporal fuera de tu proyecto. |
| Codex  | Nada. La configuración se pasa en la línea de comandos.                |

Si ya tienes uno de esos archivos de configuración, Creator Hub fusiona su entrada en él en lugar de sobrescribir tus propios servidores.


> [!WARNING]
> **📔 Nota**: `.cursor/mcp.json` es el único archivo que contiene el token de acceso en sí, porque el formato de configuración de Cursor no tiene forma de leerlo desde el entorno. Creator Hub lo añade a tu `.gitignore` antes de escribirlo, así que no lo hagas commit ni elimines esa línea. El archivo de Gemini hace referencia al token a través de una `$CREATOR_HUB_MCP_TOKEN` variable de entorno en su lugar, así que no contiene ningún secreto y no se ignora.


## Usa tu propia herramienta de IA en su lugar

Si ya trabajas en Claude Code, Cursor, Codex u otra herramienta que habla MCP, puedes apuntarla a la escena abierta en lugar de usar el panel integrado. MCP (Model Context Protocol) es una forma estándar de que una herramienta de IA llame a otro programa.

1. Abre el menú principal desde el logotipo de Creator Hub, selecciona **Configuración**, y en la **AI** pestaña despliega **Exponer el servidor MCP del asistente de IA**.
2. Haz clic en **Copiar configuración**.
3. Pégala en la configuración MCP de tu herramienta.

La configuración copiada se ve así, con un puerto y un token reales rellenados:

```json
{
  "mcpServers": {
    "creator-hub": {
      "type": "http",
      "url": "http://127.0.0.1:<PORT>/mcp",
      "headers": { "Authorization": "Bearer <TOKEN>" }
    }
  }
}
```

Para Claude Code, también puedes añadirlo desde una terminal en la carpeta de tu escena:

```bash
claude mcp add --transport http --scope local creator-hub http://127.0.0.1:<PORT>/mcp \\
  --header "Authorization: Bearer <TOKEN>"
```


> [!WARNING]
> **📔 Nota**: El puerto y el token son nuevos cada vez que Creator Hub se inicia. Si tu herramienta de repente obtiene un `401` error, vuelve a copiar la configuración. El servidor solo escucha en tu propia máquina y solo actúa sobre la escena que tienes abierta.


## Páginas relacionadas

* [Vibe Coding con IA](/docs/creator/content-creator-es/scenes-sdk7/primeros-pasos/vibe-coding/): la guía completa de las SDK Skills y de cómo hacer prompts bien.
* [Combinar con código](/docs/creator/content-creator-es/scene-editor/extender-con-codigo/overview/): abre tu escena en un editor de código.
* [Usar el Script Component](/docs/creator/content-creator-es/scene-editor/extender-con-codigo/script-component/): adjunta comportamiento a una entity desde un archivo.


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-es/scene-editor/extender-con-codigo/ai-assistant.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
