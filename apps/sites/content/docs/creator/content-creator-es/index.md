> Crea escenas, wearables y emotes de Decentraland. Creator Hub es la herramienta recomendada, con SDK Skills para asistentes de IA de programación y un CLI para flujos de trabajo avanzados.

# Bienvenido, creador

¡Construyamos Decentraland juntos!

¡Todos los creadores son bienvenidos! En Decentraland tienes una amplia gama de posibilidades creativas, ¡para personas con distintos talentos y niveles de habilidad!

Decentraland está disponible en escritorio (Windows y macOS) y en dispositivos móviles (iOS y Android). Los jugadores se mueven sin problemas entre clientes con la misma cuenta, avatar e inventario, lo que significa que tus escenas necesitan funcionar bien para todos, independientemente del dispositivo en el que estén jugando. A lo largo de esta guía encontrarás recomendaciones sobre cómo diseñar, construir y probar tus escenas para que funcionen muy bien en todas las plataformas compatibles. Para obtener orientación específica para móviles, consulta [Construcción para móviles](/docs/creator/content-creator-es/desarrollar-para-movil/cliente-movil/overview/).

## Creator Hub

Creator Hub es la herramienta recomendada para creadores de todos los niveles de conocimiento. Es una aplicación de escritorio que te permite crear [escenas](#scenes).

![](/docs/_remote/32e1b985101d928b718678f9.webp)

Descarga Creator Hub [aquí](https://decentraland.org/download/creator-hub).

## Wearables

Los Wearables son prendas de vestir que los avatares de los jugadores pueden usar. Se venden como NFTs y se compran en el [Marketplace](https://decentraland.org/marketplace/browse?section=wearables&vendor=decentraland&page=1&sortBy=newest&status=on_sale).

Aprende todo sobre [Creación de Wearables](/docs/creator/content-creator-es/wearables-y-emotes/wearables/creating-wearables/).

También puedes combinar un Wearable con código del SDK para crear un [Wearable inteligente](/docs/creator/content-creator-es/scenes-sdk7/tipos-de-proyectos/smart-wearables/). Esto activa una escena global cada vez que el jugador se pone el Wearable. Consulta [Tipos de proyecto](/docs/creator/content-creator-es/scenes-sdk7/tipos-de-proyectos/kinds-of-project/) para comprender mejor las distintas opciones.

## Emotes

Los Emotes son animaciones que el avatar de un jugador puede realizar. Se venden como NFTs y se compran en el [Marketplace](https://decentraland.org/marketplace/browse?assetType=item&section=emotes&vendor=decentraland&page=1&sortBy=newest&status=on_sale).

Aprende todo sobre [Creación de Emotes](/docs/creator/content-creator-es/wearables-y-emotes/emotes/creating-emotes/).

## escenas

El contenido 3D en Decentraland está compuesto por escenas; cada escena ocupa una cantidad finita de espacio y se muestra una junto a la otra para que los jugadores puedan caminar libremente a través de ellas.

Creator Hub te permite crear escenas con una interfaz fácil de arrastrar y soltar, y también editar código para tener control total sobre las interacciones. Puedes ejecutar vistas previas, depurar, editar código y publicar.

[Más información](/docs/creator/content-creator-es/scene-editor/primeros-pasos/about-editor/)

### ¿Cómo creo una escena de Decentraland?

1. **Instala el** [**Creator Hub**](https://decentraland.org/download/creator-hub), la aplicación oficial de escritorio para crear, previsualizar y publicar escenas. Es la herramienta recomendada para creadores de todos los niveles de habilidad.
2. **Si usas un asistente de código con IA** (como Claude Code, Cursor o Copilot), instala el oficial [Decentraland SDK Skills](/docs/creator/content-creator-es/scenes-sdk7/primeros-pasos/vibe-coding/) para que conozca patrones verificados de SDK7: `npx skills add decentraland/sdk-skills`
3. **El** [**CLI**](/docs/creator/content-creator-es/scenes-sdk7/primeros-pasos/using-the-cli/) es una alternativa para usuarios avanzados y flujos de trabajo automatizados.

### Arte 3D

Las escenas de Decentraland están compuestas por modelos 3D.

* Elige del amplio catálogo de assets predeterminados en el Scene Editor. Están listos para usar y optimizados para su uso en Decentraland

  ![](/docs/_remote/b193f61ea826d9783815e90e.webp)
* Crea tus propios modelos 3D usando Blender o tus herramientas 3D preferidas. Luego impórtalos en el Scene Editor.

  ![](/docs/_remote/6469c594dcf3e77e830e77da.webp)


> [!WARNING]
> **📔 Nota**: El contenido en Decentraland debe mantenerse dentro de ciertas [limitaciones de tamaño](/docs/creator/content-creator-es/scenes-sdk7/optimizacion/scene-limitations/) para garantizar que tu escena funcione sin problemas.
> 
> Consulta [modelado 3D](/docs/creator/content-creator-es/modelado-y-animaciones-3d/3d-models/) para ver consejos y trucos para optimizar, e información sobre las funciones y formatos compatibles para modelos 3D.


### Interactividad

Para hacer que tu escena sea interactiva:

* **Sin código**: Usa la UI del Scene Editor para colocar [Smart Items](/docs/creator/content-creator-es/scene-editor/interactividad/smart-items/) en tu escena. Son modelos que vienen preconstruidos con su propio comportamiento y son altamente personalizables. También puedes asignar los mismos comportamientos a tus propios modelos personalizados (no se requiere código).

  ![](/docs/_remote/a417642641d975327ba066bf.webp)
* **Código**: Para los desarrolladores que quieran incorporar lógica personalizada, usa el SDK para escribir código y hacer todo lo que puedas imaginar. Aprende a usar el SDK:
  * [Inicio rápido de SDK](/docs/creator/content-creator-es/scenes-sdk7/primeros-pasos/sdk-101/): sigue este mini tutorial para una introducción rápida.
  * [Flujo de trabajo de desarrollo](/docs/creator/content-creator-es/scenes-sdk7/primeros-pasos/dev-workflow/): léelo para entender la creación de escenas de principio a fin.
  * [Vibe Coding con IA](/docs/creator/content-creator-es/scenes-sdk7/primeros-pasos/vibe-coding/): crea escenas describiendo lo que quieres en lenguaje natural y deja que un asistente de programación con IA escriba el código por ti.
  * [Ejemplos](/docs/creator/content-creator-es/tutoriales-y-ejemplos/examples/): lánzate directamente a escenas de ejemplo funcionales.

    ![](/docs/_remote/3d5fae2810e08190c0c1ad38.webp)


> [!WARNING]
> **📔 Nota**: También necesitarás tener [Visual Studio Code](https://code.visualstudio.com/) instalado.


### Publicación de escenas

No necesitas poseer ningún token para empezar a construir tu escena con el Scene Editor. Para publicar tu escena, puedes elegir entre las siguientes opciones:

* **LAND en Genesis City**: Este es el mundo abierto principal de Decentraland, que está dividido en parcelas de 16x16 metros. Compra una o varias parcelas adyacentes en el [Marketplace](https://decentraland.org/marketplace/lands), y despliega tu escena allí.
* **Decentraland Worlds**: [Worlds](/docs/creator/content-creator-es/scenes-sdk7/publicacion/publishing-options/#decentraland-worlds) son tus propios espacios en el metaverso. Solo necesitas poseer un [nombre de Decentraland](https://decentraland.org/marketplace/names/claim), ¡y puedes publicar una escena tan grande como quieras!

Consulta [Tipos de proyecto](/docs/creator/content-creator-es/scenes-sdk7/tipos-de-proyectos/kinds-of-project/) para comprender mejor las distintas opciones.

Consulta [publicación](/docs/creator/content-creator-es/scenes-sdk7/publicacion/publishing/) para ver detalles y opciones especiales al publicar una escena, ya sea en Genesis City o en Worlds.

## Recursos útiles

Consulta [Recursos útiles](/docs/creator/content-creator-es/scenes-sdk7/primeros-pasos/useful-resources/) para ver una lista curada de herramientas, complementos, bibliotecas de assets y proyectos de ejemplo que pueden acelerar tu flujo de trabajo de creación.


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-es/readme.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
