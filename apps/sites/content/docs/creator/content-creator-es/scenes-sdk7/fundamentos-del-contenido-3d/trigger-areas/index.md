# Áreas de trigger

Aprende cómo usar áreas de trigger en tu escena

Las áreas de activación te permiten reaccionar al evento de que un jugador entre o salga de un área, o de que cualquier otra entidad entre o salga de un área. Esta es una herramienta fundamental para crear escenas interactivas. Úsalas para cosas como abrir una puerta cuando el jugador se acerca, o para sumar un punto cuando una pelota entra en una portería.

## Uso de áreas de activación

Para usar áreas de activación necesitas añadir un `TriggerArea` componente a una entidad, luego usar un `triggerAreaEventsSystem` para reaccionar a los eventos.

```ts
import { engine, Transform, TriggerArea, triggerAreaEventsSystem } from '@dcl/sdk/ecs'

// crear entidad
const triggerEntity = engine.addEntity()

// establecer Transform
Transform.create(triggerEntity, {
  position: Vector3.create(8, 0, 8)
  })

// Área de activación
TriggerArea.setBox(triggerEntity)

// Evento cuando se activa el área de activación
triggerAreaEventsSystem.onTriggerEnter(triggerEntity, function(result) {
  if (result.trigger?.entity !== engine.PlayerEntity) return;
  console.log('¡El jugador entró en el área de activación!')
})
```

De forma predeterminada, el `TriggerArea` componente reacciona al evento de *cualquier* jugador entre en el área. El código anterior añade `if (result.trigger?.entity !== engine.PlayerEntity) return` para comprobar que la entidad que causó el evento es el jugador actual, y no el avatar de otra persona

## Formas del área de activación

Las áreas de activación pueden ser una caja o una esfera.

```ts
import { engine, Transform, TriggerArea } from '@dcl/sdk/ecs'

// Caja
TriggerArea.setBox(triggerEntity)

// Esfera
TriggerArea.setSphere(triggerEntity)
```


> [!NOTE]
> **💡 Consejo**: La esfera es la forma más fácil de calcular para el engine, ya que se logra comprobando la distancia desde el centro de la esfera. Si tienes dudas, usa una esfera.


Para alterar el tamaño del área de activación, puedes usar la `scale` propiedad del `Transform` componente en la entidad que contiene el `TriggerArea`.

```ts
import { engine, Transform, TriggerArea } from '@dcl/sdk/ecs'

const triggerEntity = engine.addEntity()

TriggerArea.setBox(triggerEntity)

Transform.create(triggerEntity, {
  scale: Vector3.create(4, 2, 4),
})
```

### Depuración

Para depurar tu escena y ver el área cubierta por el área de activación, puedes añadir un `MeshRenderer` componente a la entidad con el área de activación, y establecer la forma que quieras depurar. Las dimensiones de la malla predeterminada coincidirán con las dimensiones del área de activación.

```ts
import { engine, Transform, TriggerArea } from '@dcl/sdk/ecs'

const triggerEntity = engine.addEntity()

TriggerArea.setBox(triggerEntity)

MeshRenderer.setBox(triggerEntity)

Transform.create(triggerEntity, {
  position: Vector3.create(8, 0, 8),
})
```

## Eventos del área de activación

Puedes usar el `triggerAreaEventsSystem` para reaccionar a los distintos eventos de un área de activación:

* `onTriggerEnter`: Se activa cuando una entidad entra en el área de activación.
* `onTriggerExit`: Se activa cuando una entidad sale del área de activación.
* `onTriggerStay`: Se activa mientras una entidad está en el área de activación, en cada frame.

```ts
import { engine, Transform, TriggerArea, triggerAreaEventsSystem } from '@dcl/sdk/ecs'

const triggerEntity = engine.addEntity()

TriggerArea.setBox(triggerEntity)

// Al entrar
triggerAreaEventsSystem.onTriggerEnter(triggerEntity, function(result) {
  console.log('¡El jugador entró en el área de activación!')
})

// Al salir
triggerAreaEventsSystem.onTriggerExit(triggerEntity, function(result) {
  console.log('¡El jugador salió del área de activación!')
})

// Mientras permanece
triggerAreaEventsSystem.onTriggerStay(triggerEntity, function(result) {
  console.log('¡El jugador está en el área de activación!')
})
```

## Respuestas a eventos del trigger

Cuando se activa un evento de área de activación, puedes usar el `result` parámetro para obtener información tanto de la entidad que fue activada como de la entidad que activó el evento.

Las siguientes propiedades están disponibles en el `result` parámetro:

* `triggeredEntity`: El ID de la entidad que fue activada (esta es la entidad que posee el área de activación)
* `triggeredEntityPosition`: La posición de la entidad que fue activada
* `triggeredEntityRotation`: La rotación de la entidad que fue activada
* `eventType`: El tipo de evento del trigger (ENTER, EXIT, STAY)
* `timestamp`: La marca de tiempo del evento del trigger
* `trigger`: Un objeto con los siguientes campos:
  * `entity`: El ID de la entidad que activó el trigger (la entidad que entró en el área de activación)
  * `layers`: Las capas de colisión de la entidad que activó el trigger
  * `position`: La posición de la entidad que activó el trigger
  * `rotation`: La rotación de la entidad que activó el trigger
  * `scale`: La escala de la entidad que activó el trigger


> [!WARNING]
> **Confusión común**: a pesar de su nombre, `result.triggeredEntity` hace **no** referencia a la entidad que activó el evento. Estos dos campos se confunden fácilmente:
> 
> * `result.trigger?.entity`: la entidad que entró en el área de activación. Esto es normalmente lo que quieres comprobar, por ejemplo para identificar qué jugador u objeto entró.
> * `result.triggeredEntity`: la entidad que posee el `TriggerArea` componente (la propia zona de activación, no la entidad que entró en ella).
> 
> Usa `result.trigger?.entity` cuando necesites identificar a la entidad que entra.


```ts
import { engine, Transform, TriggerArea, triggerAreaEventsSystem } from '@dcl/sdk/ecs'

// Entidad con el área de activación
const triggerEntity = engine.addEntity()

TriggerArea.setBox(triggerEntity)

Transform.create(triggerEntity, {
  position: Vector3.create(8, 0, 8),
})

// Al entrar
triggerAreaEventsSystem.onTriggerEnter(triggerEntity, function(result) {
  console.log('¡Una entidad entró en el área de activación!', result.triggeredEntity)
  console.log('Posición de la entidad activada: ', result.triggeredEntityPosition)
  console.log('Rotación de la entidad activada: ', result.triggeredEntityRotation)
  console.log('Tipo de evento: ', result.eventType)
  console.log('Marca de tiempo: ', result.timestamp)
  console.log('Entidad del trigger: ', result.trigger.entity)
  console.log('Capas del trigger: ', result.trigger.layers)
  console.log('Posición del trigger: ', result.trigger.position)
  console.log('Rotación del trigger: ', result.trigger.rotation)
  console.log('Escala del trigger: ', result.trigger.scale)
})
```

## Detectar jugadores que entran en un área

El uso más común de las áreas de activación es reaccionar a jugadores que entran o salen. Dos capas de colisión detectan avatares, y cuál usar depende de a quién deba reaccionar la escena:

* `ColliderLayer.CL_MAIN_PLAYER`: Detecta **solo al jugador que usa la máquina local**. Usa esto cuando la respuesta esté pensada solo para ese jugador, por ejemplo para teletransportarlo, reproducirle un sonido o rastrear su progreso en una misión. Esta también es la opción más barata en términos de rendimiento.
* `ColliderLayer.CL_PLAYER`: Detecta **todos los avatares**, tanto el jugador local como cualquier otro jugador que se esté renderizando en la escena. Usa esto cuando importe que entre cualquiera, por ejemplo una puerta que se abre cuando alguien se acerca, o contar cuántas personas hay en una habitación. Esta es la capa predeterminada si no se especifica ninguna.

El siguiente ejemplo reacciona solo al jugador local:

```ts
import { engine, Transform, TriggerArea, ColliderLayer, triggerAreaEventsSystem } from '@dcl/sdk/ecs'

// crear entidad
const triggerEntity = engine.addEntity()

// establecer Transform
Transform.create(triggerEntity, {
  position: Vector3.create(8, 0, 8),
})

// Área de activación que solo detecta al jugador local
TriggerArea.setBox(triggerEntity, ColliderLayer.CL_MAIN_PLAYER)

// Evento cuando se activa el área de activación
triggerAreaEventsSystem.onTriggerEnter(triggerEntity, function (result) {
  console.log('¡Has entrado en el área de activación!')
})
```

Al usar `CL_PLAYER`, todos los avatares activan el área de activación. Para saber si el avatar que la activó es el jugador local, compara la entidad del resultado con `engine.PlayerEntity`:

```ts
import { engine, Transform, TriggerArea, triggerAreaEventsSystem } from '@dcl/sdk/ecs'

// crear entidad
const triggerEntity = engine.addEntity()

// establecer Transform
Transform.create(triggerEntity, {
  position: Vector3.create(8, 0, 8),
})

// Área de activación, usando la capa predeterminada CL_PLAYER
TriggerArea.setBox(triggerEntity)

// Evento cuando se activa el área de activación
triggerAreaEventsSystem.onTriggerEnter(triggerEntity, function (result) {
  if (result.trigger?.entity === engine.PlayerEntity) {
    console.log('¡Has entrado en el área de activación!')
  } else {
    console.log('¡Otro jugador entró en el área de activación!')
  }
})
```

En la mayoría de los casos, en lugar de intentar detectar a todos los jugadores, el enfoque ideal es detectar solo al jugador actual (mediante `CL_MAIN_PLAYER`) y luego sincronizar los efectos de este trigger entre todos los jugadores. Por ejemplo, una puerta corredera se abre solo para tu avatar, pero luego el estado abierto de esa puerta se comparte con todos los demás jugadores.


> [!NOTE]
> **💡 Consejo**: Las áreas de activación cuya máscara solo incluye capas de avatar (`CL_PLAYER`, `CL_MAIN_PLAYER`, o ambas) están optimizadas: el engine las coloca en una capa de física solo para avatares, por lo que las superposiciones con cualquier cosa que no sea un avatar se descartan por el motor de física antes de llegar al código de tu escena. Un área de activación con una máscara exactamente igual a `CL_MAIN_PLAYER` se optimiza aún más: omite cualquier superposición con colliders que no sean del jugador local antes de llegar al manejador, por lo que detectar solo al jugador local es prácticamente gratis.


## Capas del área de activación

Usa el segundo argumento opcional del `TriggerArea` componente para establecer las capas que activarán el área de activación. De forma predeterminada, el área de activación solo se activa con la capa `ColliderLayer.CL_PLAYER`, consulta [Detectar jugadores que entran en un área](#detect-players-entering-an-area).

Puedes cambiar la capa de colisión para detectar cualquier otra entidad pasándola como segundo argumento del `TriggerArea` componente.

```ts
import { engine, Transform, TriggerArea, MeshCollider, ColliderLayer, triggerAreaEventsSystem } from '@dcl/sdk/ecs'

// Área de activación
const triggerEntity = engine.addEntity()

TriggerArea.setBox(triggerEntity, ColliderLayer.CL_CUSTOM1)

Transform.create(triggerEntity, {
  position: Vector3.create(8, 0, 8),
})

// Entidad que activará el área de activación
const movingEntity = engine.addEntity()

Transform.create(movingEntity, {
  position: Vector3.create(8, 0, 8),
})

MeshCollider.setBox(movingEntity, ColliderLayer.CL_CUSTOM1)
```

Los valores permitidos son los mismos que los de `MeshCollider` componente. Vea [Capas de colisión](/docs/creator/content-creator-es/scenes-sdk7/fundamentos-del-contenido-3d/colliders/#collision-layers) para más detalles.

* `ColliderLayer.CL_PLAYER`: cualquier avatar (local + remoto)
* `ColliderLayer.CL_MAIN_PLAYER`: solo el jugador local
* `ColliderLayer.CL_PHYSICS`
* `ColliderLayer.CL_POINTER`
* `ColliderLayer.CL_CUSTOM1` hasta `CL_CUSTOM8`
* `ColliderLayer.CL_NONE`


> [!NOTE]
> **💡 Consejo**: Las capas `CL_CUSTOM1` hasta `CL_CUSTOM8` no tienen ningún comportamiento especial por sí mismas, puedes usarlas para lo que mejor se adapte a tu escena.


También puedes configurar un área de activación para detectar múltiples capas a la vez.

```ts
import { engine, Transform, TriggerArea, ColliderLayer, triggerAreaEventsSystem } from '@dcl/sdk/ecs'

const triggerEntity = engine.addEntity()

TriggerArea.setBox(triggerEntity, ColliderLayer.CL_CUSTOM1 | ColliderLayer.CL_CUSTOM2)

Transform.create(triggerEntity, {
  position: Vector3.create(8, 0, 8),
})
```

Esto activará el área de activación cuando cualquier entidad con las capas `CL_CUSTOM1` o `CL_CUSTOM2` entre en el área de activación.


> [!NOTE]
> **💡 Consejo**: Para un ejemplo funcional que lo pruebe `TriggerArea` de extremo a extremo, consulta la [`75,-9-trigger-areas`](https://github.com/decentraland/sdk7-test-scenes/tree/main/scenes/75,-9-trigger-areas) escena de prueba, que cubre el contrato de cableado ENTER/EXIT y la síntesis por tick del SDK, `onTriggerStay` áreas rotadas, activadores en movimiento en capas de collider personalizadas y áreas adjuntas a entidades que caen. Para ver cómo se comportan las propias capas de collider, consulta [`5,5-collider-layers`](https://github.com/decentraland/sdk7-test-scenes/tree/main/scenes/5,5-collider-layers).



---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-es/scenes-sdk7/fundamentos-del-contenido-3d/trigger-areas.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
