> Migra una escena heredada de SDK6 a SDK7 usando un asistente de IA.

# Migrar escenas heredadas de SDK6

Si tienes una escena antigua de SDK6 que te gustaría poner al día, puedes migrarla a SDK7 automáticamente con la ayuda de un asistente de IA. El [Decentraland SDK Skills](/docs/creator/content-creator-es/scenes-sdk7/primeros-pasos/vibe-coding/#install-skills-for-any-ai-agent) incluyen un flujo de trabajo de migración dedicado que la IA sigue paso a paso, para que no tengas que traducir el código a mano.

Esta guía te acompaña a lo largo de todo el proceso.

## 1. Crea una nueva escena vacía de SDK7

Empieza con un proyecto de SDK7 nuevo y vacío. La IA moverá tu código heredado y tus assets a esta estructura limpia.

**Opción A — Creator Hub**

1. Abre Creator Hub.
2. Selecciona la **escenas** pestaña y haz clic **Nueva escena**.
3. Elige la **Empty Scene** template.

**Opción B — Línea de comandos**

Ejecuta lo siguiente en una carpeta vacía:

```bash
npx @dcl/sdk-commands init
```

Esto rellena la carpeta con los archivos predeterminados de un proyecto de SDK7.

## 2. Instala las SDK Skills

Las SDK Skills enseñan a tu agente de IA cómo trabajar con Decentraland SDK7, incluyendo cómo realizar una migración adecuada de SDK6 → SDK7.

Puedes instalarlas de dos maneras.

**Opción A — Pídele a la IA que las instale**

Abre tu asistente de IA en la carpeta de la escena y simplemente di:

> Instala las SDK Skills de Decentraland.

La IA conoce el comando y las instalará por ti.

**Opción B — Instalar desde la línea de comandos**

```bash
npx skills add decentraland/sdk-skills --all
```

El `--all` flag instala todas las skills disponibles. Si la omites, se abre un selector interactivo para que elijas qué skills instalar.

Para más detalles sobre qué skills están disponibles y cómo funcionan, consulta [Vibe Coding con IA](/docs/creator/content-creator-es/scenes-sdk7/primeros-pasos/vibe-coding/#install-skills-for-any-ai-agent).

## 3. Pídele a la IA que migre la escena

Coloca los archivos heredados de tu proyecto SDK6 en algún lugar donde la IA pueda leerlos — por ejemplo, en una carpeta separada de tu equipo o en un repositorio público de GitHub.

Luego pídele a la IA:

> Migra esta escena de SDK6 a SDK7.

Las skills incluyen el playbook completo de migración, así que la IA:

* Identificar el punto de entrada y los components usados en la escena de SDK6.
* Traducir entities, components y systems a sus equivalentes en SDK7.
* Mover los modelos 3D, el audio y otros assets al nuevo proyecto.
* Actualiza `scene.json` y la configuración del proyecto.
* Reemplazar las API obsoletas por sus equivalentes actuales de SDK7.

## 4. Prueba e itera

Una vez que termine la pasada de migración, previsualiza la escena y pruébala por completo.

* En Creator Hub, haz clic en **Reproducir**.
* Desde la línea de comandos, ejecuta `npm run start`.

Mientras pruebas, busca:

* Problemas visuales — modelos faltantes, posiciones o escalas incorrectas, materials rotos.
* Problemas de interacción — clics que no se disparan, triggers que no se activan, diálogos que no aparecen.
* Problemas de multiplayer o networking, si tu escena los usaba.
* Errores de consola en la ventana de previsualización.

Cuando encuentres un bug, descríbelo a la IA en lenguaje sencillo:

> El botón rojo de la mesa ya no abre la puerta. Antes reproducía un sonido y giraba la puerta 90 grados.

La IA puede usar las skills circundantes (interactividad, animations, audio, etc.) para corregir los problemas uno por uno. Itera hasta que la escena se comporte como la original.


> [!NOTE]
> **💡 Consejo**: Corrige un bug a la vez y previsualiza después de cada cambio. Es mucho más fácil detectar regresiones cuando los cambios son pequeños.


## Próximos pasos

* [Vibe Coding con IA](/docs/creator/content-creator-es/scenes-sdk7/primeros-pasos/vibe-coding/) — Referencia completa para trabajar con asistentes de IA y SDK Skills.
* [SDK Quick Start](/docs/creator/content-creator-es/scenes-sdk7/primeros-pasos/sdk-101/) — Aprende los fundamentos de SDK7 para poder revisar y refinar el código migrado.


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-es/scenes-sdk7/otros/migrate-legacy-sdk6-scenes.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
