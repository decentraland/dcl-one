> Aprende cuándo y cómo ejecutar funciones asíncronas en el código de tu escena.

# Funciones asíncronas

## Descripción general

La mayor parte del código en tu escena se ejecuta de forma sincrónica usando un solo hilo. Eso significa que los comandos se ejecutan secuencialmente, línea por línea. Cada comando debe esperar primero a que el comando anterior termine de ejecutarse antes de poder empezar.

Incluso las funciones de los sistemas de tu escena se ejecutan una por una, siguiendo un [orden de prioridad](/docs/creator/content-creator-es/scenes-sdk7/arquitectura/systems/#system-execution-order).

Ejecutar código de forma sincrónica garantiza la consistencia, ya que siempre puedes estar seguro de que sabrás el orden en que se ejecutan los comandos de tu código.

Por otro lado, tu escena necesita actualizarse muchas veces por segundo para construir el siguiente frame. Si una parte de tu código tarda demasiado en responder, entonces todo el hilo principal se bloquea y esto se traduce en una tasa de frames más lenta.

Por eso, en algunos casos quieres que algunos comandos se ejecuten de forma asíncrona. Esto significa que puedes iniciar una tarea y, mientras tanto, la escena puede seguir avanzando por las siguientes líneas de código. La escena sigue ejecutándolo todo en un solo hilo, pero mientras la tarea asíncrona espera una respuesta, el resto del código no se bloquea.

Esto es especialmente útil para tareas que dependen de servicios externos que podrían tardar en responder, ya que no quieres que ese tiempo de espera inactivo bloquee otras tareas.

Por ejemplo:

* Al recuperar datos de una API REST
* Al realizar una transacción en la blockchain


> [!WARNING]
> **📔 Nota**: Ten en cuenta que varios frames de tu escena podrían renderizarse antes de que la tarea termine de ejecutarse. Asegúrate de que el código de tu escena sea lo suficientemente flexible como para manejar los escenarios intermedios mientras se completa la tarea asíncrona.


## Ejecuta una función async

Marca cualquier función como `async` para que se ejecute de forma asíncrona, sin bloquear el resto del código de la escena mientras espera.

```ts
// declarar función async
async function myAsyncTask() {
	// ejecutar los pasos de la función
}

// llamar a la función async
myAsyncTask()

// el resto del código sigue ejecutándose
```

## La función executeTask

El `executeTask()` ejecuta una función lambda de forma asíncrona. `executeTask()` nos permite declarar y ejecutar la función en una sola instrucción.

```ts
executeTask(async () => {
	let data = await myAsyncTask()
	console.log(data)
})

// el resto del código sigue ejecutándose
```

## La función then

El `then` toma una función lambda como argumento, que solo se ejecuta una vez que la instrucción anterior ha finalizado. Esta función lambda puede, opcionalmente, tener entradas que se asignan a partir de lo que devuelve la instrucción anterior.

```ts
myAsyncTask().then((data) => {
	console.log(data)
})
```


> [!WARNING]
> **📔 Nota**: Por lo general, es mejor usar la `executeTask` enfoque en lugar de la `then` función. Si dependes demasiado de la `then` función en varios niveles anidados, puedes terminar con lo que se conoce como "callback hell", donde el código puede volverse muy difícil de leer y mantener.


## Funciones PointerEvents y RayCast

Cuando tu escena usa un `PointerEvent` o un `RayCast` component, los cálculos de colisiones se realizan de forma async en el engine. Luego, el engine devuelve un evento de resultados a la escena, que puede llegar uno o varios ticks del game loop después de que se invocara el evento.

Entonces necesitas crear un sistema para procesar estos resultados en el frame en el que llegan.


> [!WARNING]
> **📔 Nota**: Si gestionas los clics mediante el [**Registra un callback**](/docs/creator/content-creator-es/scenes-sdk7/interactividad/eventos-de-button/register-callback/) enfoque, no necesitas crear explícitamente un sistema para manejar esto, pero lo mismo ocurre en segundo plano.


Consulta [eventos de clic](/docs/creator/content-creator-es/scenes-sdk7/interactividad/eventos-de-button/click-events/) y [raycasting](/docs/creator/content-creator-es/scenes-sdk7/interactividad/raycasting/).


> [!NOTE]
> **💡 Consejo**: Si el procesamiento de los resultados de un raycast requiere muchos cálculos (como ejecutar un algoritmo de búsqueda de rutas), quizá quieras ejecutar ese cálculo en una función asíncrona.


## La sentencia await

Un `await` obliga a que la ejecución espere una respuesta antes de pasar a la siguiente línea de código. `await` Las sentencias await solo pueden usarse dentro de un bloque de código async.

```ts
// declarar función
async function myAsyncTask() {
	try {
		let response = await fetch(callUrl)
		let json = await response.json()
		console.log(json)
	} catch {
		console.log('no se pudo acceder a la URL')
	}
}

// llamar a la función
myAsyncTask()

// El resto del código sigue ejecutándose
```

El ejemplo anterior ejecuta una función que incluye una `fetch()` operación para obtener datos de una API externa. La `fetch()` operación es asíncrona, ya que no podemos predecir cuánto tardará el servidor en responder. Sin embargo, la siguiente línea necesita que el resultado de esta operación esté listo antes de que podamos analizarlo como JSON. La `await` sentencia aquí garantiza que la siguiente línea solo se ejecute una vez que esa operación haya devuelto un valor. Del mismo modo, la `response.json()` función también es asíncrona, pero la siguiente línea necesita que el JSON se haya analizado antes de poder mostrarlo en consola. La segunda `await` sentencia fuerza a que la siguiente línea solo se llame una vez que termine el análisis del JSON, sin importar cuánto tarde.

## Establece un tiempo de espera para la llamada a una función

Usa `setTimeout` para esperar un tiempo antes de que se ejecuten ciertas líneas de código. Esto toma dos argumentos:

* La función que se ejecutará
* La cantidad de milisegundos que esperar antes de ejecutar esa función

El ejemplo siguiente espera 1000 milisegundos (equivalentes a 1 segundo) antes de ejecutar una función simple que registra un mensaje en la consola.

```ts
import { timers } from '@dcl/sdk/ecs'

console.log('Esto se imprime de inmediato')

timers.setTimeout(() => {
	// función para ejecutar después del retraso
    console.log('Esto se imprime después de 1 segundo')
}, 1000)
```

El `clearTimeout` se puede usar para cancelar la ejecución de una `setTimeout` función que aún está esperando ser ejecutada. `setTimeout` devuelve un ID de temporizador (un número), que luego puedes pasar a `clearTimeout`. En este caso, la variable `timeoutId` se obtiene al hacer la `setTimeout`, y luego se pasa a `clearTimeout` para cancelarlo.

```ts
import { timers } from '@dcl/sdk/ecs'

const timeoutId = timers.setTimeout(() => {
    console.log('Espera 1 segundo antes de ejecutar esta función')
}, 1000)

timers.clearTimeout(timeoutId)
```


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-es/scenes-sdk7/patrones-de-programacion/async-functions.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
