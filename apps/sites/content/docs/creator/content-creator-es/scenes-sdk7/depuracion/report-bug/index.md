> Cómo puedes reportar un error para que sea revisado y corregido

# Reportar un error

Si encuentras cualquier bug o problema con el Decentraland SDK y el Scene Editor en Creator Hub, por favor repórtalo en nuestro repositorio de GitHub [aquí](https://github.com/decentraland/sdk/issues/new/choose).

## Antes de reportar un bug

Antes de enviar un informe de bug, por favor asegúrate de lo siguiente:

* Asegúrate de estar usando la última versión del SDK, por si el problema ya se hubiera solucionado en versiones más nuevas.
* Revisa la documentación de la función que estás intentando usar, para confirmar que se supone que debe comportarse como esperas, y que esto no sea solo un malentendido.
* Descarta que el problema pueda ser causado por cómo está construida tu scene. Consulta [Debug in Preview](/docs/creator/content-creator-es/scenes-sdk7/depuracion/debug-in-preview/) y [Debug in Prod](/docs/creator/content-creator-es/scenes-sdk7/depuracion/debug-in-prod/) para obtener consejos.
* Evita duplicados. Revisa la [lista de bugs conocidos](https://github.com/orgs/decentraland/projects/20/views/13) para evitar crear un duplicado. Si tienes más información para añadir a un issue existente, por favor añade un comentario al issue en lugar de crear un issue nuevo.

## Reportar un bug

Para reportar un bug con el SDK, necesitarás una cuenta de GitHub. Sigue estos pasos:

1. Visita el siguiente enlace: [Página de nuevo issue](https://github.com/decentraland/sdk/issues/new/choose)
2. Haz clic en **Comenzar** junto a **Informe de bug**. ![](/docs/_remote/b51c88e272f5593a42575857.webp)
3. Completa todos los campos de la plantilla. La plantilla proporciona instrucciones para cada sección. Cuantos más detalles puedas proporcionar, más rápido nuestros developers podrán identificar y resolver el problema.
4. Haz clic en **Enviar nuevo issue**.

![](/docs/_remote/dc32a849c6f1a59a64d4565b.webp)

Los developers pueden hacer preguntas de seguimiento en tu ticket si necesitan aclaraciones o información adicional. Recibirás notificaciones por email sobre las actualizaciones de tu informe de bug.


> [!WARNING]
> **📔 Nota**: Para cualquier problema no relacionado con el SDK, por favor contacta al equipo de soporte. Para comunicarte, visita esta página [intercom.decentraland.org](https://intercom.decentraland.org/) o envíanos un correo a <hello@decentraland.org>. También puedes encontrarnos en [Discord](https://decentraland.org/discord/). El equipo está disponible para ayudar de lunes a domingo, de 12 p. m. a 9 p. m. UTC y hará todo lo posible por responder lo antes posible. Puedes esperar una respuesta en un plazo de 60 min en Discord durante ese horario y en un plazo de 24 h por email.
> 
> También puedes contactar al equipo de soporte del DAO en el [Discord del DAO](https://discord.gg/bxHtcMxUs4).


**Consejos adicionales para los informes de bugs:**

* Crea un issue separado para cada bug. Si encuentras varios problemas, asegúrate de crear un nuevo issue para cada uno.
* Evita explicaciones extensas. Incluye solo la salida relevante de la línea de comandos o de la consola, o el código de la scene cuando sea necesario.
* Adjunta imágenes o videos si es posible. Puedes arrastrarlos y soltarlos fácilmente en el área de texto del issue, y GitHub se encargará de la carga y el enlace.

## Uso del Decentraland Playground

Proporcionar fragmentos de código que demuestren el problema es extremadamente útil. La reproducibilidad es crucial. Si un bug no puede reproducirse, es difícil diagnosticarlo y corregirlo.

Puedes pegar fragmentos de código dentro del texto de tu issue, pero la mejor forma de proporcionar fragmentos de código es usando el [Decentraland Playground](https://playground.decentraland.org/). Sigue estos pasos:

1. Escribe tu fragmento de código a la izquierda y visualiza tu scene en el panel derecho.
2. Haz clic en el **botón** y pega el enlace en tu informe de issue.

![](/docs/_remote/14f730a1e519a0ed272964d1.webp)

Usar el playground simplifica la reproducción del issue y garantiza que cualquiera que abra el enlace pueda ver la misma salida, independientemente de su sistema operativo, versión del SDK, versión de Node, etc. También facilita iterar rápidamente para identificar las condiciones afectadas y localizar la causa del issue.

Mantén tu ejemplo lo más conciso posible, sin dejar de reproducir el issue. Un ejemplo más simple ayuda a eliminar la ambigüedad sobre el origen del problema.


> [!WARNING]
> **📔 Nota**: No es posible importar assets a tu scene en el Playground. Así que, si reproducir el issue requiere importar un modelo 3D o un archivo de sonido, no será posible mostrarlo allí.


## Obteniendo logs

A menudo es útil obtener los logs del Explorer de Decentraland al reportar un bug.

Para obtener tus logs, simplemente abre el chat y escribe `/logs`. Esto abrirá una nueva ventana con los logs del Explorer de Decentraland. Adjunta el `Player.log` archivo a tu informe de issue.

## Editar la documentación

Si descubres problemas en el contenido del sitio de documentación, puedes enviar solicitudes de cambio. Simplemente desplázate hasta el final de la página y haz clic en el botón **Editar esta página** GitHub te guiará en la creación de un pull request con tus cambios. El equipo de la fundación revisará tus cambios y los publicará si considera que son útiles.

## Solicitudes de funciones y sugerencias

Si tienes sugerencias para añadir nueva funcionalidad o nuevas funciones en lugar de reportar un problema, por favor únete al [Servidor de Discord de Decentraland](https://dcl.gg/discord) y publica un mensaje en el [Canal de Creator Hub](https://discord.com/channels/417796904760639509/1288888172318560326) en lugar de crear un issue en GitHub.


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-es/scenes-sdk7/depuracion/report-bug.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
