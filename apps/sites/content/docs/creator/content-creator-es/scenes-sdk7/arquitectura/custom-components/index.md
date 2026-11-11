> Crea un componente personalizado para manejar datos específicos relacionados con una entity

# Componentes personalizados

Los datos de un Entity se almacenan en su [components](/docs/creator/content-creator-es/scenes-sdk7/arquitectura/entities-components/). El SDK de Decentraland proporciona una serie de Components base que gestionan distintos aspectos de un Entity, como su posición, forma, material, etc. El engine sabe cómo interpretar la información en estos, y cambiará cómo se renderiza el Entity en consecuencia en cuanto cambien sus valores.

Si la lógica de tu Scene requiere almacenar información sobre un Entity que no es manejada por los Components predeterminados del SDK, entonces puedes crear un tipo personalizado de Component en tu Scene. Luego puedes construir [systems](/docs/creator/content-creator-es/scenes-sdk7/arquitectura/systems/) que verifiquen cambios en estos Components y respondan en consecuencia.

## Acerca de definir Components

Para definir un nuevo Component, usa `engine.defineComponent`. Cada Component necesita lo siguiente:

* Un **componentName**: Un identificador de cadena único que el SDK usa internamente para identificar este tipo de Component. Puede ser cualquier cadena, siempre que sea única.
* A **schema**: Una clase que define la estructura de datos que contiene el Component.
* **valores predeterminados** *(opcional)*: Un objeto que contiene valores predeterminados para usar al inicializar una copia del Component, cuando estos no se proporcionan.

```ts
export const WheelSpinComponent = engine.defineComponent('wheelSpinComponent', {
	spinning: Schemas.Boolean,
	speed: Schemas.Float,
})
```


> [!WARNING]
> **📔 Nota**: Los Custom Components deben escribirse siempre fuera del `main()` función, en un archivo aparte. Deben interpretarse antes de que `main()` se ejecuta. El lugar recomendado para esto es en una `/components` carpeta dentro de `/src`, cada uno en su propio archivo. Así es más fácil reutilizarlos en proyectos futuros.


Una vez que hayas definido un Component personalizado, puedes crear instancias de este Component, que hacen referencia a Entities en la Scene. Cuando creas una instancia de un Component, proporcionas valores para cada uno de los campos del schema del Component. Los valores deben cumplir con los tipos declarados de cada campo.

```ts
// Crear entidades
const wheel1 = engine.addEntity()
const wheel2 = engine.addEntity()

// Crear instancias del Component
WheelSpinComponent.create(wheel1, {
	spinning: true,
	speed: 10,
})

WheelSpinComponent.create(wheel2, {
	spinning: false,
	speed: 0,
})
```

Cada Entity a la que se le añade el Component instancia una nueva copia del Component, que contiene datos específicos para ese Entity.

Tu Component personalizado también puede realizar las otras funciones comunes disponibles en otros Components:

```ts
// Obtener una instancia de solo lectura del Component desde un Entity
const readOnlyInstance = MyCustomComponent.get(myEntity)

// Obtener una instancia mutable del Component desde un Entity
const mutableInstance = MyCustomComponent.getMutable(myEntity)

// Eliminar la instancia del Component de un Entity
MyCustomComponent.deleteFrom(myEntity)
```

## Acerca de componentName

Cada Component debe tener un nombre o identificador único de Component, que lo diferencie internamente. No necesitarás usar este identificador interno en ningún otro lugar de tu código. Una buena práctica es usar el mismo nombre que asignas al Component, pero comenzando con una letra minúscula, aunque lo que realmente importa es que este identificador sea único dentro del proyecto.

Al crear Components que se compartirán como parte de una library, ten en cuenta que los nombres de Components de tu library no deben superponerse con ningún nombre de Component en el proyecto donde se use, ni con otras libraries que también use ese proyecto. Para evitar el riesgo de cualquier superposición, la mejor práctica recomendada es incluir el nombre de la library como parte de la `componentName` cadena. Puedes seguir esta fórmula: `${packageName}::${componentName}`. Por ejemplo, si construyes una`MyUtilities` library que incluye un `MoveEntity` Component, establece el `componentName` de ese Component en `MyUtilities::moveEntity`.

## Components como banderas

Puede que quieras añadir un Component que simplemente marque un Entity para diferenciarlo de otros, sin usarlo para almacenar ningún dato. Para hacerlo, deja el schema como un objeto vacío.

Esto es especialmente útil al usar [consultar Components](/docs/creator/content-creator-es/scenes-sdk7/arquitectura/querying-components/). Un simple Component de bandera puede usarse para distinguir Entities de otras, y evitar que el system itere sobre más Entities de las necesarias.

```ts
export const IsEnemyFlag = engine.defineComponent('isEnemyFlag', {})
```

Luego puedes crear un System que itere sobre todos los Entities que tengan este Component.

```ts
export function handleEnemies() {
	for (const [entity] of engine.getEntitiesWith(IsEnemyFlag)) {
		// hacer algo en cada Entity
	}
}

engine.addSystem(handleEnemies)
```

## Schemas de Components

Un schema describe la estructura de los datos dentro de un Component. Un Component puede almacenar tantos campos como quieras, y cada uno debe incluirse en la estructura del schema. El schema puede incluir tantos niveles de elementos anidados como necesites.

Cada campo del schema debe incluir una declaración de tipo. Solo puedes usar los tipos especiales de schema proporcionados por el SDK. Por ejemplo, usa el tipo `Schemas.Boolean` en lugar del tipo `booleano`. Escribe `Schemas.` y tu IDE mostrará todas las opciones disponibles.

```ts
export const WheelSpinComponent = engine.defineComponent('WheelSpinComponent', {
	spinning: Schemas.Boolean,
	speed: Schemas.Float,
})
```

El ejemplo anterior define un Component cuyo schema contiene dos valores: un `spinning` booleano y un `speed` número de punto flotante.

Puedes elegir crear el schema en línea al definir el Component, o para mayor legibilidad puedes crearlo y luego referenciarlo.

```ts
// Opción 1: definición en línea
export const WheelSpinComponent = engine.defineComponent('WheelSpinComponent', {
	spinning: Schemas.Boolean,
	speed: Schemas.Float,
})

// Opción 2: definir el schema y el Component por separado

//// schema
const mySchema = {
	spinning: Schemas.Boolean,
	speed: Schemas.Float,
}

//// Component
export const WheelSpinComponent = engine.defineComponent(
	'WheelSpinComponent',
	mySchema
)
```


> [!NOTE]
> **💡 Consejo**: Al crear una instancia de un Component, las opciones de autocompletado de VS Studio te sugerirán qué campos puedes añadir al Component al pulsar *Ctrl + Space*.


### Tipos de schema predeterminados

Los siguientes tipos básicos están disponibles para usarse dentro de los campos de un schema:

* `Schemas.Boolean`
* `Schemas.Byte`
* `Schemas.Double`
* `Schemas.Float`
* `Schemas.Int`
* `Schemas.Int64`
* `Schemas.Number`
* `Schemas.Short`
* `Schemas.String`
* `Schemas.Entity`

También existen los siguientes tipos complejos. Cada uno incluye una serie de propiedades anidadas con valores numéricos.

* `Schemas.Vector3`
* `Schemas.Quaternion`
* `Schemas.Color3`
* `Schemas.Color4`


> [!NOTE]
> **💡 Consejo**: Ver [Tipos de geometría](/docs/creator/content-creator-es/scenes-sdk7/fundamentos-del-contenido-3d/special-types/) y [Tipos de color](/docs/creator/content-creator-es/scenes-sdk7/fundamentos-del-contenido-3d/color-types/) para más detalles sobre cómo son útiles estos tipos de datos.


Por ejemplo, puedes usar estos tipos de schema en un Component como este para rastrear el movimiento gradual de un Entity. Este Component almacena una posición inicial y una final como valores Vector3, así como una velocidad y una fracción del recorrido completado como números float. Consulta [Mover ítems](/docs/creator/content-creator-es/scenes-sdk7/fundamentos-del-contenido-3d/move-entities/#move-between-two-points) para ver la implementación completa de este ejemplo.

```ts
const MoveTransportData = {
	start: Schemas.Vector3,
	end: Schemas.Vector3,
	fraction: Schemas.Float,
	speed: Schemas.Float,
}

export const LerpTransformComponent = engine.defineComponent(
	'LerpTransformComponent',
	MoveTransportData
)
```

### Tipos de array

Para establecer el tipo de un campo como un array, usa `Schemas.Array()`. Pasa el tipo de los elementos del array como una propiedad.

```ts
const MySchema = {
	numberList: Schemas.Array(Schemas.Int),
}
```

Cuando vuelvas a leer un campo array con `MyComponent.get()`, el array es de solo lectura. Los métodos que lo cambian in situ, como `.push()`, no están disponibles. Usa `MyComponent.getMutable()` cuando necesites cambiar el contenido.

### Campos opcionales

Usa `Schemas.Optional()` para permitir que un campo contenga un valor o `undefined`.

```ts
const MySchema = {
	playerId: Schemas.Optional(Schemas.String),
	score: Schemas.Optional(Schemas.Int),
}
```

Solo `undefined` se considera «no establecido». Los valores falsy como `false`, `0`, y `''` se almacenan y se leen exactamente tal como están escritos.

### Tipos de schema anidados

Para establecer que el tipo de un campo sea un objeto, usa `Schemas.Map()`. Pasa el contenido de este objeto como una propiedad. Este objeto anidado es esencialmente un schema en sí mismo, anidado dentro del schema padre.

```ts
const MySchema = {
	simpleField: Schemas.Boolean,
	myComplexField: Schemas.Map({
		nestedField1: Schemas.Boolean,
		nestedField2: Schemas.Boolean,
	}),
}
```

Como alternativa, para que sea más legible y reutilizable, podrías lograr lo mismo definiendo el schema anidado por separado y luego referenciándolo al definir el schema padre.

```ts
const MyNestedSchema = Schemas.Map({
	nestedField1: Schemas.Boolean,
	nestedField2: Schemas.Boolean,
})

const MySchema = {
	simpleField: Schemas.Boolean,
	myComplexField: MyNestedSchema,
}
```

### Tipos de enum

Puedes establecer que el tipo de un campo en un schema sea un enum. Los enums facilitan seleccionar entre un número finito de opciones, proporcionando valores legibles para cada una.

Para establecer que el tipo de un campo sea un enum, primero debes definir el enum. Luego puedes referirte a él usando `Schemas.EnumNumber` o `Schemas.EnumString`, según el tipo de enum. Estas funciones toman dos parámetros: el enum al que se hace referencia y un valor predeterminado para usar en este campo.

```ts
//// enum de String

// Definir enum
enum Color {
	Red = 'red',
	Green = 'green',
	Pink = 'pink',
}

// Definir un Component que usa este enum en un campo
const ColorComponent = engine.defineComponent('Color', {
	color: Schemas.EnumString<Color>(Color, Color.Red),
})

// Usar el Component en un Entity
ColorComponent.create(engine.addEntity(), { color: Color.Green })

//// enum de número

// Definir enum
enum CurveType {
	LINEAR,
	EASEIN,
	EASEOUT,
}

// Definir un Component que usa este enum en un campo
const CurveComponent = engine.defineComponent('curveComponent', {
	curve: Schemas.EnumNumber<CurveType>(CurveType, CurveType.LINEAR),
})

// Usar el Component en un Entity
CurveComponent.create(engine.addEntity(), { curve: CurveType.EASEIN })
```

### Tipos intercambiables

Puedes establecer que el tipo de un campo en un schema siga un `oneOf` patrón oneOf, donde se pueden aceptar diferentes tipos.

```ts
const MySchema = {
	myField: Schemas.OneOf({ type1: Schemas.Vector3, type2: Schemas.Quaternion }),
}

export const MyComponent = engine.defineComponent('MyComponent', MySchema)
```

Al crear una instancia del Component, necesitas especificar el tipo seleccionado con un `$case`, por ejemplo:

```ts
MyComponent.create(myEntity, {
	myField: {
		$case: 'type1',
		value: Vector3.create(1, 1, 1),
	},
})
```

Dejar el campo sin establecer también es válido. Un `OneOf` campo OneOf no tiene `$case` y se lee de nuevo como un objeto vacío, `{}`.

### Components de un solo tipo

Un Component no tiene por qué contener un objeto con varios campos. Para definir uno que contenga un solo valor, usa `engine.defineComponentFromSchema()` y pasa el tipo directamente:

```ts
// Un Component que contiene un número por Entity
export const Score = engine.defineComponentFromSchema('my-scene::Score', Schemas.Int)

// Un Component que contiene una lista de números por Entity
export const History = engine.defineComponentFromSchema(
	'my-scene::History',
	Schemas.Array(Schemas.Int)
)
```

Estos se comportan como cualquier otro Component, incluso cuando el valor almacenado es falsy. Un `Score` de `0` es un Component que existe y contiene `0`, no un Component ausente.

## Valores predeterminados

Suele ser útil tener valores predeterminados en tus Components, para que no sea necesario establecer explícitamente cada valor cada vez que creas una nueva copia.

El `engine.defineComponent()` función recibe un tercer argumento, que te permite pasar un objeto con valores para usar de forma predeterminada. Este objeto puede incluir algunos o todos los valores del schema. Los campos que no estén cubiertos por los valores predeterminados o por los valores que proporciones al inicializar una copia del Component se inicializan con un valor similar a cero, como `0`, `false`, o una cadena vacía, según el tipo.

```ts
// Definición

//// schema
const mySchema = {
	spinning: Schemas.Boolean,
	speed: Schemas.Float,
}

//// valores predeterminados
const myDefaultValues = {
	spinning: true,
	speed: 1,
}

//// Component
export const WheelSpinComponent = engine.defineComponent(
	'WheelSpinComponent',
	mySchema,
	myDefaultValues
)

// Uso
export function main() {
	//// Crear Entities
	const wheel = engine.addEntity()
	const wheel2 = engine.addEntity()

	//// inicializar el Component usando valores predeterminados
	WheelSpinComponent.create(wheel)

	//// inicializar el Component con un valor personalizado, usando el predeterminado para los demás
	WheelSpinComponent.create(wheel2, { speed: 5 })
}
```

El ejemplo anterior crea un `WheelSpinComponent` Component que incluye tanto un schema como un conjunto de valores predeterminados para usar. Si luego inicializas una copia de este Component sin especificar ningún valor, usará los establecidos en el predeterminado.

## Suscríbete a los cambios

Un caso de uso común es ejecutar una function solo si los datos de un cierto component cambian. Usa la [OnChange](/docs/creator/content-creator-es/scenes-sdk7/arquitectura/subscribe-to-changes/) function para evitar tener que definir un system y tener que comparar explícitamente los valores antiguos con los nuevos.

```ts
export function main() {
	//Crear Entity, etc

	WheelSpinComponent.onChange(myEntity, (componentData) => {
		if (!componentData) return
		console.log(componentData.speed)
		console.log(componentData.spinning)
	})
}
```

## Construir Systems para usar un Component

Con tu Component definido y añadido a Entities en tu Scene, puedes crear [Systems](/docs/creator/content-creator-es/scenes-sdk7/arquitectura/systems/) para realizar lógica, aprovechando estos datos almacenados en el Component.

```ts
// definir Component
export const WheelSpinComponent = engine.defineComponent('WheelSpinComponent', {
	spinning: Schemas.Boolean,
	speed: Schemas.Float,
})

// Uso
export function main() {
	// Crear entidades
	const wheel1 = engine.addEntity()
	const wheel2 = engine.addEntity()

	// Crear instancias del Component
	WheelSpinComponent.create(wheel1, {
		spinning: true,
		speed: 10,
	})

	WheelSpinComponent.create(wheel2, {
		spinning: false,
		speed: 0,
	})
}

// Definir un System para iterar sobre estos Entities
export function spinSystem(dt: number) {
	// iterar sobre todos los Entities con un WheelSpinComponent
	for (const [entity, wheelSpin] of engine.getEntitiesWith(
		WheelSpinComponent
	)) {
		// hacer algo solo si spinning == true
		if (wheelSpin.spinning) {
			// obtener un componente Transform mutable
			const transform = Transform.getMutable(entity)

			// actualizar el valor de rotación en consecuencia
			transform.rotation = Quaternion.multiply(
				transform.rotation,
				Quaternion.fromAngleAxis(dt * wheelSpin.speed, Vector3.Up())
			)
		}
	}
}

// Añadir system al engine
engine.addSystem(spinSystem)
```

El ejemplo anterior define un System que itera sobre todos los Entities que incluyen el Component personalizado `wheelSpinComponent`, y los rota ligeramente en cada tick del bucle del juego. La magnitud de esta rotación es proporcional al `speed` valor almacenado en la instancia del Component de cada Entity. El ejemplo hace uso de [consultas de Components](/docs/creator/content-creator-es/scenes-sdk7/arquitectura/querying-components/) para obtener solo los Entities relevantes.


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-es/scenes-sdk7/arquitectura/custom-components.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
