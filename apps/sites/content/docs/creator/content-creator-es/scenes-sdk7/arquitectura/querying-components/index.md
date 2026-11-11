> Aprende cómo obtener listas de entidades que tienen Components en común, para facilitar su comprobación o actualización.

# Consultando Components

Puedes [consultar componentes](/docs/creator/content-creator-es/scenes-sdk7/arquitectura/querying-components/) con el método `engine.getEntitiesWith(...components)` para llevar un registro de todas las entidades de la escena que tienen ciertos componentes.

[Sistemas](/docs/creator/content-creator-es/scenes-sdk7/arquitectura/systems/) normalmente iteran sobre las entidades de estas consultas, realizando las mismas operaciones en cada una. Tener un grupo predefinido de entidades válidas es una excelente manera de ahorrar recursos, especialmente para funciones que se ejecutan en cada tick del bucle del juego. Si en cada tick tu sistema tuviera que iterar sobre cada una de las entidades de la escena buscando las que necesita, eso sería muy ineficiente.

Puedes acceder a las entidades de una consulta de la siguiente manera.

```ts
for (const [entity] of engine.getEntitiesWith(Transform)) {
	//...
}
```

## Componentes requeridos

Al hacer una consulta, especifica qué componentes deben estar presentes en cada entidad que se añade al grupo. Puedes listar tantos componentes como quieras; la consulta solo devolverá entidades que tengan **todos** de los componentes enumerados.

```ts
for (const [entity] of engine.getEntitiesWith(
	Transform,
	Physics,
	NextPosition
)) {
	//...
}
```


> [!NOTE]
> **💡 Consejo**: Si tu consulta devuelve entidades con las que no necesitas tratar, considera crear un componente personalizado para que actúe como una [bandera](/docs/creator/content-creator-es/scenes-sdk7/arquitectura/custom-components/#components-as-flags). Este componente no necesita tener ninguna propiedad, pero puede utilizarse para marcar un subgrupo específico de entidades al que quizá quieras tratar de forma diferente.


## Usa consultas en un sistema

```ts
// Define un Sistema
function PhysicsSystem(dt: number) {
	// consulta entidades que incluyan tanto un componente Transform como un componente Physics
	for (const [entity] of engine.getEntitiesWith(Transform, Physics)) {
		const transform = Transform.getMutable(entity)
		const vel = Physics.get(entity).velocity
		transform.position.x += vel.x
		transform.position.y += vel.y
		transform.position.z += vel.z
	}
}

// Añade el sistema al engine
engine.addSystem(PhysicsSystem)
```

En el ejemplo anterior, la `PhysicsSystem` función recorre las entidades de la consulta, que se ejecuta en cada tick del bucle del juego.

* Si la escena tiene varias *ball* entidades, cada una con un `Transform`  y una `Physics` componente, entonces se procesarán y su posición se actualizará en cada tick.
* Si tu escena también tiene otras entidades, por ejemplo una *hoop*  y una *scoreBoard* que solo tengan un `Transform` pero no un `Physics` componente, entonces no se verán afectados por `PhysicsSystem`.

## Trabajar con las entidades y los componentes

La sección `getEntitiesWith` la función devuelve una colección que incluye referencias a un conjunto de entidades y también puede incluir opcionalmente referencias a los componentes enumerados.

Usando la sintaxis más simple, obtienes solo una lista de referencias a las entidades correspondientes.

```ts
for (const [entity] of engine.getEntitiesWith(myComponent, myOtherComponent)) {
	//...
}
```

Mientras iteras sobre esta lista de entidades, luego puedes obtener versiones de solo lectura o mutables de sus componentes usando `.get` o `getMutable`.

```ts
for (const [entity] of engine.getEntitiesWith(Transform)) {
	// obtener versión de solo lectura
	const transformReadOnly = Transform.get(entity)

	// obtener versión mutable
	const transformMutable = Transform.getMutable(entity)
}
```

También puedes obtener opcionalmente referencias a cada uno de los componentes enumerados directamente como parte de la colección devuelta por la consulta. Para hacerlo, simplemente declara varias referencias juntas, una por cada componente que quieras obtener. Añadir estas referencias es opcional, y no necesitas declarar referencias a *todos* los componentes en la consulta tampoco.

```ts
// devuelve referencias a la entidad y al primer componente listado
for (const [entity, component1] of engine.getEntitiesWith(
	MyCustomComponent1,
	MyCustomComponent2
)) {
	// iterar sobre la lista de entidades
}

// devuelve referencias a la entidad y a los dos primeros componentes listados
for (const [entity, component1, component2] of engine.getEntitiesWith(
	MyCustomComponent1,
	MyCustomComponent2
)) {
	// iterar sobre la lista de entidades
}
```


> [!WARNING]
> **📔 Nota**: Estas referencias son de solo lectura. Para obtener versiones mutables de esos componentes, necesitas usar la `.getMutable` función que hace referencia a la entidad.


Luego puedes referirte a estas referencias mientras iteras sobre la colección de resultados; en cada entrada tendrás acceso a la entidad y a sus referencias de componentes correspondientes.

```ts
for (const [entity, transformReadOnly] of engine.getEntitiesWith(Transform)) {
	console.log('entity id: ', entity)
	console.log('has position : ', transformReadOnly.position)
}
```

## Suscríbete a los cambios

Un caso de uso común es ejecutar una función solo si cambian los datos de un determinado componente. Usa la [OnChange](/docs/creator/content-creator-es/scenes-sdk7/arquitectura/subscribe-to-changes/) función para evitar tener que definir un sistema y comparar explícitamente los valores antiguos con los nuevos.


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-es/scenes-sdk7/arquitectura/querying-components.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
