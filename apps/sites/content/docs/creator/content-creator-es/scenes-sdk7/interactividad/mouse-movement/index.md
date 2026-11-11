> Reacciona al movimiento del mouse del jugador en tiempo real, para impulsar gestos de arrastre, girar objetos o controlar cámaras personalizadas.

# Movimiento del mouse

Tu escena puede leer el movimiento bruto del mouse del jugador en cada frame y usarlo para impulsar interacciones en tiempo real: gestos de arrastrar y deslizar, hacer girar o deslizar objetos mientras el jugador los arrastra, o dirigir una cámara personalizada como en un shooter en primera persona.

Para hacer esto, lee la `screenDelta` propiedad del `PrimaryPointerInfo` componente en `engine.RootEntity`. Esta propiedad es un `Vector2` que informa cuántos píxeles se movió el mouse desde el último frame. Los valores positivos de `x` significan que el mouse se movió a la derecha, los valores positivos de `y` significan que se movió hacia abajo, coincidiendo con el origen de la pantalla en la esquina superior izquierda — el mismo espacio de coordenadas que `UiTransform` y `UiCanvasInformation`. En los frames en los que el mouse no se mueve, ambos valores son 0.

```ts
import { engine, PrimaryPointerInfo } from '@dcl/sdk/ecs'

engine.addSystem(() => {
	const delta = PrimaryPointerInfo.getOrNull(engine.RootEntity)?.screenDelta
	if (!delta) return
	console.log(`mouse moved: ${delta.x}, ${delta.y}`)
})
```

Como `screenDelta` solo contiene el movimiento de un único frame, léelo siempre dentro de un [system](/docs/creator/content-creator-es/scenes-sdk7/arquitectura/systems/), para que ningún movimiento pase desapercibido entre frames.


> [!WARNING]
> **📔 Nota**: Evita hacer referencia a `engine.RootEntity` en la carga inicial de la escena, porque eso puede provocar errores si los Entities todavía no se han inicializado. Para evitar este problema, haz siempre referencia al entity dentro de un system. Siempre estará disponible, porque la primera ejecución del system ocurre una vez que la escena ya está correctamente inicializada.



> [!WARNING]
> **📔 Nota**: Lo descrito en este documento solo es relevante para jugadores en desktop. En la [mobile app](/docs/creator/content-creator-es/desarrollar-para-movil/cliente-movil/overview/), la entrada se basa en el tacto y no hay un cursor de movimiento libre, así que `screenDelta` siempre informa 0. Consulta [Input on mobile](/docs/creator/content-creator-es/desarrollar-para-movil/desarrollar/input-on-mobile/) para ver cómo funciona la entrada en dispositivos táctiles, y [Detect platform](/docs/creator/content-creator-es/desarrollar-para-movil/desarrollar/detect-platform/) para ofrecer controles alternativos a los jugadores de mobile.


## El movimiento del mouse mientras el cursor está bloqueado

`screenDelta` se comporta de forma distinta que las demás propiedades de `PrimaryPointerInfo` cuando el cursor está [bloqueado](/docs/creator/content-creator-es/scenes-sdk7/interactividad/eventos-de-button/click-events/#lock-or-unlock-the-cursor): `screenCoordinates` se congela en el centro de la pantalla y `worldRayDirection` siempre informa el rayo en el centro de la pantalla, pero `screenDelta` sigue informando el movimiento bruto del mouse en cada frame. Esto lo convierte en la única forma de leer el movimiento del mouse mientras el cursor está bloqueado, que es exactamente lo que necesitas para controles personalizados de cámara o de puntería.

## Arrastrar para rotar un objeto

El siguiente ejemplo permite al jugador hacer girar un cubo arrastrando el mouse lateralmente mientras mantiene presionado el botón del puntero:

```ts
import {
	engine,
	InputAction,
	inputSystem,
	MeshCollider,
	MeshRenderer,
	PrimaryPointerInfo,
	Transform,
} from '@dcl/sdk/ecs'
import { Quaternion, Vector3 } from '@dcl/sdk/math'

const DRAG_SENSITIVITY = 0.5

export function main() {
	const cube = engine.addEntity()
	Transform.create(cube, { position: Vector3.create(8, 1, 8) })
	MeshRenderer.setBox(cube)
	MeshCollider.setBox(cube)

	engine.addSystem(() => {
		// solo rota mientras el botón del puntero esté presionado
		if (!inputSystem.isPressed(InputAction.IA_POINTER)) return

		const delta = PrimaryPointerInfo.getOrNull(engine.RootEntity)?.screenDelta
		if (!delta || delta.x === 0) return

		const transform = Transform.getMutable(cube)
		transform.rotation = Quaternion.multiply(
			transform.rotation,
			Quaternion.fromEulerDegrees(0, delta.x * DRAG_SENSITIVITY, 0)
		)
	})
}
```

El mismo patrón funciona para cualquier interacción de arrastre: desliza un entity a lo largo de un riel usando `delta.x` como desplazamiento, recorre una animación, o detecta un deslizamiento rápido comprobando valores altos de delta.

## Controles de cámara con mouselook

Como `screenDelta` sigue funcionando mientras el cursor está bloqueado, puedes usarlo para crear controles de mouselook para una [virtual camera](/docs/creator/content-creator-es/scenes-sdk7/fundamentos-del-contenido-3d/camera/#using-virtual-cameras). Para hacer esto, acumula el movimiento del mouse en ángulos de yaw y pitch, y luego aplícalos a la rotación de la cámara en cada frame.

En el siguiente ejemplo, el jugador hace clic en una caja para cambiar a una virtual camera que puede dirigir con el mouse, y presiona el botón secundario (*F* o clic derecho) para volver a la cámara predeterminada. Mientras la virtual camera está activa, la escena también bloquea el cursor y congela el avatar con un [input modifier](/docs/creator/content-creator-es/scenes-sdk7/interactividad/avatares/locomotion/#freeze-the-player), para que el jugador no camine a ciegas mientras dirige la cámara.

```ts
import {
	engine,
	Entity,
	InputAction,
	InputModifier,
	inputSystem,
	MainCamera,
	MeshCollider,
	MeshRenderer,
	PointerEventType,
	pointerEventsSystem,
	PointerLock,
	PrimaryPointerInfo,
	Transform,
	VirtualCamera,
} from '@dcl/sdk/ecs'
import { Quaternion, Vector3 } from '@dcl/sdk/math'

// grados de rotación de la cámara por píxel de movimiento del mouse
const SENSITIVITY = 0.15

let cameraEntity: Entity
let cameraActive = false
let yaw = 0
let pitch = 0

export function main() {
	// el entity que sigue la cámara
	cameraEntity = engine.addEntity()
	Transform.create(cameraEntity, { position: Vector3.create(8, 3, 8) })
	VirtualCamera.create(cameraEntity, {
		defaultTransition: { transitionMode: VirtualCamera.Transition.Time(0.5) },
	})

	// haz clic en esta caja para entrar en el modo mouselook
	const box = engine.addEntity()
	Transform.create(box, { position: Vector3.create(8, 1, 4) })
	MeshRenderer.setBox(box)
	MeshCollider.setBox(box)
	pointerEventsSystem.onPointerDown(
		{
			entity: box,
			opts: { button: InputAction.IA_POINTER, hoverText: 'Controlar cámara' },
		},
		() => activateCamera(true)
	)

	// dirige la cámara con el mouse
	engine.addSystem(() => {
		if (!cameraActive) return
		if (!PointerLock.getOrNull(engine.CameraEntity)?.isPointerLocked) return

		const delta = PrimaryPointerInfo.getOrNull(engine.RootEntity)?.screenDelta
		if (!delta) return

		yaw += delta.x * SENSITIVITY
		// limita el pitch para que la cámara no pueda volcarse
		pitch = Math.max(-85, Math.min(85, pitch + delta.y * SENSITIVITY))

		Transform.getMutable(cameraEntity).rotation = Quaternion.fromEulerDegrees(pitch, yaw, 0)
	})

	// salir del modo mouselook con el botón secundario
	engine.addSystem(() => {
		if (!cameraActive) return
		if (inputSystem.isTriggered(InputAction.IA_SECONDARY, PointerEventType.PET_DOWN)) {
			activateCamera(false)
		}
	})
}

function activateCamera(active: boolean) {
	cameraActive = active

	// asigna o libera la virtual camera
	MainCamera.createOrReplace(engine.CameraEntity, {
		virtualCameraEntity: active ? cameraEntity : undefined,
	})

	// congela el avatar mientras se dirige la cámara
	InputModifier.createOrReplace(engine.PlayerEntity, {
		mode: InputModifier.Mode.Standard({ disableAll: active }),
	})

	// bloquea el cursor para que el mouse dirija la cámara de inmediato
	PointerLock.createOrReplace(engine.CameraEntity, { isPointerLocked: active })
}
```

Algunas cosas a tener en cuenta sobre este ejemplo:

* El system añade `delta.y` al pitch. Mover el mouse hacia arriba informa un `delta.y`negativo, lo que reduce el pitch e inclina la cámara hacia arriba. Invierte ese signo si prefieres controles verticales invertidos.
* El pitch se limita a un rango de -85 a 85 grados, así que la cámara nunca puede volcarse hacia atrás.
* El `SENSITIVITY` constante expresa grados de rotación por píxel de movimiento del mouse; ajústala a tu gusto.


> [!NOTE]
> **💡 Consejo**: El jugador puede desbloquear el cursor en cualquier momento pulsando *Esc* o haciendo clic derecho, lo que hace que la cámara deje de responder al mouse. Siempre ofrece a los jugadores una forma clara de salir por completo del modo de cámara, como el botón secundario en el ejemplo anterior. También puedes volver a bloquear el cursor desde código; consulta [Bloquear o desbloquear el cursor](/docs/creator/content-creator-es/scenes-sdk7/interactividad/eventos-de-button/click-events/#lock-or-unlock-the-cursor).


## Temas relacionados

Para leer la posición absoluta del cursor en la pantalla en lugar de su movimiento, usa la `screenCoordinates` propiedad del mismo componente. Consulta [Comprobar la posición del cursor del jugador](/docs/creator/content-creator-es/scenes-sdk7/interactividad/user-data/#check-the-players-cursor-position).

Para averiguar qué entity está debajo del cursor, combina la `worldRayDirection` propiedad con un raycast. Consulta [Raycasting](/docs/creator/content-creator-es/scenes-sdk7/interactividad/raycasting/).


> [!NOTE]
> **💡 Consejo**: Para ver ejemplos funcionales del estado del puntero, consulta la [`0,5-primary-cursor-info`](https://github.com/decentraland/sdk7-test-scenes/tree/main/scenes/0,5-primary-cursor-info) test scene, que lee `PrimaryPointerInfo` en cada frame y alimenta `worldRayDirection` en un raycast; [`31,20-pointer-lock-control`](https://github.com/decentraland/sdk7-test-scenes/tree/main/scenes/31,20-pointer-lock-control), que solicita y libera la captura del cursor escribiendo `PointerLock.isPointerLocked`; y [`32,20-virtual-camera-mouse-look`](https://github.com/decentraland/sdk7-test-scenes/tree/main/scenes/32,20-virtual-camera-mouse-look), que muestra `screenDelta` que sigue informando movimiento mientras `screenCoordinates` está congelado en el centro de la pantalla durante el bloqueo.



---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-es/scenes-sdk7/interactividad/mouse-movement.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
