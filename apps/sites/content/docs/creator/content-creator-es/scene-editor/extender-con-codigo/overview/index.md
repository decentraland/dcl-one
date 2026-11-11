> Combina el contenido creado en el Scene Editor con la potencia de escribir código.

# Combinar con código



<div class="video-embed"><button type="button" class="video-play" data-video-src="https://www.youtube-nocookie.com/embed/55H37rygD7M" aria-label="Play YouTube video"><svg viewBox="0 0 64 64" width="64" height="64" aria-hidden="true"><circle cx="32" cy="32" r="30" fill="currentColor"/><path d="m26 20 20 12-20 12Z" fill="#181214"/></svg><span>Play video</span></button></div>
<p class="video-source"><a href="&lt;https://www.youtube.com/watch?v=55H37rygD7M>">Watch on YouTube</a></p>



Creator Hub junto con código personalizado es una combinación muy potente para crear contenido. Puedes usar el canvas para posicionar items visualmente de forma intuitiva y luego escribir código que interactúe con estos items con total libertad. Incluso puedes colocar un smart item, que tiene su propio comportamiento predeterminado, y escribir código que reaccione cuando el item se activa.

Por ejemplo, puedes aprovechar un smart item de palanca existente, que ya viene con sus sonidos, animaciones y estados, y escribir código que detecte cuando se tira de la palanca para ejecutar tu propia lógica personalizada.

Consulta [Referenciar items en el código](/docs/creator/content-creator-es/scene-editor/extender-con-codigo/reference-items/) sobre cómo obtener items por nombre o por etiquetas desde tu código.

## Editar código

Debes instalar un editor de código en tu máquina para editar el código de tu scene. Las opciones recomendadas son:

* <img src="/docs/_remote/358795a1c08dfc479355a174.webp" alt="VS Code" data-size="line"> [Visual Studio Code](https://code.visualstudio.com/): Esta es la opción recomendada para desarrolladores experimentados.
* <img src="/docs/_remote/9510a847d1493f0d8b4cf7e3.webp" alt="Cursor" data-size="line"> [Cursor AI](https://www.cursor.com/): Este es un potente editor de código integrado con IA. Te permite elegir distintos modelos de IA para ayudarte a escribir código; hay un nivel gratuito y los modelos más avanzados requieren un plan de pago. Esta es una buena opción para desarrolladores que son nuevos en Decentraland o TypeScript, o si quieres ahorrar tiempo escribiendo código.


> [!WARNING]
> **📔 Nota**: Si estás en macOS, asegúrate de que la app del editor de código esté en el directorio Applications.


Una vez instalado, es posible que necesites seleccionar tu Code Editor en la configuración de Creator Hub. Para hacerlo,

1. Haz clic en el logo de Creator Hub en la esquina superior izquierda de la pantalla y selecciona **Configuración**.
2. Abre la **EDITOR** pestaña.
3. En **Editor de código preferido**, selecciona tu editor de código. Puede que encuentres tu editor listado en el desplegable, o quizá debas seleccionar **Elige desde tu dispositivo...** para encontrarlo.

![El cuadro de diálogo de preferencias de la app en la pestaña EDITOR, que muestra el desplegable Editor de código preferido configurado en Visual Studio Code.](/docs/_remote/be0b7d493dc2e145af5207ac.webp)

## Abrir el código de una scene

Una vez que hayas instalado un editor de código en tu máquina y lo hayas seleccionado en la configuración de Creator Hub, puedes hacer clic en el **< > CÓDIGO** botón para abrirlo en el proyecto de tu scene.

![](/docs/_remote/b1e812c7bf7175e15d141046.webp)

Esto abre una ventana separada con el editor de código. En el margen izquierdo puedes navegar por los archivos y la estructura de carpetas de tu proyecto.

![](/docs/_remote/89a525be913ad9dc95c15692.webp)

Añade tu código personalizado en el `index.ts` archivo bajo `/src`, dentro de la `main()` función. De lo contrario, puedes añadir código personalizado fuera de esa función o crear nuevos `.ts` archivos dentro de la `/src` carpeta, pero estos deben ser referenciados de alguna manera dentro de la función de `main()` : Si tienes VS Code o Cursor instalados, pero el `index.ts`.


> [!WARNING]
> **📔 Nota**Si tienes VS Code o Cursor instalados, pero el **CODE** botón no lo abre, puede ser que VS Code no esté configurado correctamente en tu máquina para abrirse desde la línea de comandos. En la mayoría de los casos, esto se gestiona como parte de la instalación predeterminada, pero si no es así, consulta [estas instrucciones de VS](https://code.visualstudio.com/docs/setup/mac#_launching-from-the-command-line) para habilitar VS Code desde la línea de comandos.


Si tienes una ventana de vista previa abierta ejecutando tu scene, cada vez que cambies el código en tus archivos y guardes, la scene se recarga automáticamente con tus cambios.

## Usar IA

Creator Hub tiene un asistente integrado [AI Assistant](/docs/creator/content-creator-es/scene-editor/extender-con-codigo/ai-assistant/) que ve tu scene abierta y la edita por ti, sin salir de la app. Es una función experimental, desactivada por defecto.

También puedes aprovechar asistentes de IA como Cursor o Claude Code para ayudarte a escribir código de scene. Por ejemplo, para usar Cursor, haz lo siguiente:

1. Abre el asistente de IA de Cursor haciendo clic en el **IA** botón en la esquina superior derecha de la pantalla
2. Allí puedes pedirle al asistente de IA que te ayude a escribir código. Tus indicaciones pueden incluir enlaces a páginas de documentación, rutas a archivos específicos de tu proyecto, o incluso imágenes. También puedes seleccionar un modelo específico para usar desde el desplegable.

Para darle a tu asistente de IA contexto de Decentraland, instala las habilidades oficiales de Decentraland SDK. Son conjuntos de instrucciones listos para usar, mantenidos por la Decentraland Foundation, que enseñan a tu agente de IA patrones verificados de SDK7 para cada tema: creación de scenes completas desde cero, añadir modelos 3D, multiplayer, despliegue y más.

```bash
npx skills add decentraland/sdk-skills
```

Consulta [Vibe Coding con IA](/docs/creator/content-creator-es/scenes-sdk7/primeros-pasos/vibe-coding/) para la guía completa, incluida la lista completa de habilidades disponibles.


> [!WARNING]
> **📔 Nota**: Los proyectos de scene más antiguos pueden contener una `/dclcontext` carpeta, descargada por el `npx sdk-commands get-context-files` comando obsoleto. Ese contexto ya no se mantiene y ha sido reemplazado por las SDK Skills. Puedes eliminar la carpeta sin problema.


## Control de versiones

Recomendamos que crees un repo para tu proyecto en GitHub y lo uses para llevar un seguimiento de las versiones de tu proyecto y colaborar con otras personas.

Si no estás familiarizado con cómo hacerlo, consulta [Inicio rápido para repositorios](https://docs.github.com/en/repositories/creating-and-managing-repositories/quickstart-for-repositories), o usa la [aplicación de escritorio de GitHub](https://desktop.github.com/download/) para un flujo más sencillo basado en UI.


> [!WARNING]
> **📔 Nota**: Sube toda la carpeta del proyecto a un repo de GitHub, pero asegúrate de que las carpetas `/node_modules` o `/bin` y el archivo `package-lock.json` estén incluidos en el archivo `.gitignore` para evitar sincronizarlos. Debería ser así si configuras el repo como de tipo `node`. Estos archivos se generan automáticamente y el contenido puede diferir entre distintas máquinas.


## Ver también

* [Vibe Coding con IA](/docs/creator/content-creator-es/scenes-sdk7/primeros-pasos/vibe-coding/): crea scenes describiendo lo que quieres a un asistente de IA.
* [Smart items - Conceptos básicos](/docs/creator/content-creator-es/scene-editor/interactividad/smart-items/)
* [Smart items - Advanced](/docs/creator/content-creator-es/scene-editor/interactividad/smart-items-advanced/)
* [Estados y condiciones](/docs/creator/content-creator-es/scene-editor/interactividad/states-and-conditions/)
* [Hacer que cualquier objeto sea smart](/docs/creator/content-creator-es/scene-editor/interactividad/make-any-item-smart/)
* [Inicio rápido de SDK](/docs/creator/content-creator-es/scenes-sdk7/primeros-pasos/sdk-101/): sigue este mini tutorial para una introducción rápida.
* [Flujo de trabajo de desarrollo](/docs/creator/content-creator-es/scenes-sdk7/primeros-pasos/dev-workflow/): léelo para entender la creación de escenas de principio a fin.
* [Ejemplos](/docs/creator/content-creator-es/tutoriales-y-ejemplos/examples/): lánzate directamente a escenas de ejemplo funcionales.


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-es/scene-editor/extender-con-codigo/overview.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
