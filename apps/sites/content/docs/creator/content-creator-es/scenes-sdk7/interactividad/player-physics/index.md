> Aplica fuerzas e impulsos al Avatar del jugador

# Física del jugador

Puedes aplicar fuerzas físicas al avatar del jugador desde el código de tu escena. Esto te permite crear mecánicas de juego como plataformas de lanzamiento, zonas de viento, efectos de retroceso, explosiones y más.

Hay dos tipos de fuerza que puedes aplicar:

* **Impulso**: Un empuje instantáneo de un solo uso. Úsalo para efectos bruscos y discretos, como lanzar al jugador por los aires o hacer que retroceda.
* **Fuerza continua**: Un empuje sostenido que se aplica en cada tick mientras esté activa. Úsalo para efectos continuos como zonas de viento, corrientes de agua o campos gravitatorios.

Ambos se aplican a través del `Physics` helper, importado desde `@dcl/sdk/ecs`.


> [!WARNING]
> **📔 Nota**: Estas fuerzas solo afectan al avatar del jugador local. Los demás jugadores ven los cambios en las posiciones de los otros jugadores, pero las fuerzas en sí no se sincronizan con los demás jugadores en multijugador. La física de cada jugador se ejecuta localmente en su propia instancia.


## Aplicar un impulso

Usa `Physics.applyImpulseToPlayer()` para darle al jugador un empuje de un solo uso en una dirección determinada.

* `vector`: Dirección y fuerza combinadas: la longitud del vector codifica la magnitud del impulso.

```ts
import { Physics } from '@dcl/sdk/ecs'
import { Vector3 } from '@dcl/sdk/math'

// Lanza al jugador directamente hacia arriba
Physics.applyImpulseToPlayer(Vector3.create(0, 50, 0))
```

También puedes pasar una dirección y una magnitud por separado. En este caso, el vector de dirección se normaliza automáticamente:

* `direction`: Dirección para empujar; se normaliza automáticamente antes de escalarse.
* `magnitude`: Fuerza del impulso.

```ts
import { Physics } from '@dcl/sdk/ecs'
import { Vector3 } from '@dcl/sdk/math'

// Lanza al jugador hacia arriba con una magnitud de 50
Physics.applyImpulseToPlayer(Vector3.create(0, 1, 0), 50)
```

Si llamas a `applyImpulseToPlayer()` varias veces dentro del mismo frame, los impulsos se acumulan: se suman y se aplican como un único impulso combinado.

### Qué tan fuerte es un impulso

El avatar tiene una masa de 1, así que la magnitud de un impulso es el cambio en la velocidad del avatar, en metros por segundo. Un impulso de `50` añade 50 m/s en esa dirección.

* La velocidad que añade un impulso se atenúa con el tiempo. En el aire se atenúa a un ritmo de 1.5 por segundo, así que después de un segundo queda aproximadamente una quinta parte. Se atenúa más rápido mientras el avatar está en el suelo.
* La velocidad añadida por los impulsos y las fuerzas tiene un límite de 50 m/s.
* Un impulso hacia arriba primero cancela cualquier velocidad descendente que tenga el avatar por caer, así que una plataforma de lanzamiento funciona igual tanto si el jugador se sube a ella como si aterriza sobre ella desde arriba.

### Activar un impulso al entrar el jugador

Un patrón común es activar un impulso cuando el jugador entra en un área. Usa un área de activación para detectar cuándo entra el jugador:

```ts
import { engine, Physics, TriggerArea, triggerAreaEventsSystem, ColliderLayer, MeshRenderer, Transform } from '@dcl/sdk/ecs'
import { Vector3 } from '@dcl/sdk/math'

const launchPad = engine.addEntity()
Transform.create(launchPad, { position: Vector3.create(8, 0, 8) })
MeshRenderer.setBox(launchPad)
TriggerArea.setBox(launchPad, ColliderLayer.CL_PLAYER)

triggerAreaEventsSystem.onTriggerEnter(launchPad, (result) => {
	if (result.trigger?.entity !== engine.PlayerEntity) return
	Physics.applyImpulseToPlayer(Vector3.create(0, 50, 0))
})
```

## Aplicar un impulso de retroceso

Usa `Physics.applyKnockbackToPlayer()` para empujar al jugador lejos de un punto con un solo impulso. Es ideal para explosiones, efectos de impacto y otros estallidos direccionales de un solo uso. La dirección se calcula automáticamente desde el punto de origen hasta la posición actual del jugador.

```ts
import { Physics } from '@dcl/sdk/ecs'
import { Vector3 } from '@dcl/sdk/math'

const explosionPosition = Vector3.create(8, 1, 8)

// Expulsa al jugador lejos del centro de la explosión
Physics.applyKnockbackToPlayer(explosionPosition, 40)
```

Puedes limitar el área de efecto con un `radius`, y controlar cómo disminuye la magnitud con la distancia usando la `KnockbackFalloff` opción:

* `fromPosition`: Origen en espacio mundial del retroceso (centro de la explosión, posición del enemigo, etc.).
* `magnitude`: Fuerza base del impulso.
* `radius`: Distancia máxima del efecto (valor predeterminado: `Infinity`).
* `falloff`: Cómo disminuye la magnitud con la distancia (valor predeterminado: `CONSTANT`).

```ts
import { Physics, KnockbackFalloff } from '@dcl/sdk/ecs'
import { Vector3 } from '@dcl/sdk/math'

Physics.applyKnockbackToPlayer(
	Vector3.create(8, 1, 8),
	40,                          // magnitud
	10,                          // radio: sin efecto más allá de 10 metros
	KnockbackFalloff.LINEAR      // la magnitud se atenúa hasta 0 en el borde del radio
)
```

El `KnockbackFalloff` enum controla cómo disminuye la fuerza con la distancia:

* `KnockbackFalloff.CONSTANT` — misma magnitud a cualquier distancia dentro del radio (predeterminado)
* `KnockbackFalloff.LINEAR` — disminución lineal suave hasta 0 en el borde del radio
* `KnockbackFalloff.INVERSE_SQUARE` — caída pronunciada físicamente realista

Si el jugador está exactamente en la posición de origen, se empuja al jugador directamente hacia arriba. Una magnitud negativa atrae al jugador hacia el punto en lugar de empujarlo lejos de él.

Los mismos `KnockbackFalloff` valores están disponibles para `applyRepulsionForceToPlayer()`.

## Aplicar una fuerza continua

Usa `Physics.applyForceToPlayer()` para aplicar una fuerza sostenida al jugador. A diferencia de un impulso, esta fuerza se aplica en cada tick mientras permanezca activa.

* `source`: Una entidad que identifica esta fuente de fuerza; úsala para actualizar o eliminar la fuerza más tarde.
* `vector`: Dirección y fuerza combinadas: la longitud del vector codifica la magnitud de la fuerza.

La posición de la entidad de origen no es relevante; solo se usa como identificador.

```ts
import { engine, Physics } from '@dcl/sdk/ecs'
import { Vector3 } from '@dcl/sdk/math'

const windZoneEntity = engine.addEntity()

// Empuja al jugador hacia la derecha continuamente
Physics.applyForceToPlayer(windZoneEntity, Vector3.create(10, 0, 0))
```

El vector de fuerza siempre está en **espacio mundial**. Si necesitas una dirección relativa a una entidad rotada, usa `Transform.localToWorldDirection()` para convertirla primero — consulta [Convertir una dirección local a espacio mundial](#convert-a-local-direction-to-world-space).

Al igual que con `applyImpulseToPlayer()`, también puedes pasar un `direction` y `magnitude` por separado — consulta [Aplicar un impulso](#apply-an-impulse):

```ts
Physics.applyForceToPlayer(windZoneEntity, Vector3.create(0, 1, 0), 50)
```

Si llamas a `applyForceToPlayer()` de nuevo con la misma entidad de origen, reemplaza la fuerza anterior de esa fuente. Si se acumulan varias fuentes de fuerza, sus vectores se suman en cada tick.

### Eliminar una fuerza continua

Para dejar de aplicar una fuerza, llama a `Physics.removeForceFromPlayer()` con una referencia a la entidad de origen que se usó para crear la fuerza. Si la entidad no está aplicando actualmente una fuerza, esta llamada se ignora de forma segura.

* `source`: La entidad usada cuando se aplicó la fuerza.

```ts
Physics.removeForceFromPlayer(windZoneEntity)
```

### Aplicar una fuerza durante una duración limitada

Usa `Physics.applyForceToPlayerForDuration()` para aplicar una fuerza durante una cantidad específica de tiempo. La duración está en segundos. La fuerza se elimina automáticamente cuando transcurre el tiempo. Si la llamas de nuevo con la misma entidad de origen, se reinicia el temporizador.

* `source`: Una entidad que identifica esta fuente de fuerza.
* `duration`: Cuánto dura la fuerza, en segundos.
* `vector`: Dirección y fuerza combinadas: la longitud del vector codifica la magnitud de la fuerza.

```ts
import { engine, Physics } from '@dcl/sdk/ecs'
import { Vector3 } from '@dcl/sdk/math'

const gustEntity = engine.addEntity()

// Aplica una fuerte fuerza ascendente durante 1.5 segundos
Physics.applyForceToPlayerForDuration(gustEntity, 1.5, Vector3.create(0, 50, 0))
```

El `direction` + `magnitude` también está disponible una sobrecarga — consulta [Aplicar un impulso](#apply-an-impulse):

```ts
Physics.applyForceToPlayerForDuration(gustEntity, 1.5, Vector3.create(0, 1, 0), 50)
```

### Ejemplo de zona de viento

Este ejemplo crea un túnel de viento que empuja al jugador mientras está dentro de él y se detiene cuando sale:

```ts
import { engine, Physics, TriggerArea, triggerAreaEventsSystem, ColliderLayer, MeshRenderer, MeshCollider, Transform } from '@dcl/sdk/ecs'
import { Vector3 } from '@dcl/sdk/math'

const windTunnel = engine.addEntity()
Transform.create(windTunnel, {
	position: Vector3.create(8, 1, 8),
	scale: Vector3.create(4, 3, 4),
})
MeshRenderer.setBox(windTunnel)
TriggerArea.setBox(windTunnel, ColliderLayer.CL_PLAYER)

triggerAreaEventsSystem.onTriggerEnter(windTunnel, (result) => {
	if (result.trigger?.entity !== engine.PlayerEntity) return
	Physics.applyForceToPlayer(windTunnel, Vector3.create(15, 0, 0))  // empuja lateralmente a lo largo del eje X
})

triggerAreaEventsSystem.onTriggerExit(windTunnel, (result) => {
	if (result.trigger?.entity !== engine.PlayerEntity) return
	Physics.removeForceFromPlayer(windTunnel)
})
```

## Aplicar una fuerza de repulsión

Usa `Physics.applyRepulsionForceToPlayer()` para empujar continuamente al jugador lejos de un punto fijo en el espacio. Esto es adecuado para efectos como un campo de repulsión magnética, una barrera de fuerza o un objeto flotante que mantiene a los jugadores a distancia. A diferencia de `applyKnockbackToPlayer()`, esta fuerza se recalcula en cada tick a medida que el jugador se mueve. La magnitud se debilita con la distancia en función del radio y la caída; consulta [Aplicar un impulso de retroceso](#apply-a-knockback-impulse) para las `KnockbackFalloff` opciones. Una magnitud negativa invierte el efecto, atrayendo continuamente al jugador hacia el punto como un vórtice o un pozo gravitatorio.


> [!WARNING]
> **📔 Nota**: El origen de la repulsión es el `fromPosition` vector que pasas — **no** la posición de la `source` entidad. La `source` entidad solo se usa como identificador para que puedas actualizar o eliminar la fuerza más tarde. Su posición, rotación y escala se ignoran por completo.


* `source`: Una entidad usada como identificador de esta fuerza; su posición no se usa.
* `fromPosition`: El punto en espacio mundial desde el que se empuja al jugador.
* `magnitude`: Fuerza base. Los valores negativos atraen en lugar de repeler.
* `radius`: Distancia máxima del efecto (valor predeterminado: `Infinity`).
* `falloff`: Cómo disminuye la magnitud con la distancia (valor predeterminado: `CONSTANT`).

```ts
import { engine, Physics, Transform, timers } from '@dcl/sdk/ecs'
import { Vector3 } from '@dcl/sdk/math'

const repulsionSource = engine.addEntity()
Transform.create(repulsionSource, { position: Vector3.create(8, 1, 8) })

// Pasa explícitamente la posición de la entidad como el origen de la repulsión
Physics.applyRepulsionForceToPlayer(
	repulsionSource,
	Transform.get(repulsionSource).position,  // origen de la repulsión — pasado explícitamente, no leído automáticamente
	50,                                         // magnitud
	10,                                         // radio de efecto en metros
)

// Elimina la fuerza después de medio segundo
timers.setTimeout(() => {
    Physics.removeForceFromPlayer(repulsionSource)
}, 500)
```

## Fuerzas mientras se planea

Mientras el jugador planea, las fuerzas continuas se comportan de manera diferente:

* Fuerzas continuas — de `applyForceToPlayer()`, `applyForceToPlayerForDuration()`, o `applyRepulsionForceToPlayer()` — son **1.5 veces más fuertes**, ya que el planeador abierto capta el flujo de aire. Las zonas de viento y las corrientes se sienten más receptivas para un jugador que planea.
* El componente vertical de una fuerza continua puede **elevar** a un jugador que planea. El límite de velocidad de caída del planeador solo limita qué tan rápido desciende el jugador; no cancela el movimiento ascendente, así que una corriente de viento inclinada o vertical empuja al jugador a lo largo de toda la dirección de la fuerza.

Impulsos de un solo uso — de `applyImpulseToPlayer()` or `applyKnockbackToPlayer()` — no se ven afectados por el planeo. Se comportan igual tanto si el planeador está abierto como cerrado.

Puedes combinar estos comportamientos para construir mecánicas como corrientes térmicas ascendentes que llevan a los jugadores que planean hacia arriba, o corredores de viento que son más fáciles de atravesar con el planeador abierto. Para ajustar la velocidad de caída o la velocidad hacia adelante del planeador, usa el `AvatarLocomotionSettings` componente, consulta [Configuración de locomoción](/docs/creator/content-creator-es/scenes-sdk7/interactividad/avatares/locomotion/#locomotion-settings).

## Convertir una dirección local a espacio mundial

Usa `Transform.localToWorldDirection()` para transformar un vector de dirección desde el espacio de coordenadas local de una entidad a espacio mundial, teniendo en cuenta toda la jerarquía de padres. Esto es útil cuando se aplican fuerzas relativas a una entidad rotada; por ejemplo, empujar al jugador lejos de una cara específica de un obstáculo giratorio.

* `entity`: La entidad de origen cuyo espacio local define la dirección.
* `localDirection`: Vector de dirección en las coordenadas locales de la entidad.

Devuelve el vector de dirección en coordenadas mundiales.

Esto aplica **solo la rotación** — no tiene en cuenta la traslación ni la escala —, por lo que es directamente adecuado para los vectores de dirección que se pasan a funciones de fuerza e impulso.

```ts
import { Physics, Transform } from '@dcl/sdk/ecs'
import { Vector3 } from '@dcl/sdk/math'

// Empuja al jugador en la dirección en que apunta el eje local +Z de la entidad en espacio mundial
const worldDir = Transform.localToWorldDirection(myEntity, Vector3.create(0, 0, 1))
Physics.applyImpulseToPlayer(worldDir, 20)
```


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-es/scenes-sdk7/interactividad/player-physics.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
