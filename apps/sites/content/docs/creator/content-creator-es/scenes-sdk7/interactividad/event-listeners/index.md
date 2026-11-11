> Eventos que la escena puede rastrear, relacionados con las acciones del jugador y los cambios de la escena.

# Escuchadores de eventos

Hay varios eventos a los que la escena puede suscribirse para conocer las acciones del jugador mientras está en la escena o cerca de ella.

Para los eventos de botón y clic que realiza el jugador, consulta [Eventos de botones](/docs/creator/content-creator-es/scenes-sdk7/interactividad/eventos-de-button/click-events/).

## El jugador entra o sale de la escena

Cada vez que un avatar entra o sale de las parcelas de tierra que componen tu escena, o se teletransporta dentro o fuera, esto crea un evento al que puedes escuchar.

Este evento lo activan todos los avatares, incluido el del jugador.

```ts
import { onEnterScene, onLeaveScene } from '@dcl/sdk/src/players'

export function main() {
	onEnterScene((player) => {
		if (!player) return
		console.log('ENTERED SCENE', player)
	})

	onLeaveScene((userId) => {
		if (!userId) return
		console.log('LEFT SCENE', userId)
	})
}
```

En el `onEnterScene` evento, la función puede acceder a todos los datos devueltos por [obtener los datos del jugador](/docs/creator/content-creator-es/scenes-sdk7/interactividad/user-data/#get-player-data) a través de la `player` propiedad. En el `evento onLeaveScene` la función solo tiene acceso al ID del jugador.

### Solo el jugador actual

Puedes filtrar los eventos activados para reaccionar solo al avatar del jugador, en lugar de a otros avatares que puedan estar alrededor.

```ts
import { getPlayer, onEnterScene, onLeaveScene } from '@dcl/sdk/src/players'

export function main() {
	let myPlayer = getPlayer()

	onEnterScene((player) => {
		if (!player) return
		console.log('ENTERED SCENE', player)

		if (myPlayer && player.userId == myPlayer.userId) {
			console.log('HE ENTRADO')
		}
	})

	onLeaveScene((userId) => {
		if (!userId) return
		console.log('LEFT SCENE', userId)

		if (myPlayer && userId == myPlayer.userId) {
			console.log('HE SALIDO')
		}
	})
}
```

Este ejemplo obtiene primero el id del jugador, luego se suscribe a los eventos y compara el `userId` devuelto por el evento con el del jugador.

### Consultar todos los jugadores en la escena

Recorre la lista completa de jugadores que están actualmente en tu escena iterando sobre todas las entidades con un `PlayerIdentityData` componente.

```ts
import { engine, PlayerIdentityData, Transform } from '@dcl/sdk/ecs'

export function main() {
	for (const [entity, data, transform] of engine.getEntitiesWith(
		PlayerIdentityData,
		Transform
	)) {
		console.log('JUGADOR: ', { entity, data, transform })
	}
}
```

## El jugador cambia el modo de cámara

Conocer el modo de cámara puede ser muy útil para ajustar mejor la mecánica de tu escena a lo que resulta más cómodo al usar este modo. Por ejemplo, los objetivos pequeños son más difíciles de pulsar en tercera persona.

El siguiente fragmento usa la `onChange` función para disparar un evento cada vez que cambia la cámara. También dispara un evento cuando la escena carga, con el modo de cámara inicial del jugador.

```ts
import { engine, CameraMode } from '@dcl/sdk/ecs'

export function main() {
	CameraMode.onChange(engine.CameraEntity, (cameraComponent) => {
		if (!cameraComponent) return
		console.log('El modo de cámara cambió', cameraComponent?.mode)
		// 0 = primera persona
		// 1 = tercera persona
	})
}
```

Ver [Comprueba el modo de cámara del jugador](/docs/creator/content-creator-es/scenes-sdk7/interactividad/user-data/#check-the-players-camera-mode).

## El jugador reproduce una animación

Usa la `onChange` función en el `AvatarEmoteCommand` componente para disparar un evento cada vez que el jugador reproduce un emote. Esto incluye tanto los emotes base (dance, clap, wave, etc.) como los emotes de tokens.

```ts
import { AvatarEmoteCommand } from '@dcl/sdk/ecs'

export function main() {
	AvatarEmoteCommand.onChange(engine.PlayerEntity, (emote) => {
		if (!emote) return
		console.log('Emote reproducido: ', emote.emoteUrn)
	})
}
```

El evento incluye la siguiente información:

* `emoteUrn`: Nombre del emote realizado (es decir: *wave*, *clap*, *kiss*)
* `loop`: Si el emote se reproduce en bucle o una sola vez
* `timestamp`: Cuándo se activó el emote.
* `state`: Un valor del `EmoteState` enum que describe el evento del ciclo de vida: `ES_STARTED` (el emote comenzó, y también es el valor cuando el campo está ausente), `ES_FINISHED` (un emote no en bucle se reprodujo hasta el final), o `ES_INTERRUPTED` (el emote se interrumpió por movimiento, teletransporte, otro emote, una detención explícita o al salir de la escena). Ver [Detectar cuándo termina un emote](/docs/creator/content-creator-es/scenes-sdk7/interactividad/avatares/avatar-animations/#detect-when-an-emote-finishes).

También puedes detectar emotes de otros jugadores en la escena pasando una referencia al otro jugador en lugar de `engine.PlayerEntity`.


> [!NOTE]
> **💡 Consejo**: Para ver un ejemplo funcional de eventos de reproducción de emotes, consulta la [`4,23-emote-finish`](https://github.com/decentraland/sdk7-test-scenes/tree/main/scenes/4,23-emote-finish) test scene, que registra cada `AvatarEmoteCommand` entrada añadida a la entidad del jugador como `STARTED`, `FINISHED` o `INTERRUPTED` — reproduce un emote completo para ver `FINISHED`, o aléjate a mitad de la reproducción para ver `INTERRUPTED`.


## El jugador cambia de perfil

Usa la `onChange` función en el `AvatarEquippedData` componente para disparar un evento cada vez que el jugador cambia uno de sus wearables, o sus emotes listados en la rueda de acceso rápido. De forma similar, usa la `onChange` función en el `AvatarBase` para disparar un evento cada vez que el jugador cambia sus propiedades base del avatar, como el color de cabello, el color de piel, la forma del avatar o el nombre.

```ts
import { AvatarEquippedData, AvatarBase } from '@dcl/sdk/ecs'

export function main() {
	AvatarEquippedData.onChange(engine.PlayerEntity, (equipped) => {
		if (!equipped) return
		console.log('Nueva lista de wearables: ', equipped.wearableUrns)
		console.log('Nueva lista de emotes: ', equipped.emoteUrns)
	})

	AvatarBase.onChange(engine.PlayerEntity, (body) => {
		if (!body) return
		console.log('Nuevo color de ojos: ', body.eyesColor)
		console.log('Nuevo color de piel: ', body.skinColor)
		console.log('Nuevo color de cabello: ', body.hairColor)
		console.log('Nuevo URN del cuerpo: ', body.bodyShapeUrn)
	})
}
```

El evento en `AvatarEquippedData` incluye la siguiente información:

* `wearableUrns`: La lista de wearables que el jugador tiene equipados actualmente.
* `emoteUrns`: La lista de emotes que el jugador tiene equipados actualmente en la rueda de acceso rápido.

El evento en `AvatarBase` incluye la siguiente información:

* `name`: El nombre del jugador.
* `bodyShapeUrn`: Los ids correspondientes al tipo de cuerpo masculino o femenino.
* `skinColor`: Color de piel del jugador como un `Color3`
* `eyesColor`: Color de ojos del jugador como un `Color3`
* `hairColor`: Color de cabello del jugador como un `Color3`
* `talking`: *true* mientras el jugador habla en el chat de voz, *false* cuando está en silencio, `undefined` cuando el cliente no está informando actividad de voz.

Porque `talking` vive en `AvatarBase`, este callback también se dispara cada vez que el jugador empieza o deja de hablar. Si solo te importa el aspecto, compara los campos que necesites en lugar de reaccionar a cada cambio. Ver [Detectar cuándo un jugador está hablando](/docs/creator/content-creator-es/scenes-sdk7/interactividad/user-data/#detect-when-a-player-is-speaking).

También puedes detectar cambios en wearables o avatares de otros jugadores en la escena, simplemente pasa una referencia al otro jugador en lugar de `engine.PlayerEntity`.

También puedes detectar cambios en los perfiles de otros jugadores en la escena, simplemente pasa una referencia al otro jugador en lugar de `engine.PlayerEntity`.

## El jugador bloquea o desbloquea el cursor

Los jugadores pueden alternar entre dos modos de cursor: *cursor bloqueado* para controlar la cámara o *cursor desbloqueado* para mover el cursor libremente por la UI.

Los jugadores desbloquean el cursor haciendo clic en el *botón derecho del ratón* o presionando la tecla *Esc* , y vuelven a bloquear el cursor haciendo clic en cualquier parte de la pantalla.

Añade un `PointerLock` componente a la `engine.CameraEntity` entidad de tu escena, y usa el `onChange` función en el `PointerLock` componente para disparar un evento cada vez que el jugador cambia entre los dos modos de cursor.

```ts
import {PointerLock} from '@dcl/sdk/ecs'

export function main() {
    PointerLock.onChange(engine.CameraEntity, (pointerLock) => {
		    if (!pointerLock) return
		    if(pointerLock.isPointerLocked){
			    // Muestra una sugerencia sobre el modo del cursor
		   }
	})
}
```

Puedes usar esta información para orientar sutilmente al jugador, por ejemplo mostrando un popup de UI que diga que este juego se disfruta mejor con el cursor desbloqueado. O también puedes forzar el modo de cursor del jugador cambiando la `isPointerLocked` en el componente. El siguiente ejemplo siempre establece el modo de cursor en desbloqueado:

```ts
import {PointerLock} from '@dcl/sdk/ecs'

export function main() {
    PointerLock.createOrReplace(engine.CameraEntity, {isPointerLocked: false});
    PointerLock.onChange(engine.CameraEntity, (pointerLock) => {
		    if (!pointerLock) return
		    if(pointerLock.isPointerLocked){
			    PointerLock.getMutable(engine.CameraEntity).isPointerLocked = false
		   }
	})
}
```

## Abrir paneles del Explorer

Tu escena puede abrir los paneles integrados del Explorer (mapa, configuración, mochila, etc.) usando la `openExplorerUi` acción restringida. La mayoría ocupa toda la pantalla, pero no todos lo hacen. Esto es útil para dirigir a los jugadores a funciones específicas del Explorer desde la UI de tu escena.

El jugador tiene que pedirlo. Llama a `openExplorerUi` desde dentro de un manejador de clic, no por sí sola:

```ts
// Abre el mapa cuando el jugador hace clic en un cubo
import { openExplorerUi } from '~system/RestrictedActions'
import { engine, ExplorerUi, InputAction, MeshCollider, MeshRenderer, pointerEventsSystem, Transform } from '@dcl/sdk/ecs'
import { Vector3 } from '@dcl/sdk/math'

const mapButton = engine.addEntity()
Transform.create(mapButton, { position: Vector3.create(8, 1, 8) })
MeshRenderer.setBox(mapButton)
MeshCollider.setBox(mapButton)

pointerEventsSystem.onPointerDown(
	{ entity: mapButton, opts: { button: InputAction.IA_POINTER, hoverText: 'Open map' } },
	() => {
		void openExplorerUi({ ui: ExplorerUi.EU_MAP })
	}
)
```

El `ExplorerUi` enum tiene los siguientes valores:

* `EU_SETTINGS` (0)
* `EU_MAP` (1)
* `EU_BACKPACK` (2)
* `EU_CAMERA_REEL` (3)
* `EU_COMMUNITIES` (4)
* `EU_PLACES` (5)
* `EU_EVENTS` (6)
* `EU_ITEM_PURCHASE` (7): el modal de confirmación de compra del ítem. El Explorer actualmente responde `REJECTED_FEATURE_DISABLED` para ello.


> [!WARNING]
> **Nota:** Como otras acciones restringidas (`movePlayerTo`, `triggerEmote`), `openExplorerUi` solo funciona mientras el jugador está de pie dentro de las parcelas de la escena.


### Solo una acción del jugador puede abrir un panel

Un panel solo se abre cuando la llamada es una respuesta directa a algo que el jugador acaba de hacer. Una llamada que no lo sea se rechaza.

**Funciona:**

* Hacer clic en una entidad 3D, con `pointerEventsSystem.onPointerDown` o `pointerEventsSystem.onPointerUp`.
* Hacer clic en un elemento de la UI de tu escena, con `onMouseDown`.
* Pulsar o soltar uno de los botones de activación, `E`, `F` o `1` hasta `4`, detectado con `inputSystem.isTriggered` y `PointerEventType.PET_DOWN` o `PointerEventType.PET_UP` (`IA_PRIMARY`, `IA_SECONDARY`, `IA_ACTION_3` hasta `IA_ACTION_6`).

**No funciona:**

* Llamar desde `main()`, así que un panel no puede saludar al jugador al cargar.
* Llamar desde un temporizador, o después de que la acción del jugador ya haya pasado.
* Llamar después de un `await` en algo que tarda, como un `fetch` o otra acción restringida. La acción del jugador solo cuenta durante aproximadamente un fotograma. Haz primero el trabajo y luego abre el panel.
* Pasar el cursor. `PET_HOVER_ENTER` y `PET_HOVER_LEAVE` no son acciones del jugador.
* Teclas de movimiento. Caminar, correr y saltar no cuentan; solo los botones de activación mencionados arriba.

`openExplorerUi` se resuelve con un `openResult` que te dice qué pasó:

| Valor                            | Significado                                                                                                               |
| -------------------------------- | ------------------------------------------------------------------------------------------------------------------------- |
| `UNSPECIFIED` (0)                | No se informó ningún resultado.                                                                                           |
| `OPENED` (1)                     | El panel se abrió.                                                                                                        |
| `WAS_ALREADY_OPEN` (2)           | Ya había otro panel en pantalla. Los paneles comparten una sola ranura, por lo que solo uno puede estar abierto a la vez. |
| `REJECTED_NOT_CURRENT_SCENE` (3) | El jugador no está de pie en tu escena.                                                                                   |
| `REJECTED_FEATURE_DISABLED` (4)  | El Explorer tiene ese panel desactivado, o el `ui` valor no es un panel que conozca.                                      |
| `REJECTED_NO_USER_GESTURE` (5)   | La llamada no fue una respuesta a una acción del jugador.                                                                 |


> [!WARNING]
> **Nota:** Si no se abre nada y no ves ningún error, comprueba `openResult`. `REJECTED_NO_USER_GESTURE` (5) significa que tu llamada se ejecutó fuera de una acción del jugador, normalmente porque está en `main()`, en un temporizador, o después de un `await`.


`WAS_ALREADY_OPEN` no solo se refiere a que tu escena llame dos veces. Todos los paneles ocupan la misma ranura, así que cualquiera que esté en pantalla rechaza a cualquier otro, y el jugador puede haber abierto ese por su cuenta un momento antes, sin intervención de tu escena. Trátalo como un resultado normal, no como un error en tu código. Con `openExplorerUiAndWait()`, descrita más abajo, la misma situación llega como un resultado `notOpened` .

### Espera hasta que el jugador cierre un panel

`openExplorerUiAndWait()` abre un panel y te entrega una Promise que se resuelve una vez que el jugador lo cierra. Úsala cuando tu escena deba continuar solo después de que el jugador termine, por ejemplo después de enviarlo a la mochila para ponerse un wearable.

```ts
import { openExplorerUiAndWait, ExplorerUi } from '@dcl/sdk/explorer-ui'

// Abre la mochila y luego continúa una vez que el jugador la cierre
async function openBackpack() {
	try {
		const outcome = await openExplorerUiAndWait({ ui: ExplorerUi.EU_BACKPACK })

		if (outcome.$case === 'closed') {
			console.log('El jugador cerró la mochila')
		}
	} catch (error) {
		console.log('La espera falló:', error)
	}
}
```

Llama a `openBackpack()` desde un manejador de clic, igual que `openExplorerUi`. `await` La espera ocurre dentro de la función, después de solicitar el panel, por lo que la acción del jugador sigue contando.

El objeto que recibes de vuelta tiene un `$case` campo que te dice cómo terminó la espera:

* `closed`: el jugador cerró el panel que abrió tu escena. Este es el resultado normal.
* `notOpened`: no se abrió nada. El `openResult` campo dice por qué, usando los valores de la tabla anterior.
* `timedOut`: se agotó el límite de tiempo que estableciste mientras el panel seguía abierto.
* `matched`: una `until` : condición terminó la espera antes de tiempo, así que el panel puede seguir en pantalla.

Cuando el panel no puede abrirse, la Promise se resuelve inmediatamente con `notOpened`. Tu escena nunca se queda esperando un panel que no está ahí.

Esos cuatro resultados cubren todos los finales ordinarios, pero la promise aún puede rechazar: la request al Explorer puede fallar por completo, dos channels pueden registrarse bajo el mismo nombre, o puede llegar un event que el helper no pueda atribuir a ninguna de las calls en curso. Por eso el ejemplo de arriba está envuelto en un `try`/`catch`. Un rechazo dentro de un `async` function llamada desde un handler de pointer no se informa en absoluto, así que sin el `catch` la espera simplemente nunca regresa y nada te dice por qué.


> [!WARNING]
> **📔 Nota**: `openExplorerUiAndWait()` requiere `@dcl/sdk` versión 7.30.0 o más reciente. Proviene del `@dcl/sdk/explorer-ui` módulo, que además reexporta `ExplorerUi`.


#### Establecer un límite de tiempo

No hay límite de tiempo por defecto. Un jugador puede dejar un panel abierto durante varios minutos, y la promise sigue pendiente todo ese tiempo. Pasa `timeoutMs` cuando tu escena necesite seguir adelante de todos modos.

```ts
import { openExplorerUiAndWait, ExplorerUi } from '@dcl/sdk/explorer-ui'

// Deja de esperar después de 30 segundos, esté o no abierto el mapa
async function openMap() {
	const outcome = await openExplorerUiAndWait({ ui: ExplorerUi.EU_MAP }, { timeoutMs: 30000 })

	if (outcome.$case === 'timedOut') {
		console.log('El jugador sigue mirando el mapa')
	}
}
```

#### Detente en un event en su lugar

El `until` opción termina la espera en cuanto llega un event en un channel, resolviendo con `matched` en lugar de `closed`. Los channels se exportan desde el mismo módulo: `ExplorerUiEvents` para los eventos de apertura y cierre de paneles, y `ItemPurchase` para el resultado de un flujo de compra. Añade el mismo channel a la opción `collect` si también quieres los events en sí, que llegan en el `events` array.

Pasa el channel por sí solo para detenerte en cualquiera de sus events, o envuélvelo en `variant()` para detenerte solo en un tipo de event:

```ts
import { ExplorerUi, ExplorerUiEvents, openExplorerUiAndWait, variant } from '@dcl/sdk/explorer-ui'

// Continúa en cuanto la mochila aparece en pantalla, sin esperar a que el jugador la cierre
async function openBackpack() {
	const outcome = await openExplorerUiAndWait(
		{ ui: ExplorerUi.EU_BACKPACK },
		{ until: variant(ExplorerUiEvents, 'opened'), collect: [ExplorerUiEvents] }
	)

	if (outcome.$case === 'matched') {
		console.log('La mochila está abierta, se recopilaron', outcome.events.length, 'events')
	}
}
```

El `$case` valores `variant()` Los values aceptados son los que llevan los events del channel: `opened` y `closed` en `ExplorerUiEvents`.

### Observa cuándo se abren o cierran paneles

Usa la `ExplorerUiEventsResult` componente para seguir los paneles que tu propia escena abrió. Este es un componente de solo crecimiento en `engine.RootEntity`, similar a `PointerEventsResult`. Cada panel que tu escena abre con `openExplorerUi` añade un event cuando aparece, y otro cuando desaparece, incluso cuando el jugador lo descarta con *Esc*.


> [!WARNING]
> **Nota:** Esta no es una vista de toda la UI del Explorer. Un panel que el jugador abre manualmente, desde los propios menús del Explorer, no produce ningún event aquí. Una escena que nunca llama `openExplorerUi` nunca ve ni uno solo.


`get()` te entrega todo el conjunto de events acumulados hasta ahora, hasta los 100 más recientes, no los que llegaron desde el último frame. Un system que lo itera sin miramientos reacciona a cada event pasado en cada tick. Compara la timestamp de cada event `timestamp` con el último tick que manejaste:

```ts
import { engine, ExplorerUiEventsResult, ExplorerUi } from '@dcl/sdk/ecs'

let lastHandledTick = -1

engine.addSystem(() => {
	let newestTick = lastHandledTick

	for (const event of ExplorerUiEventsResult.get(engine.RootEntity)) {
		// El set conserva cada event, así que omite los ticks ya manejados
		if (event.timestamp <= lastHandledTick) continue
		if (event.timestamp > newestTick) newestTick = event.timestamp

		if (event.event?.$case === 'opened') {
			console.log('Panel opened:', event.ui)
		}
		if (event.event?.$case === 'closed') {
			console.log('Panel closed:', event.ui)
		}

		// Comprueba cuál panel específico
		if (event.ui === ExplorerUi.EU_MAP && event.event?.$case === 'closed') {
			console.log('El jugador cerró el mapa')
		}
	}

	// Solo después del loop, para que todos los events del mismo tick se manejen
	lastHandledTick = newestTick
})
```

`ExplorerUiEventsResult.onChange()` en `engine.RootEntity` es la otra forma de leer el stream: el callback se ejecuta una vez por cada nuevo event, así que no hay que llevar ninguna cuenta.

Cada event contiene:

* `ui`: Qué panel trata el event (un `ExplorerUi` valor).
* `timestamp`: El tick de la escena en el que ocurrió el event.
* `requestId`: El id que tu escena pasó en la `openExplorerUi` call que abrió este panel, o `0` si la call no pasó ninguno. Úsalo para distinguir varias calls en curso.
* `event`: Ya sea `{ $case: 'opened' }` o `{ $case: 'closed' }`.


> [!NOTE]
> **💡 Consejo**: Para un ejemplo funcional de todo lo anterior, consulta la [`80,-6-explorer-ui-events`](https://github.com/decentraland/sdk7-test-scenes/tree/main/scenes/80,-6-explorer-ui-events) test scene. Imprime cada veredicto y cada event en su propio HUD, y cubre la barrera de gesto, un panel ya abierto, `requestId` correlación y cada resultado de `openExplorerUiAndWait()`.



---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-es/scenes-sdk7/interactividad/event-listeners.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
