> Teletransporta al jugador, o haz que camine hasta una nueva posición dentro de tu escena

# Mover al jugador

Para cambiar la posición del jugador en la escena, usa la `movePlayerTo()` función. Úsala para volver a hacer aparecer a los jugadores, configurar puntos de control o hacer que un jugador camine hasta un lugar como parte de una cinemática.


> [!NOTE]
> **💡 Consejo**: La forma más fácil de mover al jugador es usar el [Scene Editor in Creator Hub](/docs/creator/content-creator-es/scene-editor/primeros-pasos/about-editor/). Usa la solución sin código **Mover jugador** o los **Mover jugador aquí** Actions, consulta [Haz que cualquier item sea smart](/docs/creator/content-creator-es/scene-editor/interactividad/make-any-item-smart/).


Esta función toma un objeto con las siguientes propiedades:

* `newRelativePosition`: Dónde posicionar al jugador, expresado como un Vector3.
* `cameraTarget`: (opcional) La dirección hacia la que hacer que la cámara mire, expresada como un Vector3 que representa las coordenadas de un punto en el espacio al que mirar. Si no se proporciona ningún valor, la cámara mantendrá la misma rotación que antes de moverse.
* `avatarTarget`: (opcional) La dirección hacia la que hacer que el avatar mire, expresada como un Vector3 que representa las coordenadas de un punto en el espacio al que mirar. Si no se proporciona ningún valor, el avatar mantendrá la misma rotación que antes de moverse. Si el jugador está en modo de cámara en 1.ª persona, la rotación de la cámara y del avatar es la misma.
* `duración`: (opcional) Cuánto tiempo debe durar la transición, en segundos. Si no se proporciona ningún valor, la transición ocurrirá al instante. Si se proporciona una duración, el avatar caminará o correrá hasta esta nueva posición.


> [!WARNING]
> **📔 Nota**: Durante la transición, el avatar no se ve afectado por los colliders, por lo que puede atravesar objetos.


```ts
import { movePlayerTo } from '~system/RestrictedActions'

// crear entidad
const myEntity = engine.addEntity()
MeshRenderer.setBox(myEntity)
MeshCollider.setBox(myEntity)

Transform.create(myEntity, {
	position: { x: 4, y: 1, z: 4 },
})

// darle comportamiento a la entidad
pointerEventsSystem.onPointerDown(
	{
		entity: myEntity,
		opts: { button: InputAction.IA_POINTER, hoverText: 'Haz clic' },
	},
	function () {
		// volver a hacer aparecer al jugador
		movePlayerTo({
			newRelativePosition: Vector3.create(1, 0, 1),
			cameraTarget: Vector3.create(8, 1, 8),
			avatarTarget: Vector3.create(8, 1, 8),
		})
	}
)
```

El movimiento del jugador ocurre al instante, sin pantallas de confirmación ni transiciones de cámara.


> [!WARNING]
> **📔 Nota**: Los jugadores solo se pueden mover si ya están dentro de los límites de la escena, y solo se los puede mover a ubicaciones que estén dentro de los límites de la escena. No puedes usar `movePlayerTo()` para transportar a un jugador a otra escena. Para mover a un jugador a otra escena, consulta [Teletransportes](/docs/creator/content-creator-es/scenes-sdk7/interactividad/external-links/#teleports).


El `movePlayerTo()` función se puede esperar, por lo que si el movimiento tiene una duración, puedes usar `await` para esperar a que el jugador llegue a su destino.

```ts
import { movePlayerTo } from '~system/RestrictedActions'

pointerEventsSystem.onPointerDown(
	{
		entity: myEntity,
		opts: { button: InputAction.IA_POINTER, hoverText: 'Haz clic' },
	},
	async function () {
		await movePlayerTo({
			newRelativePosition: Vector3.create(1, 0, 1),
			cameraTarget: Vector3.create(8, 1, 8),
			avatarTarget: Vector3.create(8, 1, 8),
			duration: 2,
		})
	}
)
```


> [!WARNING]
> **📔 Nota**: Sin un `duración`, la promesa se resuelve en cuanto el teletransporte se pone en cola, no cuando el jugador ha llegado. El código después de la `await` se ejecuta mientras `Transform.get(engine.PlayerEntity)` todavía informa la posición anterior, y el movimiento que se completa un momento después puede interrumpir un emote que iniciaste allí. Si algo tiene que ocurrir al llegar, en su lugar consulta periódicamente la posición del jugador:
> 
> ```ts
> import { engine, Transform } from '@dcl/sdk/ecs'
> import { Vector3 } from '@dcl/sdk/math'
> import { movePlayerTo } from '~system/RestrictedActions'
> 
> function teleportThen(target: Vector3, onArrive: () => void) {
> 	void movePlayerTo({ newRelativePosition: target })
> 
> 	let waited = 0
> 	function waitForArrival(dt: number) {
> 		waited += dt
> 		const pos = Transform.getOrNull(engine.PlayerEntity)?.position
> 		// comparar solo en X y Z, para que una diferencia de altura no bloquee la espera
> 		const arrived = pos !== undefined && Math.hypot(pos.x - target.x, pos.z - target.z) < 0.2
> 		if (!arrived && waited < 1.5) return // abandonar después de 1.5 s
> 		engine.removeSystem(waitForArrival)
> 		onArrive()
> 	}
> 
> 	engine.addSystem(waitForArrival)
> }
> ```
> 


Si el jugador intenta moverse durante la duración de su transición, la transición se interrumpirá y la `movePlayerTo` función nunca se resolverá. Puedes evitar este comportamiento deshabilitando todas las entradas mientras la transición esté en curso usando el `InputModifier` componente, consulta [Restringir locomoción](/docs/creator/content-creator-es/scenes-sdk7/interactividad/avatares/locomotion/#restrict-locomotion).

```ts
import { movePlayerTo } from '~system/RestrictedActions'
import {InputModifier, engine} from '@dcl/sdk/ecs'

pointerEventsSystem.onPointerDown(
	{
		entity: myEntity,
		opts: { button: InputAction.IA_POINTER, hoverText: 'Haz clic' },
	},
	async function () {

		// deshabilitar todas las entradas
		InputModifier.create(engine.PlayerEntity, {
			mode: InputModifier.Mode.Standard({
				disableAll: true,
			}),
		})

		// mover al jugador
		await movePlayerTo({
			newRelativePosition: Vector3.create(1, 0, 1),
			cameraTarget: Vector3.create(8, 1, 8),
			avatarTarget: Vector3.create(8, 1, 8),
			duration: 2,
		})

		// habilitar todas las entradas
		InputModifier.deleteFrom(engine.PlayerEntity)
	}
)
```


> [!WARNING]
> **📔 Nota**: Además, el resultado del await se puede almacenar para evaluar su `éxito` propiedad y saber si el movimiento fue interrumpido (por ejemplo, por un movimiento de entrada) o completado con éxito.



> [!NOTE]
> **💡 Consejo**: Para mover a un jugador a una escena diferente, o a diferentes coordenadas en el mapa de Genesis City, consulta [Teletransportes](/docs/creator/content-creator-es/scenes-sdk7/interactividad/external-links/#teleports).



---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-es/scenes-sdk7/interactividad/avatares/move-player.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
