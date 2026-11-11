> Usa raycasting para trazar una línea en el espacio y consultar colisiones con entities de la escena.

# Raycasting

El raycasting es una herramienta fundamental en el desarrollo de juegos. Con raycasting, puedes trazar una línea imaginaria en el espacio y consultar si alguna entities es intersectada por esa línea. Esto es útil para calcular líneas de visión, trayectorias de balas, algoritmos de búsqueda de rutas y muchas otras aplicaciones.

Cuando un jugador presiona el botón de puntero, o el botón principal o secundario, se traza un rayo desde la posición del jugador en la dirección en la que está mirando, consulta [eventos de botón](/docs/creator/content-creator-es/scenes-sdk7/interactividad/eventos-de-button/click-events/) para más detalles sobre esto. Este documento cubre cómo trazar un rayo invisible desde cualquier posición y dirección arbitrarias, independiente de las acciones del jugador, que puedes usar en muchos otros escenarios.

Ten en cuenta que los raycasts solo golpean objetos con colliders. Así que si quieres detectar impactos de rayos contra un modelo 3D, haz una de estas dos cosas:

* El modelo debe contener [mallas collider](/docs/creator/content-creator-es/modelado-y-animaciones-3d/colliders/).
* El `GLTFContainer` debe estar configurado para usar la [geometría visible con máscaras de colisión](/docs/creator/content-creator-es/scenes-sdk7/fundamentos-del-contenido-3d/colliders/#colliders-on-3d-models).
* Añade un [componente MeshCollider](/docs/creator/content-creator-es/scenes-sdk7/fundamentos-del-contenido-3d/colliders/).

También es una buena práctica asignar collision layers personalizadas [collision layers](/docs/creator/content-creator-es/scenes-sdk7/fundamentos-del-contenido-3d/colliders/#collision-layers) a los modelos 3D, para que los rayos solo tengan que calcular colisiones contra las entities relevantes, en lugar de contra todo lo que tiene un collider.

## Crear un rayo

Todos los rayos tienen un punto de origen y una dirección. El punto de origen se basa en la posición de una entity, tomando los valores del componente Transform de la entity. La dirección de un rayo puede definirse de 4 maneras diferentes:

* **local**: Una dirección relativa a la dirección frontal de la entity, también afectada por la transformación de cualquier entity padre. Esto es útil para detectar obstáculos frente a vehículos respetando su rumbo.
* **global**: Ignora la rotación de la entity y apunta en una dirección como si la rotación de la entity fuera 0. Esto es útil, por ejemplo, para apuntar siempre hacia abajo.
* **global target**: Traza una línea entre la posición de la entity y una posición global objetivo en la escena. Ignora la rotación de la entity. Útil, por ejemplo, para crear juegos de defensa de torres; la torreta de cada torre puede apuntar a una coordenada precisa en el espacio.
* **target entity**: Traza una línea entre la posición de la entity y la posición de una segunda target entity. Ignora la rotación de cualquiera de las entities.

El siguiente código crea un raycast con una dirección local:

```ts
const myEntity = engine.addEntity()
Transform.create(myEntity, {
  position: Vector3.create(4, 1, 4),
})

raycastSystem.registerLocalDirectionRaycast(
  {
    entity: myEntity,
    opts: { direction: Vector3.Forward() },
  },
  function (raycastResult) {
    // función callback
  }
)
```

Usa las siguientes funciones para crear raycasts proporcionando la dirección de distintas maneras:

* `raycastSystem.registerLocalDirectionRaycast()`: crea un raycast con una **local** dirección. El `dirección` campo espera un `Vector3` que describe un vector relativo a la entity y a su rotación (p. ej., `Vector3.Forward()` terminaría usando el vector forward del Transform de la entity)
* `raycastSystem.registerGlobalDirectionRaycast()`: crea un raycast con una **global** dirección. El `dirección` campo espera un `Vector3` que describe la dirección global.
* `raycastSystem.registerGlobalTargetRaycast()`: crea un raycast con una dirección definida por una **global target** posición. El `target` campo espera un `Vector3` campo describe una posición global en la escena.
* `raycastSystem.registerTargetEntityRaycast()`: crea un raycast con una dirección definida hacia una **target entity** posición. El `targetEntity` campo espera una referencia a una entity; la posición de esta entity se usará como objetivo del rayo.

Los siguientes campos opcionales están disponibles al crear un rayo con cualquiera de los métodos anteriores:

* `maxDistance`: *number* para establecer la longitud con la que se trazará este rayo. Si no se establece, el valor predeterminado es 16 metros.
* `queryType`: *RaycastQueryType* valor de enum, para definir si el rayo devolverá todas las entities golpeadas o solo la primera. Las siguientes opciones están disponibles:
  * `RaycastQueryType.RQT_HIT_FIRST`: *(predeterminado)* solo devuelve la primera entity golpeada, comenzando desde el punto de origen.
  * `RaycastQueryType.RQT_QUERY_ALL`: devuelve todas las entities golpeadas, desde el origen hasta la distancia máxima del rayo.
* `originOffset`: En lugar de iniciar el raycast desde la posición de origen de la entity, añade un offset para iniciar la consulta desde una posición relativa. Por ejemplo, puedes usar un pequeño offset para evitar que el rayo choque con el propio collider de la entity. Si no se establece, el valor predeterminado es `Vector3.Zero()`.
* `collisionMask`: Detecta solo colisiones con ciertas collision layers. Usa esto junto con una collision layer personalizada, o para detectar solo la physics layer o la pointer events layer. Consulta [collision layers](/docs/creator/content-creator-es/scenes-sdk7/fundamentos-del-contenido-3d/colliders/#collision-layers). Si no se establece, la layer predeterminada usada es `ColliderLayer.CL_PHYSICS`.
* `continuous`: Si es true, seguirá ejecutando una consulta de raycast en cada frame. Si es false, el rayo solo se usará en el frame actual. Si no se establece, el valor predeterminado es false.
* Cuando se establece la dirección con una dirección local o global, el `dirección` campo tiene como valor predeterminado `Vector3.Forward()`.
* Cuando se establece la dirección con un target global, el `target` campo tiene como valor predeterminado `Vector3.Zero()`.
* Cuando se establece la dirección con un target entity, el `targetEntity` campo tiene como valor predeterminado la root entity de la escena, ubicada en `Vector3.Zero()`.


> [!WARNING]
> **📔 Nota**: El `continuous` propiedad debe usarse con precaución, ya que ejecutar una consulta de raycast en cada frame puede ser muy costoso para el rendimiento. Cuando sea posible, usa un system (o la `interval` function de la librería Utils) para ejecutar consultas de raycast a un intervalo regular más espaciado, consulta [raycasting recurrente](#recurrent-raycasting).


A continuación se muestran ejemplos usando cada uno de los cuatro métodos para determinar la dirección del rayo:

```ts
// RAYCAST DE DIRECCIÓN LOCAL
raycastSystem.registerLocalDirectionRaycast(
  {
    entity: myEntity,
    opts: {
      queryType: RaycastQueryType.RQT_QUERY_ALL,
      direction: Vector3.Forward(),
      maxDistance: 30,
    },
  },
  function (raycastResult) {
    console.log(raycastResult.hits)
  }
)
// RAYCAST DE DIRECCIÓN GLOBAL
raycastSystem.registerGlobalDirectionRaycast(
  {
    entity: myEntity,
    opts: {
      queryType: RaycastQueryType.RQT_QUERY_ALL,
      direction: Vector3.Forward(),
      maxDistance: 30,
    },
  },
  function (raycastResult) {
    console.log(raycastResult.hits)
  }
)
// RAYCAST DE POSICIÓN OBJETIVO GLOBAL
raycastSystem.registerGlobalTargetRaycast(
  {
    entity: myEntity,
    opts: {
      queryType: RaycastQueryType.RQT_QUERY_ALL,
      target: Vector3.Zero(),
    },
  },
  (raycastResult) => {
    console.log(raycastResult.hits)
  }
)
// RAYCAST DE TARGET ENTITY
const targetEntity = engine.addEntity()
Transform.create(targetEntity, { position: Vector3.create(8, 1, 10) })

raycastSystem.registerTargetEntityRaycast(
  {
    entity: myEntity,
    opts: {
      queryType: RaycastQueryType.RQT_QUERY_ALL,
      targetEntity: targetEntity,
    },
  },
  (raycastResult) => {
    console.log(raycastResult.hits)
  }
)
```


> [!WARNING]
> **📔 Nota**: `raycastSystem`, `RaycastQueryType` y `ColliderLayer` debe importarse mediante
> 
> > `import { raycastSystem, RaycastQueryType, ColliderLayer } from "@dcl/sdk/ecs"`
> 
> Consulta [Importaciones](/docs/creator/content-creator-es/scenes-sdk7/primeros-pasos/coding-scenes/#imports) para saber cómo manejarlos fácilmente.


## Resultado del raycast

La función callback que maneja el raycast recibe un objeto que contiene datos sobre el propio rayo y sobre cualquier entities que haya golpeado.

* `globalOrigin`: La posición desde la que se originó el rayo, relativa a la escena.
* `dirección`: La dirección global hacia la que apuntaba el rayo, como un `Vector3`.
* `hits`: Un array con un objeto por cada entity que fue golpeada. Si no hubo entities golpeadas, este array está vacío. Si el raycast usó `RaycastQueryType.RQT_HIT_FIRST`, este array solo contendrá un objeto.

Cada objeto en el `hits` array incluye:

* `entityId`: Número de Id de la entity que fue golpeada por el rayo.
* `meshName`: *String* con el nombre interno del mesh específico del modelo 3D que fue golpeado. Esto es útil cuando un modelo 3D está compuesto por múltiples meshes.
* `position`: *Vector3* para la posición donde el rayo intersectó con la entity golpeada (relativa a la escena)
* `length`: Longitud del rayo desde su origen hasta la posición donde ocurrió el golpe contra la entity.
* `normalHit`: *Vector3* para la normal de la superficie golpeada en el espacio mundial.
* `globalOrigin`: *Vector3* para la posición desde donde se origina el rayo (relativa a la escena)
* `dirección`: La dirección global hacia la que apuntaba el rayo, como un `Vector3`.

El siguiente ejemplo itera sobre las entities que fueron golpeadas:

```ts
const myEntity = engine.addEntity()
Transform.create(myEntity, {
  position: Vector3.create(4, 1, 4),
})

raycastSystem.registerLocalDirectionRaycast(
  {
    entity: myEntity,
    opts: {
      queryType: RaycastQueryType.RQT_QUERY_ALL,
      direction: Vector3.Forward(),
      maxDistance: 30,
    },
  },
  function (raycastResult) {
    if (raycastResult.hits.length > 0) {
      for (const hit of raycastResult.hits) {
        if (hit.entityId) {
          console.log('entity golpeada ', hit.entityId)
        }
      }
    } else {
      console.log('no entities golpeadas')
    }
  }
)
```


> [!WARNING]
> **📔 Nota**: Puedes obtener un resultado de raycast al golpear una entity en una escena diferente.


## Manejar entities golpeadas

Cuando obtienes un resultado de raycast que golpeó una entity, puedes usar el `entityId` para interactuar con la entity y sus Components. Una entity es [nada más que un número](/docs/creator/content-creator-es/scenes-sdk7/arquitectura/entities-components/#overview), así que el `entityId` valor en sí mismo puede interpretarse como un `Entity` tipo.

```ts
const hitEntity = hit.entityId as Entity
const transform = Transform.get(hitEntity)
console.log(transform.position)
```

## Capas de colisión

Es una buena práctica comprobar solo colisiones contra entities que sean relevantes, para que la escena tenga mejor rendimiento. El `collisionMask` campo te permite enumerar solo collision layers específicas — la physics layer (paredes y suelos de la escena), la pointer layer (pointer events), las player layers (avatares), o 8 custom layers que puedes asignar libremente. Consulta [collision layers](/docs/creator/content-creator-es/scenes-sdk7/fundamentos-del-contenido-3d/colliders/#collision-layers).

Por defecto, el `collisionMask` campo está establecido en `ColliderLayer.CL_PHYSICS`. Puedes cambiar este valor para enumerar otras layers, o combinar varias con el `|` separador.

```ts
raycastSystem.registerLocalDirectionRaycast(
  {
    entity: myEntity,
    opts: {
      queryType: RaycastQueryType.RQT_QUERY_ALL,
      direction: Vector3.Forward(),
      maxDistance: 30,
      collisionMask:
        ColliderLayer.CL_CUSTOM1 |
        ColliderLayer.CL_CUSTOM3 |
        ColliderLayer.CL_POINTER,
    },
  },
  (raycastResult) => {
    log(raycastResult.hits)
  }
)
```

## Raycasting recurrente

Al usar las funciones de la `raycastSystem`, el comportamiento predeterminado es crear un solo rayo, que consultará colisiones una sola vez. Como alternativa, puedes establecer el `continuous` campo en *true* para ejecutar una consulta y la función callback en cada tick del bucle del juego.

El siguiente ejemplo seguirá ejecutando la consulta de raycast a partir de este punto

```ts
raycastSystem.registerLocalDirectionRaycast(
  {
    entity: myEntity,
    opts: {
      queryType: RaycastQueryType.RQT_QUERY_ALL,
      direction: Vector3.Forward(),
      maxDistance: 30,
      continuous: true,
    },
  },
  function (raycastResult) {
    log(raycastResult.hits)
  }
)
```


> [!WARNING]
> **📔 Nota**: El `continuous` propiedad debe usarse con precaución, ya que ejecutar una consulta de raycast en cada frame puede ser muy costoso para el rendimiento.


Cuando ya no se necesiten, elimina cualquier raycast recurrente. Para hacerlo, debes usar `raycastSystem.removeRaycasterEntity`.

```ts
raycastSystem.removeRaycasterEntity(myEntity)
```

Cuando sea posible, usa un system (o la `interval` function de la librería Utils) para ejecutar consultas de raycast a un intervalo regular más espaciado, como solo una vez por segundo o cada quinto de segundo.

```typescript
// components personalizados
const CubeOscilator = engine.defineComponent('CubeOscilator', {
  t: Schemas.Float,
})

const TimerComponent = engine.defineComponent('TimerComponent', {
  t: Schemas.Float,
})

const RAY_INTERVAL = 0.1

// comprobar rayos
engine.addSystem((dt) => {
  for (const [entity] of engine.getEntitiesWith(TimerComponent)) {
    const timer = TimerComponent.getMutable(entity)
    timer.t += dt

    if (timer.t > RAY_INTERVAL) {
      timer.t = 0
      raycastSystem.registerGlobalDirectionRaycast(
        {
          entity: myEntity,
          opts: {
            queryType: RaycastQueryType.RQT_HIT_FIRST,
            direction: Vector3.Forward(),
            maxDistance: 16,
          },
        },
        function (raycastResult) {
          log(raycastResult.hits)
        }
      )
    }
  }
})

TimerComponent.create(engine.addEntity())

// sistema del cubo oscilante
engine.addSystem((dt) => {
  for (const [entity, cube] of engine.getEntitiesWith(
    CubeOscilator,
    Transform
  )) {
    CubeOscilator.getMutable(entity).t += dt
    Transform.getMutable(entity).position.y = 2 + Math.cos(cube.t)
  }
})

// crear cubo
const cubeEntity = engine.addEntity()
Transform.create(cubeEntity, { position: { x: 8, y: 1, z: 8 } })
CubeOscilator.create(cubeEntity)
MeshRenderer.setBox(cubeEntity)
MeshCollider.setBox(cubeEntity)
```

El ejemplo anterior ejecuta un raycast recurrente cada 0,1 segundos. Usa un componente de temporizador y la `dt` propiedad del system para sincronizarlos uniformemente. También incluye un cubo que oscila hacia arriba y hacia abajo, controlado por otro system, para entrar y salir de la trayectoria del rayo.


> [!NOTE]
> **💡 Consejo**: Usa la `interval` function en la [librería Utils del SDK](https://github.com/decentraland/sdk7-utils) para una forma más sencilla de ejecutar una función a un intervalo fijo.


## Raycasts mediante un system

Otra forma de realizar raycasts recurrentes es ejecutarlos desde la función recurrente de un system. Esto te permite tener mucho más control sobre cuándo y cómo funcionan. En lugar de registrar una función callback, puedes realizar una consulta de raycast con `raycastSystem.registerRaycast` y luego comprobar los datos devueltos por esta operación, todo dentro de la función del system.

Ten en cuenta que, como el raycast se ejecuta en un system, el resultado solo estará disponible en el siguiente tick, necesitando dos ejecuciones del system. Una para registrar el raycast para el siguiente frame y la siguiente para procesar su resultado.

```ts
engine.addSystem((deltaTime) => {
		const result = raycastSystem.registerRaycast(
			myEntity,
			raycastSystem.localDirectionOptions({
				collisionMask: ColliderLayer.CL_CUSTOM1 | ColliderLayer.CL_CUSTOM3 | ColliderLayer.CL_POINTER,
				originOffset: Vector3.create(0, 0.4, 0),
				maxDistance: 16,
				queryType: RaycastQueryType.RQT_HIT_FIRST,
				direction: Vector3.Forward(),
				continuous: true // no abuses de la propiedad 'continuous' porque el raycasting es costoso para el rendimiento
			})
		)
		if (result) {
			// hacer algo
		}
	})
```

## Colisionar con el jugador

Puedes detectar avatares directamente con raycasts incluyendo cualquiera de las collision layers de avatar en la mask:

* `ColliderLayer.CL_PLAYER`: coincide con cualquier avatar — el jugador local Y cualquier otro jugador renderizado en la escena.
* `ColliderLayer.CL_MAIN_PLAYER`: coincide solo con el jugador local (principal).

Ambas layers pueden combinarse o usarse de forma independiente. El valor predeterminado `collisionMask` de un raycast es `CL_PHYSICS`, que NO golpea avatares — debes activarlo explícitamente.

```ts
// Golpea solo al jugador local (ignora otros avatares)
raycastSystem.registerLocalDirectionRaycast(
  {
    entity: myEntity,
    opts: {
      direction: Vector3.Forward(),
      collisionMask: ColliderLayer.CL_MAIN_PLAYER,
    },
  },
  (raycastResult) => {
    if (raycastResult.hits.length > 0) {
      console.log('Golpeó al jugador local')
    }
  }
)

// Golpea cualquier avatar (local + remoto)
raycastSystem.registerLocalDirectionRaycast(
  {
    entity: myEntity,
    opts: {
      direction: Vector3.Forward(),
      collisionMask: ColliderLayer.CL_PLAYER,
    },
  },
  (raycastResult) => {
    // raycastResult.hits[i].entityId es 0 para golpes a avatares remotos (sin id de entity local de la escena)
    console.log(raycastResult.hits)
  }
)
```

Cuando un raycast golpea al jugador local, el `hit.entityId` es `engine.PlayerEntity`. Los golpes en avatares remotos no llevan un id de entity local de la escena (el campo es `0`) porque el jugador remoto no es una entity en el world de tu escena — pero el golpe sigue reportándose con su `position`, `length`, `normalHit`, y otros datos geométricos.


> [!NOTE]
> **💡 Consejo**: Para detectar solo **otros** players (excluyendo al jugador local), usa `CL_PLAYER` y filtra `hit.entityId !== engine.PlayerEntity` en la callback.


## Raycasts desde el jugador

Para trazar un rayo desde la posición del jugador en la dirección hacia la que apunta la cámara, puedes trazar un rayo usando la cámara o el avatar [Entities reservadas](/docs/creator/content-creator-es/scenes-sdk7/arquitectura/entities-components/#reserved-entities).


> [!NOTE]
> **💡 Consejo**: Para la mayoría de los casos, quizá te convenga más usar [Eventos de puntero](/docs/creator/content-creator-es/scenes-sdk7/interactividad/eventos-de-button/click-events/) en lugar de raycasts.


El siguiente ejemplo traza un rayo hacia delante desde la posición de la cámara del jugador, usando la `engine.CameraEntity` entity.

```ts
raycastSystem.registerGlobalDirectionRaycast(
  {
    entity: engine.CameraEntity,
    opts: {
      queryType: RaycastQueryType.RQT_HIT_FIRST,
      direction: Vector3.rotate(
        Vector3.Forward(),
        Transform.get(engine.CameraEntity).rotation
      ),
    },
  },
  function (raycastResult) {
    console.log(raycastResult)
  }
)
```


> [!WARNING]
> **📔 Nota**: Ten en cuenta que en 3rd person, en el futuro el cursor podría no comportarse igual que en 1st person. Se recomienda usar esto solo si el jugador está en 1st person.


## Raycast desde la posición del cursor

También puedes trazar un rayo desde la posición del cursor del jugador hacia el mundo 3D. Esto puede usarse para arrastrar objetos, juegos de disparos, etc.

En este ejemplo, detectamos cuando el jugador presiona la tecla E y luego trazamos un rayo desde la posición del cursor hacia el mundo 3D. Después comprobamos si el rayo golpeó alguna entity y, si es así, hacemos algo con ella.

```ts
import { engine, Entity, InputAction, inputSystem, PointerEventType, RaycastQueryType, raycastSystem, TextShape, Transform } from '@dcl/sdk/ecs'
import { PrimaryPointerInfo } from '@dcl/sdk/ecs'

let cooldown = 1
let rayFrequency = 0.1
let mousePressed = false

export function main() {
   engine.addSystem(rayCastSystem)
}

const rayCastSystem = (t: number) => {

    if (inputSystem.isTriggered(InputAction.IA_PRIMARY, PointerEventType.PET_DOWN)) {
      mousePressed = true
    }

    if (inputSystem.isTriggered(InputAction.IA_PRIMARY, PointerEventType.PET_UP)) {
      mousePressed = false
    }

    if (!mousePressed) {
      cooldown = 0
      raycastSystem.removeRaycasterEntity(engine.CameraEntity)
      return
    }

    cooldown += t
    if (cooldown < rayFrequency) return
    cooldown = 0

    const pointerInfo = PrimaryPointerInfo.getOrCreateMutable(engine.RootEntity)
    let dir = pointerInfo.worldRayDirection

    raycastSystem.registerGlobalDirectionRaycast(
      {
        entity: engine.CameraEntity,
        opts: {
          queryType: RaycastQueryType.RQT_HIT_FIRST,
          direction: dir,
        },
      },
      function (raycastResult) {
        let result = raycastResult.hits[0]

        // hacer algo en la posición del golpe
        if (result && result.position) {
          console.log("x:", result.position.x, ", y:", result.position.y, ", z:", result.position.z)
        }

        // hacer algo con la entity golpeada
        const entity = result.entityId as Entity
        if (entity) {
          console.log("entity: ", entity)
        }
      }
    )
}

```


> [!NOTE]
> **💡 Consejo**: En este ejemplo usamos el botón principal (E) para activar el raycast. No usamos el botón de puntero (clic izquierdo) porque hacer clic y arrastrar también desplaza el ángulo de la cámara por defecto. Si quieres evitar rotar la cámara mientras arrastras, puedes usar un [Virtual Camera](/docs/creator/content-creator-es/scenes-sdk7/fundamentos-del-contenido-3d/camera/) para fijar el ángulo de la cámara.


## Sintaxis avanzada

### Crear un componente Raycast

Un componente Raycast describe el rayo invisible que se usa para consultar entities intersectadas. El rayo se traza comenzando en la posición de la entity, tal como la define el componente Transform y afectado por la de cualquier entity padre. La dirección puede definirse de varias maneras,

Los Rays se definen usando los siguientes datos:

* `dirección`: Un objeto que contiene un `$case` campo para seleccionar el tipo de dirección, y un campo adicional que dependerá de este tipo, que determina esta dirección. Los siguientes son los valores aceptados para `$case`:
  * `'localDirection'`: Una dirección relativa a la dirección frontal de la entity, afectada también por la transformación de cualquier entity padre. Esto es útil para detectar obstáculos delante de vehículos respetando su rumbo. La rotación está definida por el `localDirection` campo, como un `Vector3` que describe una rotación.
  * `'globalDirection'`: Ignora la rotación de la entity, y apunta en una dirección como si la rotación de la entity fuera 0. Esto es útil, por ejemplo, para apuntar siempre hacia abajo. La rotación está definida por el `globalDirection` campo, como un `Vector3` que describe una rotación.
  * `'globalTarget'`: Traza una línea entre la posición de la entity y una posición global objetivo en la escena. Ignora la rotación de la entity. Es útil para crear juegos de tower defense; la torreta de cada torre puede apuntar a una coordenada específica en el espacio. El objetivo está definido por el `globalTarget` campo, como un `Vector3` que describe la posición global.
  * `'targetEntity'`: Traza una línea entre la posición de la entity y la posición de una segunda entity objetivo. Ignora la rotación de ambas entities. El objetivo está definido por el `targetEntity` campo, que contiene una referencia a la entity.
* `maxDistance`: *number* para establecer la longitud con la que se trazará este rayo.
* `queryType`: *RaycastQueryType* valor de enum, para definir si el rayo devolverá todas las entities golpeadas o solo la primera. Las siguientes opciones están disponibles:
  * `RaycastQueryType.RQT_HIT_FIRST`: solo devuelve la primera entity impactada, empezando desde el punto de origen.
  * `RaycastQueryType.RQT_QUERY_ALL`: devuelve todas las entities golpeadas, desde el origen hasta la distancia máxima del rayo.
* `collisionMask`: Detecta solo colisiones con ciertas collision layers. Usa esto junto con una collision layer personalizada, o para detectar solo la physics layer o la pointer events layer. Consulta [collision layers](/docs/creator/content-creator-es/scenes-sdk7/fundamentos-del-contenido-3d/colliders/#collision-layers). Por defecto, el valor es `ColliderLayer.CL_POINTER | ColliderLayer.CL_PHYSICS`.
* `originOffset`: En lugar de iniciar el raycast desde la posición de origen de la entity, agrega un offset para comenzar la consulta desde una posición relativa. Por ejemplo, puedes usar un pequeño offset para evitar que el rayo colisione con el propio modelo 3D de la entity.
* `continuous`: Si es true, seguirá ejecutando una consulta de raycast en cada frame. Si es false, el ray solo se usará en el frame actual. Por defecto, este valor es false.


> [!WARNING]
> **📔 Nota**: El `continuous` propiedad debe usarse con precaución, ya que ejecutar una consulta de raycast en cada frame puede ser muy costoso para el rendimiento. Cuando sea posible, usa un system (o la `interval` function de la librería Utils) para ejecutar consultas de raycast a un intervalo regular más espaciado, consulta [raycasting recurrente](#recurrent-raycasting).


El siguiente ejemplo usa una rotación global para determinar la dirección y solo devuelve la primera entity que es impactada en el frame en que se envía el ray.

```typescript
const entity1 = engine.addEntity()

Transform.create(entity1, {
  position: Vector3.create(8, 1, 0)
})

Raycast.createOrReplace(entity1, {
  direction: {
    $case: "globalDirection",
    globalDirection: Vector3.create(0, 0, 1)
  },
  maxDistance: 16,
  queryType: RaycastQueryType.RQT_HIT_FIRST
})
```

El ejemplo siguiente lanza un ray en la dirección frontal de la entity, devolviendo solo el primer elemento impactado. Lo hace de forma continua. También incluye un pequeño offset de 0.5 para evitar que el ray golpee el collider propio de la entity.

```typescript
const entity1 = engine.addEntity()

Transform.create(entity1, {
  position: Vector3.create(8, 1, 0)
})

Raycast.createOrReplace(entity1, {
  direction: {
    $case: "localDirection",
    localDirection: Vector3.Forward()
  },
  maxDistance: 16,
  queryType: RaycastQueryType.RQT_HIT_FIRST,
  originOffset: Vector3.create(0.5, 0, 0),
  continuous: true
})
```

Este ejemplo traza un ray entre dos entities. Devuelve todas las entities que son impactadas entre ellas.

```ts
const entity1 = engine.addEntity()

Transform.create(entity1, {
  position: Vector3.create(8, 1, 0)
})

const entity2 = engine.addEntity()

Transform.create(entity2, {
  position: Vector3.create(0, 1, 8)
})

Raycast.createOrReplace(entity1, {
  direction: {
    $case: "targetEntity",
    targetEntity: entity2
  },
  maxDistance: 16,
  queryType: RaycastQueryType.RQT_QUERY_ALL
})
```

### Componente de resultados de Raycast


> [!WARNING]
> **📔 Nota**: La forma más sencilla de trabajar con los resultados del raycast es usar `raycastSystem`, y registrar una función callback como parte de la misma instrucción que crea el ray. El`RaycastResult` component se usa internamente por esa interfaz, pero también se expone para permitir una lógica personalizada más avanzada.


Después de crear un componente Raycast, la entity a la que se añade este component tendrá un `RaycastResult` component. Este component incluye información sobre cualquier impacto del ray. Configura un System para comprobar estos datos.

El `RaycastResult` component contiene los siguientes datos:

* `globalOrigin`: La posición desde la que se originó el rayo, relativa a la escena.
* `dirección`: La dirección global hacia la que apuntaba el rayo, como un `Vector3`.
* `hits`: Un array con un objeto por cada entity que fue golpeada. Si no hubo entities golpeadas, este array está vacío. Si el raycast usó `RaycastQueryType.RQT_HIT_FIRST`, este array solo contendrá un objeto.

Cada objeto en el `hits` array incluye:

* `entityId`: Número de Id de la entity que fue golpeada por el rayo.
* `meshName`: *String* con el nombre interno del mesh específico del modelo 3D que fue golpeado. Esto es útil cuando un modelo 3D está compuesto por múltiples meshes.
* `position`: *Vector3* para la posición donde el rayo intersectó con la entity golpeada (relativa a la escena)
* `length`: Longitud del rayo desde su origen hasta la posición donde ocurrió el golpe contra la entity.
* `normalHit`: *Vector3* para la normal de la superficie golpeada en el espacio mundial.
* `globalOrigin`: *Vector3* para la posición desde donde se origina el rayo (relativa a la escena)
* `dirección`: La dirección global hacia la que apuntaba el rayo, como un `Vector3`.

El ejemplo siguiente muestra cómo puedes acceder a los resultados de una entity individual usando un System:

```typescript

const rayEntity = engine.addEntity()

Transform.create(rayEntity, {
  position: Vector3.create(8, 1, 0)
})

// devolver todas las entities
Raycast.createOrReplace(rayEntity, {
  direction: {
    $case: "globalDirection",
    globalDirection: Vector3.create(0, 0, 1)
  },
  maxDistance: 16,
  queryType: RaycastQueryType.RQT_QUERY_ALL
})

engine.addSystem(() => {
  const rayResult = RaycastResult.get(rayEntity)
  console.log(rayResult.hits)
})
```

El siguiente ejemplo muestra cómo puedes acceder a `RaycastResult` components de todas las entities en la escena, usando una [consulta de components](/docs/creator/content-creator-es/scenes-sdk7/arquitectura/querying-components/).

```typescript
engine.addSystem(() => {
  for (const [_, result] of engine.getEntitiesWith(RaycastResult)) {
    console.log(result.hits)
  }
})
```


> [!WARNING]
> **📔 Nota**: Los resultados de un raycast no llegan en el mismo tick del bucle del juego en que creaste el raycast. Los resultados pueden tardar uno o varios ticks en llegar.


En una escena en la que uses varios tipos de rays para diferentes propósitos (como búsqueda de rutas, comprobación de línea de visión, trazado de proyectiles, etc.), quizá quieras usar distintos [collision layers](/docs/creator/content-creator-es/scenes-sdk7/fundamentos-del-contenido-3d/colliders/#collision-layers), para evitar calcular colisiones irrelevantes.


> [!NOTE]
> **💡 Consejo**: Para ver un ejemplo funcional de raycasting, consulta el [`77,-1-raycast-unit-tests`](https://github.com/decentraland/sdk7-test-scenes/tree/main/scenes/77,-1-raycast-unit-tests) test scene, que afirma el comportamiento del raycast mediante `@dcl/sdk/testing` unit tests en los `localDirection`, `globalDirection`, `globalTarget` y `targetEntity` formatos de dirección, incluidos los rays lanzados desde padres transformados. Para ver cómo se comportan las collider layers que un ray puede impactar, consulta [`5,5-collider-layers`](https://github.com/decentraland/sdk7-test-scenes/tree/main/scenes/5,5-collider-layers).



---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-es/scenes-sdk7/interactividad/raycasting.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
