> Deslizamiento de pingüino con barrido cronometrado que modifica la desaceleración y la puntuación según la distancia al objetivo.

# Slide Penguin

Desliza un pingüino hacia un objetivo y usa barridos temporizados para afectar qué tan rápido se desacelera.

Nota de implementación: actualmente no está desplegado.

* [Juega en Decentraland](https://decentraland.org/jump?position=-140%2C101)

## Información del proyecto

**Mantenido por:** Comunidad · ToxSam

[Ver la fuente upstream](https://github.com/ToxSam/SlidePenguin)

**Licencia:** No identificado en este catálogo. Consulta el repositorio upstream y al mantenedor para conocer los términos aplicables antes de reutilizarlo.


> [!WARNING]
> **Código mantenido por la comunidad — revísalo antes de usarlo**
> 
> Este proyecto comunitario se mantiene fuera de las organizaciones de GitHub de Decentraland Foundation y DAO. Su código fuente y su despliegue en vivo pueden cambiar de forma independiente.
> 
> La inclusión en este catálogo no implica una auditoría de seguridad ni garantiza la seguridad. Revisa el código, las dependencias y los scripts de instalación antes de ejecutarlo o reutilizarlo.
> 
> [Reportar un problema del catálogo](https://github.com/decentraland/docs/issues/new). Para posibles vulnerabilidades, sigue la política de seguridad del proyecto upstream. No incluyas detalles de explotación ni credenciales en incidencias públicas.


## Explora el código

Estos enlaces apuntan a una instantánea específica de la fuente para que las explicaciones sigan siendo útiles a medida que continúa el desarrollo upstream. Son referencias de aprendizaje, no una evaluación de seguridad.

### Convertir el barrido en un modificador de movimiento temporal

`handleSwipe` aplica un tiempo de reutilización a la reducción de la desaceleración y registra cuándo expira ese efecto. `penguinMovementSystem` restaura la desaceleración base después de esa ventana, actualiza la velocidad y avanza el Transform usando el tiempo de fotograma.

**Contexto del proyecto:** Sigue el `PenguinMovement` y `SwipeBox` componentes y constantes de ajuste para entender la conexión entre la entrada y el movimiento.

**Referencias de la fuente:**

* [`handleSwipe`](https://github.com/ToxSam/SlidePenguin/blob/19978bf4ca9e793ce87cd5637efc1abf3a3b686c/src/systems.ts#L519-L598)
* [`penguinMovementSystem`](https://github.com/ToxSam/SlidePenguin/blob/19978bf4ca9e793ce87cd5637efc1abf3a3b686c/src/systems.ts#L864-L913)

### Calcular qué tan cerca cae un objeto de un objetivo

`calculateDistance2D` y `calculateScore` ignora la altura, normaliza la distancia horizontal con respecto a una distancia máxima y la mapea a una puntuación redondeada limitada por `MAX_SCORE`.

**Contexto del proyecto:** Este pequeño módulo es un punto de partida útil para entender la puntuación por proximidad; sus entradas son la posición final del pingüino y la posición del objetivo.

**Referencias de la fuente:**

* [`calculateDistance2D` y `calculateScore`](https://github.com/ToxSam/SlidePenguin/blob/19978bf4ca9e793ce87cd5637efc1abf3a3b686c/src/scoring.ts#L5-L39)


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-es/tutoriales-y-ejemplos/published-games/slide-penguin.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
