> Flujo de trabajo recomendado para crear una escena de Decentraland con Creator Hub, desde la ideación hasta las pruebas y la publicación.

# Flujo de trabajo de desarrollo

Este documento describe los pasos recomendados para desarrollar una escena para Decentraland, desde la ideación hasta la publicación y más allá.

## ¿Cómo creo una escena de Decentraland?

1. **Instala el** [**Creator Hub**](https://decentraland.org/download/creator-hub), la aplicación de escritorio oficial para crear, previsualizar y publicar escenas. Es la herramienta recomendada para creadores de todos los niveles de experiencia.
2. **Si usas un asistente de código de IA** (como Claude Code, Cursor o Copilot), instala el oficial [Decentraland SDK Skills](/docs/creator/content-creator-es/scenes-sdk7/primeros-pasos/vibe-coding/) para que conozca patrones verificados de SDK7: `npx skills add decentraland/sdk-skills`
3. **La sección** [**CLI**](/docs/creator/content-creator-es/scenes-sdk7/primeros-pasos/using-the-cli/) es una alternativa para usuarios avanzados y flujos de trabajo automatizados.

Las secciones siguientes amplían cada etapa de ese flujo de trabajo.

## Instala el Creator Hub

Asegúrate de tener instalado el Creator Hub de Decentraland.

* [Guía de instalación](/docs/creator/content-creator-es/scene-editor/primeros-pasos/editor-installation/)

Si tienes pensado trabajar con código, asegúrate también de instalar [Visual Studio Code](https://code.visualstudio.com/) o [Cursor AI](https://www.cursor.com/).


> [!NOTE]
> **💡 Consejo**: también puedes usar asistentes de IA para generar código de escena a partir de descripciones en lenguaje natural. Consulta [Vibe Coding con IA](/docs/creator/content-creator-es/scenes-sdk7/primeros-pasos/vibe-coding/) para saber cómo empezar con el desarrollo asistido por IA.


## Diseña tu experiencia

Piensa en cuánto espacio necesitas ocupar, qué tipo de distribución quieres, qué mecánicas quieres que los jugadores puedan realizar, etc. Los siguientes documentos pueden servirte de guía:

* [Guía de UX y UI](/docs/creator/content-creator-es/scenes-sdk7/disenar-la-experiencia/ux-ui-guide/)
* [Restricciones de diseño para juegos](/docs/creator/content-creator-es/scenes-sdk7/disenar-la-experiencia/design-games/)
* [Directrices de MVP de escenas](/docs/creator/content-creator-es/scenes-sdk7/disenar-la-experiencia/mvp-guidelines/)

## Dónde publicar

En Decentraland, el contenido se publica en parcelas de tierra adyacentes dentro de una cantidad finita de espacio. Los jugadores pueden caminar libremente de una a otra. Cada escena es su propio pequeño mundo contenido, los elementos de una escena no pueden extenderse a otra escena, y el código de cada escena está aislado del resto en un sandbox.

El permiso para publicar en cada una de estas se controla mediante tokens. No necesitas LAND para desarrollar una escena, pero sí necesitarás acceso a LAND una vez que estés listo para publicar.

Como alternativa, tienes la opción de publicar en Decentraland [Worlds](/docs/creator/content-creator-es/scenes-sdk7/publicacion/publishing-options/#decentraland-worlds), que son escenas aisladas e independientes.

Estas son las opciones disponibles:

* Alquilar LAND
* Comprar LAND
* Obtener permisos de un propietario de LAND
* Publicar en un Decentraland World, consulta [worlds](/docs/creator/content-creator-es/scenes-sdk7/publicacion/publishing-options/#decentraland-worlds) para obtener más información.

Consulta [Opciones de publicación](/docs/creator/content-creator-es/scenes-sdk7/publicacion/publishing-options/) para más detalles.

## Templates y ejemplos

Al crear una nueva escena, elige entre varias escenas base de template que incluyen algo de código básico y modelos 3D. Úsalas para empezar más rápido.

* [Escenas de ejemplo](/docs/creator/content-creator-es/tutoriales-y-ejemplos/examples/): aquí puedes encontrar una colección seleccionada de escenas y repositorios funcionales, cada uno mostrando mecánicas que puedes reutilizar o tomar como referencia.
* [Bibliotecas de ayuda](https://studios.decentraland.org/resources?sdk_version=SDK7&resource_type=Library): estas pueden simplificar muchas tareas comunes.

## Recursos artísticos

Si eres un artista con experiencia o tienes acceso a alguien que lo sea, puedes crear `.gltf` o `.glb` personalizados modelos para tu escena. Consulta [Conceptos esenciales de modelos 3D](/docs/creator/content-creator-es/modelado-y-animaciones-3d/3d-models/) para obtener consejos sobre cómo crear modelos 3D para Decentraland.

Sin embargo, no necesitas crear tus propios assets. Consulta [Recursos útiles](/docs/creator/content-creator-es/scenes-sdk7/primeros-pasos/useful-resources/) para bibliotecas de assets y herramientas de IA generativa que puedes usar para buscar modelos 3D, así como otras herramientas que pueden acelerar tu flujo de trabajo.

## Ejecuta una vista previa local

Para ejecutar una vista previa de tu escena, ábrela en el Creator Hub y haz clic en el **Reproducir** botón. Como alternativa, si estás trabajando desde la línea de comandos, ejecuta `npm run start` en la carpeta raíz de tu proyecto.

* [Previsualiza tu escena](/docs/creator/content-creator-es/scenes-sdk7/primeros-pasos/preview-scene/) para más detalles.
* Consulta la [Depurar una escena](/docs/creator/content-creator-es/scenes-sdk7/depuracion/debug-in-preview/) para obtener consejos sobre cómo depurar cualquier problema.
* Activa **Comprimir activos** en el **Cliente de escritorio** submenú de **Opciones de reproducción** (o ejecuta `npm run start -- --local-ab` desde la CLI) para cargar tus modelos 3D como paquetes de assets optimizados. La vista previa se ejecuta con más fluidez y muestra los modelos tal como se verán una vez publicados. Consulta [Previsualizar con activos optimizados](/docs/creator/content-creator-es/scenes-sdk7/primeros-pasos/preview-scene/#preview-with-optimized-assets).


> [!NOTE]
> **💡 Consejo**: cuando usas el Creator Hub, cada vez que haces un cambio en tu escena, la vista previa se actualiza automáticamente. Incluso mientras se ejecuta.


## Publicar en Decentraland

Una vez que estés satisfecho con tu escena, es hora de publicarla en Decentraland. Para ello, necesitas ser propietario de LAND, de un Decentraland NAME o de un nombre de ETH ENS, o tener permisos otorgados por alguien que sí los tenga.

Consulta [publicación](/docs/creator/content-creator-es/scenes-sdk7/publicacion/publishing/) para ver instrucciones sobre cómo hacerlo.

Como alternativa, puedes publicar en [Worlds](/docs/creator/content-creator-es/scenes-sdk7/publicacion/publishing-options/#decentraland-worlds), un espacio 3D personal que no requiere LAND.

## Promocionar

Ahora que tu escena ya está disponible, ¡difúndela! Aquí tienes algunas formas de hacerlo:

* Compártela en redes sociales (#DCL)
* Anúnciala en [Discord](https://dcl.gg/discord)
* Envíala para que aparezca destacada en [events.decentraland.org](https://events.decentraland.org/)
* Organiza un evento en tu escena
* Añade un punto de spawn en una zona de alto tráfico que enlace con tu escena

## Itera

Una vez que tu escena haya estado en línea un tiempo y hayas recibido comentarios de los jugadores, ¡estarás en una gran posición para iterar sobre ella!

Actualiza tu contenido con mejoras y nuevas funciones, desplegando nuevas versiones de tu escena en las mismas coordenadas.

## Devolver a la comunidad

Si creas una escena, juego o aplicación de la que te sientas orgulloso, ¡considera hacerla de código abierto! Así otras personas pueden aprender de tu código y construir sobre tu trabajo. También puedes compartir todo el proyecto en [Awesome Repository](https://github.com/decentraland-scenes/Awesome-Repository).

Si construyes una pieza de funcionalidad reutilizable, puede que quieras convertirla en una biblioteca que otras personas puedan importar en sus proyectos.


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-es/scenes-sdk7/primeros-pasos/dev-workflow.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
