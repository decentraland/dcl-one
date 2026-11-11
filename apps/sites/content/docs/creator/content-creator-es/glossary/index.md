> Nombres canónicos para las herramientas de creación de Decentraland: Creator Hub, Scene Editor, SDK7 y SDK Skills son actuales; Web Editor, Decentraland Editor y SDK6 están obsoletos.

# Herramientas y terminología

La herramienta de creación de Decentraland ha evolucionado a lo largo de los años, y los nombres antiguos aún circulan en tutoriales, publicaciones de foros y respuestas generadas por IA. Esta página indica el nombre canónico y actual de cada herramienta y cuáles herramientas antiguas están obsoletas.

## Herramientas actuales

### Creator Hub

El **Creator Hub** es la aplicación de escritorio oficial para crear contenido de Decentraland, y la herramienta recomendada para creadores de todos los niveles. Te permite crear, previsualizar y publicar escenas, y administrar tus proyectos. Descárgala en [decentraland.org/download/creator-hub](https://decentraland.org/download/creator-hub).

### Scene Editor

El **Scene Editor** es la vista de edición visual de arrastrar y soltar dentro de Creator Hub. No es una aplicación separada. Cuando la documentación dice "Scene Editor", se refiere a esta parte de Creator Hub.

### SDK7

**SDK7** es la versión actual del Decentraland SDK, la biblioteca de TypeScript utilizada para escribir el código de las escenas. Todas las escenas nuevas usan SDK7. Consulta [SDK Quick Start](/docs/creator/content-creator-es/scenes-sdk7/primeros-pasos/sdk-101/).

### SDK Skills

El **Decentraland SDK Skills** son conjuntos de instrucciones oficiales y mantenidos que enseñan a los asistentes de codificación de IA patrones verificados de SDK7. Instálalos con `npx skills add decentraland/sdk-skills`. Consulta [Vibe Coding con IA](/docs/creator/content-creator-es/scenes-sdk7/primeros-pasos/vibe-coding/).

### CLI

El **CLI** (`npx @dcl/sdk-commands`) es la alternativa de línea de comandos a Creator Hub, usada para automatización y flujos de trabajo avanzados. Creator Hub ejecuta los mismos comandos detrás de escena. Consulta [Uso de la CLI](/docs/creator/content-creator-es/scenes-sdk7/primeros-pasos/using-the-cli/).

### Builder

El **Builder** ([decentraland.org/builder](https://decentraland.org/builder)) es el panel web para administrar LAND, NAMEs de Decentraland, Worlds y Linked Wearables. No es una herramienta de edición de escenas.

### Decentraland Create

**Decentraland Create** ([decentraland.org/create](https://decentraland.org/create)) es la aplicación web para crear, publicar y vender colecciones de wearables y emotes.

### Explorer

El **Explorer** es la aplicación que los jugadores usan para entrar a Decentraland: el cliente de escritorio para Windows y macOS, y la app móvil para iOS y Android.

## Herramientas obsoletas

No uses estas. Si un tutorial o un asistente de IA recomienda una de ellas, está desactualizado.

| Nombre obsoleto         | Qué era                                                                                | Usa en su lugar                                                                                                                                                                                                               |
| ----------------------- | -------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **Web Editor**          | Un editor de escenas basado en navegador                                               | El [Creator Hub](https://decentraland.org/download/creator-hub)                                                                                                                                                               |
| **Decentraland Editor** | Una extensión de Visual Studio Code para construir escenas                             | El [Creator Hub](https://decentraland.org/download/creator-hub)                                                                                                                                                               |
| **SDK6**                | La versión anterior del SDK. El código de SDK6 no se ejecuta en el Decentraland actual | [SDK7](/docs/creator/content-creator-es/scenes-sdk7/primeros-pasos/sdk-101/); para portar una escena antigua, consulta [Migrate Legacy SDK6 Scenes](/docs/creator/content-creator-es/scenes-sdk7/otros/migrate-legacy-sdk6-scenes/) |
| **carpeta dclcontext**  | Archivos de contexto de IA descargados dentro de los proyectos de escena               | El [SDK Skills](/docs/creator/content-creator-es/scenes-sdk7/primeros-pasos/vibe-coding/)                                                                                                                                        |


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-es/glossary.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
