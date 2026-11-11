> Cómo depurar tu escena que se está ejecutando dentro de Decentraland

# Depurar en producción

Cuando ejecutas una escena que ya está desplegada en Decentraland, hay varias cosas que puedes probar para depurarla.

## Antes de desplegar

### Preview

Antes de desplegar tu escena en Decentraland, asegúrate de que la escena funcione bien en Preview usando la última versión del SDK de Decentraland. Consulta [depurar en Preview](/docs/creator/content-creator-es/scenes-sdk7/depuracion/debug-in-preview/).

## Recarga rápida

Si necesitas recargar la escena sobre la que estás parado, escribe lo siguiente en el chat y presiona Enter:

`/reload`

## Acerca del pipeline de publicación

Ten en cuenta que después de cada publicación, un proceso interno optimiza todos los modelos 3D antes de que puedan renderizarse. Esto suele tardar solo unos segundos, pero puede tardar más en escenas muy grandes o cuando los servidores están ocupados. Hasta que termine, a los jugadores se les sirve la versión anterior de tu escena.

Puedes comprobar el estado actual de este proceso para tu escena usando [esta herramienta](https://decentraland.github.io/opscli/). Si la conversión está completa, las tres variaciones de los assets deberían tener luces verdes.

## Logs y métricas del Multiplayer Server

Para una [Multiplayer Server](/docs/creator/content-creator-es/scenes-sdk7/networking/authoritative-servers/) scene desplegada, usa [Scene Insights](/docs/creator/content-creator-es/scenes-sdk7/depuracion/scene-insights/) para buscar en los últimos 7 días de logs del servidor, seguir nuevas líneas en vivo e inspeccionar métricas de CPU y memoria. Incluye el inicio de sesión con wallet y comandos que tú o tu asistente de IA pueden ejecutar desde el proyecto de la escena.

## Logs de la escena

Cuando usas Decentraland normalmente, no es posible abrir la consola para comprobar mensajes de debug. Para que la consola esté disponible, debes abrir Decentraland con el `scene-console` parámetro. Luego puedes alternar la consola presionando la tecla de acento grave en tu teclado: **\`**. Esta tecla está a la izquierda de la tecla 1 en la mayoría de los teclados en inglés.

Para abrir Decentraland con el `scene-console` parámetro, o bien:

* Escribe el siguiente deep link en una ventana del navegador: `decentraland://?position=0,0&scene-console=true`. Esto abrirá la aplicación de escritorio de Decentraland si la tienes instalada.
* Escribe lo siguiente en la línea de comandos:
  * **macOS**: `open Decentraland.app --args --position 0,0 --scene-console true`
  * **winOS**: `"C:\\Users\\[YOUR-USER]\\Downloads\\Decentraland_windows64\\Decentraland.exe" --position 0,0 --scene-console true`


> [!NOTE]
> **💡 Consejo**: Cambia el **position** parámetro por las coordenadas de tu escena, para cargar directamente en tu escena.


Cuando ejecutas Decentraland con el `scene-console`, puedes abrir la consola de tres maneras:

* Haz clic en el ![](/docs/_remote/92e8eb247fcf289c2feff8f8.webp) icono en la esquina superior derecha
* Presiona la **\`** tecla para una consola breve
* Presiona Shift + **\`** para abrir una vista más grande de la consola

Ten en cuenta que los mensajes de cada escena activa se registrarán en la consola, así que algunas de las cosas que veas en la consola podrían no ser relevantes para tu escena.

## Reportar un bug

Si encuentras un problema que no está en tu escena, sino en el SDK de Decentraland en general, consulta [Reportar un bug](/docs/creator/content-creator-es/scenes-sdk7/depuracion/report-bug/).


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-es/scenes-sdk7/depuracion/debug-in-prod.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
