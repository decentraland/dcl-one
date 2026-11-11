> Aprende cómo manejar los clics de los usuarios en tu escena.

# Registrar callback

La forma más fácil de manejar los eventos de botones es registrar una función callback para una entity concreta. Cada vez que se interactúa con esa entity usando un botón específico, la función callback se llama.

Si necesitas añadir el mismo comportamiento a múltiples entities similares, considera usar el [basado en Systems](/docs/creator/content-creator-es/scenes-sdk7/interactividad/eventos-de-button/system-based-events/) enfoque en lugar de agregar callbacks a cada entity. El enfoque basado en Systems puede resultar más eficiente, ya que iteras sobre una lista de entities similares.

El enfoque de Register callback es especialmente útil si quieres describir un comportamiento que afecta a una sola entity, ya que es más sencillo.


> [!WARNING]
> **📔 Nota**:\
> Para que una entity sea interactiva, **debe** tener un [collider](/docs/creator/content-creator-es/scenes-sdk7/fundamentos-del-contenido-3d/colliders/). Consulta [obstáculos](/docs/creator/content-creator-es/scenes-sdk7/interactividad/eventos-de-button/click-events/#obstacles) para más detalles.


## Pulsación del puntero

Usa `pointerEventsSystem.onPointerDown()` para detectar pulsaciones de un botón específico.

Esta sentencia requiere dos parámetros:

* `los datos`: Un objeto que contiene lo siguiente:
  * `entity`: La entity a manejar
  * `opts`: Un objeto con datos adicionales opcionales:
    * `button`: Qué botón escuchar. Consulta [Botones del puntero](/docs/creator/content-creator-es/scenes-sdk7/interactividad/eventos-de-button/click-events/#pointer-buttons) para ver las opciones compatibles. Si no se especifica ningún botón, entonces se escuchan todos los botones, incluidos los botones de movimiento como avanzar y saltar.
    * `maxDistance`: La distancia máxima entre la entity y el **avatar**del jugador, en metros. 10 por defecto. Consulta [Límites de distancia](#distance-limits).
    * `maxCameraDistance`: La distancia máxima entre la entity y la **cámara activa**, en metros. No se establece por defecto. Consulta [Límites de distancia](#distance-limits).
    * `hoverText`: Qué cadena mostrar en la sugerencia de hover. "Interactuar" por defecto.
    * `showFeedback`: Si es false, oculta tanto la sugerencia de hover como el resaltado de borde de esta entity. *true* por defecto.
    * `showHighlight`: Si es true, los jugadores verán el resaltado de borde al pasar el cursor sobre la entity. *true* por defecto. Este valor solo se considera si `showFeedback` es *true*.
* `cb`: Una función callback que se ejecuta cada vez que ocurre un evento de pulsación del botón mientras se apunta a la entity

```ts
pointerEventsSystem.onPointerDown(
	{
		entity: myEntity,
		opts: { button: InputAction.IA_PRIMARY, hoverText: 'Clic' },
	},
	function () {
		console.log('entity pulsada')
	}
)
```

El comando anterior deja registrada la función callback, y se llamará cada vez que ocurra el evento de botón relacionado. Ten en cuenta que la callback debe ser una función síncrona; `async` las funciones no son compatibles aquí y provocan un error.


> [!WARNING]
> **📔 Nota**:\
> Solo se puede registrar una `pointerEventsSystem.onPointerDown` por entity. Una vez añadida, seguirá escuchando eventos hasta que se elimine el listener. No lo ejecutes de forma recurrente dentro de un System, porque eso seguiría reescribiendo el comportamiento del evento del puntero.


## Feedback de hover

Es muy importante dar a los jugadores algún tipo de indicación de que se puede interactuar con una entity.

Al registrar una acción de entrada con el `EventsSystem`, por defecto los jugadores verán:

* Un resaltado de borde en la entity
* Una sugerencia de hover cerca del cursor con un icono del botón que necesitan pulsar y una cadena que dice "Interactuar".

Estos elementos se pueden activar o desactivar y personalizar.

El feedback de hover en la UI muestra un icono diferente según la entrada que selecciones en el `button` campo. En PC, muestra un icono con una `E` para `InputAction.IA_PRIMARY`, una `F` para `para`InputAction.IA\_SECONDARY `y un ratón para`.

Cambia la cadena cambiando el `hoverText` valor. Mantén esta cadena corta, para que se lea rápido y no resulte demasiado intrusiva en la pantalla.

```ts
pointerEventsSystem.onPointerDown(
	{
		entity: myEntity,
		opts: { button: InputAction.IA_PRIMARY, hoverText: 'Abrir puerta' },
	},
	function () {
		// abrir puerta
	}
)
```

Para ocultar la sugerencia de hover, pero dejar el resaltado de borde, establece el valor de `hoverText` en "".

```ts
pointerEventsSystem.onPointerDown(
  {entity: myEntity, opts: { button: InputAction.IA_PRIMARY, hoverText: ''}},
  function () {
    console.log("entity interactiva sorpresa pulsada")
  }
)
```

Para ocultar el resaltado de borde pero dejar la sugerencia de hover, establece `showHighlight` en *false*.

```ts
pointerEventsSystem.onPointerDown(
	{
		entity: myEntity,
		opts: {
			button: InputAction.IA_PRIMARY,
			hoverText: 'Abrir puerta',
			showHighlight: false,
		},
	},
	function () {
		console.log('puerta secreta abierta')
	}
)
```

Para ocultar tanto la sugerencia de hover como el resaltado de borde, establece `showFeedback` en *false*. Al hacer esto, el cursor no muestra ningún icono, texto ni ningún resaltado de borde.

```ts
pointerEventsSystem.onPointerDown(
  {entity: myEntity, opts: { button: InputAction.IA_PRIMARY, showFeedback: false}},
  function () {
    console.log("puerta secreta abierta")
  }
)
```

### Cambiar feedback existente

Al registrar una acción de entrada con el `EventsSystem`, esto está creando un `PointerEvents` component y agregándolo a la entity interactiva entre bastidores. Este component maneja el comportamiento de la sugerencia de hover de la UI. Para cambiar el comportamiento del feedback de hover, modifica este component. Consulta [Mostrar feedback](/docs/creator/content-creator-es/scenes-sdk7/interactividad/eventos-de-button/system-based-events/#show-feedback) para saber más sobre cómo tratar con este component.

```ts
const hoverFeedback = PointerEvents.getMutable(myEntity)

if (hoverFeedback.pointerEvents[0]?.eventInfo) {
	hoverFeedback.pointerEvents[0].eventInfo.hoverText = 'Cerrar puerta'
}
```

## Límites de distancia

De forma predeterminada, un jugador solo puede interactuar con una entity cuando su **avatar** avatar está a 10 metros de ella. Si el jugador está demasiado lejos, el efecto de resaltado de la entity se vuelve rojo en lugar de verde, y los eventos del puntero no se disparan.

Usa `maxDistance` para cambiar ese rango:

```ts
// Solo se puede hacer clic cuando el avatar del jugador está a 5 metros
pointerEventsSystem.onPointerDown(
	{ entity: myEntity, opts: { maxDistance: 5 } },
	function () {
		console.log('entity pulsada')
	}
)
```

### Limitar por distancia de la cámara

`maxCameraDistance` mide desde la **cámara actualmente activa** en lugar de desde el avatar. Esto es útil cuando tu scene usa una [Virtual Camera](/docs/creator/content-creator-es/scenes-sdk7/fundamentos-del-contenido-3d/camera/#using-virtual-cameras) colocada lejos del jugador, y quieres que se pueda hacer clic en la entity según lo que la cámara pueda ver.

```ts
// Solo se puede hacer clic cuando la cámara activa está a 15 metros
pointerEventsSystem.onPointerDown(
	{ entity: myEntity, opts: { maxCameraDistance: 15 } },
	function () {
		console.log('entity pulsada')
	}
)
```

### Cómo se combinan los dos límites

Las dos opciones son comprobaciones independientes. Cuáles se ejecutan depende de cuáles establezcas:

| Lo que estableces        | Lo que necesita el jugador                                |
| ------------------------ | --------------------------------------------------------- |
| Ninguno                  | Avatar a 10 metros (el valor predeterminado)              |
| `maxDistance` solo       | Avatar a `maxDistance`                                    |
| `maxCameraDistance` solo | Cámara a `maxCameraDistance`                              |
| Ambos                    | **Cualquiera** con que pase la comprobación es suficiente |

Establecer ambos es una forma de decir "lo bastante cerca como para acercarte andando, **o** y lo bastante cerca como para verlo claramente":

```ts
// Se puede hacer clic cuando el avatar está a 3 metros O la cámara está a 20 metros
pointerEventsSystem.onPointerDown(
	{ entity: myEntity, opts: { maxDistance: 3, maxCameraDistance: 20 } },
	function () {
		console.log('entity pulsada')
	}
)
```


> [!WARNING]
> **📔 Nota**: `maxCameraDistance` requiere `@dcl/sdk` versión 7.28.0 o posterior. El soporte para ello ya está disponible en el Explorer basado en Bevy. Si tu scene debe comportarse de forma idéntica en todos los clientes hoy, confía en `maxDistance` y trata `maxCameraDistance` como una mejora.


### maxPlayerDistance está obsoleto

`maxPlayerDistance` es un alias obsoleto de `maxDistance`: ambos miden la distancia desde el avatar. Usa `maxDistance` en escenas nuevas.

Si una scene establece ambos, se usa el **mayor** de los dos como el límite de distancia del avatar.

## Pulsación del puntero

Usa `pointerEventsSystem.onPointerUp` para registrar una función callback que se llama cuando el jugador indicado suelta el botón mientras apunta a la entity.

```ts
pointerEventsSystem.onPointerUp(
	{
		entity: myEntity,
		opts: { button: InputAction.IA_PRIMARY, hoverText: 'Botón soltado' },
	},
	function () {
		console.log('botón soltado')
	}
)
```

Esta sentencia requiere dos parámetros:

* `los datos`: Un objeto que contiene lo siguiente:
  * `entity`: La entity a manejar
  * `opts`: Un objeto con datos adicionales opcionales:
    * `button`: Qué botón escuchar. Consulta [Botones del puntero](/docs/creator/content-creator-es/scenes-sdk7/interactividad/eventos-de-button/click-events/#pointer-buttons) para ver las opciones compatibles. Si no se especifica ningún botón, entonces se escuchan todos los botones, incluidos los botones de movimiento como avanzar y saltar.
    * `hoverText`: Qué cadena mostrar en la sugerencia de hover. "Interactuar" por defecto.
    * `showFeedback`: Si es false, oculta la sugerencia de hover de esta entity. *true* por defecto.
    * `maxDistance`: Qué tan lejos puede estar el avatar del jugador de la entity y aun así interactuar con ella, en metros. 10 por defecto. Si el jugador está demasiado lejos, no habrá feedback de hover y los eventos del puntero no funcionarán. Consulta [Límites de distancia](#distance-limits).
* `cb`: Una función callback que se ejecuta cada vez que ocurre un evento de botón al soltar mientras se apunta a la entity.

Una misma entity puede tener registradas dos callbacks diferentes, una para `pointerEventsSystem.onPointerDown` y una para `pointerEventsSystem.onPointerUp`. La entity solo puede registrar una callback de cada una, [Manejar varios botones](#handle-multiple-buttons) para detectar distintos botones en una misma callback.


> [!WARNING]
> **📔 Nota**: El feedback de hover para un evento de botón al soltar solo se muestra cuando el botón está actualmente pulsado. Si el jugador apunta a la entity sin mantener pulsado el botón, no verá ningún feedback, o el feedback del evento de pulsación del botón, si existe.


## Entrada y salida del hover

Usa `pointerEventsSystem.onPointerHoverEnter` para ejecutar una callback cuando el cursor del jugador empieza a apuntar a una entity, y `pointerEventsSystem.onPointerHoverLeave` cuando el cursor deja de apuntar a ella.

```ts
pointerEventsSystem.onPointerHoverEnter(
    {
        entity: myEntity,
        opts: { button: InputAction.IA_POINTER },
    },
    function () {
        console.log('El cursor empezó a pasar sobre la entity')
    }
)

pointerEventsSystem.onPointerHoverLeave(
    {
        entity: myEntity,
        opts: { button: InputAction.IA_POINTER },
    },
    function () {
        console.log('El cursor dejó de pasar sobre la entity')
    }
)
```

Estas callbacks son útiles para efectos de hover personalizados, como reproducir un sonido o animar la entity cuando el jugador la apunta.

## Eliminar callbacks

Para eliminar una función callback, usa la función de eliminación correspondiente:

```ts
pointerEventsSystem.removeOnPointerDown(myEntity)
pointerEventsSystem.removeOnPointerUp(myEntity)
pointerEventsSystem.removeOnPointerHoverEnter(myEntity)
pointerEventsSystem.removeOnPointerHoverLeave(myEntity)
```

Una vez eliminada, el feedback de hover en la entity ya no debería mostrarse, y la entity ya no debería ser interactiva.

Cada función de eliminación solo quita la entrada que agregó su función de registro correspondiente. Registrar y eliminar repetidamente la misma callback no deja entradas obsoletas en la entity.

## Interacciones de proximidad

Para interacciones que se activan según la proximidad del jugador en lugar de la dirección del cursor, consulta [**Proximity Events**](/docs/creator/content-creator-es/scenes-sdk7/interactividad/eventos-de-button/proximity-events/). El `pointerEventsSystem` incluye ayudantes equivalentes (`onProximityDown`, `onProximityUp`, `onProximityEnter`, y `onProximityLeave`) que siguen el mismo patrón que las funciones descritas en esta página.

Una entity puede tener tanto un handler del cursor como un handler de proximidad para el mismo tipo de evento. Ambas callbacks se ejecutan, cada una cuando se cumple su propia condición.

### Datos de la acción de entrada

Obtén datos de una acción de entrada, como el botón que se pulsó, la entity que fue alcanzada, la dirección y la longitud del ray, etc. Consulta ([Consulta la documentación](https://github.com/decentraland/docs/tree/main/README.md)) para ver una descripción de todos los datos disponibles.

Para obtener estos datos, pasa un parámetro a la función callback. Este parámetro contiene la estructura completa de datos sobre el evento de entrada.

```ts
pointerEventsSystem.onPointerDown(
	{ entity: myEntity, opts: { button: InputAction.IA_PRIMARY } },
	function (cmd) {
		console.log(cmd.hit?.entityId)
	}
)
```

### Manejar varios botones

No puedes registrar más de una `onPointerDown` en una sola entity. Idealmente deberías usar el [basado en Systems](/docs/creator/content-creator-es/scenes-sdk7/interactividad/eventos-de-button/system-based-events/) enfoque, ya que esto te permite manejar tantos inputs distintos como quieras y mostrar una sugerencia de feedback de hover de UI para cada botón.

Como alternativa, puedes usar el enfoque de Register callback y establecer el `button` campo como `InputAction.IA_ANY`.

```ts
pointerEventsSystem.onPointerDown(
  {entity: myEntity, opts: { button: InputAction.IA_ANY}},
  function (cmd) {
      if(cmd.button === InputAction.IA_POINTER){
        // haz X
      } else if (cmd.button === InputAction.IA_PRIMARY){
        // haz Y
      }
  }
)
```

Este enfoque no es ideal, ya que la sugerencia de hover muestra una sola cadena y no especificará qué acción activar. Ten en cuenta que esto hará que la función callback se ejecute para cada acción de entrada, incluidas las teclas de movimiento, así que debes filtrar solo las acciones que te interesen.


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-es/scenes-sdk7/interactividad/eventos-de-button/register-callback.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
