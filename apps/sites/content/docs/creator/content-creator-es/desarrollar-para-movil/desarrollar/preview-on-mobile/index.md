> Previsualiza tu escena en un dispositivo móvil real desde el Creator Hub o el CLI.

# Vista previa en móvil

Puedes previsualizar tu escena directamente en la app móvil de Decentraland desde Creator Hub o desde la línea de comandos. Esta es la única forma fiable de confirmar que tu UI, la gestión de input y el rendimiento se mantienen en un dispositivo real.

## Requisitos previos

* La app móvil de Decentraland instalada en tu teléfono — consulta los [enlaces de descarga](/docs/creator/content-creator-es/desarrollar-para-movil/cliente-movil/overview/#get-the-mobile-app).
* El teléfono y tu máquina de desarrollo deben estar en la **misma red local** (Wi-Fi). La vista previa se sirve desde tu computadora; el código QR enlaza a una URL de la LAN.

## Opción A — Desde Creator Hub

1. Abre tu escena en Creator Hub.
2. Haz clic en la flecha junto al **Reproducir** botón para abrir **Opciones de reproducción**, luego elige **Mostrar código QR para móvil**.
3. Escanea el código QR mostrado con la cámara de tu teléfono. El enlace abre la app móvil de Decentraland y carga la vista previa de tu escena. Haz clic en el área oscura alrededor del popup para cerrarlo.

<figure><img src="/docs/_remote/c6aa2bdbe9bdd432e1e7e96d.webp" alt="Creator Hub Play Options menu with the Show QR Code for Mobile option"><figcaption><p>La opción "Show QR Code for Mobile" en el menú Play Options de Creator Hub.</p></figcaption></figure>

## Opción B — Desde la línea de comandos

Desde la raíz de tu proyecto de escena, ejecuta:

```bash
npm run start -- --mobile
```

La CLI imprime un código QR en la terminal que apunta a la URL de la LAN de tu escena. Escanéalo con tu teléfono para cargar la escena en la app móvil de Decentraland.

<figure><img src="/docs/_remote/ce46ee596043fea52191ae3d.webp" alt="QR code printed by the Decentraland CLI for mobile preview"><figcaption><p>Un código QR impreso por <code>npm run start -- --mobile</code>. Escanéalo con tu teléfono para abrir la escena en la app móvil de Decentraland.</p></figcaption></figure>


> [!NOTE]
> **💡 Consejo**: Cuando pasas `--mobile`, el Explorer de escritorio tampoco se inicia. Si quieres probar ambos a la vez, ejecuta `npm run start` en una terminal para escritorio y `npm run start -- --mobile` en otra para móvil.


## Recarga en caliente

Al igual que con la vista previa de escritorio, la vista previa móvil se recarga automáticamente cuando cambias archivos de la escena. No necesitas volver a escanear el código QR después de cada edición.

## Solución de problemas

* **El código QR no abre la app** — asegúrate de que la app de Decentraland esté instalada y de haberla abierto al menos una vez en el dispositivo.
* **El teléfono no puede acceder al servidor de vista previa** — confirma que el teléfono y la máquina de desarrollo estén en la misma red Wi-Fi. Algunas redes corporativas aíslan a los clientes; si es así, usa un punto de acceso personal o una red diferente.
* **La escena se carga en el teléfono pero se ve diferente que en escritorio** — eso es esperado. Usa esto como el momento para validar tu [área segura](/docs/creator/content-creator-es/desarrollar-para-movil/desarrollar/safe-area/), [dimensionamiento de la UI](/docs/creator/content-creator-es/desarrollar-para-movil/desarrollar/ui-best-practices/#sizing), y [asignaciones de input](/docs/creator/content-creator-es/desarrollar-para-movil/desarrollar/input-on-mobile/).

## Relacionado

* [Previsualiza tu escena](/docs/creator/content-creator-es/scenes-sdk7/primeros-pasos/preview-scene/)
* [Uso de la CLI](/docs/creator/content-creator-es/scenes-sdk7/primeros-pasos/using-the-cli/)
* [Aparece destacado en Discover móvil](/docs/creator/content-creator-es/desarrollar-para-movil/publicar/get-featured/)


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-es/desarrollar-para-movil/desarrollar/preview-on-mobile.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
