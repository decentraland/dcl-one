> Usa el CLI de Decentraland para ejecutar y desplegar escenas. Creator Hub es la herramienta recomendada para la mayoría de los creadores; el CLI se adapta mejor a flujos de trabajo avanzados y automatizados.

# Usando el CLI


> [!NOTE]
> **💡 Consejo**: Si estás trabajando con un asistente de programación con IA (como Claude Code, Cursor o Copilot), instala las Skills oficiales de Decentraland SDK antes de que escriba cualquier código, para que conozca patrones verificados de SDK7:
> 
> ```bash
> npx skills add decentraland/sdk-skills
> ```
> 
> Ver [Vibe Coding con IA](/docs/creator/content-creator-es/scenes-sdk7/primeros-pasos/vibe-coding/) para ver el flujo de trabajo completo asistido por IA.


Para crear escenas para Decentraland, puedes usar cualquiera de estas opciones:

* El [Creator Hub](/docs/creator/content-creator-es/scene-editor/primeros-pasos/editor-installation/)
* La interfaz de línea de comandos (CLI)

Ambas herramientas te permiten compilar y previsualizar tu scene en un entorno de desarrollo "off-chain". Después de probar tu scene localmente, puedes subir tu contenido al servidor de contenido, vinculándolo con tu LAND o WORLD.

Aunque el Scene Editor en Creator Hub es más fácil de usar, la CLI te ofrece más flexibilidad y puede utilizarse fácilmente en procesos automatizados.


> [!WARNING]
> **📔 Nota**: El Scene Editor ejecuta las mismas operaciones de línea de comandos entre bastidores.



> [!NOTE]
> **💡 Consejo**: Ver [guía de instalación](/docs/creator/content-creator-es/scene-editor/primeros-pasos/editor-installation/) para obtener instrucciones sobre cómo instalar Creator Hub.


## Antes de comenzar

Para trabajar con la escena mediante la línea de comandos, instala las siguientes dependencias antes de ejecutar comandos de CLI con la escena:

* [Node.js](https://nodejs.org) (versión 20 o posterior)

## Iniciar un nuevo proyecto

Ejecuta `npx @dcl/sdk-commands init` en una carpeta vacía para poblarla con los archivos predeterminados de una [escena](/docs/creator/content-creator-es/scenes-sdk7/tipos-de-proyectos/scene-metadata/) proyecto. También puedes crear una nueva scene con el **New Scene** botón en Creator Hub, que produce la misma estructura de proyecto.

Para comenzar desde otro tipo de proyecto, usa el `--project` indicador. Por ejemplo, para crear un [proyecto de smart wearable](/docs/creator/content-creator-es/scenes-sdk7/tipos-de-proyectos/smart-wearables/) proyecto:

```bash
npx @dcl/sdk-commands init --project smart-wearable
```

Las opciones disponibles para `--project` son `scene-template` (la predeterminada), `px-template`, `smart-wearable`, y `library`.

## Actualizar la versión del SDK de una scene

Ejecuta el siguiente comando en la carpeta de la escena:

```bash
npm i @dcl/sdk@latest
```

Puedes confirmar que funcionó comprobando el `archivo package.json` de la escena, y buscando allí la `@dcl/sdk` versión.

## Ejecutar una vista previa

Ejecuta `npm run start` en el nivel raíz de un proyecto de scene, workspace o smart wearable para abrir una vista previa en el cliente de escritorio de Decentraland.

```bash
npm run start
```

Para previsualizar tu scene en la [aplicación móvil de Decentraland](/docs/creator/content-creator-es/desarrollar-para-movil/cliente-movil/overview/) en su lugar, ejecuta `npm run start -- --mobile` (alias `-- -m`). La CLI imprime un código QR que abre la escena en un teléfono conectado a la misma red Wi-Fi que tu computadora. Ver [Vista previa en móvil](/docs/creator/content-creator-es/desarrollar-para-movil/desarrollar/preview-on-mobile/) para la guía completa.

```bash
npm run start -- --mobile
```

Para que la vista previa se ejecute con mayor fluidez y renderice los modelos 3D tal como se verán una vez publicados, agrega el `--asset-bundles` indicador. Ver [Vista previa con assets optimizados](/docs/creator/content-creator-es/scenes-sdk7/primeros-pasos/preview-scene/#preview-with-optimized-assets).

```bash
npm run start -- --asset-bundles
```

Ver [previsualizar escenas](/docs/creator/content-creator-es/scenes-sdk7/primeros-pasos/preview-scene/) para obtener detalles y opciones especiales al ejecutar una vista previa.

## Compilar

Ejecuta `npm run build` para compilar tu proyecto. Las scenes de Decentraland se escriben en TypeScript, pero se compilan a JavaScript minificado al publicarse. Ver [programación de scenes](https://github.com/decentraland/docs/blob/main/creator/sdk7/getting-started/coding-scenes.md) para más detalles.

El comando de compilación es opcional, ya que también se ejecuta en segundo plano antes del despliegue (aunque puedes agregar un indicador para omitirlo).

El comando de compilación realiza comprobaciones de tipos más rigurosas que las que se ejecutan con `npm run start`, por lo que ejecutarlo a veces puede ser útil para depurar una escena.

## Desplegar una escena

Ejecuta `npm run deploy` para publicar tu escena en Decentraland. Este comando abre una ventana del navegador donde puedes firmar con tu wallet para autorizar el despliegue.

Ver [publicación](/docs/creator/content-creator-es/scenes-sdk7/publicacion/publishing/) para obtener detalles y opciones especiales al publicar una escena.

## Solución de problemas

Si tienes problemas, consulta la [sección de solución de problemas](/docs/creator/content-creator-es/scenes-sdk7/depuracion/troubleshooting/) .


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-es/scenes-sdk7/primeros-pasos/using-the-cli.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
