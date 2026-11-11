> Resumen de la aplicación móvil de Decentraland para iOS y Android, con enlaces de descarga y una lista de verificación para preparar las escenas para móviles.

# Resumen

Decentraland ya está disponible en móvil. Los jugadores pueden explorar tus escenas desde dispositivos iOS y Android, además del cliente de escritorio y la web. Como creador, puedes adaptar tus escenas para que se vean geniales y funcionen bien en dispositivos táctiles.

![Aplicación móvil de Decentraland — Genesis City](/docs/_remote/a10f68e6dd27dfa895f53969.webp)



<div class="video-embed"><button type="button" class="video-play" data-video-src="https://www.youtube-nocookie.com/embed/FBr2gye3qh8" aria-label="Play YouTube video"><svg viewBox="0 0 64 64" width="64" height="64" aria-hidden="true"><circle cx="32" cy="32" r="30" fill="currentColor"/><path d="m26 20 20 12-20 12Z" fill="#181214"/></svg><span>Play video</span></button></div>
<p class="video-source"><a href="&lt;https://www.youtube.com/watch?v=FBr2gye3qh8>">Watch on YouTube</a></p>



## Obtén la aplicación móvil

* [Descargar para iOS (App Store)](https://apps.apple.com/app/decentraland/id6478403840?utm_source=docs&utm_medium=internal&utm_content=ios)
* [Descargar para Android (Google Play)](https://play.google.com/store/apps/details?id=org.decentraland.godotexplorer&pcampaignid=web_share&utm_source=docs&utm_medium=internal&utm_content=android)

## Desarrollar para móvil

### Referencia

* [Escenas de ejemplo](/docs/creator/content-creator-es/desarrollar-para-movil/cliente-movil/sample-scenes/) — escenas de código abierto creadas por el equipo de Decentraland y optimizadas para jugar en móvil.
* [Funciones que faltan](/docs/creator/content-creator-es/desarrollar-para-movil/cliente-movil/missing-features/) — funciones disponibles en el cliente de escritorio que aún no son compatibles con la app móvil.
* [Requisitos de hardware](/docs/creator/content-creator-es/desarrollar-para-movil/cliente-movil/hardware-requirements/) — especificaciones mínimas y recomendadas de hardware para ejecutar Decentraland en móvil.

### Desarrollar

* [Detectar la plataforma desde el código](/docs/creator/content-creator-es/desarrollar-para-movil/desarrollar/detect-platform/) — usa `isMobile()` para ramificar la lógica y la UI de tu escena por plataforma.
* [Previsualiza tu escena en móvil](/docs/creator/content-creator-es/desarrollar-para-movil/desarrollar/preview-on-mobile/) — previsualiza directamente en un teléfono a través de Creator Hub o la CLI.
* [área segura móvil](/docs/creator/content-creator-es/desarrollar-para-movil/desarrollar/safe-area/) — los márgenes de hardware del dispositivo (notch, indicador de inicio) y las regiones que ocupan los controles propios del cliente; elige el área en la que se coloca tu UI con `screenInset`.
* [mejores prácticas de UI para móvil.](/docs/creator/content-creator-es/desarrollar-para-movil/desarrollar/ui-best-practices/) — qué hacer y qué no hacer, recomendaciones de tamaño y limitaciones actuales.
* [Entrada en móvil](/docs/creator/content-creator-es/desarrollar-para-movil/desarrollar/input-on-mobile/) — asignaciones de entrada adaptadas al tacto y cuáles `InputAction`s que evitar.
* [Optimizar el rendimiento](/docs/creator/content-creator-es/desarrollar-para-movil/desarrollar/optimize-performance/) — límites de escenas móviles, cómo previsualizarlos en Creator Hub y objetivos de rendimiento.

### Publicar

* [Aparece destacado en Discover móvil](/docs/creator/content-creator-es/desarrollar-para-movil/publicar/get-featured/) — requisitos de envío para la sección Discover móvil.
* [Curación de iOS](/docs/creator/content-creator-es/desarrollar-para-movil/publicar/ios-curation/) — cómo funciona la curación de contenido en iOS y cómo enviar tu escena o World para revisión.

## Lista de verificación rápida

Antes de publicar una escena que deba funcionar bien en móvil, asegúrate de que:

* [ ] Has previsualizado la escena en un dispositivo real — consulta [Previsualiza tu escena en móvil](/docs/creator/content-creator-es/desarrollar-para-movil/desarrollar/preview-on-mobile/).
* [ ] Todos los elementos críticos de UI permanecen dentro del [área segura móvil](/docs/creator/content-creator-es/desarrollar-para-movil/desarrollar/safe-area/).
* [ ] Tu UI tiene un tamaño lo suficientemente grande para el tacto — sigue las [mejores prácticas de UI](/docs/creator/content-creator-es/desarrollar-para-movil/desarrollar/ui-best-practices/).
* [ ] Las acciones clave no están vinculadas a `IA_SECONDARY` o `IA_ACTION_3`–`IA_ACTION_6` (la `F` y `1`/`2`/`3`/`4` teclas), cuyos botones están ocultos por defecto en móvil, a menos que tu escena los muestre — consulta [Entrada en móvil](/docs/creator/content-creator-es/desarrollar-para-movil/desarrollar/input-on-mobile/).
* [ ] De tu escena [Rendimiento](/docs/creator/content-creator-es/desarrollar-para-movil/desarrollar/optimize-performance/) supera el 90 % en el perfil de alta calidad gráfica en un teléfono de gama media — consulta [Requisitos de hardware](/docs/creator/content-creator-es/desarrollar-para-movil/cliente-movil/hardware-requirements/) para dispositivos de referencia.


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-es/desarrollar-para-movil/cliente-movil/overview.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
