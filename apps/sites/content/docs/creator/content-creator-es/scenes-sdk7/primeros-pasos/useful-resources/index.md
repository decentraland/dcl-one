> Herramientas, complementos, librerías de assets y proyectos de ejemplo que aceleran tu flujo de trabajo de creación en Decentraland.

# Recursos útiles

Esta página recopila herramientas, addons, bibliotecas de assets y proyectos de ejemplo que pueden acelerar tu flujo de trabajo al crear experiencias para Decentraland.

## Decentraland SDK Skills

Dale a tu asistente de codificación de IA una guía específica de Decentraland para crear, iterar y publicar tus experiencias rápidamente. Los Decentraland SDK Skills funcionan con herramientas como Claude Code, Cursor, Codex, Cline y Windsurf, y se mantienen actualizados con los últimos cambios del SDK.

🔗 [github.com/decentraland/sdk-skills](https://github.com/decentraland/sdk-skills)

Consulta [Vibe Coding con IA](/docs/creator/content-creator-es/scenes-sdk7/primeros-pasos/vibe-coding/) para ver consejos sobre cómo usar SDK Skills en tu flujo de trabajo de IA.

## Decentraland SDK7 Test Scenes

Usa las Decentraland SDK7 Test Scenes para ver ejemplos funcionales de características específicas del SDK. El repositorio incluye muchas escenas de prueba aisladas, y el mundo de prueba en vivo te permite probarlas antes de usar sistemas similares en tus propias experiencias.

🔗 [github.com/decentraland/sdk7-test-scenes](https://github.com/decentraland/sdk7-test-scenes/)

## Tabla de clasificación del servidor multijugador

Mira cómo se puede usar el servidor Multijugador de Decentraland para crear una tabla de clasificación resistente. El servidor controla el estado del juego, valida cada puntuación, conserva los totales por jugador y transmite una tabla de clasificación ordenada que cada cliente de Decentraland puede renderizar.

🔗 [escena de prueba de tabla de clasificación del servidor multijugador](https://github.com/decentraland/sdk7-test-scenes/tree/main/scenes/90,-9-authoritative-server-leaderboard)

Consulta [Servidores autoritativos](/docs/creator/content-creator-es/scenes-sdk7/networking/authoritative-servers/) para aprender a construir sobre el servidor autoritativo.

## Decentraland Tools for Blender

Acelera tu flujo de trabajo con este addon de Blender, que gestiona las cuadrículas de parcelas, los límites de wearables, la optimización de texturas, la generación de LOD y la exportación GLB con ajustes de Decentraland integrados.

🔧 [extensions.blender.org/add-ons/decentraland-tools](https://extensions.blender.org/add-ons/decentraland-tools/)

## Assets de arte 3D

Una buena experiencia tendrá un gran arte 3D que la acompañe. Si te interesa crear esos modelos 3D tú mismo, consulta la [sección de Modelado 3D](/docs/creator/content-creator-es/modelado-y-animaciones-3d/3d-models/). Pero si prefieres centrarte en la parte de codificación o diseño del juego, ¡no necesitas crear tus propios assets!

Aquí tienes algunas fuentes de modelos 3D que puedes usar en una escena de Decentraland:

* [IWB Catalog](https://dcl-iwb.co/)
* [Asset Ovi](https://assetovi.com/)
* [SketchFab](https://sketchfab.com/)
* [Clara.io](https://clara.io/)
* [Archive3D](https://archive3d.net/)
* [SketchUp 3D Warehouse](https://3dwarehouse.sketchup.com/)
* [Thingiverse](https://www.thingiverse.com/)
* [ShareCG](https://www.sharecg.com/)
* [CGTrader](https://cgtrader.com)

También puedes usar herramientas de IA generativa para crear tus propios modelos 3D. Echa un vistazo a:

* [Blender MCP](https://github.com/ahujasid/blender-mcp) + [Claude Desktop](https://claude.ai/download): conecta un asistente de IA directamente a [Blender](https://www.blender.org/), para que puedas crear y editar modelos 3D describiendo lo que quieres. Todas son herramientas gratuitas.
* [Meshy](https://www.meshy.ai/)
* [Luma AI](https://lumalabs.ai/genie)
* [Tripo3D](https://www.tripo3d.ai/app)
* [Rodin](https://hyper3d.ai/rodin)


> [!WARNING]
> **📔 Nota**: Los modelos deben estar en los `.gltf` o `.glb` formatos admitidos, y deben tener una cantidad de triángulos, texturas y materiales que se ajuste a las [limitaciones de la escena](/docs/creator/content-creator-es/scenes-sdk7/optimizacion/scene-limitations/). Si obtienes modelos de un sitio de terceros, presta atención a las restricciones de licencia del contenido que descargas.


## Servidor Blender MCP

Usa Blender MCP para conectar un agente de IA, como Claude Code, a Blender. Puede ayudar a crear y editar modelos GLTF/GLB, materiales y texturas a partir de simples indicaciones de texto.

🔗 [blender.org/lab/mcp-server](https://www.blender.org/lab/mcp-server/)


> [!WARNING]
> **📔 Nota**: Blender recomienda usar MCP en un entorno seguro y sin archivos sensibles.


## Assets 3D de código abierto

Una colección seleccionada de assets 3D de alta calidad, incluidos props, entornos y estructuras, completamente gratuita y de código abierto. Construida sobre los estándares GLB y glTF.

🔗 [opensource3dassets.com](https://www.opensource3dassets.com/en)

## Assets de Genesis Plaza

Reutiliza cualquiera de los assets o texturas de Genesis Plaza en tus propias experiencias. Puedes descargar el archivo .blend, o usar la Biblioteca de Assets de Blender de Stom para arrastrar y soltar desde un repositorio. También puedes descargar assets de paisajismo procedimental a juego, como árboles y arbustos.

* 📂 [Descarga el archivo .blend](https://drive.google.com/file/d/1beZHX1D8zR9iMCEmcPQU6etxpZXohZ5P/view?usp=sharing)
* 🐈‍⬛ Encuentra el repositorio de Stom en [github.com/stom66/dcl-genesis-plaza-assets](https://github.com/stom66/dcl-genesis-plaza-assets)
* ℹ️ [Mira un video sobre cómo usar el repositorio](https://files.catbox.moe/eo5hhb.mp4)
* 🌺 Haz coincidir los assets procedurales de Decentraland [con estos árboles, arbustos y rocas](https://drive.google.com/drive/folders/1Hxoog-ov_igbNTU3H7EwOX634EVCbhYQ?usp=drive_link)

## Optimizador de escenas de Decentraland

Prueba esta útil herramienta para extraer, deduplicar y comprimir texturas de archivos GLB y optimizar aún más tus escenas.

🔗 [github.com/decentraland/SceneOptimizer](https://github.com/decentraland/SceneOptimizer)


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-es/scenes-sdk7/primeros-pasos/useful-resources.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
