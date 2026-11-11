> Aprende lo esencial sobre entities y components en una escena de Decentraland

# Entities y Components

Las escenas de Decentraland se construyen alrededor de [*entities*, *components* y *systems*](https://en.wikipedia.org/wiki/Entity%E2%80%93component%E2%80%93system). Este es un patrón común usado en la arquitectura de varios game engines, que permite una fácil composabilidad y escalabilidad.

![](/docs/_remote/0b852c135fff08d1f4c9243d.webp)

## Descripción general

*Entities* son la unidad básica para construir todo en las escenas de Decentraland. Todos los objetos 3D visibles e invisibles y los reproductores de audio en tu escena serán un entity cada uno. Un entity no es más que un id, que puede ser referenciado por components. El entity en sí no tiene propiedades ni métodos propios; simplemente sirve para agrupar varios components juntos.

*Components* definen los rasgos de un entity. Por ejemplo, un `Transform` component almacena las coordenadas, rotación y escala del entity. Un `MeshRenderer` component le da al entity una forma visible (como un cubo o una esfera) cuando se renderiza en la escena, un `Material` component le da al entity un color o textura. También puedes crear components personalizados para servir los datos requeridos por tu escena; por ejemplo, un custom `health` podría almacenar el valor de salud restante de un entity, y añadirlo a entities que representen enemigos no jugadores en un juego.

Si estás familiarizado con el desarrollo web, piensa en los entities como el equivalente de *Elements* en un *DOM* tree, y en los components como *attributes* de esos elements.

En la [Scene Editor en Creator Hub](/docs/creator/content-creator-es/scene-editor/primeros-pasos/about-editor/), puedes ver los components que pertenecen a un entity seleccionándolo.

![](/docs/_remote/f49a93029e97b3d8ef464fb3.webp)


> [!WARNING]
> **📔 Nota**: En versiones anteriores del SDK, Entities eran *objects* instanciados, y podían extenderse para añadir funciones. A partir de la versión 7.0 del SDK, entities son solo un ID. Esta estructura encaja mejor con los principios de [programación orientada a datos](/docs/creator/content-creator-es/scenes-sdk7/arquitectura/data-oriented-programming/) y puede ayudar al rendimiento de la escena.


![](/docs/_remote/28d14c88b2d418f3eab3d2ee.webp)

Components como `Transform`, `Material` o cualquiera de los *shape* components están estrechamente ligados al renderizado de la escena. Si los valores en estos components cambian, eso por sí solo basta para que el engine cambie cómo se renderiza la escena en el siguiente frame.

El engine es la parte de la escena que se sitúa en el medio y gestiona todas las demás partes. Determina qué entities se renderizan y cómo interactúan los players con ellas. También coordina qué functions de [systems](/docs/creator/content-creator-es/scenes-sdk7/arquitectura/systems/) se ejecutan y cuándo.

Los components están pensados para almacenar datos sobre su entity referenciado. Solo pueden almacenar estos datos; no pueden modificarlos por sí mismos. Todos los cambios en los valores de los components son realizados por [Systems](/docs/creator/content-creator-es/scenes-sdk7/arquitectura/systems/). Los systems están completamente desacoplados de los components y de los entities en sí. Entities y components son agnósticos respecto a qué *systems* están actuando sobre ellos.

## Sintaxis para entities y components

El ejemplo de abajo muestra algunas operaciones básicas para declarar y configurar entities y components básicos.

```ts
export function main() {
	// Crear un entity
	const door = engine.addEntity()

	// Darle al entity una posición mediante un component Transform
	Transform.create(door, {
		position: Vector3.create(5, 1, 5),
	})

	// Darle al entity una forma visible mediante un component GltfContainer
	GltfContainer.create(door, {
		src: 'assets/models/door.glb',
	})
}
```


> [!WARNING]
> **📔 Nota**: En versiones anteriores del SDK, era necesario añadir manualmente un entity al engine para empezar a renderizarlo. A partir de la versión 7 del SDK, los entities se añaden implícitamente al engine en cuanto se les asigna un component.


Cuando se crea un component, siempre se asigna a un entity padre. Los valores del component entonces afectan al entity.


> [!NOTE]
> **💡 Consejo**: En lugar de crear entities uno por uno, puedes generar un árbol entero de entities y components de una sola vez desde un [composite](/docs/creator/content-creator-es/scenes-sdk7/arquitectura/composites/) .


## Eliminar entities

Para eliminar un entity del engine, usa `engine.removeEntity()`. Esta function devuelve un `booleano`: `true` si se eliminó el entity, `false` si se rechazó la eliminación.

```ts
export function main() {
	// Crear un entity
	const door = engine.addEntity()

	// Darle al entity una forma visible mediante un component GltfContainer
	GltfContainer.create(door, {
		src: 'assets/models/door.glb',
	})

	// Eliminar entity
	const removed = engine.removeEntity(door)
	console.log('Entity eliminado:', removed) // true
}
```

Si un entity eliminado tiene cualquier child entity, estos cambian su parent de vuelta al default `engine.RootEntity` entity, que está posicionado en la posición base de la escena, con una escala de *1*.

### Entities reservados por el renderer

Algunos ids de entity están reservados por el renderer para avatares de players remotos. No puedes eliminar estos entities. Si llamas a `engine.removeEntity()` en un entity reservado por el renderer, devuelve `false` y deja todos los components intactos.

El [entities reservados con nombre](#reserved-entities) (`engine.RootEntity`, `engine.PlayerEntity`, `engine.CameraEntity`) son un caso especial: `engine.removeEntity()` sigue devolviendo `false` para estos (sus ids nunca se liberan), pero sus components **son** se purgan. Esto significa que puedes limpiar tus propios components de ellos (por ejemplo, eliminando un `InputModifier` de `engine.PlayerEntity`), aunque el id del entity en sí nunca se libera.

```ts
// Intentando eliminar un entity reservado
const result = engine.removeEntity(engine.PlayerEntity)
console.log(result) // false — el id del entity no se libera
// Pero los components propios de la escena en PlayerEntity se purgan
```

Puedes comprobar el valor de retorno al eliminar entities para manejar casos especiales:

```ts
if (!engine.removeEntity(someEntity)) {
	console.log('Entity no pudo ser eliminado (reservado)')
}
```

### Eliminar un entity con sus children

Para eliminar un entity y también todos sus children (y cualquier child de sus children, recursivamente), usa el `removeEntityWithChildren()` helper.

```ts
export function main() {
	// Crear parent entity
	const door = engine.addEntity()

	// Crear child entity
	const doorKnob = engine.addEntity()

	// Darles a los entities una forma visible
	GltfContainer.create(door, {
		src: 'models/door.glb',
	})
	GltfContainer.create(doorKnob, {
		src: 'models/doorKnob.glb',
	})

	// Parent
	Transform.create(doorKnob, {
		parent: door,
	})

	// Eliminar tanto al parent como a los children
	removeEntityWithChildren(engine, door)
}
```


> [!WARNING]
> **Nota:** Si un entity reservado por el renderer se encuentra en cualquier lugar del árbol, `removeEntityWithChildren` elimina todos los demás descendientes pero deja el entity reservado en su lugar. El entity reservado `Transform.parent` apuntará a un entity eliminado. Esto solo sucede si tu scene pone un entity reservado como hijo de un entity de la escena, lo cual es poco común.



> [!NOTE]
> **💡 Consejo**: En lugar de eliminar un entity del engine, en algunos casos podría ser mejor hacerlo invisible, por si quieres poder cargarlo de nuevo sin ningún retraso. Ver [Hacer invisible](/docs/creator/content-creator-es/scenes-sdk7/fundamentos-del-contenido-3d/shape-components/#make-invisible)


### Eliminando entities detrás de escena

Un entity es solo un id al que hacen referencia sus components. Así que al eliminar un entity, en realidad estás eliminando cada uno de los components que referencian a este entity. Si eliminas manualmente todos los components de un entity, le parecerá al player igual que hacer `engine.removeEntity()`. Sin embargo, `engine.removeEntity()` también realiza algo de contabilidad interna adicional, marcando el id del entity como ya no usado, así que siempre es la forma recomendada de eliminar un entity.

## Entities anidados

Un entity puede tener otros entities como children. Gracias a esto, podemos organizar entities en árboles, igual que el HTML de una página web.

![](/docs/_remote/28a71287061591ece4e52409.webp)

Para establecer un entity como parent de otro, el child entity debe tener un `Transform` component. Luego puedes establecer el `parent` campo con una referencia al entity parent.

```ts
export function main() {
	// Crear entidades
	const parentEntity = engine.addEntity()

	const childEntity = engine.addEntity()

	// Establecer parent
	Transform.create(childEntity, {
		parent: parentEntity,
	})
}
```

Una vez asignado un parent, se puede leer en el child entity desde el `parent` campo en su `Transform` component.

```ts
// Obtener parent desde un entity
const parent = Transform.get(childEntity).parent
```

Si un entity parent tiene un `Transform` component que afecta a su posición, escala o rotación, sus child entities también se ven afectados. Cualquier valor de posición o rotación se suma, cualquier valor de escala se multiplica.

Si tanto el entity parent como el child entity no tienen un `Transform` component, se usan los siguientes valores predeterminados.

* Para **position**, el centro del parent es *0, 0, 0*
* Para **rotation** la rotación del parent es el quaternion *0, 0, 0, 1* (equivalente a los ángulos de Euler *0, 0, 0*)
* Para **scale**, el parent se considera que tiene un tamaño de *1*. Cualquier cambio de tamaño del parent afecta a la escala y la posición en proporción.

Los entities sin component de shape son invisibles en la escena. Estos pueden usarse como envoltorios para manejar y posicionar varios entities como un grupo.

Para separar un child entity de su parent, puedes asignar el parent del entity a `engine.RootEntity`.

```ts
const mutableChildTransform = Transform.getMutable(childEntity)
mutableChildTransform.parent = engine.RootEntity
```


> [!WARNING]
> **📔 Nota**: Cuando trabajes con entities anidados que están sincronizados con otros players, usa el `parentEntity()` function en lugar de la `parent` entity en el Transform. Ver [Entities parentadas](/docs/creator/content-creator-es/scenes-sdk7/networking/serverless-multiplayer/#parented-entities)


En el Scene Editor, puedes ver toda la jerarquía de entities anidados en tu scene en el panel del lado izquierdo.

![](/docs/_remote/87863329d94332c21873d9dd.webp)

## Obtener un entity por ID

Cada entity en tu scene tiene un número único *id*. Puedes recuperar un component que se refiera a un entity específico del engine basándote en este ID.

```typescript
// obtener un component Transform
Transform.get(1000 as Entity)
```


> [!WARNING]
> **📔 Nota**: Los ids de entity entre *0* y *511* están reservados por el engine para entities fijos, como el avatar del player, la scene base, etc.


Por ejemplo, si el click de un player o un [raycast](/docs/creator/content-creator-es/scenes-sdk7/interactividad/raycasting/) golpea un entity, esto devolverá el id del entity golpeado, y puedes usar el comando anterior para obtener el component Transform del entity que coincida con ese id. También puedes obtener cualquier otro component de ese entity de la misma forma.

## Obtener un entity por nombre

Cuando añades entities mediante arrastrar y soltar en el Scene Editor, cada entity tiene un nombre único. Usa la `engine.getEntityOrNullByName()` function para referenciar uno de estos entities desde tu código. Pasa el nombre del entity como una string, tal como aparece en la UI del Scene Editor, en la vista de árbol de la izquierda.

```ts
function main() {
	const door = engine.getEntityOrNullByName('door3')
}
```


> [!WARNING]
> **📔 Nota**: Asegúrate de usar solo `engine.getEntityOrNullByName()` dentro de la `main()` function, en functions que se ejecutan después de `main()`, o en un system. Si se usa fuera de uno de esos contextos, los entities creados en la UI del Scene Editor pueden que aún no estén instanciados.


Eres libre de realizar cualquier acción sobre un entity obtenido mediante este método, como añadir o eliminar components, modificar valores de components existentes, o eliminar el entity del engine.

```ts
function main() {
	// obtener entity
	const door = engine.getEntityOrNullByName('door-3')
	// verificar que el entity existe
	if (door) {
		// añadir un callback de pointer events
		pointerEventsSystem.onPointerDown(
			{
				entity: door,
				opts: { button: InputAction.IA_PRIMARY, hoverText: 'Abrir' },
			},
			function () {
				// abrir puerta
			}
		)
	}
}
```

Todos los entities añadidos mediante la UI del Scene Editor tienen un `Name` component, puedes iterar sobre todos ellos así:

```ts
function main() {
	for (const [entity, name] of engine.getEntitiesWith(Name)) {
		console.log({ entity, name })
	}
}
```

## Añadir o reemplazar un component

Cada entity solo puede tener un component de un tipo dado. Por ejemplo, si intentas asignar un Transform a un entity que ya tiene uno, esto causará un error.

Para evitar este error, puedes usar `.createOrReplace` en lugar de `.create`. Este comando sobrescribe cualquier component existente del mismo tipo si existe, de lo contrario crea un nuevo component igual que `.create`.

```ts
Transform.createOrReplace(door, {
	position: Vector3.create(5, 1, 5),
})
```


> [!WARNING]
> **📔 Nota**: Dado que `.createOrReplace` realiza una comprobación adicional antes de crear el component, siempre es más eficiente usar `.create`. Si estás seguro de que el entity aún no tiene un component como el que estás añadiendo, usa `.create`.


## Acceder a un component desde un entity

Puedes acceder a los components de un entity usando la `.get()` o el `getMutable()` funciones.

```ts
export function main() {
	// Crear entity
	const box = engine.addEntity()

	// Crear y añadir el component a ese entity
	Transform.create(box)

	// Obtener la versión de solo lectura del component
	let transform = Transform.get(box)

	// Obtener la versión mutable del component
	let transform = Transform.getMutable(box)
}
```

El `método has()` function obtiene una referencia de solo lectura al component. No puedes cambiar ningún valor desde esta referencia del component.

Si deseas cambiar los valores del component, usa la `getMutable()` function en su lugar. Si cambias los valores en la versión mutable del component, estás afectando directamente al entity al que pertenece ese component.

Consulta [datos mutables](/docs/creator/content-creator-es/scenes-sdk7/patrones-de-programacion/mutable-data/) para más detalles.


> [!WARNING]
> **📔 Nota**: Usa `getMutable()` solo si realmente vas a hacer cambios en los valores del component. De lo contrario, usa siempre `método has()`. Esta práctica sigue los principios de [programación orientada a datos](/docs/creator/content-creator-es/scenes-sdk7/arquitectura/data-oriented-programming/), y puede ayudar significativamente al rendimiento de la escena.


```ts
// Obtener la versión mutable del component
let transform = Transform.getMutable(box)

// cambiar un valor del component
transform.scale.x = 5
```

El ejemplo de arriba modifica directamente el valor de *x* scale en el component Transform.

Si no estás completamente seguro de que el entity tenga el component que intentas recuperar, usa `getOrNull()` o `getMutableOrNull()`.


> [!WARNING]
> **📔 Nota**: Evita usar `getOrNull()` o `getMutableOrNull()` cuando sea posible, ya que estas functions implican comprobaciones adicionales y por lo tanto son menos eficientes que `.get()` y `getMutable()`.


```ts
//  getOrNull
const transformOrNull = Transform.getOrNull(myEntity)

//  getMutableOrNull
const mutableTransformOrNull = Transform.getMutableOrNull(myEntity)
```

Si el component que intentas recuperar no existe en el entity:

* `método has()` y `getMutable()` devuelve un error.
* `getOrNull()` y `getMutableOrNull()` devuelve `Null`.

## Eliminar un component de un entity

Para eliminar un component de un entity, usa el `deleteFrom()` método del tipo de component.

```ts
Transform.deleteFrom(myEntity)
```

Si intentas eliminar un component que no existe en el entity, esta acción no generará ningún error.


> [!WARNING]
> **📔 Nota**: Para eliminar todos los components de un entity a la vez, ver [esta sección](#remove-entities)


## Comprobar un component

Puedes comprobar si un entity tiene una instancia de cierto component usando la `has()` function. Esta function devuelve *true* si el component está presente, y *false* si no lo está. Esto puede ser muy útil para usarlo en lógica condicional en tu scene.

```ts
const hasTransform = Transform.has(myEntity)
```


> [!NOTE]
> **💡 Consejo**: También puedes [consultar components](/docs/creator/content-creator-es/scenes-sdk7/arquitectura/querying-components/) para obtener una lista completa de components que contienen un component específico, o un conjunto específico de components. No iteres manualmente sobre todos los entities de la escena para comprobar cada uno con un `has()`, ese enfoque es mucho menos eficiente.


## Comprobar cambios en un component

Usa la `onChange` function para ejecutar una función callback cada vez que los valores del component cambien para un entity dado. Esto funciona con cualquier component, y es un atajo genial para ayudarte a mantener tu código legible.

La función callback puede incluir un parámetro de entrada que contiene el nuevo estado del component.

```ts
Transform.onChange(cubeEntity, (newTransform) => {
	if (!newTransform) return
	console.log(
		'La posición del Cube cambió: ',
		newTransform.position,
		newTransform.rotation
	)
})

VisibilityComponent.onChange(cubeEntity, (newVisibilityComponent) => {
	if (!newVisibilityComponent) return
	console.log('La visibilidad del Cube cambió: ', newVisibilityComponent.visible)
})
```

Si el component se elimina del entity, entonces la function se llama con una entrada de `undefined`.


> [!NOTE]
> **💡 Consejo**: El `.onChange()` function funciona tanto con native components del SDK como con [custom components](/docs/creator/content-creator-es/scenes-sdk7/arquitectura/custom-components/) definidos por el creator.


## Obtener child entities

Para acceder a todos los entities que son children directos de un entity parent, usa `getEntitiesWithParent`. Toma como argumentos el `engine` el `parent` entity y devuelve una lista de todos los entities que tienen ese entity particular como parent. Ten en cuenta que solo devuelve children directos, no children de children.

```ts
import { getEntitiesWithParent } from '@dcl/sdk/ecs'

const children = getEntitiesWithParent(engine, myEntity)
for (const child of children) {
   // procesar cada child entity
}
```

Para acceder en su lugar a todos los descendants de un entity, sin importar lo profundamente anidados que estén, usa la function `getComponentEntityTree()`. En lugar de recorrer manualmente la jerarquía nivel por nivel, esta function devuelve una lista plana de todos los descendants que es fácil de iterar. También filtra solo los entities que tienen un component dado o una lista de components.

```ts
import { getComponentEntityTree } from '@dcl/sdk/ecs'

export function main() {
	// Crear un entity parent con children anidados
	const parentEntity = engine.addEntity()
	Transform.create(parentEntity, {
		position: Vector3.create(8, 0, 8),
	})

	// ... asume que el parent tiene múltiples children y grandchildren

	// Iterar sobre todos los descendants del entity parent
	for (const descendantEntity of getComponentEntityTree(
		engine,
		parentEntity,
		Transform
	)) {
		// Acceder a cada descendant entity
		const transform = Transform.get(descendantEntity)
		console.log('Posición del descendant:', transform.position)
	}
}
```

El `getComponentEntityTree` function toma tres parámetros:

* `engine`: La instancia del engine que ejecuta los entities
* `entity`: El root entity desde el que empezar
* `component`: El component por el que filtrar (normalmente `Transform` para jerarquías espaciales)

La function devuelve un generator que produce cada descendant entity en la estructura del árbol. Solo los entities que tengan el component especificado se incluirán en los resultados.

Puedes combinar esto con otras comprobaciones de components para encontrar entities específicas en tu jerarquía:

```ts
// Encontrar todos los descendants con un nombre específico
for (const descendantEntity of getComponentEntityTree(
	engine,
	parentEntity,
	Transform
)) {
	const name = Name.getOrNull(descendantEntity)
	if (name && name.value === 'targetEntity') {
		console.log('Entity objetivo encontrado:', descendantEntity)
	}
}
```

## Entities reservadas

Ciertos ids de entity están reservados para entities especiales que existen en cada scene. Se puede acceder a ellos mediante los siguientes alias:

* `engine.RootEntity`
* `engine.PlayerEntity`
* `engine.CameraEntity`


> [!WARNING]
> **📔 Nota**: Evita referirte a estos entities antes de que se inicialicen. Para evitar este problema, refiérete a estos entities en la `main()` function, o en un system.


## El root entity

Todos los entities de la scene son children del `engine.RootEntity`, directamente o indirectamente.

Este entity no tiene un component Transform, pero se usa para manejar varios components que representan ajustes más globales, como [control del skybox](/docs/creator/content-creator-es/scenes-sdk7/interactividad/skybox-control/), [posición del cursor](/docs/creator/content-creator-es/scenes-sdk7/interactividad/user-data/#check-the-players-cursor-position), o [dimensiones de la pantalla](/docs/creator/content-creator-es/scenes-sdk7/ui-2d/ui-positioning/#responsive-ui-size).

## El player entity

El `engine.PlayerEntity` entity representa el avatar del player.

Obtener el `Transform` component del player para obtener la posición y rotación actuales del player, ver [datos del usuario](/docs/creator/content-creator-es/scenes-sdk7/interactividad/user-data/). El Transform del player es de solo lectura; para modificarlo usa la `movePlayerTo()` function, [saber más](/docs/creator/content-creator-es/scenes-sdk7/interactividad/avatares/move-player/).

También puedes adjuntar objetos al player estableciéndolos como children de este entity, aunque la [Attach to Player](/docs/creator/content-creator-es/scenes-sdk7/fundamentos-del-contenido-3d/entity-positioning/#attach-an-entity-to-an-avatar) suele ser la mejor opción para eso.

## El camera entity

El `engine.CameraEntity` entity representa la camera del player.

Obtener el `Transform` component de la camera para obtener la posición y rotación de la camera. El Transform de este entity también es de solo lectura. Para modificar el ángulo o la posición de la camera, usa un [Virtual camera](/docs/creator/content-creator-es/scenes-sdk7/fundamentos-del-contenido-3d/camera/#using-virtual-cameras).

También puedes obtener el `CameraMode` component de la camera para saber si el player está usando el modo de camera en primera o tercera persona, ver [modo de camera](/docs/creator/content-creator-es/scenes-sdk7/interactividad/user-data/#check-the-players-camera-mode).


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-es/scenes-sdk7/arquitectura/entities-components.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
