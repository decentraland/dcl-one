> Aprende cómo manejar los clics de los usuarios en tu escena.

# Eventos de clic

Una escena de Decentraland puede detectar acciones de entrada de todos los botones que se usan para controlar el avatar del jugador. Estos incluyen clics del puntero, varios botones de acción y las teclas que se usan para mover al avatar. Los eventos de botones pueden venir de un mouse y teclado en el cliente de escritorio, de los controles en pantalla en el [app móvil](/docs/creator/content-creator-es/desarrollar-para-movil/cliente-movil/overview/), de un controlador VR o de otros dispositivos de entrada — el SDK interpreta todo esto de la misma manera.

Puedes detectar acciones de entrada contra una Entity. Esto implica presionar un botón mientras el cursor del jugador apunta al collider de esa Entity. También puedes detectar *global* evento de entrada global, que implica presionar o activar la entrada en cualquier momento, sin importar hacia dónde apunta el puntero.


> [!WARNING]
> **📔 Nota**: Las Entity deben tener un [collider](/docs/creator/content-creator-es/scenes-sdk7/fundamentos-del-contenido-3d/colliders/) para responder a acciones de entrada. `MeshRenderer` los modelos también deben recibir un `MeshCollider` componente. Los modelos de un `GltfContainer` pueden tener su propia geometría de colisión incorporada, o pueden configurarse para usar su geometría visible; también se les puede dar un `MeshCollider` componente.


Existen varias formas diferentes de manejar las acciones de entrada, según el caso de uso.

* [**Registrar un callback**](/docs/creator/content-creator-es/scenes-sdk7/interactividad/eventos-de-button/register-callback/): La forma más fácil de añadir interacción a una sola Entity. Escribe una sola instrucción para configurar una función callback y la retroalimentación al pasar el cursor.
* [**Basado en System**](/docs/creator/content-creator-es/scenes-sdk7/interactividad/eventos-de-button/system-based-events/): Ideal para manejar múltiples Entity con comportamiento similar. Usa un System para iterar sobre Entity similares y consultar las acciones de entrada en cada una, manejándolas todas con la misma lógica. La retroalimentación al pasar el cursor debe configurarse por separado. Este enfoque también se requiere para manejar acciones de entrada globales.
* [**Avanzado**](/docs/creator/content-creator-es/scenes-sdk7/interactividad/eventos-de-button/advanced-button-events/): Lee los datos de respuesta en bruto de cada Entity, incluidas las marcas de tiempo y un historial de eventos de entrada. Esto puede ser útil para definir patrones de interacción personalizados.

## Usa el Scene Editor en Creator Hub

La forma más fácil de manejar eventos de clic en una Entity es usar el Scene Editor. Usa el disparador sin código **On Click** o **On Input Action** Disparadores en un elemento para llamar acciones al hacer clic sobre él. O usa **On Global Click**, **On Global Primary** o **On Global Secondary** Disparadores para reaccionar a eventos de botones globales. Consulta [Haz que cualquier item sea smart](/docs/creator/content-creator-es/scene-editor/interactividad/make-any-item-smart/).

## Ejemplo simple

Para detectar clics en una Entity, usa `pointerEventsSystem.onPointerDown`.

```ts
pointerEventsSystem.onPointerDown(
	{
		entity: myEntity,
		opts: { button: InputAction.IA_PRIMARY, hoverText: 'Click' },
	},
	function () {
		console.log('clicked entity')
	}
)
```

Consulta [**Registrar un callback**](/docs/creator/content-creator-es/scenes-sdk7/interactividad/eventos-de-button/register-callback/) para más información.

## Retroalimentación al pasar el cursor

Es importante hacer que los jugadores sepan que una Entity es interactiva. De lo contrario, podrían perder por completo la experiencia que construiste. No es una buena experiencia hacer clic en todos los objetos esperando que alguno responda.

Cuando usas el [**Registrar un callback**](/docs/creator/content-creator-es/scenes-sdk7/interactividad/eventos-de-button/register-callback/) método, se muestran dos tipos de retroalimentación cada vez que el jugador pasa el cursor sobre el objeto:

* El borde de la Entity se resalta (solo en el cliente de escritorio de Decentraland 2.0). El resaltado es verde si la Entity está lo suficientemente cerca para hacer clic, rojo si la Entity está demasiado lejos.
* Aparece una pista de hover cerca del cursor con texto de UI, indicando lo que sucederá si hacen clic.

Al usar el [**Basado en System**](/docs/creator/content-creator-es/scenes-sdk7/interactividad/eventos-de-button/system-based-events/) método, puedes lograr los mismos resultados añadiendo un `PointerEvents` componente a las Entity clicables.

Tanto el resaltado de la Entity como la pista de hover pueden deshabilitarse mediante propiedades en estos métodos y componentes.

El resaltado del borde contornea la Entity sobre cuyo collider está el cursor, más todos los hijos de esa Entity. Nunca contornea la Entity padre ni sus hermanas. Solo las shapes de un `MeshRenderer` y los modelos de un `GltfContainer` reciben un contorno, `TextShape` el texto no. Si usas una caja invisible como un área clicable más grande, haz que las partes visibles sean hijos de esa Entity de caja; de lo contrario, no se contornea nada.

También podrías implementar [pistas avanzadas personalizadas](/docs/creator/content-creator-es/scenes-sdk7/interactividad/eventos-de-button/system-based-events/#advanced-custom-hints), por ejemplo podrías reproducir un sonido, hacer que la Entity cambie de color, gire o se agrande mientras se apunta a ella, etc. Hagas lo que hagas, asegúrate de que sea un signo claro.

## Obstáculos

Los eventos de botón emiten rayos que solo interactúan con la primera Entity en su trayectoria que está suscrita a la collision layer de pointer events. Esto es cierto siempre que la Entity esté dentro de sus límites de distancia. Si la Entity define límites de distancia tanto para la cámara como para el avatar, basta con que cualquiera de las dos esté lo suficientemente cerca.

Registra el pointer event en la misma Entity que tiene el collider. Un pointer event registrado en una Entity padre, o en una Entity hija que no tenga su propio collider, nunca se dispara.

Para que una Entity sea interceptada por el rayo de un pointer event, debe cumplirse una de estas condiciones:

* El modelo debe contener [collider meshes](/docs/creator/content-creator-es/modelado-y-animaciones-3d/colliders/).
* El `GltfContainer` deben configurarse para usar la [geometría visible con máscaras de colisión](/docs/creator/content-creator-es/scenes-sdk7/fundamentos-del-contenido-3d/colliders/#colliders-on-3d-models).
* La Entity debe tener un [componente MeshCollider](/docs/creator/content-creator-es/scenes-sdk7/fundamentos-del-contenido-3d/colliders/).

Si el collider de otra Entity está en el camino de la Entity con la que el jugador quiere interactuar, el jugador no podrá hacer clic en la Entity que está detrás, a menos que la Entity no tenga collider, o que este collider esté configurado para no responder a la collision layer de pointer events.

```ts
// Entity clicable
const clickableEntity = engine.addEntity()
MeshRenderer.setBox(clickableEntity)
MeshCollider.setBox(clickableEntity)
Transform.create(clickableEntity, { position: Vector3.create(8, 1, 8) })

pointerEventsSystem.onPointerDown(
	{
		entity: clickableEntity,
		opts: {
			button: InputAction.IA_POINTER,
			hoverText: 'Click',
		},
	},
	function () {
		console.log('clicked entity')
		const t = Transform.getMutable(clickableEntity)
		t.scale.y += 0.2
	}
)

// sin bloquear clics
const nonBlocker = engine.addEntity()
MeshRenderer.setBox(nonBlocker)
MeshCollider.setBox(nonBlocker, ColliderLayer.CL_PHYSICS)
Transform.create(nonBlocker, { position: Vector3.create(10, 1, 8) })

// bloqueador de clics
const blocker = engine.addEntity()
MeshRenderer.setBox(blocker)
MeshCollider.setBox(blocker, ColliderLayer.CL_POINTER)
Transform.create(blocker, { position: Vector3.create(8, 1, 10) })
```


> [!WARNING]
> **📔 Nota**: Para que una Entity no solo intercepte un pointer event, sino que también devuelva datos, la Entity también necesita tener un `PointerEvents` componente. El `pointerEventsSystem` helpers también se encargan de este requisito.


## Botones de puntero

Las siguientes entradas pueden manejarse mediante cualquiera de los enfoques para detectar eventos de entrada.

* `InputAction.IA_POINTER`: **botón izquierdo del mouse** en una computadora, **botón de interacción** en móvil.
* `InputAction.IA_PRIMARY`: **E** tecla en una computadora, **Botón E** en móvil.
* `InputAction.IA_SECONDARY`: **F** tecla en una computadora, **Botón F** en móvil.
* `InputAction.IA_ACTION_3`: **1** tecla en una computadora, **botón 1** en móvil. *Oculto por defecto en móvil (SDK 7.32.0+) — úsalo solo si tu escena lo muestra con `TouchScreenControls`.*
* `InputAction.IA_ACTION_4`: **2** tecla en una computadora, **botón 2** en móvil. *Oculto por defecto en móvil (SDK 7.32.0+) — úsalo solo si tu escena lo muestra con `TouchScreenControls`.*
* `InputAction.IA_ACTION_5`: **3** tecla en una computadora, **botón 3** en móvil. *Oculto por defecto en móvil (SDK 7.32.0+) — úsalo solo si tu escena lo muestra con `TouchScreenControls`.*
* `InputAction.IA_ACTION_6`: **4** tecla en una computadora, **botón 4** en móvil. *Oculto por defecto en móvil (SDK 7.32.0+) — úsalo solo si tu escena lo muestra con `TouchScreenControls`.*
* `InputAction.IA_JUMP`: **Space** tecla en una computadora, **botón de salto** en móvil.
* `InputAction.IA_FORWARD`: **W** tecla en una computadora.
* `InputAction.IA_LEFT`: **A** tecla en una computadora.
* `InputAction.IA_RIGHT`: **D** tecla en una computadora.
* `InputAction.IA_BACKWARD`: **S** tecla en una computadora.
* `InputAction.IA_WALK`: **Control** tecla en una computadora.
* `InputAction.IA_MODIFIER`: **Shift** tecla en una computadora.

Cada `InputAction` se abstrae de la entrada literal en el teclado para que pueda asignarse a diferentes entradas según el dispositivo. Por esta misma razón, no todos los botones del teclado pueden rastrearse para eventos de botón; solo los botones que se usan para movimiento e interacción. Esta limitación intencional garantiza que el contenido sea compatible entre escritorio, el [cliente móvil](/docs/creator/content-creator-es/desarrollar-para-movil/cliente-movil/overview/), y dispositivos futuros como controladores VR y otros mandos de juego.


> [!WARNING]
> **📱 Móvil**: En el [cliente móvil](/docs/creator/content-creator-es/desarrollar-para-movil/cliente-movil/overview/), cada acción de entrada puede dirigirse a un botón en pantalla, pero de forma predeterminada solo se muestran tres botones: `IA_POINTER` (botón de interacción), `IA_PRIMARY` (botón E) y `IA_JUMP`. `IA_SECONDARY` (F) y `IA_ACTION_3`–`IA_ACTION_6` (la `1`/`2`/`3`/`4` botones) comienzan ocultos (SDK 7.32.0 y posteriores; los SDK anteriores muestran los ocho, con los inferiores detrás de un desbordamiento "+"). Al diseñar para móvil, prefier `IA_POINTER`, `IA_PRIMARY` y `IA_JUMP` para tus acciones clave. Si necesitas F o un botón numerado, muéstralo con [Controles en pantalla](/docs/creator/content-creator-es/scenes-sdk7/interactividad/touch-screen-controls/). Consulta [Entrada en móvil](/docs/creator/content-creator-es/desarrollar-para-movil/desarrollar/input-on-mobile/).


## Tipos de eventos de puntero

Cada entrada puede producir los siguientes tipos de eventos de puntero. Cada uno de los siguientes es un valor en el `PointerEventType` enum.

* `PET_DOWN`: El jugador presiona un botón específico mientras tiene el cursor apuntando al collider de la Entity.
* `PET_UP`: El jugador suelta un botón específico mientras tiene el cursor apuntando al collider de la Entity.
* `PET_HOVER_ENTER`: El cursor del jugador comienza a apuntar al collider de la Entity.
* `PET_HOVER_LEAVE`: El cursor del jugador deja de apuntar al collider de la Entity.
* `PET_PROXIMITY_ENTER`: El jugador camina dentro del rango de proximidad de la Entity, sin importar hacia dónde esté mirando.
* `PET_PROXIMITY_LEAVE`: El jugador sale del rango de proximidad de la Entity.

Consulta [**Eventos de proximidad**](/docs/creator/content-creator-es/scenes-sdk7/interactividad/eventos-de-button/proximity-events/) para saber cómo usar interacciones basadas en proximidad.

## Datos de una acción de entrada

Todas las acciones de entrada incluyen datos sobre el evento, incluidas cosas como el botón que se activó y dónde apuntaba el puntero en ese momento.

La siguiente información se puede obtener de cualquier evento de entrada:

* `analógico`: Número opcional, solo presente en eventos de una entrada analógica (como un joystick), que almacena el valor analógico de la entrada.
* `botón`: Qué id de botón se presionó. El número corresponde al `InputAction` enum, que enumera todos los botones disponibles.
* `state`: Tipo de evento de puntero, del enum `PointerEventType`. *0* se refiere a `PointerEventType.PET_UP`, *1* para `PointerEventType.PET_DOWN`, *2* para `PointerEventType.PET_HOVER_ENTER`, *3* para `PointerEventType.PET_HOVER_LEAVE`, *4* para `PointerEventType.PET_PROXIMITY_ENTER`, *5* para `PointerEventType.PET_PROXIMITY_LEAVE`
* `timestamp`: Un [marca de tiempo Lamport](https://en.wikipedia.org/wiki/Lamport_timestamp) para identificar cada evento de botón.

  > Nota: Esta marca de tiempo no está numerada según la hora actual. Piénsala como un contador que comienza en 0 y se incrementa en 1 por cada evento.
* `hit`: Un objeto que contiene los siguientes datos sobre el evento hit:
  * `entityId`: Número de id de la Entity que fue alcanzada por el rayo.
  * `meshName`: *String* con el nombre interno del mesh específico en el modelo 3D que fue alcanzado. Esto es útil cuando un modelo 3D está compuesto por múltiples meshes.
  * `globalOrigin`: *Vector3* para la posición desde donde se origina el rayo (relativa a la escena)
  * `direction`: *Vector3* con el vector de dirección del rayo, en coordenadas globales
  * `posición`: *Vector3* para la posición donde el rayo intersectó con la Entity alcanzada (relativa a la escena)
  * `length`: Longitud del rayo desde su origen hasta la posición donde ocurrió el impacto contra la Entity.
  * `normalHit`: *Vector3* con un vector de dirección normalizado, que describe el ángulo de la normal del impacto en el espacio mundial.

Estos datos se acceden de distintas maneras según el enfoque que estés usando para manejar las acciones de entrada.

Usando el [**Registrar un callback**](/docs/creator/content-creator-es/scenes-sdk7/interactividad/eventos-de-button/register-callback/) enfoque, el primer parámetro que se pasa a la función callback contiene toda esta estructura de datos.

```ts
pointerEventsSystem.onPointerDown({ entity: myEntity }, function (cmd) {
	console.log(cmd.hit?.entityId)
})
```

Usando el [**Basado en System**](/docs/creator/content-creator-es/scenes-sdk7/interactividad/eventos-de-button/system-based-events/) enfoque, usa `inputSystem.getInputCommand()` para obtener estos datos.

```ts
engine.addSystem(() => {
	const cmd = inputSystem.getInputCommand(
		InputAction.IA_POINTER,
		PointerEventType.PET_DOWN,
		myEntity
	)
	if (cmd) {
		console.log(cmd.hit?.entityId)
	}
})
```


> [!WARNING]
> **📔 Nota**: Para que una Entity no solo intercepte un pointer event, sino que también devuelva datos, la Entity también necesita tener un `PointerEvents` componente. El `pointerEventsSystem` helpers también se encargan de este requisito.


Usando el [**Avanzado**](/docs/creator/content-creator-es/scenes-sdk7/interactividad/eventos-de-button/advanced-button-events/) enfoque, el `PointerEventsResult` contiene una lista con un historial reciente de todos los eventos de puntero contra esa Entity.

```ts
engine.addSystem(() => {
	for (const [entity] of engine.getEntitiesWith(PointerEventsResult)) {
		const pointerEvents = PointerEventsResult.get(entity)

		for (const event of pointerEvents) {
			console.log(event.hit?.entityId)
		}
	}
})
```

## Bloquear o desbloquear el cursor


> [!NOTE]
> **💡 Nota**: `PointerLock` es un concepto del cliente de escritorio (cursor del mouse bloqueado vs. desbloqueado). No tiene efecto en el [app móvil](/docs/creator/content-creator-es/desarrollar-para-movil/cliente-movil/overview/), donde la entrada se basa en el tacto.


Los jugadores en el cliente de escritorio de Decentraland pueden alternar entre dos modos de interacción distintos con su cursor:

* Cursor bloqueado: mover el mouse desplaza la cámara y los eventos de clic ocurren en la mira en el centro de la pantalla
* Cursor desbloqueado: el cursor es libre de moverse independientemente de la cámara, y los jugadores pueden hacer clic en cualquier lugar de la pantalla para interactuar ya sea con el espacio 3D o con la UI

Estos modos de interacción son ideales para distintos tipos de mecánicas de juego, y quizá quieras hacer que los jugadores adopten uno u otro según lo que mejor se adapte a tu escena. También puede convenirte asegurarte momentáneamente de que el cursor del jugador esté desbloqueado para permitirle interactuar con una UI; esto es especialmente útil para jugadores nuevos que no están familiarizados con cómo se controla esto.

Para cambiar el estado del cursor del jugador, usa el `PointerLock` componente en la `engine.CameraEntity` Entity.

```ts
import {PointerLock} from '@dcl/sdk/ecs'
    
PointerLock.createOrReplace(engine.CameraEntity, {isPointerLocked: false})
```


> [!WARNING]
> **📔 Nota**: El engine proporciona un `PointerLock` componente en la `engine.CameraEntity` de forma predeterminada. Usa `createOrReplace()` o `getMutable()` en lugar de `create()`, que arroja un error si el componente ya existe.


También puedes consultar el estado del cursor del jugador leyendo el `PointerLock` estado del componente.

```ts
import {PointerLock} from '@dcl/sdk/ecs'

const isPointerLocked = PointerLock.getOrNull(engine.CameraEntity)?.isPointerLocked
```

Usa la `.onChange` función para reaccionar a cambios en el estado del puntero. El siguiente ejemplo impone que el cursor esté siempre desbloqueado.

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


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-es/scenes-sdk7/interactividad/eventos-de-button/click-events.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
