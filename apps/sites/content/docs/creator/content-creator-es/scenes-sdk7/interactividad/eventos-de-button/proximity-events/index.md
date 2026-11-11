> Detecta cuándo un jugador entra o sale de la proximidad de una entidad.

# Eventos de proximidad

Las interacciones de proximidad permiten que las entities reaccionen a eventos de botón cuando un player entra, está cerca y lo está mirando aproximadamente, sin requerir que apunten su cursor al entity.

Los pointer events tradicionales usan ray-casting: el player debe apuntar su cursor al collider de un entity antes de poder presionar un botón para disparar el evento. Los eventos de proximidad, en cambio, comprueban los entities que están dentro de un área dada alrededor del avatar, sin importar hacia dónde apunta el cursor del player.

El área interactiva considerada para las interacciones de proximidad es una amplia porción triangular de una esfera, proyectada hacia adelante desde la posición del player. A diferencia de los pointer events, la dirección hacia la que mira el avatar es importante, independientemente de la dirección hacia la que mira la cámara.

Para definir una lógica personalizada más específica, quizá quieras trabajar con los datos en bruto y usar el [Avanzado](/docs/creator/content-creator-es/scenes-sdk7/interactividad/eventos-de-button/advanced-button-events/) enfoque.

Para que una entity sea interactiva, debe tener un [collider](/docs/creator/content-creator-es/scenes-sdk7/fundamentos-del-contenido-3d/colliders/). Consulta [obstáculos](/docs/creator/content-creator-es/scenes-sdk7/interactividad/eventos-de-button/click-events/#obstacles) para más detalles.

## Registrar callbacks de proximidad

El `pointerEventsSystem` incluye funciones auxiliares para eventos de proximidad, siguiendo el mismo patrón que [callbacks de eventos de puntero](/docs/creator/content-creator-es/scenes-sdk7/interactividad/eventos-de-button/register-callback/).

### Pulsaciones de botón de proximidad

Usa `pointerEventsSystem.onProximityDown` para detectar pulsaciones de botón mientras el player está dentro del rango. A diferencia de `onPointerDown`, esto no requiere que el player apunte su cursor al entity.

```ts
pointerEventsSystem.onProximityDown(
    {
        entity: myEntity,
        opts: {
            button: InputAction.IA_PRIMARY,
            hoverText: 'Press E',
            maxDistance: 5,
        },
    },
    function () {
        console.log('Player pressed button near entity')
    }
)
```

Usa `pointerEventsSystem.onProximityUp` para detectar cuándo el player suelta un botón mientras está dentro del rango.

```ts
pointerEventsSystem.onProximityUp(
    {
        entity: myEntity,
        opts: {
            button: InputAction.IA_PRIMARY,
            hoverText: 'Suelta E',
            maxDistance: 5,
        },
    },
    function () {
        console.log('El player soltó el botón cerca del entity')
    }
)
```


> [!WARNING]
> **📔 Nota**: Solo uno `onProximityDown` y uno `onProximityUp` pueden registrarse por entity. Una vez añadidos, siguen escuchando hasta que se eliminen. No llames a estos dentro de un loop de System, ya que eso seguiría reescribiendo el comportamiento.


## Tipos de evento de proximidad

Se han añadido dos nuevos valores a la `PointerEventType` enum:

* `PET_PROXIMITY_ENTER`: Se activa cuando el player entra en el rango de proximidad del entity.
* `PET_PROXIMITY_LEAVE`: Se activa cuando el player sale del rango de proximidad del entity.

Estos pueden usarse con un `PointerEvents` Component y el `inputSystem` para el [enfoque basado en System](/docs/creator/content-creator-es/scenes-sdk7/interactividad/eventos-de-button/system-based-events/#proximity-events). Ese enfoque también admite pulsaciones de botón de proximidad usando el `InteractionType.PROXIMITY` campo.

## Prioridad

Cuando múltiples entities están dentro del rango y podrían responder a la misma input, solo uno se activa, por defecto el más cercano. Usa el `prioridad` campo para controlar cuál tiene prioridad. Los entities con un **número más alto** como valor de prioridad responden primero.

Si el player está tanto dentro del rango de un entity con una interacción de proximidad como si el cursor del player apunta a un entity que tiene una interacción de pointer, al entity con la interacción de pointer siempre se le da prioridad. De nuevo, solo un entity se activa.

```ts
pointerEventsSystem.onProximityDown(
    {
        entity: doorEntity,
        opts: {
            button: InputAction.IA_PRIMARY,
            hoverText: 'Abrir puerta',
            maxDistance: 5,
            priority: 2,
        },
    },
    function () {
        console.log('Puerta activada')
    }
)

pointerEventsSystem.onProximityDown(
    {
        entity: floorEntity,
        opts: {
            button: InputAction.IA_PRIMARY,
            hoverText: 'Pisa aquí',
            maxDistance: 5,
            priority: 1,
        },
    },
    function () {
        console.log('Suelo activado')
    }
)
```

En el ejemplo anterior, si ambos entities están dentro del rango, el entity de la puerta responde porque tiene la prioridad más alta.

El `prioridad` campo también puede establecerse en las propiedades de un `PointerEvents` component.

## Opciones

Las funciones auxiliares de proximidad aceptan las mismas opciones que sus equivalentes de pointer:

* `button`: Qué botón escuchar. Consulta [Botones del puntero](/docs/creator/content-creator-es/scenes-sdk7/interactividad/eventos-de-button/click-events/#pointer-buttons) para las opciones admitidas.
* `maxDistance`: Distancia máxima desde la **avatar** del player al entity, en metros. 10 por defecto.
* `hoverText`: Texto para mostrar en la UI cuando el player está cerca del entity.
* `showHighlight`: Si es true, muestra un resaltado del borde en el entity cuando el player está dentro del rango. *true* por defecto.
* `showFeedback`: Si es true, muestra feedback de hover, flotando alrededor del centro del entity. *true* por defecto.
* `prioridad`: Resuelve conflictos cuando múltiples entities están en proximidad. Los valores más altos tienen prioridad. Si múltiples entities tienen el mismo valor de prioridad, se elige el más cercano.

`maxPlayerDistance` es un alias obsoleto de `maxDistance`. Ambas miden la distancia desde el avatar, así que usa `maxDistance` en escenas nuevas.

### Qué cuenta como estar dentro del rango

Los eventos de proximidad se miden desde el **centro de la cápsula del player**, aproximadamente a la altura del pecho, hasta el punto más cercano en el collider del entity. Eso está aproximadamente un metro por encima de donde se mide un click del cursor, por lo que el mismo punto puede registrar una distancia ligeramente distinta en cada ruta.

La distancia por sí sola no es suficiente. Para disparar un evento de proximidad, el player también debe:

* Estar **mirando hacia** el entity, dentro de un cono de aproximadamente 120 grados frente a él.
* Tener una **línea de visión clara** hacia él, sin nada en medio.


> [!WARNING]
> **📔 Nota**: Los eventos de proximidad ignoran `maxCameraDistance`. La posición de la cámara no hace ninguna diferencia. Solo se consideran la distancia, la orientación y la línea de visión del avatar. Consulta [Límites de distancia](/docs/creator/content-creator-es/scenes-sdk7/interactividad/eventos-de-button/register-callback/#distance-limits) para ver cómo funciona el límite de la cámara en los eventos de cursor.


## Eliminar callbacks

Para eliminar un callback de proximidad, usa la función de eliminación correspondiente:

```ts
pointerEventsSystem.removeOnProximityDown(myEntity)
pointerEventsSystem.removeOnProximityUp(myEntity)
pointerEventsSystem.removeOnProximityEnter(myEntity)
pointerEventsSystem.removeOnProximityLeave(myEntity)
```

Una vez eliminado, el entity ya no reacciona a ese evento de proximidad.

## Entrada y salida de proximidad

Usa `pointerEventsSystem.onProximityEnter` para ejecutar un callback cuando el player entra en el rango de proximidad del entity, y `pointerEventsSystem.onProximityLeave` cuando salen. Esto puede usarse para mostrar hints de feedback adicionales al player, por ejemplo un sound o una animación cuando se puede interactuar con un item.

```ts
pointerEventsSystem.onProximityEnter(
    {
        entity: myEntity,
        opts: {
            button: InputAction.IA_POINTER,
            hoverText: 'Cerca',
            maxDistance: 5,
        },
    },
    function () {
        console.log('Player entered proximity')
    }
)

pointerEventsSystem.onProximityLeave(
    {
        entity: myEntity,
        opts: {
            button: InputAction.IA_POINTER,
            hoverText: 'Cerca',
            maxDistance: 5,
        },
    },
    function () {
        console.log('Player left proximity')
    }
)
```

## Ejemplo: puerta de proximidad

El siguiente ejemplo abre o cierra una puerta cuando el player presiona un botón mientras está cerca, sin necesidad de apuntar a la puerta.

```ts
const doorPivot = engine.addEntity()
Transform.create(doorPivot, { position: Vector3.create(3, 0, 4) })

const door = engine.addEntity()
GltfContainer.create(door, { src: 'assets/door.glb' })
Transform.create(door, { position: Vector3.create(-1, 0, 0), parent: doorPivot })

let isDoorOpen = false
const closedRot = Quaternion.fromEulerDegrees(0, 0, 0)
const openRot = Quaternion.fromEulerDegrees(0, 90, 0)

pointerEventsSystem.onProximityDown(
    {
        entity: door,
        opts: {
            button: InputAction.IA_PRIMARY,
            hoverText: 'Abrir / Cerrar',
            maxDistance: 5,
            priority: 1,
        },
    },
    function () {
        if (isDoorOpen) {
            Tween.setRotate(doorPivot, openRot, closedRot, 700)
            isDoorOpen = false
        } else {
            Tween.setRotate(doorPivot, closedRot, openRot, 700)
            isDoorOpen = true
        }
    }
)
```


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-es/scenes-sdk7/interactividad/eventos-de-button/proximity-events.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
