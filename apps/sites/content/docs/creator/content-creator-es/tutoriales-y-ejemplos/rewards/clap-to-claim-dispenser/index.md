> Un dispensador de wearable que los jugadores reclaman haciendo el emote clap en un área de trigger.

# Dispensador de reclamo con clap

Permite que los jugadores reclamen un wearable haciendo el emote clap frente a un dispenser, un mecanismo usado en Metaverse Fashion Week. El README recorre cómo apuntar el dispenser a tu propia campaña de Rewards.

## Información del proyecto

**Mantenido por:** Decentraland

[Ver fuente](https://github.com/decentraland/sdk7-goerli-plaza/tree/main/clap-claim-dispenser)

**Licencia:** No identificado en este catálogo. Consulta el repositorio upstream y al mantenedor para conocer los términos aplicables antes de reutilizarlo.

## Explora el código

Estos enlaces apuntan a una instantánea específica de la fuente para que las explicaciones sigan siendo útiles a medida que continúa el desarrollo upstream. Son referencias de aprendizaje, no una evaluación de seguridad.

### Activa un dispenser mientras el jugador está frente a él

Cada dispenser almacena su campaña en un `DispenserComponent`. Una esfera `TriggerArea` frente a él establece `isActive` y muestra una notificación emergente de "clap to claim" al entrar, y borra ambos al salir.

**Contexto del proyecto:** El ID de la campaña y la clave del dispenser provienen de `ClaimConfig` en `src/modules/claiming/claimConfig.ts`.

**Referencias de la fuente:**

* [`createDispenserWithWearable`](https://github.com/decentraland/sdk7-goerli-plaza/blob/54a07775ee340f0ce22eb23280ef0e9c35f5e386/clap-claim-dispenser/src/modules/dispensers.ts#L28-L89)

### Reclama cuando el jugador reproduce el emote clap

`initClapToClaim` observa `AvatarEmoteCommand` en la Entity del player. Cuando el emote es `clap`, espera a que termine la animación y reclama de cada dispenser que esté activo en ese momento.

**Contexto del proyecto:** La gestión de la solicitud y la respuesta de claim sigue el mismo flujo de Rewards que el ejemplo del dispenser de reclamo de recompensas.

**Referencias de la fuente:**

* [`initClapToClaim`](https://github.com/decentraland/sdk7-goerli-plaza/blob/54a07775ee340f0ce22eb23280ef0e9c35f5e386/clap-claim-dispenser/src/modules/clapToClaim.ts#L6-L33)


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-es/tutoriales-y-ejemplos/rewards/clap-to-claim-dispenser.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
