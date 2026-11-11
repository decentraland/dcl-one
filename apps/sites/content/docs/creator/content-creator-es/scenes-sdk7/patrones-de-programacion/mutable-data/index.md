> Aprende cómo manejar datos de solo lectura y mutables de los componentes

# Datos mutables

Cuando se hace referencia a datos de un [component](/docs/creator/content-creator-es/scenes-sdk7/arquitectura/entities-components/), puedes obtener tanto la versión mutable como la de solo lectura (inmutable).

Siempre deberías trabajar con las versiones de solo lectura de los datos cuando sea posible. Esta práctica puede traer una mejora muy significativa en el rendimiento de tu scene, en comparación con trabajar siempre con versiones mutables de esos mismos datos.

El `.get()` La función en un component devuelve una versión de solo lectura (inmutable) del component. Solo puedes leer sus valores, pero no puedes cambiar ninguna de sus propiedades.

El `.getMutable()` devuelve una versión del component que te permite cambiar sus valores. Usa versiones mutables solo cuando planees hacer cambios en un component; de lo contrario, usa siempre `método has()`.

```ts
// obtener una versión de solo lectura (inmutable)
const immutableTransform = Transform.get(myEntity)

// lo siguiente NO funciona:
// immutableTransform.position.y = 2

const mutableTransform = Transform.getMutable(myEntity)

// la siguiente línea SÍ cambia la posición de la entidad
mutableTransform.position.y = 2
```

Una buena práctica es iterar sobre components de solo lectura para comprobar valores, y luego obtener la versión mutable de un component individual solo cuando se requiera un cambio.

```ts
// altura máxima codificada
const MAX_HEIGHT = 10

// Definir el system
function HeightLimitSystem(dt: number) {
	// iterar sobre todas las entities que tienen un component Transform
	for (const [entity] of engine.getEntitiesWith(Transform)) {
		// obtener valores de solo lectura
		const currentHeight = Transform.get(entity).position.y

		// comparar valores
		if (currentHeight > MAX_HEIGHT) {
			// obtener la versión mutable para hacer un cambio
			const mutableTransform = Transform.getMutable(entity)

			// cambiar Transform
			mutableTransform.position.y = MAX_HEIGHT
		}
	}
}

// Añadir system al engine
engine.addSystem(HeightLimitSystem)
```

En el ejemplo anterior, un system comprueba los valores de solo lectura de una entity's `Transform` component. En cada tick comprueba si la posición *y* es mayor que una altura máxima codificada. Si la altura en el Transform resulta estar por encima de este límite, entonces y solo entonces obtenemos la versión mutable de Transform. Esto puede parecer trabajo extra para la scene, pero en una scene en la que comprobamos valores en cada tick del game loop, y solo hacemos cambios ocasionalmente, se traduce en enormes mejoras de rendimiento.

Esta práctica sigue los principios de [programación orientada a datos](/docs/creator/content-creator-es/scenes-sdk7/arquitectura/data-oriented-programming/). También se está adoptando gradualmente como una práctica estándar de la industria en el gaming, debido a la gran mejora que supone.


> [!WARNING]
> **📔 Nota**: En versiones anteriores del SDK (6.x o anteriores), los components siempre se trataban como mutables. Ese patrón puede ser un poco más sencillo de aprender, pero era mucho menos eficiente de ejecutar.



---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-es/scenes-sdk7/patrones-de-programacion/mutable-data.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
