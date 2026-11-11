> Este conjunto te ayudará a entender cómo funcionan las cosas en el cliente y el SDK de Decentraland.

# Fundamentos de programación

## Las herramientas de desarrollo

A muy alto nivel, Decentraland **Kit de desarrollo de software** (SDK) te permite hacer lo siguiente:

* Generar uno predeterminado *proyecto* que contiene una escena de Decentraland, incluidos todos los assets necesarios para renderizar y ejecutar tu contenido.
* Compila, prueba y previsualiza el contenido de tu escena localmente en tu navegador web, completamente sin conexión y sin tener que realizar ninguna transacción de Ethereum ni poseer LAND.
* Escribe código TypeScript usando la API de Decentraland para añadir comportamiento interactivo y dinámico a la escena.
* Sube el contenido de tu escena al servidor de contenido.
* Vincula tus tokens LAND a la URL del contenido que has subido.

Nuestro SDK incluye lo siguiente:

* **Creator Hub**: Una aplicación independiente que, entre otras cosas, te permite crear escenas con una interfaz fácil de arrastrar y soltar. Puedes ejecutar vistas previas, depurar, editar código y publicar. [Leer más](/docs/creator/content-creator-es/scene-editor/primeros-pasos/about-editor/)
* **El ECS de Decentraland**: Un paquete TypeScript que contiene el framework de métodos auxiliares que te permite crear experiencias interactivas. Úsalo para crear y manipular objetos en la escena y también para facilitar transacciones dentro del mundo entre jugadores u otras aplicaciones. ( [referencia más reciente de ECS](https://github.com/decentraland/ecs-reference/blob/master/docs-latest/decentraland-ecs.md))
* **Ejemplos de escenas**: Inspírate y toma buenas prácticas de código de los [ejemplos de escenas](/docs/creator/content-creator-es/tutoriales-y-ejemplos/examples/).

Otras herramientas heredadas:

* **El Web Editor**: Una herramienta basada en web para crear escenas simples y publicarlas.

## Requisitos

Para desarrollar una escena localmente, no necesitas poseer tokens LAND. Desarrollar y probar una escena puede hacerse completamente sin conexión, sin necesidad de desplegar una escena en la red de Ethereum (el sistema que Decentraland usa para establecer la propiedad de LAND, de un Decentraland Name) ni en el servidor de contenido.

Debes tener:

* **Creator Hub**: Una aplicación independiente que, entre otras cosas, te permite crear escenas con una interfaz fácil de arrastrar y soltar. Puedes ejecutar vistas previas, depurar, editar código y publicar. [Leer más](/docs/creator/content-creator-es/scene-editor/primeros-pasos/about-editor/).

Si planeas editar el código de la escena, también tendrás que instalar uno de los siguientes:

* <img src="/docs/_remote/358795a1c08dfc479355a174.webp" alt="VS Code" data-size="line"> **Visual Studio Code**: Descárgalo [aquí](https://code.visualstudio.com/). Te ayuda a escribir código mucho más rápido y con menos errores. Un editor de código fuente marca errores de sintaxis, autocompleta mientras escribes e incluso muestra sugerencias inteligentes que dependen del contexto en el que te encuentres. También puedes hacer clic en un objeto del código para ver la definición completa de su clase y qué atributos soporta.
* <img src="/docs/_remote/9510a847d1493f0d8b4cf7e3.webp" alt="Cursor" data-size="line"> **Cursor AI**: Descárgalo [aquí](https://www.cursor.com/). Un potente editor de código integrado con IA. Te permite elegir diferentes modelos de IA para ayudarte a escribir código, todos ellos gratis. El asistente de IA no solo autocompleta mientras escribes; también puedes pedirle que refactorice una gran base de código, escriba documentación y más.


> [!NOTE]
> **💡 Consejo**: Puedes usar asistentes de IA como Cursor, OpenDCL o Claude Code para crear escenas completas a partir de descripciones en lenguaje natural — no se requiere experiencia en TypeScript. Consulta [Vibe Coding con IA](/docs/creator/content-creator-es/scenes-sdk7/primeros-pasos/vibe-coding/) para comenzar.


## Lenguajes y sintaxis compatibles

Decentraland emplea [TypeScript (.ts)](https://www.typescriptlang.org/docs/handbook/jsx.html) como el lenguaje predeterminado para escribir escenas.

TypeScript es un superconjunto de JavaScript, así que si estás familiarizado con JavaScript verás que es casi lo mismo, pero TypeScript incluye declaraciones de tipos. Gracias a las declaraciones de tipos, es posible contar con características como autocompletado y mejores pistas de depuración; esto acelera los tiempos de desarrollo y permite crear una base de código más sólida. Estas características son componentes clave de una buena experiencia de desarrollo.

Cuando se compila una escena, el código TypeScript que escribiste se compila a JavaScript minificado, para hacerlo más ligero. El código fuente original en TypeScript nunca se sube a los servidores, solo la versión compilada de JavaScript.

### Otros lenguajes

Puedes usar otra herramienta o lenguaje en lugar de TypeScript y compilarlo a JavaScript, siempre que tus scripts compilados estén contenidos dentro de un único archivo JavaScript que coincida con la ruta establecida en el `main` campo principal de tu escena `scene.json` archivo (por defecto *bin/index.js*). Todas las declaraciones de tipos proporcionadas están hechas en TypeScript, y otros lenguajes y transpiladores no cuentan con soporte oficial.

## escenas

El contenido que despliegas en tu LAND se llama una **scene**. Una escena es un programa interactivo que renderiza contenido 3D; puede ser un juego, una experiencia interactiva, una galería de arte, ¡lo que quieras!

Las escenas se despliegan en LAND virtual en Decentraland. LAND es un asset escaso y no fungible mantenido en un smart contract de Ethereum. Despliega en una sola **parcela**, una parcela de LAND de 16 metros por 16 metros, o a múltiples parcelas adyacentes.

Cuando los jugadores visitan Decentraland, descargan y renderizan el contenido de cada escena mientras recorren el mapa. Descargan las escenas cuando se alejan de ellas.

También puedes ejecutar una escena localmente en tu máquina ejecutando una vista previa desde la CLI.

## Entidades y Componentes

Las escenas tridimensionales en Decentraland se basan en una [arquitectura Entity-Component-System](https://en.wikipedia.org/wiki/Entity%E2%80%93component%E2%80%93system) , donde todo en una escena es una *entity*Entity. Las Entities tienen *components*, cada component le proporciona a la entity a la que pertenece propiedades específicas. Una entity de puerta probablemente tendrá al menos un component Transform (que establece posición, rotación y escala) y otro para darle una forma. Los components son solo un lugar para almacenar datos; no realizan ninguna acción por sí mismos.

![](/docs/_remote/28d14c88b2d418f3eab3d2ee.webp)

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

Las Entities pueden anidarse dentro de otras entities para formar una estructura en árbol. Si estás familiarizado con el desarrollo web, quizá te resulte útil pensar en las entities como elementos en un árbol DOM y en los components como los atributos de cada uno de estos elementos.

![](/docs/_remote/28a71287061591ece4e52409.webp)

Las Entities son un concepto abstracto. Una entity es solo un id, que se usa como referencia para agrupar distintos components.

Consulta [Entities y Components](/docs/creator/content-creator-es/scenes-sdk7/arquitectura/entities-components/) para echar un vistazo en profundidad a ambos conceptos y a cómo los usan las escenas de Decentraland.

### Custom components

El conjunto predeterminado de components (como `Transform`, `GltfContainer`, `Material`, etc.) es interpretado por el engine y tiene consecuencias directas en cómo se verá la entity, su posición, si emite sonidos, etc.

También puedes definir *custom components* components personalizados para almacenar datos que podrían ser útiles para las mecánicas de tu escena. El engine no sabrá cómo interpretar qué significan los valores de estos components, ni tendrán consecuencias directas sobre cómo se renderiza la escena. Sin embargo, puedes escribir lógica en el código de tu escena para monitorear estos valores y responder a ellos. Por ejemplo, puedes definir un component personalizado "doorState" para hacer seguimiento del estado abierto/cerrado de la puerta. En este caso, el component no es más que un lugar para almacenar un valor que lleve el registro de este estado. Para ver la puerta abrirse y cerrarse en tu escena, tendrás que implementar por separado la lógica que usa estos valores para afectar la rotación de la puerta, un valor del `Transform` component que el engine sí sabe interpretar.

Consulta [Componentes personalizados](/docs/creator/content-creator-es/scenes-sdk7/arquitectura/custom-components/) para más información.

### Obtener entities por nombre

Las entities añadidas mediante arrastrar y soltar en Scene Editor dentro de Creator Hub también son accesibles por código para seguir editándolas y añadir comportamiento.

Usa `engine.getEntityOrNullByName()` para obtener una entity, pasando el nombre asignado a la entity en la UI de Scene Editor. Cada una debe tener un nombre único.

```ts
function main() {
	const door = engine.getEntityOrNullByName('door3')
}
```

Luego puedes hacer lo que quieras con esa entity, como añadir nuevos components, modificar sus components existentes, duplicarla o borrarla.

Consulta [Obtener entity por nombre](/docs/creator/content-creator-es/scenes-sdk7/arquitectura/entities-components/#get-an-entity-by-name) para más información.

Si la entity es un [Smart item](/docs/creator/content-creator-es/scene-editor/interactividad/smart-items/), también puedes llamar a su **Actions** o suscribirte a su **Triggers** a través del código. Consulta [Elementos de referencia](/docs/creator/content-creator-es/scene-editor/extender-con-codigo/reference-items/).

## Systems

Las entities y los components son lugares para almacenar información sobre los objetos en una escena. *Systems* contienen funciones que cambian con el tiempo la información almacenada en components.

Los systems son donde implementamos la lógica del juego; llevan a cabo las acciones que deben actualizarse o comprobarse periódicamente en cada tick del game loop.

Un system es una función pura y simple que se llama una vez en cada tick (hasta 30 veces por segundo), siguiendo el [*patrón de actualización*](http://gameprogrammingpatterns.com/update-method.html).

```ts
// Sistema básico
function mySystem() {
	console.log('mi sistema se está ejecutando')
}

engine.addSystem(mySystem)

// System con dt
function mySystemDT(dt: number) {
	console.log('tiempo transcurrido desde el último frame:  ', dt)
}

engine.addSystem(mySystemDT)
```

Una sola escena puede tener 0 o muchos systems ejecutándose al mismo tiempo. Los systems pueden activarse o desactivarse en distintos momentos durante la duración de la escena. Por lo general, es una buena práctica mantener comportamientos independientes en systems separados.

Consulta [Systems](/docs/creator/content-creator-es/scenes-sdk7/arquitectura/systems/) para más detalles sobre cómo se usan los systems en una escena.

### El bucle de juego

El [bucle de juego](http://gameprogrammingpatterns.com/game-loop.html) es la columna vertebral del código de una escena de Decentraland. Recorre parte del código a intervalos regulares y hace lo siguiente:

* Escuchar la entrada del jugador
* Actualizar la escena
* Volver a renderizar la escena

En la mayoría de los programas tradicionales, todos los eventos se activan directamente por las acciones del jugador. Nada en el estado del programa cambiará hasta que el jugador haga clic en un botón, abra un menú, etc.

Pero los entornos interactivos y los juegos son distintos. No todos los cambios en la escena son necesariamente causados por las acciones de un jugador. Tu escena podría tener objetos animados que se mueven por sí solos o incluso personajes no jugadores con su propia IA. Algunas acciones del jugador también podrían requerir varios ticks para completarse; por ejemplo, si la apertura de una puerta necesita durar un segundo completo, la rotación de la puerta debe actualizarse incrementalmente unas 30 veces mientras se mueve.

Llamamos a cada iteración sobre el loop un *tick*. Las escenas de Decentraland se renderizan a 30 ticks por segundo, siempre que sea posible. Si la máquina tiene dificultades para renderizar cada tick, puede resultar en actualizaciones menos frecuentes.

En cada tick, la escena se actualiza; luego la escena se vuelve a renderizar, basándose en los valores actualizados.

En las escenas de Decentraland, no hay un game loop declarado explícitamente, sino que los [Systems](/docs/creator/content-creator-es/scenes-sdk7/arquitectura/systems/) systems de la escena conforman el game loop.

La compilación y el renderizado de la escena se realizan en el backend; no necesitas encargarte de eso mientras desarrollas tu escena.

## Consulta de components

Puedes [consultar components](/docs/creator/content-creator-es/scenes-sdk7/arquitectura/querying-components/) con el método `engine.getEntitiesWith(...components)` para hacer un seguimiento de todas las entities de la scene que tienen ciertos components.

A menudo tiene sentido consultar components dentro de un [system](/docs/creator/content-creator-es/scenes-sdk7/arquitectura/systems/), para luego recorrer cada una de las entities devueltas y realizar el mismo conjunto de acciones en cada una.

Si intentas iterar sobre todas las entities de la escena en cada tick del game loop, eso podría tener un coste importante en el rendimiento. Al referirte solo a las entities devueltas por una consulta, te aseguras de trabajar solo con las relevantes.

```ts
// Define un System
function boxHeightSystem(dt: number) {
	// consulta entities que incluyan tanto components MeshRenderer como Transform
	for (const [entity] of engine.getEntitiesWith(MeshRenderer, Transform)) {
		const transform = Transform.get(entity)
		console.log('un cubo está a la altura:  ', transform.position.y)
	}
}

// Añade el system al engine
engine.addSystem(boxHeightSystem)
```

## Ciclo de vida de la scene

Si empiezas a escribir líneas sueltas de código directamente en `index.ts`, tu código puede carecer de contexto importante. Por ejemplo, te faltará información sobre la entidad del jugador, o sobre entities que se añadieron mediante arrastrar y soltar en Creator Hub. En el momento en que se lean tus líneas de código, esas cosas aún no se han cargado.

Para evitar ese escenario, siempre se recomienda escribir el código de carga inicial de tu escena usando la `main()` función (en el `index.ts` archivo) como punto de entrada. Esta función se ejecuta solo después de que todo el contexto inicial de la escena ya esté cargado; esto incluye cualquier cosa añadida mediante la UI de Scene Editor.

Puedes escribir tu código fuera de la `main()` función cuando:

* El código es llamado indirectamente por `main()`
* El código define un system o añade un system al engine
* El código está dentro de un [función async](/docs/creator/content-creator-es/scenes-sdk7/patrones-de-programacion/async-functions/)


> [!WARNING]
> **📔 Nota**: Para cuando se ejecuta por primera vez el código dentro de una función async o un system, todo en la escena ya está correctamente inicializado.
> 
> [Componente personalizado](/docs/creator/content-creator-es/scenes-sdk7/arquitectura/custom-components/) las definiciones son una excepción; siempre deben escribirse fuera de la `main()` función, en un archivo aparte. Deben interpretarse antes de que `main()` se ejecute.


## Mutabilidad

Puedes elegir trabajar con versiones mutables o inmutables (solo lectura) de un component. La `.get()` función get de un component devuelve una versión inmutable del component. Solo puedes leer sus valores, pero no puedes cambiar ninguna de sus propiedades.

El `.getMutable()` función set devuelve una representación del component que te permite cambiar sus valores. Usa las versiones mutables solo cuando planees hacer cambios en un component. Trabajar con versiones inmutables de components supone una gran mejora en el rendimiento.

```ts
// obtener una versión inmutable (solo lectura)
const immutableTransform = Transform.get(myEntity)

// lo siguiente NO funciona:
// \timmutableTransform.position.y = 2

const mutableTransform = Transform.getMutable(myEntity)

// lo siguiente SÍ cambia la posición de la entity
mutableTransform.position.y = 2
```

Consulta [datos mutables](/docs/creator/content-creator-es/scenes-sdk7/patrones-de-programacion/mutable-data/) para más detalles.

## Integrándolo todo

El *engine* es lo que se sitúa entre *entities*, y *components* por un lado y *systems* por el otro.

![](/docs/_remote/0b852c135fff08d1f4c9243d.webp)

Todos los valores almacenados en los components de la escena representan el estado de la escena en ese momento. Con cada tick del game loop, el engine ejecuta las funciones de cada uno de los systems para actualizar los valores almacenados en los components.

Después de que se ejecuten todos los systems, los components de cada entity tendrán nuevos valores. Cuando el engine renderiza la escena, usará estos nuevos valores actualizados y los jugadores verán cómo las entities cambian para reflejar sus nuevos estados.

```ts
export function main() {
	// Crear un entity
	const cube = engine.addEntity()

	// Darle al entity una posición mediante un component Transform
	Transform.create(cube, {
		position: Vector3.create(5, 1, 5),
	})

	// Dale a la entity una forma visible mediante un component MeshRenderer
	MeshRenderer.setBox(cube)
}

// Define un System
function rotationSystem(dt: number) {
	// consulta entities que incluyan tanto components MeshRenderer como Transform
	for (const [entity] of engine.getEntitiesWith(MeshRenderer, Transform)) {
		const transform = Transform.getMutable(entity)
		transform.rotation = Quaternion.multiply(
			transform.rotation,
			Quaternion.fromAngleAxis(dt * 10, Vector3.Up())
		)
	}
}

// Añade el system al engine
engine.addSystem(rotationSystem)
```

En el ejemplo anterior, un `cubo` entity y un `system rotationSystem` se añaden al engine. La `cubo` entity tiene un `Transform`, y un `MeshRenderer` component. En cada tick del game loop, el `system rotationSystem` system se llama, y cambia los valores de rotación en el `Transform` component de la `cubo` entity.

Ten en cuenta que la mayor parte del código anterior se ejecuta solo una vez, al cargar la escena. La excepción es el `system rotationSystem` system, que se llama en cada tick del game loop.

## Desacoplamiento de la escena

Tus escenas no se ejecutan en el mismo contexto que el engine (también conocido como el main thread). Creamos el SDK de una forma que está completamente desacoplada del motor de renderizado. Lo diseñamos así tanto por motivos de seguridad como de rendimiento.

Debido a este desacoplamiento, el código de tu escena no tiene acceso al DOM ni al objeto `window` , por lo que no puedes acceder a datos como el navegador o la ubicación geográfica del jugador.

El desacoplamiento funciona mediante el protocolo RPC; este protocolo asigna una pequeña parte del cliente solo para renderizar la escena y controlar eventos.

También hemos abstraído el protocolo de comunicación. Esto nos permite ejecutar las escenas localmente en un WebWorker.

No queremos que los desarrolladores interfieran con los internos del engine ni siquiera que necesiten saber qué hay dentro del engine. Necesitamos garantizar una experiencia consistente para los jugadores en todo el mapa de Decentraland, y es más probable que ocurran errores en ese nivel "bajo".

Este desacoplamiento también es importante para evitar que las escenas vecinas interfieran con la experiencia de los jugadores mientras están en la escena de otra persona. Un jugador podría tener varias escenas cercanas cargadas al mismo tiempo, cada una ejecutando su propio código. Algunas acciones (como abrir enlaces externos o mover al jugador) solo están permitidas cuando el jugador está parado en esa escena concreta, no si la escena está cargada pero el jugador está fuera.

## Tree Shaking

Al convertir el código fuente en TypeScript al código compilado en JavaScript minificado, el proceso realiza [tree shaking](https://en.wikipedia.org/wiki/Tree_shaking) para asegurarse de que solo se conviertan las partes del código que realmente se están usando. Esto ayuda a mantener el código final de la escena lo más ligero posible. Es especialmente útil al usar bibliotecas externas, ya que a menudo estas incluyen mucha funcionalidad que no se usa y que de otro modo engordaría la escena.

Como consecuencia del tree shaking, cualquier código que quieras que tu escena ejecute necesita ser referenciado de una forma u otra por los puntos de entrada de tu código: la `main()` función en `index.ts`. Los systems también pueden añadirse al engine en el `index.ts` archivo, sin hacer referencia a `main()`. Cualquier código que no esté referenciado explícita o indirectamente por estos archivos no se incluirá en la escena.

Por ejemplo, supón que tienes un archivo llamado `extraContent.ts` con el siguiente contenido, la entity no se renderizará y el system no empezará a ejecutarse:

```ts
// extraContent.ts

const myEntity = engine.addEntity()
Transform.create(myEntity, {
	position: { x: 8, y: 0, z: 8 },
})
MeshRenderer.setBox(myEntity)

function mySystem(dt: number) {
	console.log('system en ejecución')
}

engine.addSystem(mySystem)
```

Para hacer que se ejecute como parte de tu escena, puedes referenciarlo desde `index.ts` de la siguiente manera:

```ts
// en extraContent.ts

export function addEntities() {
	const myEntity = engine.addEntity()
	Transform.create(myEntity, {
		position: { x: 8, y: 0, z: 8 },
	})
	MeshRenderer.setBox(myEntity)
}

export function mySystem(dt: number) {
	console.log('system en ejecución')
}

/////////////////////////////

// en index.ts

import { addEntities, mySystem } from './extraContent'

export function main() {
	addEntities()
}

engine.addSystem(mySystem)
```

La excepción a esta regla son las definiciones de componentes personalizados. Estas no deben accederse a través del `main()` punto de entrada de la función, ya que necesitan interpretarse antes que todo lo demás.

## Importaciones

Todas las funciones, objetos, components y demás elementos usados por la escena deben importarse en cada archivo para poder usarlos. Esto es una consecuencia de [tree-shaking](#tree-shaking), ya que evita empaquetar todo el SDK y en su lugar solo incluye las partes que usa la escena.

Los snippets a lo largo de la documentación omiten las líneas de importación al inicio de cada archivo para mantenerlos limpios, pero para que funcionen debes agregarlas a la escena.

Cuando usas VS Studio Code para escribir tus escenas, las opciones inteligentes de autocompletado deberían encargarse de gestionar las importaciones por ti mientras escribes, sin que tengas que ser consciente de ello.

Cuando pegues un snippet en tu escena, sin embargo, probablemente verás algunos elementos marcados en rojo que no están importados en ese archivo. Para corregirlo:

* Haz clic en cada palabra subrayada
* Haz clic en el icono de la bombilla a la izquierda de la línea
* Selecciona **Agregar importación desde**
* Aparece una línea de importación al inicio del archivo.

![](/docs/_remote/d183f4383d8bff7a66afa15b.webp)

Si hay muchas cosas distintas que importar, también puedes seleccionar **Agregar todas las importaciones faltantes** del mismo dropdown.

Ten en cuenta que las importaciones deben hacerse en cada archivo donde se use un elemento.

VS Studio Code debería ser capaz de resolver por sí solo las rutas correctas a tus importaciones. Si por cualquier motivo tiene problemas para hacerlo, un truco es pegar las siguientes sentencias de importación vacías al inicio de tu archivo. VS Studio debería poder encargarse a partir de ahí.

```ts
import {} from '@dcl/sdk/ecs'
import {} from '@dcl/sdk/math'
```

## Versiones del SDK

Al desarrollar una nueva escena, usas por defecto la `@latest` versión estable del SDK.

Puedes instalar la `@next` versión del SDK si quieres aprovechar o previsualizar funciones próximas que aún no han llegado a la última versión estable.

Para hacerlo, abre el `package.json` archivo de tu escena y cambia las siguientes líneas:

```json
  "devDependencies": {
    "@dcl/js-runtime": "next",
    "@dcl/sdk": "next"
  },
```

Luego ejecuta el siguiente comando en la carpeta de tu proyecto de escena:

```
npm i
```

Consulta [gestionar dependencias](/docs/creator/content-creator-es/scenes-sdk7/libraries/manage-dependencies/) para más detalles.


> [!WARNING]
> **📔 Nota**: Ten en cuenta que la versión @next puede presentar problemas de vez en cuando. La sintaxis y el nombre de las nuevas funciones podrían cambiar antes de que se publique una versión estable.



---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-es/scenes-sdk7/primeros-pasos/coding-scenes.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
