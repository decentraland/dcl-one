> Comportamientos reutilizables de NPC y UI de diálogo para escenas SDK7.

# NPC Toolkit

Usa comportamientos de NPC reutilizables y UI de diálogo para escenas de SDK7.

## Información del proyecto

**Mantenido por:** Decentraland

[Ver fuente](https://github.com/decentraland-scenes/dcl-npc-toolkit)

**Licencia:** [Apache-2.0](https://github.com/decentraland-scenes/dcl-npc-toolkit/blob/main/LICENSE). Consulta los términos upstream antes de reutilizarlo; el código y los assets pueden tener términos distintos.

## Explora el código

Estos enlaces apuntan a una instantánea específica de la fuente para que las explicaciones sigan siendo útiles a medida que continúa el desarrollo upstream. Son referencias de aprendizaje, no una evaluación de seguridad.

### Separar revelar el diálogo de avanzar en él

`handleDialogClick`, `rushText` y `confirmText` primero revela los caracteres restantes si el texto todavía se está escribiendo. La entrada posterior avanza el script o selecciona una rama de diálogo numérica, con callbacks para acciones del juego.

**Contexto del proyecto:** Lee esto junto con los `Dialog` datos y `npcDialogComponent` estado para entender cómo la entrada de UI dirige una conversación.

**Referencias de la fuente:**

* [`handleDialogClick`, `rushText` y `confirmText`](https://github.com/decentraland-scenes/dcl-npc-toolkit/blob/60e327c67a126d92e47ecf8e7eda342ce1191076/src/dialog.ts#L489-L549)

### Conecta la proximidad y la entrada del puntero a una interacción con NPC

`addClickReactions` y `addTriggerArea` configura callbacks del puntero y un trigger de esfera hijo, con ajustes para activación solo por clic y para continuar el diálogo cuando el jugador se aleja. `activate` y `endInteraction` coordina el NPC activo, el tiempo de reutilización, el diálogo y el comportamiento opcional orientado al jugador.

**Contexto del proyecto:** Estas funciones dependen de los mapas de callbacks y del sistema de seguimiento del toolkit.

**Referencias de la fuente:**

* [`addClickReactions` y `addTriggerArea`](https://github.com/decentraland-scenes/dcl-npc-toolkit/blob/60e327c67a126d92e47ecf8e7eda342ce1191076/src/npc.ts#L410-L531)
* [`activate` y `endInteraction`](https://github.com/decentraland-scenes/dcl-npc-toolkit/blob/60e327c67a126d92e47ecf8e7eda342ce1191076/src/npc.ts#L769-L823)


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-es/tutoriales-y-ejemplos/ui-camera-player/npc-toolkit.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
