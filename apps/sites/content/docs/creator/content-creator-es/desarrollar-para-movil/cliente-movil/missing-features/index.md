> Funciones disponibles en el cliente de escritorio de Decentraland que aún no son compatibles con la aplicación móvil, e inconsistencias conocidas entre plataformas.

# Funciones faltantes


> [!NOTE]
> Esta página hace seguimiento de la diferencia de funciones entre el cliente de escritorio (Unity) de Decentraland y la app móvil. Tiene como fuente el [rastreador de paridad de funciones de godot-explorer](https://github.com/decentraland/godot-explorer/issues/2402) y se actualiza regularmente. Las ETA son estimaciones y están sujetas a cambios.
> 
> Última revisión: **octubre de 2026** (versión v1.15.0 de la app móvil).


## Novedades lanzadas recientemente

Estas ya no son diferencias a partir de la app móvil **v1.15.0**, que se está desplegando en las tiendas de apps en octubre de 2026:

* [`stopEmote` acción restringida](/docs/creator/content-creator-es/scenes-sdk7/interactividad/avatares/avatar-animations/#animate-only-the-upper-body). Los emotes de la parte superior del cuerpo iniciados por una escena también se pausan cuando el jugador sale de la escena, se reanudan al volver y se detienen cuando la escena se descarga — v1.15.0
* [`Tween` `MoveRotateScale` mode](/docs/creator/content-creator-es/scenes-sdk7/fundamentos-del-contenido-3d/move-entities/#simultaneous-tweens) — v1.15.0
* Las posiciones de otros jugadores se reportan a las escenas en relación con la escena, como la del jugador local (`getPlayer({ userId }).position`, `onEnterScene`, elementos adjuntos a otros jugadores) — v1.15.0
* Movimiento del jugador, primer pase de paridad con Unity: tamaño de la cápsula, altura del escalón, arco de salto y gravedad, velocidad de carrera y control en el aire — v1.15.0
* Los raycasts hacia abajo desde el jugador reportan la correcta `hitDistance`  en geometría inclinada — v1.15.0
* Los emotes de la parte superior del cuerpo se ven como de la parte superior del cuerpo para otros jugadores móviles — v1.15.0

## Funciones de SDK que faltan en móvil

* [Fuentes personalizadas (`fontSrc` en `TextShape`, `Label`, `Button`, `Input` y `Dropdown`)](/docs/creator/content-creator-es/scenes-sdk7/ui-2d/ui_text/#custom-fonts) — la app móvil ignora `fontSrc` y renderiza el respaldo `font` — Sin ETA
* [`AvatarNametag` (etiqueta creada por la escena encima del nametag de un jugador)](https://github.com/decentraland/godot-explorer/issues/2811) — Sin ETA
* [secuencias de Tween (`TweenSequence`)](/docs/creator/content-creator-es/scenes-sdk7/fundamentos-del-contenido-3d/move-entities/#tween-sequences) — no implementadas; los tweens secuenciados no se animan — Sin ETA
* [los mosaicos de nueve cortes de SDK7 UiBackground en lugar de estirarse](https://github.com/decentraland/godot-explorer/issues/2060) — Sin ETA
* Compresión de malla Draco (`KHR_draco_mesh_compression`) — la app móvil no tiene un decodificador Draco en runtime. Los modelos comprimidos con Draco `.gltf`/`.glb` se cargan solo desde los assets optimizados de la escena, donde el servidor de assets ya los ha descomprimido. Cuando una escena no tiene assets optimizados, como en la vista previa local, no logran cargarse sin un error. Exporta tus modelos sin compresión Draco para estar seguro. Un diálogo de error para este caso se está rastreando en [#2432](https://github.com/decentraland/godot-explorer/issues/2432) — Sin ETA
* Smart Items — no oficialmente compatibles en móvil

## Funciones del cliente de escritorio que no están en móvil

* [Chat de voz de proximidad](https://github.com/decentraland/godot-explorer/issues/888) — T1 2027
* [Señalar en el mundo](https://github.com/decentraland/godot-explorer/issues/1736) — Sin ETA
* [Cambio de color de la placa de nombre](https://github.com/decentraland/godot-explorer/issues/1684) — Sin ETA
* [Comunidades](https://github.com/decentraland/godot-explorer/issues/656) — T1 2027
* [Galería de fotos](https://github.com/decentraland/godot-explorer/issues/680) — diciembre de 2026
* [Transmisiones de la comunidad](https://github.com/decentraland/godot-explorer/issues/676) — T1 2027
* [Insignias de perfil](https://github.com/decentraland/godot-explorer/issues/678) — diciembre de 2026
* [Misiones diarias](https://github.com/decentraland/godot-explorer/issues/682) — Sin ETA
* [Reacciones en el chat](https://github.com/decentraland/godot-explorer/issues/1824) — T1 2027
* [Traducción automática del chat](https://github.com/decentraland/godot-explorer/issues/2260) — T1 2027
* [Chat: mensajes directos](https://github.com/decentraland/godot-explorer/issues/1120) — T1 2027
* [Compatibilidad con DCL Cast en móvil](https://github.com/decentraland/godot-explorer/issues/1881) — Sin ETA
* [URLs de video de YouTube / Google Drive no compatibles](https://github.com/decentraland/godot-explorer/issues/2081) — Restringido en móvil por cumplimiento de la tienda
* [Ranuras de vestimenta en la mochila](https://github.com/decentraland/godot-explorer/issues/1625) — diciembre de 2026
* [Regalar wearables a otros jugadores](https://github.com/decentraland/godot-explorer/issues/2957) — diciembre de 2026
* [aviso de "Sesión terminada" cuando la misma cuenta inicia sesión en otro dispositivo](https://github.com/decentraland/godot-explorer/issues/2880) — Sin ETA
* [Categorías de Descubrir (explora experiencias por categoría)](https://github.com/decentraland/godot-explorer/issues/3037) — Sin ETA

## Inconsistencias entre plataformas

* **Movimiento del jugador y colisiones frente a Unity**
  * [Pendientes, bordes, paredes, aterrizaje y plataformas móviles](https://github.com/decentraland/godot-explorer/issues/2852) y [comportamiento de deslizamiento](https://github.com/decentraland/godot-explorer/issues/2854) — corrección en revisión, sin ETA
  * [Los colliders de la escena más delgados que 1 cm solo colisionan desde arriba, y los bloqueadores invisibles y algunas capas de colisión del SDK no bloquean al jugador](https://github.com/decentraland/godot-explorer/issues/2853) — Sin ETA
  * Animaciones de aparición, aterrizaje y salto, blending de movimiento ([#1553](https://github.com/decentraland/godot-explorer/issues/1553), [#1554](https://github.com/decentraland/godot-explorer/issues/1554), [#2855](https://github.com/decentraland/godot-explorer/issues/2855), [#2856](https://github.com/decentraland/godot-explorer/issues/2856)) — corrección en revisión, sin ETA
* [Algunos emotes de otros clientes no se reproducen en móvil: emotes activados por la escena de jugadores de Bevy Web, y emotes de jugadores de Unity que llegan a través de LiveKit](https://github.com/decentraland/godot-explorer/issues/2986) — corrección en revisión, sin ETA
* [En una vista previa móvil de Creator Hub, el avatar del jugador móvil no es visible para los testers de escritorio](https://github.com/decentraland/godot-explorer/issues/2773) — corrección en curso, sin ETA
* [Los dientes del avatar se renderizan oscuros/grises en lugar de blancos](https://github.com/decentraland/godot-explorer/issues/1994) — Sin ETA

## Restricciones de entrada/plataforma

* **Entrada solo táctil** — sin estados de hover del mouse, atajos de teclado ni clic derecho.
* **Soporte limitado para gestos** — pellizcar para hacer zoom (cambiar entre cámara en primera y tercera persona) se añadió en v1.14.0; actualmente no hay otros gestos previstos.

## Informar una función que falta

Si encuentras una limitación que no aparece aquí, por favor [repórtala](/docs/creator/content-creator-es/scenes-sdk7/depuracion/report-bug/) para que podamos documentarla y priorizarla.


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-es/desarrollar-para-movil/cliente-movil/missing-features.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
