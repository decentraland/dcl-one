> Colección de mascotas con aceleración y frenado para el movimiento, además de animación desfasada de huevos colocados en el editor.

# My Dear Pet

![Vista previa de My Dear Pet](/docs/_remote/3412460113d21fec792e322a.webp)

Un juego de colección de mascotas.

* [Jugar en Decentraland](https://decentraland.org/jump?realm=marshcolony.dcl.eth)

## Información del proyecto

**Mantenido por:** Decentraland DAO

[Ver fuente](https://github.com/dcl-regenesislabs/marsh-colony)

**Licencia:** No identificado en este catálogo. Consulta el repositorio upstream y al mantenedor para conocer los términos aplicables antes de reutilizarlo.

## Explora el código

Estos enlaces apuntan a una instantánea específica de la fuente, por lo que las explicaciones siguen siendo útiles mientras continúa el desarrollo upstream. Son referencias de aprendizaje, no una evaluación de seguridad.

### Dar aceleración y frenado a las mascotas en movimiento

`petMotionStep` mantiene una velocidad por entity, limita la velocidad objetivo usando la distancia de frenado restante y devuelve un paso de movimiento del tamaño de un frame. Quien lo llama sigue suministrando la ruta y actualizando el Transform.

**Contexto del proyecto:** Lee los valores de distancia de llegada, aceleración y frenado en `shared/config` junto con este helper al estudiar su sensación de movimiento.

**Referencias de fuente:**

* [`petMotionStep`](https://github.com/dcl-regenesislabs/marsh-colony/blob/55dea4d3e13b3d821aa20bd277fbc4e0bc920b6f/src/client/petMotion.ts#L9-L35)

### Anima props colocados en el editor con distintas fases

`setupEggShake` encuentra egg entities con nombre, recuerda sus rotaciones originales y aplica una pequeña rotación sinusoidal con una fase diferente para cada egg. Esto añade variación mientras conserva la orientación creada de cada prop.

**Contexto del proyecto:** Depende de los nombres de egg generados desde la escena de Creator Hub.

**Referencias de fuente:**

* [`setupEggShake`](https://github.com/dcl-regenesislabs/marsh-colony/blob/55dea4d3e13b3d821aa20bd277fbc4e0bc920b6f/src/client/eggShake.ts#L29-L53)


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-es/tutoriales-y-ejemplos/published-games/my-dear-pet.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
