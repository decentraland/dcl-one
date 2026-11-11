> Cómo funciona la curación de contenido en iOS y cómo enviar tu escena o mundo para revisión.

# Curación de iOS

Para cumplir con [las Directrices de revisión de la App Store de Apple sobre contenido generado por usuarios (UGC)](https://developer.apple.com/app-store/review/guidelines/#user-generated-content), Decentraland ha introducido un **proceso de revisión manual** para todas las escenas y Worlds que aparecen en la **sección Discover de la app de iOS**.


> [!NOTE]
> Este proceso de revisión se aplica solo a iOS. La sección Discover y la búsqueda en Android siguen mostrando todo el contenido publicado sin restricciones.


## Cómo funciona

* Todo el contenido debe ser revisado manualmente antes de aparecer en la sección Discover de la app de iOS.
* Las escenas y Worlds que no sean aprobados se ocultarán de la sección Discover de iOS.
* Los eventos vinculados a escenas o Worlds no aprobados también se ocultarán en la sección Discover de iOS.
* **Todas las escenas siguen siendo accesibles** en iOS en cualquier momento mediante deep linking o los `/goto` y `/world` comandos de chat: la curación solo afecta la visibilidad en Discover.

## Envía tu escena o World para revisión en iOS

Para solicitar que tu escena o World sea revisado para incluirlo en la sección Discover de iOS, envía una **Solicitud de revisión de contenido** en el servidor de Discord de Decentraland:

1. Únete al [Discord de Decentraland](https://dcl.gg/discord).
2. Ve al **#rl-mobile-curation** canal.
3. Publica tu solicitud incluyendo:
   * Nombre de la escena o del World
   * Coordenadas o nombre del World (p. ej. `0,0` o `MyWorld.dcl.eth`)
   * Una breve descripción del contenido

Las solicitudes se revisan en **24 horas**. Se te notificará en el canal una vez que tu contenido haya sido aprobado o si se necesita algún seguimiento.

## Relacionado

* [Aparecer destacado](/docs/creator/content-creator-es/desarrollar-para-movil/publicar/get-featured/)
* [Buenas prácticas de UI para móvil](/docs/creator/content-creator-es/desarrollar-para-movil/desarrollar/ui-best-practices/)
* [Previsualizar en móvil](/docs/creator/content-creator-es/desarrollar-para-movil/desarrollar/preview-on-mobile/)


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-es/desarrollar-para-movil/publicar/ios-curation.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
