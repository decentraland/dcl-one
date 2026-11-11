> Guías sobre cómo probar SDK7 y dar feedback

# Pruebas y feedback

## Documentación

Encuentra la documentación completa de SDK7 en la **SDK7** sección de la documentación.

## Usar el playground

Para probar SDK7 fácilmente, visita [el playground](https://playground.decentraland.org/). Escribe código de SDK7 en la parte izquierda de la pantalla y mira la escena resultante a la derecha. A medida que cambias el código, la escena se actualiza automáticamente.

Si encuentras algún problema, copia una URL de la configuración actual de tu escena con el **Compartir** botón, y [reporta un problema](#reporting-issues) incluyendo ese enlace a tu código.

## Usando SDK 7

Instala el [Creator Hub](/docs/creator/content-creator-es/scene-editor/primeros-pasos/about-editor/).


> [!WARNING]
> **📔 Nota**: Ten en cuenta que las escenas que usan SDK7 no tienen el `decentraland-ecs` paquete instalado. En su lugar, tienen el `@dcl/sdk` paquete.


## En producción

Puedes desplegar escenas con SDK7 a través del **Publicar** botón en el Scene Editor en Creator Hub.

## Escenas de ejemplo

Puedes encontrar más escenas de ejemplo hechas con SDK7 aquí:

* [Ejemplos de escenas](/docs/creator/content-creator-es/tutoriales-y-ejemplos/examples/)

## Reporte de problemas

Si encuentras un problema, consulta [Reportar un error](/docs/creator/content-creator-es/scenes-sdk7/depuracion/report-bug/).


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-es/scenes-sdk7/referencia-de-api/alpha-testing-guidelines.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
