> Aprende cómo manejar los clics de los usuarios en tu escena.

# Eventos basados en Systems

Si tu scene tiene múltiples entities similares que se activan todas usando la misma lógica, puedes escribir un único system para iterar sobre todas ellas y describir ese comportamiento solo una vez. Esta también es la opción más eficiente y más [orientado a datos](/docs/creator/content-creator-es/scenes-sdk7/arquitectura/data-oriented-programming/) enfoque.

También puedes usar un system para detectar [eventos globales de entrada](#global-input-events), de modo que la scene reaccione cada vez que se pulsa una tecla, sin importar hacia dónde apunta el cursor del jugador.

Si lo único que quieres hacer es hacer clic o pulsar un botón en una sola entity para activarla, la forma más sencilla es usar la [Registrar un callback](/docs/creator/content-creator-es/scenes-sdk7/interactividad/eventos-de-button/register-callback/) enfoque.

Para definir una lógica personalizada más específica, quizás quieras trabajar con los datos sin procesar y usar la [Avanzado](/docs/creator/content-creator-es/scenes-sdk7/interactividad/eventos-de-button/advanced-button-events/) enfoque.

Para que una entity sea interactiva, debe tener un [collider](/docs/creator/content-creator-es/scenes-sdk7/fundamentos-del-contenido-3d/colliders/). Consulta [obstáculos](/docs/creator/content-creator-es/scenes-sdk7/interactividad/eventos-de-button/click-events/#obstacles) para más detalles. Esto también se aplica a los eventos de clic por proximidad.

## Usar un system

Comprueba los eventos de botón ejecutando una de las funciones auxiliares en el namespace de entrada `inputSystem` en cada tick dentro de un [system](/docs/creator/content-creator-es/scenes-sdk7/arquitectura/systems/).

Por ejemplo, el siguiente system usa la `inputSystem.isTriggered()` función para comprobar si se hizo clic en el pointer. En cada tick, comprueba si se pulsó el botón. Si `inputSystem.isTriggered()` devuelve *true*, el system ejecuta alguna lógica personalizada en respuesta.

```ts
engine.addSystem(() => {
	if (inputSystem.isTriggered(InputAction.IA_POINTER, PointerEventType.PET_DOWN)) {
		// Lógica en respuesta a la pulsación del botón
	}
})
```

Las siguientes funciones auxiliares están disponibles en el `inputSystem` namespace, y pueden llamarse de manera similar en cada tick:

* `inputSystem.isTriggered`: Devuelve true si ocurrió una acción de entrada desde el último tick.
* `inputSystem.isPressed`: Devuelve true si actualmente se está manteniendo presionada una entrada. Devolverá true en cada tick hasta que el botón se suelte de nuevo.
* `inputSystem.getInputCommand`: Devuelve un objeto con datos sobre la acción de entrada.

Consulta las secciones de abajo para más detalles sobre cada una.

Cuando manejes eventos de botones en una entity, dale siempre feedback al jugador, para que sepa que se puede interactuar con una entity. Si añades un `PointerEvents` component a una entity, los jugadores verán una sugerencia al pasar el cursor sobre esa entity. Consulta [Mostrar feedback](#show-feedback) para aprender cómo puedes añadir sugerencias al pasar el cursor en entities interactivas.

### Eventos globales de entrada

Usa `inputSystem.isTriggered` para detectar eventos de pulsación y liberación de botón en cualquiera de las entradas rastreadas por el SDK.

```ts
engine.addSystem(() => {
	if (
		inputSystem.isTriggered(InputAction.IA_POINTER, PointerEventType.PET_DOWN)
	) {
		// Lógica en respuesta a la pulsación del botón
	}
})
```

El ejemplo anterior comprueba si se pulsó el botón, sin importar dónde está el puntero ni qué entities puedan estar en su trayectoria.

`inputSystem.isTriggered()` devuelve *true* solo si el botón indicado se pulsó en el tick actual del game loop. Si el botón no se pulsó, o ya estaba pulsado desde el tick anterior, devuelve *false*.


> [!WARNING]
> **📔 Nota**: El jugador necesita estar dentro de los límites de la scene para que se detecte el evento del puntero. El cursor del jugador también necesita estar bloqueado; las pulsaciones hechas con el cursor libre no se detectan.
> 
> Toda entity con la que quieras interactuar debe tener tanto un [`PointerEvents` componente](#show-feedback)  y una [collider](/docs/creator/content-creator-es/scenes-sdk7/fundamentos-del-contenido-3d/colliders/).


La sección `inputSystem.isTriggered` función toma los siguientes argumentos obligatorios:

* `InputAction`: Qué entrada escuchar, como un valor del `InputAction` enum. Consulta [Botones del puntero](/docs/creator/content-creator-es/scenes-sdk7/interactividad/eventos-de-button/click-events/#pointer-buttons) para ver las opciones compatibles.
* `PointerEventType`: Qué tipo de evento escuchar, como un valor del `PointerEventType` enum. Consulta [Tipos de eventos del puntero](/docs/creator/content-creator-es/scenes-sdk7/interactividad/eventos-de-button/click-events/#types-of-pointer-events) para ver las opciones compatibles.

### Activar una entity

Para detectar eventos de botón mientras apuntas a una entity concreta, pasa un tercer argumento opcional a `inputSystem.isTriggered` para especificar contra qué entity comprobar.

```ts
// Dale a la entity un component PointerEvents
PointerEvents.create(myEntity, {
	pointerEvents: [
		{
			eventType: PointerEventType.PET_DOWN,
			eventInfo: {
				button: InputAction.IA_PRIMARY,
				showFeedback: false,
			},
		},
	],
})

// create system
engine.addSystem(() => {
	if (
		inputSystem.isTriggered(
			InputAction.IA_POINTER,
			PointerEventType.PET_DOWN,
			myEntity
		)
	) {
		// Lógica en respuesta a pulsar el botón sobre myEntity
	}
})
```

El ejemplo anterior comprueba en cada tick si se pulsó una sola entity codificada de forma fija con el botón del puntero (botón izquierdo del ratón).

La sección `inputSystem.isTriggered` función toma los siguientes argumentos:

* `InputAction`: Qué entrada escuchar, como un valor del `InputAction` enum. Consulta [Botones del puntero](/docs/creator/content-creator-es/scenes-sdk7/interactividad/eventos-de-button/click-events/#pointer-buttons) para ver las opciones compatibles.
* `PointerEventType`: Qué tipo de evento escuchar, como un valor del `PointerEventType` enum. Consulta [Tipos de eventos del puntero](/docs/creator/content-creator-es/scenes-sdk7/interactividad/eventos-de-button/click-events/#types-of-pointer-events) para ver las opciones compatibles.
* `Entity` *(opcional)*: Qué Entity comprobar para estos eventos. Si no se proporciona ningún valor, comprobará pulsaciones globales del botón, sin importar hacia dónde apuntaba el cursor del jugador.

Ten en cuenta que en este ejemplo también estamos añadiendo un `PointerEvents` component a la entity con la que queremos interactuar. Este paso es necesario; sin este component la entity no podrá ser detectada por ninguna de las funciones de `inputSystem`. consulta [Mostrar feedback](#show-feedback) para más detalles sobre el `PointerEvents` componente.


> [!WARNING]
> **📔 Nota**: y un [collider](/docs/creator/content-creator-es/scenes-sdk7/fundamentos-del-contenido-3d/colliders/). Consulta [obstáculos](/docs/creator/content-creator-es/scenes-sdk7/interactividad/eventos-de-button/click-events/#obstacles) para más detalles.


Si hay varias entities con las que el jugador puede interactuar de la misma manera, considera usar `inputSystem.getInputCommand`. Este comando devuelve información sobre un comando de clic global, incluido el ID de la entity; puedes usarlo para ejecutar una sola función que pueda encargarse de todas.

```ts
engine.addSystem(() => {
	const result = inputSystem.getInputCommand(
		InputAction.IA_POINTER,
		PointerEventType.PET_DOWN
	)
	if (result) {
		if (result.hit?.entityId === myEntity) {
			// manejar clic
		}
	}
})
```


> [!WARNING]
> **📔 Nota**: Toda entity con la que quieras interactuar debe tener tanto un [`PointerEvents` componente](#show-feedback)  y una [collider](/docs/creator/content-creator-es/scenes-sdk7/fundamentos-del-contenido-3d/colliders/).


Consulta [Datos de la acción de entrada](#data-from-input-action) para más información. Este método también te proporciona datos más detallados sobre el impacto del pointer event.

### Pulsación del botón hacia arriba

También puedes comprobar eventos de liberación del puntero de forma muy similar, usando `PointerEventType.PET_UP`.

```ts
engine.addSystem(() => {
	if (
		inputSystem.isTriggered(
			InputAction.IA_POINTER,
			PointerEventType.PET_UP,
			myEntity
		)
	) {
		// Lógica en respuesta a la liberación del botón mientras se apunta a myEntity
	}
})
```


> [!WARNING]
> **📔 Nota**: Al comprobar eventos de liberación del puntero contra una entity específica, no se tiene en cuenta dónde apuntaba el cursor cuando se pulsó el botón. Solo se considera hacia dónde apunta el cursor cuando se suelta el botón.
> 
> También ten en cuenta que la entity debe tener tanto un [`PointerEvents` componente](#show-feedback)  y una [collider](/docs/creator/content-creator-es/scenes-sdk7/fundamentos-del-contenido-3d/colliders/).


### Comprobar botones presionados

Comprueba si un botón se está manteniendo presionado actualmente usando `inputSystem.isPressed()` dentro de un system.

```ts
engine.addSystem(() => {
	if (inputSystem.isPressed(InputAction.IA_POINTER)) {
		// Lógica en respuesta a que el botón se mantenga presionado
	}
})
```

`inputSystem.isPressed()` devuelve *true* si el botón se está manteniendo presionado actualmente, sin importar cuándo se pulsó, ni dónde apunta el cursor del jugador. De lo contrario devuelve *false*.

La sección `inputSystem.isPressed` función toma un solo argumento:

* `InputAction`: Qué entrada escuchar, como un valor del `InputAction` enum. Consulta [Botones del puntero](/docs/creator/content-creator-es/scenes-sdk7/interactividad/eventos-de-button/click-events/#pointer-buttons) para ver las opciones compatibles.

### Manejar múltiples entities

Si tu scene tiene múltiples entities afectadas por eventos de puntero de la misma manera, tiene sentido escribir un system que itere sobre todas ellas.

```ts
engine.addSystem(() => {
	const activatedEntites = engine.getEntitiesWith(PointerEvents)
	for (const [entity] of activatedEntites) {
		if (
			inputSystem.isTriggered(
				InputAction.IA_POINTER,
				PointerEventType.PET_DOWN,
				entity
			)
		) {
			// Lógica en respuesta a interactuar con una entity
		}
	}
})
```

Este ejemplo usa una [consulta de components](/docs/creator/content-creator-es/scenes-sdk7/arquitectura/querying-components/) para iterar sobre todas las entities con un `PointerEvents` component. Luego comprueba cada una de estas entities con `inputSystem.isTriggered`, iterándolas una por una. Si se detecta una acción de entrada en cualquiera de estas entities, lleva a cabo una lógica personalizada.

En lugar de iterar sobre *todos* las entities con un `PointerEvents` component en un solo system, quizá quieras escribir diferentes systems para manejar entities que deban comportarse de formas distintas. El enfoque recomendado es marcar distintos tipos de entities con [components personalizados](/docs/creator/content-creator-es/scenes-sdk7/arquitectura/custom-components/), e iterarlas en systems separados.

```ts
engine.addSystem(() => {
	const activatedDoors = engine.getEntitiesWith(IsDoor, PointerEvents)
	for (const [entity] of activatedDoors) {
		if (
			inputSystem.isTriggered(
				InputAction.IA_POINTER,
				PointerEventType.PET_DOWN,
				entity
			)
		) {
			openDoor(entity)
		}
	}
})

engine.addSystem(() => {
	const pickedUpGems = engine.getEntitiesWith(IsGem, PointerEvents)
	for (const [entity] of pickedUpGems) {
		if (
			inputSystem.isTriggered(
				InputAction.IA_POINTER,
				PointerEventType.PET_DOWN,
				entity
			)
		) {
			pickUp(entity)
		}
	}
})
```

Este ejemplo tiene un system que itera sobre todas las entities que tienen un componente personalizado llamado `IsDoor` y otro que itera sobre todas las entities que tienen un componente personalizado llamado `isGem`. En ambos systems, comprueba cada entity coincidente para ver si se activó con el botón del puntero.

Esta forma de organizar el código de tu scene es muy [orientado a datos](/docs/creator/content-creator-es/scenes-sdk7/arquitectura/data-oriented-programming/) y debería dar como resultado un uso muy eficiente de los recursos de memoria.

\## Varios botones en una entity

Usando helpers como `pointerEventsSystem.onPointerDown`, cada entity queda limitada a tener un solo tipo de evento. No puedes registrar dos eventos de botón diferentes. No tienes esa limitación cuando usas eventos basados en system. Por ejemplo, aquí hay una entity con la que los jugadores pueden interactuar tanto pulsando E como F, y cada acción activa funcionalidades diferentes.

```ts
    engine.addSystem(() => {
        if (inputSystem.isTriggered(InputAction.IA_PRIMARY, PointerEventType.PET_DOWN, myEntity)) {
            // reaccionar a E
        }
        if (inputSystem.isTriggered(InputAction.IA_SECONDARY, PointerEventType.PET_DOWN, myEntity)) {
            // reaccionar a F
        }
    })
```

## Mostrar feedback

Para mostrar sugerencias de UI mientras apuntas a una entity, usa las propiedades en el `PointerEvents` componente.

```ts
// crear entity
const myEntity = engine.addEntity()

// darle a la entity un component PointerEvents
PointerEvents.create(myEntity, {
	pointerEvents: [
		{
			eventType: PointerEventType.PET_DOWN,
			eventInfo: {
				button: InputAction.IA_POINTER,
			},
		},
	],
})
```

Siempre que el cursor del jugador apunte a los colliders de esta entity, la UI mostrará una sugerencia al pasar el cursor para indicar que se puede interactuar con la entity. Consulta las secciones de abajo para más detalles sobre lo que puedes configurar.


> [!WARNING]
> **📔 Nota**: El `PointerEvents` el component solo se encarga de mostrar el feedback al pasar el cursor. Para manejar los propios eventos de botón con lógica personalizada, consulta [Using-a-system](#using-a-system).


La sección `PointerEvents` el component requiere al menos una definición de evento de puntero. Cada definición de evento de puntero puede configurarse con lo siguiente:

* `eventType`: Qué tipo de evento escuchar, como un valor del `PointerEventType` enum. Consulta [Tipos de eventos del puntero](/docs/creator/content-creator-es/scenes-sdk7/interactividad/eventos-de-button/click-events/#types-of-pointer-events) para ver las opciones compatibles.
* `eventInfo`: Un objeto que puede contener los siguientes campos:
  * `button` (*requeridos*): Qué entrada escuchar, como un valor del `InputAction` enum. Consulta [Botones del puntero](/docs/creator/content-creator-es/scenes-sdk7/interactividad/eventos-de-button/click-events/#pointer-buttons) para ver las opciones compatibles.
  * `hoverText` *(opcional)*: Qué cadena mostrar en la sugerencia de feedback al pasar el cursor. "Interact" por defecto.
  * `showFeedback` *(opcional)*: Si es false, oculta tanto la sugerencia al pasar el cursor como el resaltado del borde para esta entity. *true* por defecto.
  * `showHighlight` *(opcional)*: Si es true, los jugadores verán el resaltado del borde al pasar el cursor sobre la entity. *true* por defecto. Este valor solo se considera si `showFeedback` es *true*.
  * `maxDistance` *(opcional)*: Qué tan lejos puede estar el **avatar** del jugador de la entity y seguir pudiendo interactuar con ella, en metros. El valor predeterminado es *10 metros*. Consulta [Distancia máxima](#max-distance).
  * `maxCameraDistance` *(opcional)*: Qué tan lejos puede estar la **cámara activa** de la entity y seguir pudiendo interactuar con ella, en metros. No se establece por defecto. Consulta [Distancia máxima](#max-distance).
  * `priority` *(opcional)*: Qué entity prevalece cuando varias se superponen. Los valores más altos tienen prioridad. *0* por defecto.

Un solo `PointerEvents` component puede contener múltiples definiciones de eventos de puntero, que pueden detectar distintos eventos para distintos botones. Cada entity solo puede tener *uno* `PointerEvents` component, pero este component puede incluir múltiples objetos en su `pointerEvents` array, uno por cada evento al que responder.

```ts
PointerEvents.create(NPCEntity, {
	pointerEvents: [
		{
			eventType: PointerEventType.PET_DOWN,
			eventInfo: {
				button: InputAction.IA_POINTER,
				hoverText: 'Saludar',
			},
		},
		{
			eventType: PointerEventType.PET_DOWN,
			eventInfo: {
				button: InputAction.IA_PRIMARY,
				hoverText: 'Ahogar',
			},
		},
		{
			eventType: PointerEventType.PET_UP,
			eventInfo: {
				button: InputAction.IA_PRIMARY,
				hoverText: 'Dejar de ahogar',
			},
		},
		{
			eventType: PointerEventType.PET_DOWN,
			eventInfo: {
				button: InputAction.IA_SECONDARY,
				hoverText: 'Dar gema',
			},
		},
	],
})
```

Los jugadores verán múltiples etiquetas, una por cada evento de puntero, mostradas radialmente alrededor del cursor.

El ejemplo de abajo combina el uso de `PointerEvents` para mostrar sugerencias al pasar el cursor, junto con un system que realmente maneja la acción del jugador con lógica personalizada.

```ts
// crear entity
const myEntity = engine.addEntity()

// dar feedback al pasar el cursor a la entity
PointerEvents.create(myEntity, {
	pointerEvents: [
		{
			eventType: PointerEventType.PET_DOWN,
			eventInfo: {
				button: InputAction.IA_POINTER,
				hoverText: 'Abrir',
			},
		},
	],
})

// manejar eventos de clic en la entity
engine.addSystem(() => {
	if (
		inputSystem.isTriggered(
			InputAction.IA_POINTER,
			PointerEventType.PET_DOWN,
			myEntity
		)
	) {
		// Lógica personalizada en respuesta a una acción de entrada
	}
})
```

### Feedback al pasar el cursor

Cuando un jugador pasa el cursor sobre un elemento con un `PointerEvents` component, ve:

* Un resaltado del borde en la entity
* Una sugerencia de feedback cerca del cursor con un icono para el botón que necesita pulsar y una cadena que dice "Interact".

Estos elementos pueden activarse o personalizarse.

El feedback al pasar el cursor en la UI muestra un icono diferente según qué entrada selecciones en el `button` campo. En PC, muestra un icono con una `E` para `InputAction.IA_PRIMARY`, una `F` para `InputAction.IA_SECONDARY`, y un ratón para `InputAction.IA_POINTER`.

Cambia la cadena cambiando el `hoverText` valor. Mantén esta cadena corta, para que sea rápida de leer y no resulte demasiado invasiva en la pantalla.

```ts
// crear entity
const chest = engine.addEntity()

// darle a la entity un component PointerEvents
PointerEvents.create(chest, {
	pointerEvents: [
		{
			eventType: PointerEventType.PET_DOWN,
			eventInfo: {
				button: InputAction.IA_POINTER,
				hoverText: 'Abrir',
			},
		},
	],
})
```

En el ejemplo anterior, el evento de puntero incluye un valor para `hoverText`. Este campo define la cadena que se mostrará en la UI mientras el jugador apunta a la entity. Por defecto, esta cadena dice *Interact*.


> [!NOTE]
> **💡 Consejo**: El `hoverText` la cadena debería describir la acción que ocurre al interactuar. Por ejemplo `Abrir`, `Activar`, `Coger`, `Selecciona`. Estas cadenas deberían ser lo más cortas posible, para evitar robar demasiado foco al jugador.


Si una entity tiene múltiples eventos de puntero, las sugerencias de feedback para cada uno de estos se muestran radialmente alrededor del cursor.

La sección `hoverText` de un `.UP` el evento de puntero solo se muestra mientras el jugador ya está manteniendo pulsada la tecla correspondiente y apuntando a la entity.

Si una entity tiene tanto un `DOWN` evento de puntero como un `UP` evento de puntero, la sugerencia de la `DOWN` acción se muestra mientras el botón no se está pulsando. La sugerencia cambia a la del `UP` evento solo cuando el botón se pulsa y permanece pulsado.

```ts
// crear entity
const entity = engine.addEntity()

// darle a la entity un component PointerEvents
PointerEvents.create(entity, {
	pointerEvents: [
		{
			eventType: PointerEventType.PET_DOWN,
			eventInfo: {
				button: InputAction.IA_POINTER,
				hoverText: 'Arrastrar',
			},
		},
		{
			eventType: PointerEventType.PET_UP,
			eventInfo: {
				button: InputAction.IA_POINTER,
				hoverText: 'Soltar',
			},
		},
	],
})
```

Para ocultar la sugerencia al pasar el cursor, pero dejar el resaltado del borde, establece el valor de `hoverText` a "".

```ts
// crear entity
const chest = engine.addEntity()

// darle a la entity un component PointerEvents
PointerEvents.create(chest, {
	pointerEvents: [
		{
			eventType: PointerEventType.PET_DOWN,
			eventInfo: {
				button: InputAction.IA_POINTER,
				hoverText: '',
			},
		},
	],
})
```

Para ocultar el resaltado del borde pero dejar la sugerencia al pasar el cursor, establece `showHighlight` en *false*.

```ts
// crear entity
const chest = engine.addEntity()

// darle a la entity un component PointerEvents
PointerEvents.create(chest, {
	pointerEvents: [
		{
			eventType: PointerEventType.PET_DOWN,
			eventInfo: {
				button: InputAction.IA_POINTER,
				hoverText: 'Abrir puerta',
				showHighlight: false,
			},
		},
	],
})
```

Para ocultar tanto la sugerencia al pasar el cursor como el resaltado del borde, establece `showFeedback` en *false*. Al hacer esto, el cursor no muestra ningún icono, texto ni resaltado del borde. También podrías simplemente quitar el `PointerEvents` component de la entity.

```ts
// crear entity
const chest = engine.addEntity()

// darle a la entity un component PointerEvents
PointerEvents.create(chest, {
	pointerEvents: [
		{
			eventType: PointerEventType.PET_DOWN,
			eventInfo: {
				button: InputAction.IA_POINTER,
				showFeedback: false,
			},
		},
	],
})
```

### Distancia máxima

Algunas entities pueden estar diseñadas para ser interactivas solo a corta distancia. Si un jugador está demasiado lejos de una entity, la sugerencia al pasar el cursor no se mostrará junto al cursor.

Por defecto, las entities solo se pueden pulsar cuando el **avatar** del jugador está dentro de *10 metros* de la entity. Puedes cambiar ese rango estableciendo la propiedad `maxDistance` de un evento de puntero.

```ts
// crear entity
const myEntity = engine.addEntity()

// darle a la entity un component PointerEvents
PointerEvents.create(myEntity, {
	pointerEvents: [
		{
			eventType: PointerEventType.PET_DOWN,
			eventInfo: {
				button: InputAction.IA_POINTER,
				maxDistance: 6,
			},
		},
	],
})

// implementar un system para manejar el evento de puntero
engine.addSystem(() => {
	// obtener datos sobre el evento de puntero
	const cmd = inputSystem.getInputCommand(
		InputAction.IA_POINTER,
		PointerEventType.PET_DOWN,
		myEntity
	)

	// comprobar si el clic estuvo lo suficientemente cerca
	if (cmd && cmd.hit && cmd.hit.length < 6) {
		// hacer algo
	}
})
```

El ejemplo anterior establece la distancia máxima para las sugerencias al pasar el cursor en *6 metros*. Asegúrate de que la lógica para manejar las acciones de entrada también siga las mismas reglas. Consulta [Datos de la acción de entrada](#data-from-input-action) para saber cómo obtener la distancia de una acción de entrada.

`maxDistance` se mide desde el **avatar**del jugador, por lo que se comporta igual en primera y tercera persona.

Para limitar la interacción por la distancia a la que está el **cámara activa** en su lugar, usa `maxCameraDistance`. Establecer ambos significa que basta con que pase cualquiera de los dos controles. Consulta [Límites de distancia](/docs/creator/content-creator-es/scenes-sdk7/interactividad/eventos-de-button/register-callback/#distance-limits) para el conjunto completo de reglas.


> [!WARNING]
> **📔 Nota**: `maxPlayerDistance` es un alias obsoleto de `maxDistance`. Ambos miden la distancia desde el avatar, así que usa `maxDistance` en escenas nuevas.


## Sugerencias personalizadas avanzadas

La sección `PointerEvents` el component añade fácilmente sugerencias de UI cuando el cursor del jugador empieza a pasar sobre una entity. En general, es bueno que las sugerencias se comporten de forma coherente con lo que los jugadores están acostumbrados a ver en otras scenes de Decentraland. Sin embargo, en algunos casos quizá quieras señalar que algo es interactivo de una forma personalizada. Por ejemplo, podrías reproducir un sonido sutil cuando el jugador empieza a pasar el cursor sobre la entity. También podrías mostrar un resaltado brillante alrededor de la entity mientras se pasa el cursor por encima, y ocultarlo cuando ya no se esté pasando el cursor. También podría usarse para mecánicas de juego específicas.

Usa el `inputSystem.isTriggered()` función junto con `PointerEventType.PET_HOVER_ENTER`  y `PointerEventType.PET_HOVER_LEAVE` eventos para llevar a cabo comportamientos personalizados cada vez que el cursor del jugador empieza a apuntar al collider de la entity, y cada vez que el cursor deja de apuntar a él.

El ejemplo de abajo agranda las entities a un tamaño de *1.5* cuando el cursor empieza a apuntar a su collider, y las vuelve a poner en un tamaño de *1* cuando el cursor las deja.

```ts
engine.addSystem(() => {
	const meshEntities = engine.getEntitiesWith(MeshRenderer)
	for (const [entity] of meshEntities) {
		if (
			inputSystem.isTriggered(
				InputAction.IA_POINTER,
				PointerEventType.PET_HOVER_ENTER,
				entity
			)
		) {
			Transform.getMutable(entity).scale = Vector3.create(1.5, 1.5, 1.5)
		}

		if (
			inputSystem.isTriggered(
				InputAction.IA_POINTER,
				PointerEventType.PET_HOVER_LEAVE,
				entity
			)
		) {
			Transform.getMutable(entity).scale = Vector3.create(1, 1, 1)
		}
	}
})
```


> [!WARNING]
> **📔 Nota**: Toda entity con la que quieras interactuar debe tener tanto un [`PointerEvents` componente](#show-feedback)  y una [collider](/docs/creator/content-creator-es/scenes-sdk7/fundamentos-del-contenido-3d/colliders/).


## Eventos de proximidad

Comprueba pulsaciones de botón por proximidad sin importar hacia dónde apunte el cursor usando `PET_DOWN` en una entity que usa `InteractionType.PROXIMITY`. Establece el `interactionType` campo en la definición del evento de puntero, junto con `eventInfo`, para marcar el evento como basado en proximidad.

Consulta [**Eventos de proximidad**](/docs/creator/content-creator-es/scenes-sdk7/interactividad/eventos-de-button/proximity-events/) para más detalles, incluyendo las funciones auxiliares disponibles.

```ts
PointerEvents.create(myEntity, {
	pointerEvents: [
		{
			eventType: PointerEventType.PET_DOWN,
			eventInfo: {
				button: InputAction.IA_PRIMARY,
				hoverText: 'Presiona E',
				maxDistance: 5
			},
			interactionType: InteractionType.PROXIMITY,
		},
	],
})

engine.addSystem(() => {
	if (inputSystem.isTriggered(InputAction.IA_PRIMARY, PointerEventType.PET_DOWN, myEntity)) {
		console.log('El jugador presionó el botón cerca de la Entity')
	}
})
```

Con ayudas como `pointerEventsSystem.onProximityDown`, un Entity puede tener un controlador de cursor y un controlador de proximidad al mismo tiempo, y ambas callbacks se ejecutan. Lo que no puede hacer es registrar dos botones diferentes para el mismo controlador. Los eventos basados en System no tienen esa limitación: defines una entrada por comportamiento. Por ejemplo, aquí hay un Entity que puede interactuar tanto al apuntar con el cursor y presionar E, como al acercarse y presionar E sin apuntarle con el cursor.

```ts
	PointerEvents.create(myEntity, {
        pointerEvents: [
            {
                eventType: PointerEventType.PET_DOWN,
                eventInfo: {
                    button: InputAction.IA_POINTER,
                    hoverText: 'Clic',
                    maxDistance: 15,
                },
                interactionType: InteractionType.CURSOR,
            },
            {
                eventType: PointerEventType.PET_DOWN,
                eventInfo: {
                    button: InputAction.IA_PRIMARY,
                    hoverText: 'Presiona E',
                    maxDistance: 15,
                },
                interactionType: InteractionType.PROXIMITY,
            },
        ],
    })
    
    engine.addSystem(() => {
        if (inputSystem.isTriggered(InputAction.IA_POINTER, PointerEventType.PET_DOWN, myEntity)) {
            console.log('El jugador hizo clic en la Entity')
        }
        if (inputSystem.isTriggered(InputAction.IA_PRIMARY, PointerEventType.PET_DOWN, myEntity)) {
            console.log('El jugador presionó el botón cerca de la Entity')
        }
    })
```

También puedes usar `inputSystem.isTriggered()` con los `PET_PROXIMITY_ENTER`  y `PET_PROXIMITY_LEAVE` tipos de evento para reaccionar cuando un jugador entra o sale del rango de proximidad de un Entity. Esto es útil para mostrar información adicional para Entities activadas por proximidad, por ejemplo, reproducir un sonido o una animación.

Para que esto funcione, el Entity debe tener un `PointerEvents` componente con el tipo de evento de proximidad correspondiente definido.

```ts
PointerEvents.create(myEntity, {
	pointerEvents: [
		{
			eventType: PointerEventType.PET_PROXIMITY_ENTER,
			eventInfo: {
				button: InputAction.IA_POINTER,
				maxDistance: 5,
			},
		},
		{
			eventType: PointerEventType.PET_PROXIMITY_LEAVE,
			eventInfo: {
				button: InputAction.IA_POINTER,
				maxDistance: 5,
				showFeedback: false,
			},
		},
	],
})

engine.addSystem(() => {
	if (inputSystem.isTriggered(InputAction.IA_POINTER, PointerEventType.PET_PROXIMITY_ENTER, myEntity)) {
		console.log('El jugador entró en proximidad')
	}
	if (inputSystem.isTriggered(InputAction.IA_POINTER, PointerEventType.PET_PROXIMITY_LEAVE, myEntity)) {
		console.log('El jugador salió de proximidad')
	}
})
```

## Datos de la acción de entrada

Obtener datos de una acción de entrada, como el botón que se presionó, el Entity que fue golpeado, la dirección y la longitud del rayo, etc. Ver ([Ver documentación](https://github.com/decentraland/docs/tree/main/README.md)) para una descripción de todos los datos disponibles.

Para obtener estos datos, usa `inputSystem.getInputCommand`. Esta función devuelve la estructura completa de datos con información sobre el evento de entrada.

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

Si no hubo ninguna acción de entrada que coincida con la consulta, entonces `inputSystem.getInputCommand` devuelve *null*. Asegúrate de manejar este escenario en tu lógica.

### Distancia máxima de clic

Para aplicar una distancia máxima, de modo que un Entity solo sea clickable a corta distancia, obtén `hit.length` propiedad de los datos del evento.

```ts
// implementar un system para manejar el evento de puntero
engine.addSystem(() => {
	// obtener datos sobre el evento de puntero
	const cmd = inputSystem.getInputCommand(
		InputAction.IA_POINTER,
		PointerEventType.PET_DOWN,
		myEntity
	)

	// comprobar si el clic estuvo lo suficientemente cerca
	if (cmd && cmd.hit && cmd.hit.length < 6) {
		// hacer algo
	}
})
```


> [!WARNING]
> **📔 Nota**: Si ignoras cualquier evento que esté lejos, asegúrate de configurar el `maxDistance` parámetro en el `PointerEvents` componente para comportarse de forma consistente.


### Diferentes meshes dentro de un modelo

A menudo, *.glTF* los modelos 3D están compuestos por múltiples meshes, y cada uno tiene un nombre interno individual. Todos los eventos de botón incluyen la información de qué mesh específica fue clicada, así que puedes usar esta información para desencadenar diferentes comportamientos de clic en cada caso.

Para ver cómo se nombran los meshes dentro del modelo, debes abrir el modelo 3D con una herramienta de edición, como [Blender](https://www.blender.org/) por ejemplo.

![](/docs/_remote/5f167d2485c1df5d331a16b6.webp)


> [!NOTE]
> **💡 Consejo**: También puedes conocer el nombre del mesh clicado registrándolo y leyéndolo en la consola.


Accedes a la propiedad `meshName` como parte del objeto `hit` , que se devuelve por el evento de clic.

En el ejemplo de abajo tenemos un modelo de casa que incluye un mesh llamado `firePlace`. Queremos encender la chimenea solo cuando se haga clic en su mesh correspondiente.

```ts
engine.addSystem(() => {
	const cmd = inputSystem.getInputCommand(
		InputAction.IA_POINTER,
		PointerEventType.PET_DOWN,
		myEntity
	)
	if (cmd && cmd.hit?.meshName === 'firePlace') {
		// light fire
	}
})
```


> [!WARNING]
> **📔 Nota**: Esto solo se aplica a eventos basados en pointer, no a eventos basados en proximidad, ya que en ese caso todos los meshes del modelo podrían estar en proximidad del jugador al mismo tiempo.



---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-es/scenes-sdk7/interactividad/eventos-de-button/system-based-events.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
