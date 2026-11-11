> Limpieza cooperativa de discoteca con sondeo compartido de componentes y props adjuntos al avatar que reflejan los objetos transportados.

# Clean The Club

![Vista previa de Clean The Club](/docs/_remote/a8f103a70354f2dfa74d73d7.webp)

Un juego cooperativo de limpieza en el que los jugadores compiten para restaurar una discoteca antes de la próxima fiesta.

* [Juega en Decentraland](https://decentraland.org/jump?realm=cleantheclub.dcl.eth)

## Información del proyecto

**Mantenido por:** Comunidad · CleanTheClub

[Ver la fuente upstream](https://github.com/CleanTheClub/CleanTheClub)

**Licencia:** [Apache-2.0](https://github.com/CleanTheClub/CleanTheClub/blob/main/LICENSE). Verifica los términos upstream antes de reutilizarlo; el código y los assets pueden tener términos diferentes.


> [!WARNING]
> **Código mantenido por la comunidad — revísalo antes de usarlo**
> 
> Este proyecto de la comunidad se mantiene fuera de las organizaciones de GitHub de Decentraland Foundation y DAO. Su código fuente y su despliegue en vivo pueden cambiar de forma independiente.
> 
> La inclusión en este catálogo no implica una auditoría de seguridad ni garantiza que sea seguro. Revisa el código, las dependencias y los scripts de instalación antes de ejecutarlo o reutilizarlo.
> 
> [Reportar un problema del catálogo](https://github.com/decentraland/docs/issues/new). Para posibles vulnerabilidades, sigue la política de seguridad del proyecto upstream. No incluyas detalles del exploit ni credenciales en issues públicas.


## Explora el código

Estos enlaces apuntan a una instantánea específica de la fuente para que las explicaciones sigan siendo útiles mientras continúa el desarrollo upstream. Son referencias de aprendizaje, no una evaluación de seguridad.

### Compartir un escaneo de componente entre varios Systems

onClutterPoll recopila entradas de ClutterSync una vez por intervalo de sondeo, construye una búsqueda por ID de elemento y pasa la misma instantánea a los suscriptores. Cada consumidor puede hacer su propio filtrado y detección de cambios.

**Contexto del proyecto:** Esto es útil para estudiar cómo evitar escaneos repetidos de ECS; depende del esquema compartido y del intervalo de sondeo de phase-gate, y las suscripciones deben ir en la inicialización del client.

**Referencias de la fuente:**

* [onClutterPoll](https://github.com/CleanTheClub/CleanTheClub/blob/8ffc6c1fa24ff0da22a1d82ecdf2f9ce72f47f73/src/client/clutterWatcher.ts#L15-L54)

### Mostrar el equipo que lleva otro jugador

updateRemoteCarry adjunta una jerarquía de props a la mano izquierda del otro avatar, selecciona un modelo del gear actual o del estado de transporte, y escala la bolsa según su fracción de llenado. También elimina la jerarquía cuando no hay nada que mostrar.

**Contexto del proyecto:** Léelo junto con los handlers de mensajes de carry, la búsqueda del modelo de theme y los offsets de ajuste del modelo: eso proporciona el estado y el posicionamiento específico del asset.

**Referencias de la fuente:**

* [updateRemoteCarry](https://github.com/CleanTheClub/CleanTheClub/blob/8ffc6c1fa24ff0da22a1d82ecdf2f9ce72f47f73/src/client/carrySystem.ts#L805-L846)


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-es/tutoriales-y-ejemplos/published-games/clean-the-club.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
