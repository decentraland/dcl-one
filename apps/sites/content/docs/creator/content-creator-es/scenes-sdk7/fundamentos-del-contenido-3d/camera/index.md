> Aprende a controlar la cámara del jugador

# Cámara

Como creador, puedes tener control total sobre la cámara del jugador. De forma predeterminada, los jugadores pueden elegir libremente entre un modo de cámara en 1.ª o 3.ª persona mientras exploran tu escena, pero puedes imponer una modalidad de cámara diferente.

Las cámaras virtuales pueden ser estáticas, pueden girar para mirar siempre al jugador o a alguna otra Entity, o pueden estar adjuntas al jugador o a alguna otra Entity para acompañarlo siempre.


> [!WARNING]
> **📔 Nota**: Para cambiar entre las cámaras predeterminadas en 1.ª y 3.ª persona, consulta [áreas modificadoras de cámara](#1st-and-3rd-person-camera-modes).


## modos de cámara en 1.ª y 3.ª persona

Por lo general, los jugadores son libres de alternar entre la cámara en primera y tercera persona presionando V en el teclado. Usa un `CameraModeArea` para forzar el modo de cámara a 1.ª o 3.ª persona para todos los jugadores que estén dentro de un área específica en tu escena.

```ts
const entity = engine.addEntity()

CameraModeArea.create(entity, {
	area: Vector3.create(4, 3, 4),
	mode: CameraType.CT_FIRST_PERSON,
})
```

Si el modo de cámara actual de un jugador no coincide con el de la `CameraModeArea`, cambiará a ese modo de cámara. Aparece un toast en pantalla para aclarar que este cambio se debe a la escena. Mientras esté dentro, los jugadores no pueden cambiar su modo de cámara. Cuando un jugador sale de la `CameraModeArea`, su modo de cámara se restaura al que tenía antes de entrar.

Usa `CameraModeArea` en regiones donde los jugadores tendrían una experiencia significativamente mejor al usar un modo de cámara específico. Por ejemplo, la primera persona es ideal si el jugador necesita hacer clic en un objeto pequeño, o la tercera persona puede ser útil para que los jugadores noten alguna Entity que tu escena tiene adjunta sobre su cabeza. No asumas que los jugadores saben cómo cambiar de modo de cámara; muchos jugadores primerizos podrían no saber que tienen esa opción, o no recordar la tecla para hacerlo.


> [!WARNING]
> **📔 Nota**: Las áreas modificadoras de cámara se ven afectadas por *posición* y *rotación* de Transform component de su Entity anfitriona, pero no se ven afectadas por *escala*.



> [!WARNING]
> **📔 Nota**: Si superpones varias áreas modificadoras de cámara, la última que sea instanciada por el código de tu escena tendrá prioridad sobre las demás.


Al crear un `CameraModeArea` component, debes proporcionar lo siguiente:

* `area`: Tamaño del área modificadora
* `mode`: Qué modo de cámara forzar en esta área, desde el `CameraType` enum.

Los modos de cámara compatibles son:

* `CameraType.CT_FIRST_PERSON`
* `CameraType.CT_THIRD_PERSON`

### Consultar el modo de cámara

Puedes consultar el modo de cámara del jugador usando el `CameraMode` component en el `engine.CameraEntity`.

```ts
const cameraMode = CameraMode.get(engine.CameraEntity)
if (cameraMode.mode === CameraType.CT_FIRST_PERSON) {
	console.log('El jugador está usando la cámara en 1.ª persona')
} else {
	console.log('El jugador está usando la cámara en 3.ª persona')
}
```

También puedes suscribirte a los cambios en el modo de cámara usando el `onChange` función en el `CameraMode` componente.

```ts
CameraMode.onChange(engine.CameraEntity, (cameraMode) => {
	if (!cameraMode) return
	console.log('El modo de cámara del jugador cambió a', cameraMode.mode)
})
```

## Uso de cámaras virtuales

Para usar un comportamiento de cámara personalizado en tu escena, necesitas dos cosas:

* Crear una Virtual Camera: crea una Entity en tu escena y asígnale un `VirtualCamera`.
* Asignar esa cámara virtual: añade una `MainCamera` componente a la [entidad reservada](/docs/creator/content-creator-es/scenes-sdk7/arquitectura/entities-components/#reserved-entities) `engine.CameraEntity`, con una referencia a la Entity con el `VirtualCamera` componente.

La cámara quedará entonces adjunta a la Entity con el `VirtualCamera` component. Si la Entity se mueve o rota, la cámara se mueve con ella.

```ts
function main() {
	const myCustomCamera = engine.addEntity()
	Transform.create(myCustomCamera, {
		position: Vector3.create(1, 2, 1),
	})
	VirtualCamera.create(myCustomCamera, {})

	const mainCamera = MainCamera.createOrReplace(engine.CameraEntity, {
		virtualCameraEntity: myCustomCamera,
	})
}
```

En este ejemplo, la cámara siempre estará en una posición fija en la escena, mientras el jugador permanezca dentro de los límites de la escena. En cuanto el jugador salga de los límites de la escena, se restaurará el comportamiento predeterminado de la cámara.

### Campo de visión

Establece el campo de visión (FOV) de una cámara virtual con la `fov` propiedad, en grados. Solo se aplica mientras esta cámara virtual está activa.

```ts
const cinematicCamera = engine.addEntity()
Transform.create(cinematicCamera, {
	position: Vector3.create(8, 3, 2),
})
VirtualCamera.create(cinematicCamera, {
	fov: 45,
})
```

Un FOV más estrecho (valor más bajo) acerca la imagen, útil para cinemáticas o apuntar. Un FOV más amplio (valor más alto) muestra más de la escena, útil para vistas panorámicas.


> [!WARNING]
> **📔 Nota**: Establece `fov` en cada cámara virtual que crees. Omitirlo no te da un valor neutro de 60 grados: el cliente reutiliza instancias de cámara agrupadas sin restablecer la lente, por lo que una cámara sin `fov` conserva cualquier campo de visión que una cámara anterior haya dejado en esa instancia. Una cámara pensada para mostrar una vista aérea amplia puede abrirse ya muy acercada.


Tu escena puede incluir tantas Entities con un `VirtualCamera`component como quieras, y alternar dinámicamente entre varias cámaras virtuales a medida que el jugador se mueve, o mientras realiza ciertas acciones. Solo una cámara virtual está activa en un momento dado; esto lo asigna el `MainCamera` component en `engine.CameraEntity`.

Para volver al comportamiento de cámara predeterminado, establece el valor en `undefined` en `MainCamera.virtualCameraEntity`. El jugador entonces es libre de alternar entre las cámaras en 1.ª y 3.ª persona. Si quieres que el jugador use solo uno de estos dos modos, puedes usar un [áreas modificadoras de cámara](#1st-and-3rd-person-camera-modes) para forzar uno de los dos.


> [!WARNING]
> **📔 Nota**: Libera la cámara con `undefined`, nunca con `0`. Solo un valor ausente devuelve el control al jugador. Un valor de `0` es `engine.RootEntity`, que no tiene `VirtualCamera`, así que el engine conserva la última cámara virtual activa y el jugador queda atrapado en esa vista.
> 
> Esto detecta el código que guarda y restaura la cámara, porque `virtualCameraEntity` es opcional y se lee como `undefined` en una `MainCamera`. Un `?? 0` o `|| 0` fallback lo convierte en un `0` que luego se vuelve a escribir. Comprueba la Entity antes de restaurarla:
> 
> ```ts
> function restoreCamera(previous: Entity | undefined) {
> 	const camera = MainCamera.getMutableOrNull(engine.CameraEntity)
> 	if (!camera) return
> 	// Solo restaura una cámara que realmente exista; de lo contrario, libera el control
> 	camera.virtualCameraEntity = previous && VirtualCamera.has(previous) ? previous : undefined
> }
> ```
> 



> [!WARNING]
> **📔 Nota**: Las áreas modificadoras de cámara solo tienen efecto en el jugador si no hay cámaras virtuales activas. Si la escena está usando actualmente una cámara virtual y el jugador entra en un área modificadora de cámara, no ocurre nada.
> 
> Si un modelo 3D incluye un `cámara` node como parte de su contenido, esto no puede ser usado por el SDK. Debes crear todas las cámaras como Entities con el SDK.


```ts
function main() {
	// cámara virtual personalizada
	const myCustomCamera = engine.addEntity()
	Transform.create(myCustomCamera, {
		position: Vector3.create(1, 2, 1),
	})
	VirtualCamera.create(myCustomCamera, {})

	const mainCamera = MainCamera.createOrReplace(engine.CameraEntity, {
		virtualCameraEntity: myCustomCamera,
	})

	// cubo clicable
	const clickCube = engine.addEntity()
	Transform.create(clickCube, { position: Vector3.create(8, 0, 8) })
	MeshRenderer.setBox(clickCube)
	MeshCollider.setBox(clickCube)
	pointerEventsSystem.onPointerDown(
		{
			entity: clickCube,
			opts: { button: InputAction.IA_POINTER, hoverText: 'Restablecer cámara' },
		},
		() => {
			// restablecer la cámara al comportamiento predeterminado
			const mainCamera = MainCamera.getMutable(engine.CameraEntity)
			mainCamera.virtualCameraEntity = undefined
		}
	)
}
```


> [!NOTE]
> **💡 Consejo**: Cuando la cámara se aleja del avatar, suele ser una buena práctica congelar también los movimientos del avatar. De esa forma, el jugador no avanza a ciegas hacia obstáculos. Consulta [Modificadores de entrada](/docs/creator/content-creator-es/scenes-sdk7/interactividad/avatares/locomotion/#freeze-the-player)


Para permitir que el jugador dirija una cámara virtual con el mouse, lee la `screenDelta` propiedad de `PrimaryPointerInfo` component para ver cuánto se movió el cursor en cada frame, y luego aplica ese movimiento a la rotación de la cámara. Esto funciona incluso mientras el cursor está bloqueado. Consulta [Movimiento del mouse](/docs/creator/content-creator-es/scenes-sdk7/interactividad/mouse-movement/) para ver un ejemplo completo de mouselook.

## Vista aérea

Puedes usar una cámara virtual para ver la escena desde una vista cenital; esto puede ser una variación divertida de la perspectiva normal de tu avatar y habilitar diferentes mecánicas de juego.

Debes evitar colocar una cámara apuntando en una dirección perfectamente vertical hacia abajo. En su lugar, inclina siempre la cámara ligeramente; incluso 1 grado es suficiente. Esto se debe a que los controles de dirección del jugador se basan en la perspectiva de la cámara, no en la orientación del avatar. Si la cámara está mirando desde una vista aérea perfecta, no existe una definición clara de qué dirección es cuál. Una inclinación imperceptible de 1 grado es suficiente para establecer una dirección hacia adelante.

```ts
function main() {
	// cámara de vista aérea
	const myCustomCamera = engine.addEntity()
	Transform.create(myCustomCamera, {
		position: Vector3.create(8, 5, 8),
		rotation: Quaternion.fromEulerDegrees(91, 0, 0),
		// Ten en cuenta que la rotación es 91º, no 90º
	})
	VirtualCamera.create(myCustomCamera, {})

	const mainCamera = MainCamera.createOrReplace(engine.CameraEntity, {
		virtualCameraEntity: myCustomCamera,
	})
}
```

## Campo de visión

Puedes establecer un campo de visión (FOV) personalizado en una cámara virtual con la `fov` propiedad. El valor está en grados. Establécelo siempre: no hay un valor predeterminado al que recurrir, y una cámara sin `fov` hereda la lente que dejó una cámara anterior que usó la misma instancia reutilizada.

```ts
VirtualCamera.create(myCustomCamera, {
	fov: 90,
})
```

Un FOV más amplio (valor más alto) muestra más de la escena a la vez y crea una sensación de velocidad, lo que puede ser útil para juegos de carreras. Un FOV más estrecho (valor más bajo) acerca la imagen y es útil para apuntar o para planos cinematográficos.

### Cambiar el nivel de zoom

El cliente lee `fov` solo en el momento en que una cámara se vuelve activa. Cambiar `fov` en la cámara que ya está activa no hace nada.

Para hacer zoom, crea una cámara virtual por cada nivel de zoom y cambia `MainCamera.virtualCameraEntity` entre ellas:

```ts
import { engine, MainCamera, Transform, VirtualCamera } from '@dcl/sdk/ecs'
import { Vector3 } from '@dcl/sdk/math'

// Dos cámaras en el mismo lugar, una amplia y otra acercada
const position = Vector3.create(8, 2, 8)

const wideCamera = engine.addEntity()
Transform.create(wideCamera, { position })
VirtualCamera.create(wideCamera, { fov: 70 })

const zoomedCamera = engine.addEntity()
Transform.create(zoomedCamera, { position })
VirtualCamera.create(zoomedCamera, { fov: 30 })

// Cambia entre ellas para hacer zoom
function zoomIn() {
	MainCamera.createOrReplace(engine.CameraEntity, { virtualCameraEntity: zoomedCamera })
}

function zoomOut() {
	MainCamera.createOrReplace(engine.CameraEntity, { virtualCameraEntity: wideCamera })
}
```

Volver a asignar la cámara que ya está activa tampoco hace nada, así que cambia a una Entity diferente cada vez.

## Transiciones de cámara

Cada vez que la escena cambia entre cámaras virtuales, o entre el comportamiento de cámara predeterminado y las cámaras virtuales, los jugadores ven una transición. La posición, la rotación y cualquier otro parámetro de la cámara virtual cambian suavemente durante un periodo de tiempo.

Los ajustes de transición en una cámara virtual determinan cómo haces la transición *a* esa cámara, desde cualquier otra cámara de la escena, incluida la predeterminada. No afectan cómo haces la transición *out* de esa cámara.

Al volver a la propia cámara del jugador, cuando limpias el `virtualCameraEntity` de la `MainCamera`, siempre toma una transición fija de 0,5 segundos con aceleración y desaceleración suaves. El `defaultTransition` de la cámara virtual que dejas no lo cambia.

```ts
VirtualCamera.create(myCustomCamera1, {
	defaultTransition: { transitionMode: VirtualCamera.Transition.Time(6) },
})
```


> [!NOTE]
> **💡 Consejo**: Para evitar tener una transición y cambiar instantáneamente a una cámara, establece el tiempo o la velocidad de transición en 0.


Según tu caso de uso, quizá prefieras establecer la velocidad de la transición en lugar de la duración:

* **Tiempo fijo**: Estableces la duración de la transición; la cámara se moverá tan rápido como sea necesario para completar el recorrido en ese periodo de tiempo.
* **Velocidad fija**: Estableces qué tan rápido quieres que se mueva la cámara virtual durante la transición; la duración dependerá de la distancia. El valor usado para la velocidad se interpreta como **metros por segundo**.

A continuación hay ejemplos para ambos modos de transición:

```ts
// duración fija
VirtualCamera.create(myCustomCamera1, {
	defaultTransition: { transitionMode: VirtualCamera.Transition.Time(6) },
})

// velocidad fija
VirtualCamera.create(myCustomCamera1, {
	defaultTransition: { transitionMode: VirtualCamera.Transition.Speed(3) },
})
```

A continuación hay un ejemplo completo con dos cámaras virtuales y transiciones entre ellas:

```ts
function main() {
	// cámara virtual personalizada 1
	const myCustomCamera1 = engine.addEntity()
	Transform.create(myCustomCamera1, {
		position: Vector3.create(1, 2, 1),
	})
	VirtualCamera.create(myCustomCamera1, {
		defaultTransition: { transitionMode: VirtualCamera.Transition.Time(1) },
	})

	// cámara virtual personalizada 2
	const myCustomCamera2 = engine.addEntity()
	Transform.create(myCustomCamera2, {
		position: Vector3.create(1, 2, 1),
	})
	VirtualCamera.create(myCustomCamera2, {
		defaultTransition: { transitionMode: VirtualCamera.Transition.Time(3) },
	})

	const mainCamera = MainCamera.createOrReplace(engine.CameraEntity, {
		virtualCameraEntity: myCustomCamera1,
	})

	// cubo clicable
	const clickCube = engine.addEntity()
	Transform.create(clickCube, { position: Vector3.create(8, 0, 8) })
	MeshRenderer.setBox(clickCube)
	MeshCollider.setBox(clickCube)
	pointerEventsSystem.onPointerDown(
		{
			entity: clickCube,
			opts: { button: InputAction.IA_POINTER, hoverText: 'Restablecer cámara' },
		},
		() => {
			// restablecer la cámara al comportamiento predeterminado
			const mainCamera = MainCamera.getMutable(engine.CameraEntity)
			mainCamera.virtualCameraEntity =
				mainCamera.virtualCameraEntity == myCustomCamera1
					? myCustomCamera2
					: myCustomCamera1
		}
	)
}
```

Las transiciones siempre se mueven en línea recta, sin considerar ningún obstáculo en el camino. En su lugar, podrías crear una transición manualmente usando otra cámara virtual como intermediaria; así tendrías control total sobre sus movimientos. Esta cámara virtual intermedia podría realizar un [Tween](/docs/creator/content-creator-es/scenes-sdk7/fundamentos-del-contenido-3d/move-entities/#move-between-two-points) desde la posición de la primera cámara hasta la posición de la segunda cámara, o seguir una trayectoria más personalizada que evite obstáculos o tome un desvío cinematográfico.

## Seguimiento de cámara

Puedes configurar una cámara virtual para que siempre mire en la dirección del jugador, o de alguna Entity específica de la escena. La posición de la cámara permanecerá estática, pero su rotación cambiará para mantener siempre esta Entity centrada.

Esto se puede lograr con la `lookAtEntity` propiedad en el `VirtualCamera` component. Para seguir al jugador, usa la [entidad reservada](/docs/creator/content-creator-es/scenes-sdk7/arquitectura/entities-components/#reserved-entities) `engine.PlayerEntity`.

```ts
const myCustomCamera1 = engine.addEntity()
Transform.create(myCustomCamera1, {
	position: Vector3.create(1, 2, 1),
})
VirtualCamera.create(myCustomCamera1, {
	lookAtEntity: engine.PlayerEntity,
})
```

Si una Entity está siendo seguida por la cámara, esto solo cambiará la rotación, no la posición de la cámara.

A medida que la cámara rota, el Transform de la Entity con el `VirtualCamera` component no cambia. Sin embargo, puedes leer la rotación de la cámara desde el Transform en `engine.CameraEntity`. La rotación y la posición de esta Entity serán absolutas; no estarán condicionadas por las de la Entity con el `VirtualCamera` component. La rotación de este transform se ve afectada como por el comportamiento `lookAtEntity` .


> [!WARNING]
> **📔 Nota**: Si configuras la cámara virtual con un `lookAtEntity` que referencia la misma Entity que contiene la cámara virtual, o la `engine.CameraEntity` entity, el comportamiento resultante será el mismo que no asignar ninguna Entity en absoluto.


## Adjuntar al jugador

Otro uso de la cámara virtual es seguir al jugador desde una distancia o ángulo personalizados, adjuntando una cámara virtual a la entity del jugador. Ten en cuenta que el jugador no puede cambiar libremente la rotación de la cámara, así que en este caso la rotación de la cámara quedará fijada a la de la cámara virtual. Esto podría ser útil, por ejemplo, para juegos de carreras, donde se espera que el jugador mire siempre hacia adelante.

```ts
function main() {
	const myCustomCamera = engine.addEntity()
	Transform.create(myCustomCamera, {
		position: Vector3.create(0, 1, 5),
		parent: engine.PlayerEntity,
	})
	VirtualCamera.create(myCustomCamera, {
		defaultTransition: { transitionMode: VirtualCamera.Transition.Time(2) },
	})

	const mainCamera = MainCamera.createOrReplace(engine.CameraEntity, {
		virtualCameraEntity: myCustomCamera,
	})
}
```

## Cámaras y colliders

Cuando la cámara de un jugador se mueve en modo de 3.ª persona, la cámara puede quedar bloqueada por colliders o no, dependiendo de las collision layers asignadas a las Entities. Ten esto en cuenta al diseñar tu escena; quizá quieras evitar que la cámara atraviese paredes u otras Entities.

Consulta [Colliders](/docs/creator/content-creator-es/scenes-sdk7/fundamentos-del-contenido-3d/colliders/#cameras-and-colliders) para obtener más detalles sobre cómo configurar colliders para tu escena.

## Modo espectador (cámara de observador)

Puedes crear un modo espectador que cambie al jugador de un movimiento normal del avatar a una cámara libre o que siga al jugador. Esto es útil para roles de observador en juegos competitivos, cámaras de director para eventos en vivo o sistemas de repetición.

El patrón combina varias funciones del SDK:

| Función                             | API del SDK                               | Propósito                                 |
| ----------------------------------- | ----------------------------------------- | ----------------------------------------- |
| Vista de cámara personalizada       | `VirtualCamera` + `MainCamera`            | Reemplaza la cámara del jugador           |
| Congelar el movimiento del avatar   | `InputModifier` (`disableAll: true`)      | Libera WASD para controlar la cámara      |
| Seguir a los jugadores en la escena | `onEnterScene` / `onLeaveScene`           | Construye una lista de objetivos a seguir |
| Controles de cámara                 | `inputSystem.isPressed(InputAction.IA_*)` | WASD para pitch/yaw, E/F para zoom        |
| Mouse-look                          | `PrimaryPointerInfo.screenDelta`          | Gira la cámara con el mouse               |

### Arquitectura del rig de cámara

Usa un **rig de dos Entities** para que yaw y pitch permanezcan independientes:

```
rigRoot (entity)           -- posición mundial + rotación yaw
└── rigCamera (child)      -- rotación pitch + desplazamiento orbital
    └── VirtualCamera
```

`rigRoot` contiene el yaw (giro izquierda/derecha) y se interpola hacia el objetivo seguido o hacia un pivote de cámara libre. `rigCamera` maneja el pitch (inclinación arriba/abajo) y la distancia orbital desde la raíz. Separar yaw y pitch en dos Transforms mantiene la matemática euler simple.

### Activar y desactivar

```ts
// Activar modo espectador
const rigRoot = engine.addEntity()
Transform.create(rigRoot, {
  position: Vector3.create(8, 8, 8),
  rotation: Quaternion.fromEulerDegrees(0, 0, 0),
})

const rigCamera = engine.addEntity()
Transform.create(rigCamera, { parent: rigRoot })
VirtualCamera.create(rigCamera, {})

MainCamera.createOrReplace(engine.CameraEntity, { virtualCameraEntity: rigCamera })
InputModifier.createOrReplace(engine.PlayerEntity, {
  mode: InputModifier.Mode.Standard({ disableAll: true }),
})
```

```ts
// Desactivar modo espectador
// IMPORTANTE: limpia MainCamera ANTES de eliminar la Entity VirtualCamera.
// Si eliminas primero la Entity, el engine sigue enlazando a una Entity muerta
// y la vista cae a los pies del jugador.
const mainCamera = MainCamera.getMutableOrNull(engine.CameraEntity)
if (mainCamera) mainCamera.virtualCameraEntity = undefined

engine.removeEntity(rigCamera)
engine.removeEntity(rigRoot)

InputModifier.createOrReplace(engine.PlayerEntity, {
  mode: InputModifier.Mode.Standard({ disableAll: false }),
})
```


> [!CAUTION]
> **Advertencia:** Limpia siempre `MainCamera.virtualCameraEntity` antes de eliminar la Entity de la cámara. Eliminar primero la Entity deja al engine apuntando a una referencia muerta, lo que provoca que la vista se rompa.


### Límites de cámara

El engine desactiva `VirtualCamera` Entities que se mueven fuera de los límites de parcel de tu escena. Si la cámara sale del área de la escena, deja de funcionar en silencio. Limita la posición de la cámara para que permanezca dentro del AABB de la escena en cada frame, con un pequeño margen.

```ts
// Límites de ejemplo para una escena de 1x1 parcel (16 m x 16 m)
const BOUNDS_MIN = Vector3.create(0, 0, 0)
const BOUNDS_MAX = Vector3.create(16, 20, 16)
const BOUNDS_MARGIN = 0.5
```

Para escenas más grandes, establece los límites para que coincidan con tus `scene.json` parcels. El límite de altura de la escena es de 330 metros independientemente de la cantidad de parcels. Una escena de 4x4 parcels podría usar `Vector3.create(64, 330, 64)`.

### Seguir a los jugadores

Usa `onEnterScene` y `onLeaveScene` para construir una lista en vivo de jugadores en tu escena. Luego, los jugadores pueden ir alternando entre objetivos a seguir con teclas (por ejemplo, `IA_ACTION_3` y `IA_ACTION_4`). Al seguir a un jugador, la raíz del rig interpola hacia la posición Transform del jugador seguido, y la cámara hija orbita a una distancia configurable.

```ts
import { onEnterScene, onLeaveScene } from '@dcl/sdk/src/players'

const playerEntities = new Map<string, Entity>()
let playerIds: string[] = []

onEnterScene((player) => {
  if (!player) return
  playerIds.push(player.userId)
  playerEntities.set(player.userId, player.entity)
})

onLeaveScene((userId) => {
  if (!userId) return
  playerIds = playerIds.filter((id) => id !== userId)
  playerEntities.delete(userId)
})
```

Para seguir a un jugador en cada frame, interpola la raíz del rig hacia su posición:

```ts
const followEntity = playerEntities.get(followTargetId)
if (followEntity) {
  const targetPos = Transform.get(followEntity).position
  const rootTransform = Transform.getMutable(rigRoot)
  rootTransform.position = Vector3.lerp(
    rootTransform.position,
    Vector3.add(targetPos, Vector3.create(0, 1, 0)),
    0.1
  )
}
```

Consulta [El jugador entra o sale de la escena](/docs/creator/content-creator-es/scenes-sdk7/interactividad/event-listeners/#player-enters-or-leaves-scene) para más sobre `onEnterScene` y `onLeaveScene`.

### Mouse-look mientras observas

Leer `PrimaryPointerInfo.screenDelta` cada frame para rotar la cámara con el mouse. Esto funciona mientras el puntero está bloqueado. Ver [Movimiento del mouse](/docs/creator/content-creator-es/scenes-sdk7/interactividad/mouse-movement/) para ver un ejemplo completo de mouselook.

```ts
const MOUSE_SENSITIVITY = 0.15 // grados por píxel
let yaw = 0
let pitch = 45

function spectateMouseLook() {
  const isLocked = PointerLock.getOrNull(engine.CameraEntity)?.isPointerLocked ?? false
  if (!isLocked) return

  const delta = PrimaryPointerInfo.getOrNull(engine.RootEntity)?.screenDelta
  if (!delta) return

  yaw = (yaw + delta.x * MOUSE_SENSITIVITY) % 360
  // delta.y crece hacia abajo, así que mouse-up es negativo: sumarlo inclina la cámara hacia arriba; limitar para evitar el giro
  pitch = Math.max(-25, Math.min(80, pitch + delta.y * MOUSE_SENSITIVITY))
}
```


> [!WARNING]
> **Nota:** `screenDelta` es solo para escritorio. En móvil siempre reporta `0`. Si tu escena apunta a móviles, diseña una alternativa basada en touch.


### Implementación de referencia completa

El [`33,20-spectate-mode`](https://github.com/decentraland/sdk7-test-scenes/tree/main/scenes/33,20-spectate-mode) la test scene contiene un módulo de spectate autónomo en `src/spectate.ts`. Admite modos free-cam y follow-cam, controles WASD + mouse-look, ciclado de players, zoom de distancia orbital, limitación de bounds y un HUD en pantalla que muestra los controles y el objetivo de seguimiento actual.

Para usarlo en tu propio proyecto:

1. Copia `src/spectate.ts` en tu escena.
2. Actualiza `PIVOT`, `BOUNDS_MIN`y `BOUNDS_MAX` al inicio del archivo para que coincidan con los parcels de tu escena. Este es el error de integración más común.
3. Conecta `toggleSpectate()` a cualquier trigger: una entidad clicable, un botón de UI o una pulsación de tecla.


> [!NOTE]
> **💡 Consejo**: Para ver ejemplos funcionales de control de cámara, consulta la [`2,22-virtual-cameras`](https://github.com/decentraland/sdk7-test-scenes/tree/main/scenes/2,22-virtual-cameras) test scene, que alterna varias `VirtualCamera` entities a través de `MainCamera` incluyendo `lookAtEntity` apuntada al jugador y una cámara interpolada; [`32,20-virtual-camera-mouse-look`](https://github.com/decentraland/sdk7-test-scenes/tree/main/scenes/32,20-virtual-camera-mouse-look), que controla una cámara en mouselook mientras el puntero está bloqueado; [`33,20-spectate-mode`](https://github.com/decentraland/sdk7-test-scenes/tree/main/scenes/33,20-spectate-mode), que implementa una cámara completa de spectate/observer con modos follow-cam y free-cam; y [`9,99-modifier-areas`](https://github.com/decentraland/sdk7-test-scenes/tree/main/scenes/9,99-modifier-areas), que fuerza un modo de cámara dentro de un volumen con `CameraModeArea`.



---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-es/scenes-sdk7/fundamentos-del-contenido-3d/camera.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
