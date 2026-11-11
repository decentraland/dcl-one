> El servicio de Decentraland Rewards crea y entrega wearables y emotes a los jugadores mediante solicitudes HTTP, gestionando transacciones y comisiones.

# Resumen

El [Rewards dApp](https://decentraland.org/rewards/) es una poderosa herramienta que permite a los creadores recompensar a los usuarios con Decentraland Wearables o Emotes mediante solicitudes HTTP. Puede usarse para motivar a los usuarios, promocionar escenas o aumentar la retención. Las recompensas pueden otorgarse directamente desde una escena o desde un servidor.

El sistema Rewards gestiona la mayor parte de las complejidades involucradas en el minting de items en la blockchain, incluyendo:

* Agrupar múltiples items acuñados en una sola transacción para reducir el tiempo de espera y las comisiones
* Gestionar el pago de las comisiones de transacción
* Gestionar reintentos en caso de fallos de blockchain o reordenamientos
* Limitar el número de items acuñados por dirección de usuario
* Minimizar los exploits de automatización mediante un sistema de captcha
* Verificar la presencia del usuario en la escena mediante nodos Catalyst
* Monitorear el estado de la transacción
* Informar a los usuarios sobre el item que recibirán antes de que la transacción se confirme
* Emite Notifications cuando se otorgan y reciben recompensas
* ![](/docs/_remote/0aefd219f02433f2b3736381.webp)

## Limitaciones

Aunque el servicio ofrece algunas protecciones contra la automatización para evitar que una sola persona acuñe Wearables masivamente, es imposible estar 100% seguro frente a exploits. **NO se recomienda usar Rewards para acuñar Wearables/Emotes con un nivel de rareza más escaso que \[EPIC]\(**[**Ver documentación**](/docs/creator/content-creator-es/readme/)**#rarity) (100 copias o menos)**.


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-es/recompensas/overview.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
