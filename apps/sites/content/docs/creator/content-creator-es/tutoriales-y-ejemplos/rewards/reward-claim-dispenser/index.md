> Dispensadores de wearables y cajas de airdrop que reclaman recompensas de una campaña de Rewards.

# Dispensador de reclamo de recompensa

Entrega a los jugadores wearables de una campaña de Rewards de Decentraland. La escena incluye un dispenser de click-to-claim, un dispenser de botones y una palanca que airdropea cajas reclamables.

## Información del proyecto

**Mantenido por:** Decentraland

[Ver fuente](https://github.com/decentraland/sdk7-goerli-plaza/tree/main/reward-claim)

**Licencia:** [Apache-2.0](https://github.com/decentraland/sdk7-goerli-plaza/blob/main/reward-claim/LICENSE). Consulta los términos upstream antes de reutilizarlo; el código y los assets pueden tener términos distintos.

## Explora el código

Estos enlaces apuntan a una instantánea específica de la fuente para que las explicaciones sigan siendo útiles mientras continúa el desarrollo upstream. Son referencias de aprendizaje, no una evaluación de seguridad.

### Solicita una recompensa con un fetch firmado

`claimToken` limita los clics repetidos, omite los dispensers ya reclamados con éxito en esta sesión y comprueba que el jugador tenga una wallet. `requestToken` luego envía la clave del dispenser, el catalyst actual y la dirección del jugador al servidor de Rewards usando `signedFetch`.

**Contexto del proyecto:** Las pruebas están activadas por defecto (`CONFIG_CLAIM_TESTING_ENABLED` en `src/config.ts`), que usa una campaña de prueba incluida. Para usar la tuya, reemplaza el ID de campaña de marcador de posición y la clave del dispenser en `ClaimConfig` y desactiva las pruebas. Consulta la guía de inicio de Rewards para crear una campaña y un dispenser.

**Referencias de la fuente:**

* [`claimToken` y `requestToken`](https://github.com/decentraland/sdk7-goerli-plaza/blob/54a07775ee340f0ce22eb23280ef0e9c35f5e386/reward-claim/src/dispenser/claiming/claim.ts#L26-L99)
* [`ClaimConfig`](https://github.com/decentraland/sdk7-goerli-plaza/blob/54a07775ee340f0ce22eb23280ef0e9c35f5e386/reward-claim/src/dispenser/claiming/claimConfig.ts#L21-L34)

### Mapea las respuestas de Rewards a la retroalimentación del jugador

`processResponse` muestra una UI de dispenser diferente para campañas no disponibles, dispensers sin stock, rewards ya asignadas y reclamaciones exitosas. Las campañas no iniciadas, deshabilitadas y finalizadas comparten una sola rama, que muestra una pantalla de no iniciada o finalizada solo cuando la configuración de la campaña define `startDate` o `endDate`, y una pantalla final genérica en caso contrario.

**Contexto del proyecto:** Los estados de la UI se definen en `dispenserUi.tsx`. Las fechas de inicio y fin de la campaña son campos opcionales en la configuración de la campaña.

**Referencias de la fuente:**

* [`processResponse`](https://github.com/decentraland/sdk7-goerli-plaza/blob/54a07775ee340f0ce22eb23280ef0e9c35f5e386/reward-claim/src/dispenser/claiming/claim.ts#L101-L197)

### Reintenta una reclamación con un captcha

Cuando el servidor de Rewards solicita un captcha, `getCaptcha` obtiene una imagen de captcha y la muestra en la UI del dispenser. Enviar la respuesta llama a `claimToken` de nuevo con el ID y el valor del captcha.

**Contexto del proyecto:** Que se requiera un captcha es una configuración del dispenser en el dashboard de Rewards.

**Referencias de la fuente:**

* [`getCaptcha`](https://github.com/decentraland/sdk7-goerli-plaza/blob/54a07775ee340f0ce22eb23280ef0e9c35f5e386/reward-claim/src/dispenser/claiming/claim.ts#L209-L216)
* [Botón de enviar captcha](https://github.com/decentraland/sdk7-goerli-plaza/blob/54a07775ee340f0ce22eb23280ef0e9c35f5e386/reward-claim/src/dispenser/claiming/ui/dispenserUi.tsx#L640-L643)

### Reclama desde una caja enviada por airdrop

`randomCrateSpawn` suelta una caja con paracaídas en un punto aleatorio del área del escenario. `DropCrate` lanza raycasts hacia abajo para encontrar el suelo y hace un tween de la caja hasta él, y al hacer clic en la caja se reclama la recompensa y desaparece.

**Contexto del proyecto:** La caída de la caja se activa mediante la `Activate` acción de una palanca de Creator Hub en `main`.

**Referencias de la fuente:**

* [`randomCrateSpawn` y `SpawnCrate`](https://github.com/decentraland/sdk7-goerli-plaza/blob/54a07775ee340f0ce22eb23280ef0e9c35f5e386/reward-claim/src/crate.ts#L40-L119)
* [`DropCrate`](https://github.com/decentraland/sdk7-goerli-plaza/blob/54a07775ee340f0ce22eb23280ef0e9c35f5e386/reward-claim/src/crate.ts#L142-L189)


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-es/tutoriales-y-ejemplos/rewards/reward-claim-dispenser.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
