> Cómo las acciones de entrada se mapean al toque en el cliente móvil de Decentraland.

# Entrada en móvil

El sistema de entrada de Decentraland está diseñado para ser independiente del dispositivo. El mismo `InputAction` enum que el SDK expone en escritorio también se enruta desde los controles en pantalla en móvil, por lo que la mayoría de las escenas funcionan sin cambios. Sin embargo, hay algunas reglas y detalles que vale la pena conocer cuando estés desarrollando para touch.

Para ver el modelo de entrada completo, consulta [Eventos de clic](/docs/creator/content-creator-es/scenes-sdk7/interactividad/eventos-de-button/click-events/). Esta página se centra en lo específico de móvil.

<figure><img src="/docs/_remote/d29f6b96a1d1db9049d689d6.webp" alt="The on-screen controls with the + menu expanded, showing every input button"><figcaption><p>Los controles en pantalla con el menú "+" desplegado — todas las acciones de entrada visibles</p></figcaption></figure>

## A qué corresponde touch

Cada `InputAction` que existe en escritorio también está disponible en el cliente móvil — los controles en pantalla simplemente enrutan el touch a las mismas entradas del SDK. Esto significa que cualquier escena que dependa de los `InputAction` valores estándar funcionará en móvil sin necesidad de cambios.

Los controles en pantalla se asignan de la siguiente manera:

* **Joystick en pantalla** — controla `IA_FORWARD` / `IA_BACKWARD` / `IA_LEFT` / `IA_RIGHT` y el movimiento del avatar.
* **Botón de interacción (abajo a la derecha)** — activa `IA_POINTER` (el botón izquierdo del mouse en escritorio) contra lo que el jugador esté apuntando.
* **Botón E** — activa `IA_PRIMARY` (la `E` tecla en escritorio).
* **Botón F** — activa `IA_SECONDARY` (la `F` tecla en escritorio). Oculto por defecto (ver abajo).
* **Botón de salto** — activa `IA_JUMP` (la `Space` tecla en escritorio). Este es el botón más grande del HUD y siempre es fácil de alcanzar.
* **Botones 1 / 2 / 3 / 4** — activan `IA_ACTION_3` / `IA_ACTION_4` / `IA_ACTION_5` / `IA_ACTION_6` respectivamente. Ocultos por defecto (ver abajo).
* **Arrastrar la cámara** — rota la cámara; no se expone como un `InputAction`.

## Personalización de controles

Las escenas pueden reconfigurar los controles en pantalla — ocultar el joystick o la mira, ocultar cualquier botón (incluido el de salto), cambiar qué hace el botón central, intercambiar los íconos de los botones o reemplazar por completo los controles nativos con UI personalizada. Consulta [Controles en pantalla](/docs/creator/content-creator-es/scenes-sdk7/interactividad/touch-screen-controls/) y [Vinculación de entrada de UI](/docs/creator/content-creator-es/scenes-sdk7/ui-2d/ui_input_binding/).

<figure><img src="/docs/_remote/c911e3ac3460b2381dd7c1b5.webp" alt="Customized on-screen controls"><figcaption><p>Un HUD personalizado: un botón central con un nuevo ícono asignado a una acción diferente</p></figcaption></figure>

## Entradas que se deben evitar para acciones clave en móvil

Cada `InputAction` pueden enrutar a un botón en pantalla, pero una escena creada con SDK 7.32.0 o posterior comienza mostrando solo tres de ellos: el botón de interacción, el botón E y el botón de salto. `IA_SECONDARY` (F) y `IA_ACTION_3`–`IA_ACTION_6` (la `1`/`2`/`3`/`4` botones) están **ocultos por defecto**, así que una acción de tecla vinculada a uno de ellos no tiene botón en un teléfono a menos que tu escena lo muestre. En SDKs anteriores se muestran los ocho botones, con los de menor prioridad detrás de un desbordamiento "+" que no es fácil de alcanzar durante la partida.

A menos que tu escena los muestre explícitamente con [Controles en pantalla](/docs/creator/content-creator-es/scenes-sdk7/interactividad/touch-screen-controls/), evita vincular acciones de tecla a:

* `IA_SECONDARY` — la `F` tecla en escritorio / `F` botón en móvil
* `IA_ACTION_3` — la `1` tecla en escritorio / `1` botón en móvil
* `IA_ACTION_4` — la `2` tecla en escritorio / `2` botón en móvil
* `IA_ACTION_5` — la `3` tecla en escritorio / `3` botón en móvil
* `IA_ACTION_6` — la `4` tecla en escritorio / `4` botón en móvil

Prefiere en su lugar:

* `IA_POINTER` para el botón principal de toque / interacción
* `IA_PRIMARY` para la acción del botón E
* `IA_JUMP` cuando una acción se asigna de forma natural al salto (es el botón más grande y más accesible del HUD móvil)
* Triggers basados en proximidad (consulta [Eventos de proximidad](/docs/creator/content-creator-es/scenes-sdk7/interactividad/eventos-de-button/proximity-events/)) cuando una acción debe activarse automáticamente a medida que el jugador se acerca a una entidad

## Bloqueo del cursor

El [`PointerLock` componente](/docs/creator/content-creator-es/scenes-sdk7/interactividad/eventos-de-button/click-events/#lock-or-unlock-the-cursor) es un concepto del cliente de escritorio (cursor del mouse bloqueado vs. desbloqueado). No se aplica a la interacción táctil en móvil y es seguro dejarlo en tu escena — no tiene efecto allí.

## Relacionado

* [Controles en pantalla](/docs/creator/content-creator-es/scenes-sdk7/interactividad/touch-screen-controls/)
* [Vinculación de entrada de UI](/docs/creator/content-creator-es/scenes-sdk7/ui-2d/ui_input_binding/)
* [Eventos de clic](/docs/creator/content-creator-es/scenes-sdk7/interactividad/eventos-de-button/click-events/)
* [Eventos de proximidad](/docs/creator/content-creator-es/scenes-sdk7/interactividad/eventos-de-button/proximity-events/)
* [Detectar la plataforma desde el código](/docs/creator/content-creator-es/desarrollar-para-movil/desarrollar/detect-platform/)
* [Buenas prácticas de UI para móvil](/docs/creator/content-creator-es/desarrollar-para-movil/desarrollar/ui-best-practices/)


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-es/desarrollar-para-movil/desarrollar/input-on-mobile.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
