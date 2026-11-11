> Referencia ítems en tu código por nombre o por etiqueta.

# Referenciar ítems en el código

Puedes hacer referencia a elementos que se agregan mediante la interfaz de arrastrar y soltar de Creator Hub en tu código. Esto es útil para añadir un comportamiento sofisticado a esos elementos, o para hacer referencia a ellos desde otras partes de tu código.

## Obtener por nombre

Al usar Creator Hub y agregar entities arrastrándolas al canvas, cada entity tiene un nombre único. Usa la `engine.getEntityOrNullByName()` función para hacer referencia a una de estas entities desde tu código.

Usa el `EntityNames` enum para acceder fácilmente a los nombres de las entities que agregaste mediante Creator Hub, o escribe el nombre como una string tal como aparece en la vista del entity tree de la scene en Scene Editor.

```ts
import { EntityNames } from '../assets/scene/entity-names'

function main() {

	// Usa el enum EntityNames
	const door1 = engine.getEntityOrNullByName(EntityNames.Door_1)

	// Escribe el nombre como una string
	const door2 = engine.getEntityOrNullByName('Door 2')

	// Asegúrate de que ambas doors existan en la scene
	if (door1 && door2) {
		// 
	}

}
```

![](/docs/_remote/6c4b9156f30308b7eb4c3fc9.webp)

El `EntityNames` enum contiene la lista completa de entities agregadas por Creator Hub y se actualiza de inmediato en cuanto haces cualquier cambio. Si importas `EntityNames.` en tu código, tu IDE te presentará un desplegable con todos los nombres de las entities disponibles.

![](/docs/_remote/c76350566b1b5677df83aaca.webp)

También puedes usar la `engine.getEntityByName<EntityNames>()` función, pasando `<EntityNames>` como un [genérico de TypeScript](https://www.typescriptlang.org/docs/handbook/2/generics.html), para validar que realmente existe un entity con ese nombre en tu scene. Si el entity referenciado se renombra en Creator Hub, este método te mostrará una advertencia con un error. Como el resultado de esta función no puede ser `null`, puedes evitar comprobar que la entity exista.

```ts
import { EntityNames } from '../assets/scene/entity-names'

function main() {

	const door1 = engine.getEntityByName<EntityNames>(EntityNames.Door_1)

	// No es necesario comprobar null
	console.log(Transform.get(door1).position.x)

}
```


> [!WARNING]
> **📔 Nota**: Asegúrate de usar solo `engine.getEntityOrNullByName()` y `engine.getEntityByName()` dentro de la `main()` en funciones que se ejecutan después de `main()`, o en un system. Si se usa fuera de uno de esos contextos, es posible que las entities creadas en el Scene Editor aún no se hayan instanciado.


Una vez que hayas obtenido una referencia a un entity con cualquiera de los métodos anteriores, eres libre de realizar cualquier acción con él, como agregar o quitar components, modificar valores de components existentes, o incluso quitar el entity del engine.

```ts
import { EntityNames } from '../assets/scene/entity-names'

function main() {
	// obtener entity
	const door = engine.getEntityOrNullByName(EntityNames.Door_3)
	// verificar que la entity exista
	if (door) {
		// agregar una callback de pointer events
		pointerEventsSystem.onPointerDown(
			{
				entity: door,
				opts: { button: InputAction.IA_PRIMARY, hoverText: 'Open' },
			},
			function () {
				// abrir puerta
			}
		)
	}
}
```

Todas las entities agregadas mediante el Scene Editor tienen un `Nombre` component, también puedes iterar sobre todas ellas así:

```ts
function main() {
	for (const [entity, name] of engine.getEntitiesWith(Name)) {
		console.log({ entity, name })
	}
}
```

## Obtener por tag

También puedes obtener entities por sus tags. Los tags son una forma de agrupar entities y son útiles para identificar entities que tienen el mismo propósito o comportamiento.

Añade Tags a un entity mediante la **sección Tags** en la parte superior del panel de propiedades del item. Puedes elegir entre tags genéricos como **Tag Group 1** hasta **Tag Group 4**, o crear los tuyos propios con un nombre más específico.

![](/docs/_remote/feb4023234754c8ebd2b7590.webp)


> [!NOTE]
> **💡 Consejo**: Un solo entity puede tener múltiples tags asignados.
> 
> <img src="/docs/_remote/22e3f2ff154ade672d73545c.webp" alt="" data-size="original">


Luego puedes obtener todas las entities que tengan un tag específico usando la `engine.getEntitiesByTag()` función. Esto es ideal cuando quieres iterar sobre un grupo de entities que tienen el mismo propósito o comportamiento.

```ts
import { engine } from '@dcl/sdk/ecs'

export function main() {
	const taggedEntities = engine.getEntitiesByTag('myTag')
  
	for (const entity of taggedEntities) {
      // Hacer algo con cada entity
    }
}
```

También puedes agregar o quitar tags de un entity desde tu código. Esto es útil si quieres cambiar los tags según alguna lógica, o para generar entities dinámicamente que tengan tags específicos.

```ts
import { Tags } from '@dcl/sdk/ecs'

Tags.remove(entity, tagName);
Tags.add(entity, tagName);
```

## Obtener todos los children de un item

Una vez que tengas una referencia a un item en particular, puedes obtener todos los items que están agrupados como sus children en el entity tree a la izquierda de la pantalla. El siguiente script obtiene el entity padre y luego itera sobre cada uno de sus children que tienen un componente Transform. Después puedes aplicar cualquier lógica personalizada que quieras mientras iteras sobre cada uno.

```ts
import { engine, Entity, Transform, Name, getEntitiesWithParent } from '@dcl/sdk/ecs'
import { EntityNames } from '../assets/scene/entity-names'

function main() {
	// obtener entity padre
	const parent = engine.getEntityByName<EntityNames>(EntityNames.ParentEntity)

	// obtener lista completa de hijos
	const children = getEntitiesWithParent(engine, parent)
	for (const child of children) {
   		// procesar cada entity hijo
	}
}
```

## Disparadores de smart item

Puedes detectar los smart item's **eventos Trigger**, y responder a ellos con código personalizado. Por ejemplo, podrías colocar un smart item de botón y activar código personalizado cuando se hace clic en el botón.

Usa `getTriggerEvents` para obtener un objeto que pueda manejar los trigger events de un smart item específico, y luego usar la `.on()` función del objeto devuelto para suscribirse a una función callback. Esta función callback se ejecuta cada vez que ocurre el trigger event.

Por ejemplo, si una scene tiene un botón con el siguiente **On Click** evento genérico,

![](/docs/_remote/ed2a86256c21aaf6084b2d47.webp)

```ts
import { engine } from '@dcl/sdk/ecs'
import { getTriggerEvents, getActionEvents } from '@dcl/asset-packs/dist/events'
import { TriggerType } from '@dcl/asset-packs'
import { EntityNames } from '../assets/scene/entity-names'

function main() {
	const restart = engine.getEntityOrNullByName(EntityNames.Restart_Button)
	if (restart) {
		const restart_event = getTriggerEvents(restart)
		restart_event.on(TriggerType.ON_CLICK, () => {
			// restartGame()
		})
	}
}
```

También puedes suscribirte de manera similar a cualquier otro tipo de trigger events, como **ON\_PLAYER\_ENTERS\_AREA**, **ON\_SPAWN**, **ON\_TWEEN\_END**, etc.

## Acciones de smart item

Puedes detectar la activación de la **Actions**, y responder a ello con código personalizado. Por ejemplo, podrías colocar un smart item de puerta y ejecutar código personalizado cada vez que la **Open** action se llama.

Usa `getActionEvents` para obtener un objeto para manejar las actions de un smart item específico. Luego puedes usar la `.on()` función del objeto devuelto para suscribirse a una función callback. Esta función callback se ejecuta cada vez que ocurre la action, sin importar si la action fue activada por otro smart item o incluso por código personalizado propio.

Por ejemplo, si una scene tiene una puerta con la siguiente action predeterminada, **Open** puedes escribir el código siguiente para ejecutar código personalizado cada vez que se abra la puerta.

![](/docs/_remote/d8d14bd55320cbd4d130d66f.webp)

```ts
import { engine } from '@dcl/sdk/ecs'
import { getTriggerEvents, getActionEvents } from '@dcl/asset-packs/dist/events'
import { TriggerType } from '@dcl/asset-packs'
import { EntityNames } from '../assets/scene/entity-names'

function main() {
	const door = engine.getEntityOrNullByName(EntityNames.Wooden_Door)
	if (door) {
		// detectar actions
		const actions = getActionEvents(door)
		actions.on('Open', () => {
			console.log('Door opened!!')
			// código personalizado
		})

		// detectar triggers
		const triggers = getTriggerEvents(door)
		triggers.on(TriggerType.ON_CLICK, () => {
			console.log('Door clicked!!')
			// código personalizado
		})
	}
}
```

También puedes emitir action events desde tu código; esto te permite aprovechar actions que ya están definidas dentro del component Action del smart item. El siguiente fragmento llama a la action "Open" en un smart item de puerta cada vez que se activa un smart item de botón.

```ts
import { engine } from '@dcl/sdk/ecs'
import { getTriggerEvents, getActionEvents } from '@dcl/asset-packs/dist/events'
import { TriggerType } from '@dcl/asset-packs'
import { EntityNames } from '../assets/scene/entity-names'

function main() {
	const button = engine.getEntityOrNullByName(EntityNames.Red_Button)
	const door = engine.getEntityOrNullByName(EntityNames.Wooden_Door)
	if (button && door) {
		// referencias a actions y triggers
		const buttonTriggers = getTriggerEvents(button)
		const doorActions = getActionEvents(door)

		// detectar triggers en el botón
		buttonTriggers.on(TriggerType.ON_INPUT_ACTION, () => {
			// abrir puerta
			doorActions.emit('Open', {})
		})
	}
}
```


> [!NOTE]
> **💡 Consejo**: Si no estás tratando de hacer algo muy complicado, en lugar de escribir código también puedes crear un smart item personalizado para manejar las actions que quieres realizar. Consulta [Cómo convertir cualquier elemento en un smart item](/docs/creator/content-creator-es/scene-editor/interactividad/make-any-item-smart/).


## Otros componentes de smart item

Los smart items pueden incluir components especiales que forman parte de la biblioteca asset-packs, como `States` o `Counter`. Estos components no forman parte de el Decentraland SDK, pero pueden obtenerse mediante la `getComponents()` función de la biblioteca. Luego puedes leer o escribir valores de estos components desde el código de tu scene, para tener una integración aún más estrecha entre el comportamiento del smart item y el código.

El ejemplo siguiente lee y registra el valor de un component State de un smart item de cofre, cada vez que se activan las actions del cofre.

```ts

import { engine } from '@dcl/sdk/ecs'
import { getComponents } from '@dcl/asset-packs'
import { getTriggerEvents } from '@dcl/asset-packs/dist/events'
import { TriggerType } from '@dcl/asset-packs'
import { EntityNames } from '../assets/scene/entity-names'


export function main() {

    const chest = engine.getEntityByName<EntityNames>(EntityNames.chest)
 
    if (chest) {

        const chestTriggers = getTriggerEvents(chest)

        chestTriggers.on(TriggerType.ON_INPUT_ACTION, () => {
            const { States } = getComponents(engine)
            let state = States.getMutableOrNull(chest)?.currentValue
            console.log( "chest new state ", state)
        })
    }
}
```


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-es/scene-editor/extender-con-codigo/reference-items.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
