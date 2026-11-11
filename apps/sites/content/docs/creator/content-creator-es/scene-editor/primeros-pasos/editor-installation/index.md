> Cómo instalar el Scene Editor.

# Instalación de Creator Hub

![Creator Hub](/docs/_remote/9100f2b541affb1c6fecfe89.webp)

Descarga Creator Hub [AQUÍ](https://decentraland.org/download/creator-hub).

## Actualización de Creator Hub

La aplicación Creator Hub busca actualizaciones cada vez que la abres y se actualiza automáticamente si hay una nueva versión disponible.

También puedes buscar actualizaciones manualmente. Para ello,

1. Haz clic en el logo de Creator Hub en la esquina superior izquierda de la pantalla para abrir el menú principal.
2. Haz clic **Buscar actualizaciones**.

![El menú principal de Creator Hub, abierto desde el logo de la esquina superior izquierda, con las opciones Acerca de Creator Hub, Buscar actualizaciones, Configuración, Informar de un error y Ayuda y soporte.](/docs/_remote/1394f3aa47147ab15685f4dd.webp)

El menú principal también es donde encuentras la app **Configuración**, y enlaces para informar de un error o recibir soporte.

## Edición de código

Si también piensas leer y editar código en tu escena, también deberás instalar una de estas opciones:

* <img src="/docs/_remote/358795a1c08dfc479355a174.webp" alt="VS Code" data-size="line"> [Visual Studio Code](https://code.visualstudio.com/): Esta es la opción recomendada para desarrolladores con experiencia.
* <img src="/docs/_remote/9510a847d1493f0d8b4cf7e3.webp" alt="Cursor" data-size="line"> [Cursor AI](https://www.cursor.com/): Este es un potente editor de código integrado con IA. Te permite elegir distintos modelos de IA para ayudarte a escribir código; hay una versión gratuita disponible, y los modelos más avanzados requieren un plan de pago. Esta es una buena opción para desarrolladores que son nuevos en Decentraland o TypeScript, o si quieres ahorrar tiempo al escribir código.

Puede que necesites seleccionar tu Code Editor en la configuración de Creator Hub. Para ello,

1. Haz clic en el logo de Creator Hub en la esquina superior izquierda de la pantalla y selecciona **Configuración**.
2. Abre la **EDITOR** pestaña.
3. En **editor de código preferido**, selecciona tu editor de código. Puede que encuentres tu editor en el desplegable, o quizá debas seleccionar **Elegir desde tu dispositivo...** para encontrarlo.

![El cuadro de diálogo de Preferencias de la app en la pestaña EDITOR, mostrando el desplegable de editor de código preferido configurado en Visual Studio Code, las casillas de Play Options y la casilla de App Warnings.](/docs/_remote/be0b7d493dc2e145af5207ac.webp)

### habilidades de IA

Si planeas usar un agente de IA para ayudarte a escribir código, recomendamos instalar las habilidades de Decentraland SDK. Son conjuntos de instrucciones predefinidos que enseñan a tu agente de IA a trabajar con Decentraland SDK, de modo que cometa menos errores y obtenga mejores resultados desde el primer prompt. Para instalarlas todas, ejecuta el siguiente comando en la carpeta de tu proyecto de escena:

```bash
npx skills add decentraland/sdk-skills --all
```

Consulta [Vibe coding](/docs/creator/content-creator-es/scenes-sdk7/primeros-pasos/vibe-coding/) para obtener más detalles sobre cómo instalar habilidades específicas y cómo usarlas.

## Solución de problemas

Si encuentras problemas, consulta la [solución de problemas](/docs/creator/content-creator-es/scenes-sdk7/depuracion/troubleshooting/) sección.


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-es/scene-editor/primeros-pasos/editor-installation.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
