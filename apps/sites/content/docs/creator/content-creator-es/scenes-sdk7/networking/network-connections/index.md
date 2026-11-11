> Cómo comunicar tu escena con servidores externos y APIs.

# Conexiones de red

Tu escena puede aprovechar servicios externos que exponen APIs; puedes usar esto para obtener datos de precios actualizados, datos meteorológicos o cualquier otro tipo de información expuesta por una API.

También puedes configurar tu propio servidor externo para ayudar a tu escena y servir para sincronizar datos entre tus jugadores. Esto puede hacerse con un servidor que exponga una REST API o con un servidor que use WebSockets.

## Llamar a una REST API

El código de tu escena puede enviar llamadas a una REST API para obtener datos.

Como el servidor puede tardar en enviar su respuesta, debes ejecutar este comando como una [función asíncrona](/docs/creator/content-creator-es/scenes-sdk7/patrones-de-programacion/async-functions/), usando `executeTask()`.

```ts
executeTask(async () => {
	try {
		let response = await fetch(callUrl)
		let json = await response.json()
		console.log(json)
	} catch {
		console.log('no se pudo alcanzar la URL')
	}
})
```

El comando fetch también puede incluir un segundo argumento opcional que agrupa los encabezados, el método HTTP y el cuerpo HTTP en un solo objeto.

* **url**: Dirección a la que enviar la solicitud
* **init**: Un `RequestInit` objeto que puede contener:
  * **method** : Método HTTP a usar (GET, POST, DELETE, etc)
  * **body**: Contenido del cuerpo de la solicitud. Debe enviarse como un objeto JSON convertido a string.
  * **headers**: Headers adicionales para incluir en la solicitud.
  * **redirect**: Estrategia de redirección ('follow' | 'error' | 'manual')
  * **timeout**: Cuánto tiempo esperar una respuesta antes de que la solicitud falle. De forma predeterminada, 30000 milisegundos (30 segundos).

```ts
executeTask(async () => {
	try {
		let response = await fetch(callUrl, {
			headers: { 'Content-Type': 'application/json' },
			method: 'POST',
			body: JSON.stringify(myBody),
		})
		let json = await response.json()
		console.log(json)
	} catch {
		console.log('no se pudo alcanzar la URL')
	}
})
```

El comando fetch devuelve un `objeto response` con los siguientes datos:

* `headers`: Un `objeto ReadOnlyHeaders. Llama al` método get() para obtener un header específico, o al `método has()` para comprobar si un header está presente. `has()` método para comprobar si un header está presente.
* `ok`: Booleano
* `redirected`: Booleano
* `status`: Número del código de estado
* `statusText`: Texto para el código de estado
* `type`: Tendrá uno de los siguientes valores: *basic*, *cors*, *default*, *error*, *opaque*, *opaqueredirect*
* `url`: URL que se envió
* `json()`: Obtiene el cuerpo en formato JSON.
* `text()`: Obtiene el cuerpo como texto.


> [!WARNING]
> **📔 Nota**: `json()` y `text()` son mutuamente excluyentes. Si obtienes el cuerpo de la respuesta en uno de los dos formatos, ya no podrás obtener el otro del `objeto response` objeto.



> [!WARNING]
> **📔 Nota**: Cada escena de Decentraland solo tiene permitido realizar una `fetch` comando a la vez. Esto no afecta a cómo debe estructurarse el código de la escena, ya que las solicitudes se ponen en cola internamente. Si tu escena requiere enviar varias solicitudes a diferentes endpoints, ten en cuenta que cada solicitud solo se envía cuando la anterior ha recibido respuesta.


## Solicitudes firmadas

Puedes emplear una medida de seguridad adicional para certificar que una solicitud se origina en una sesión de jugador dentro de Decentraland. Puedes enviar tus solicitudes con una firma adicional, firmada usando una clave efímera que la sesión de Decentraland genera para cada jugador basada en la dirección del jugador. El servidor que recibe la solicitud puede verificar entonces que el mensaje firmado realmente coincide con una dirección que está actualmente activa en el mundo.

Este tipo de medidas de seguridad es especialmente valioso cuando puede existir un incentivo para que un jugador abuse del sistema, para farmear tokens o puntos en un juego.

Para enviar una solicitud firmada, todo lo que necesitas hacer es usar la `signedFetch()` función, exactamente de la misma manera que usarías la `fetch()` función.

```ts
import { signedFetch } from '~system/SignedFetch'

executeTask(async () => {
	try {
		let response = await signedFetch({
			url: callUrl,
			init: {
				headers: { 'Content-Type': 'application/json' },
				method: 'POST',
				body: JSON.stringify(myBody),
			},
		})

		if (!response.ok) {
			throw new Error('Respuesta no válida')
		}

		let json = JSON.parse(response.body)

		console.log('Respuesta recibida: ', json)
	} catch {
		console.log('no se pudo alcanzar la URL')
	}
})
```

La solicitud incluye una serie adicional de headers que contienen un mensaje firmado y un conjunto de metadatos para interpretarlo. El mensaje firmado consiste en todo el contenido de la solicitud cifrado usando la clave efímera del jugador.

El `signedFetch()` se diferencia de la `fetch()` función en que la respuesta es una promesa de un mensaje HTTP completo, expresado como un `FlatFetchResponse` objeto. Esto incluye las siguientes propiedades:

* `body`
* `headers`
* `ok`
* `status`
* `statusText`

El **body** siempre es una string, que puedes analizar como en el ejemplo anterior.

### Validar una solicitud firmada

Para hacer uso de las solicitudes firmadas, el servidor que las recibe debe validar que las firmas coincidan con el resto de la solicitud y que la marca de tiempo codificada dentro del mensaje firmado sea actual.

Puedes encontrar un ejemplo sencillo de un servidor que realiza esta tarea en la siguiente escena de ejemplo:

[Validar autenticidad del jugador](https://github.com/decentraland-scenes/validate-player-authenticity)

## Tiempo de espera de la solicitud

Si una solicitud HTTP tarda demasiado en recibir respuesta, falla para que se puedan enviar otras solicitudes. Para `fetch()`, el umbral de tiempo de espera predeterminado es de 30 segundos, pero puedes asignar un valor diferente en cada solicitud configurando la `timeout` propiedad. El valor de `timeout` se expresa en milisegundos.

```ts
fetch('https://some-url.com', { timeout: 1000 })
```

## Usa WebSockets

También puedes enviar y obtener datos desde un servidor WebSocket, siempre que este servidor use una conexión segura con *wss*.

```ts
var socket = new WebSocket('url')

socket.onmessage = function (event) {
	console.log('Mensaje de WebSocket recibido:', event)
}
```

La sintaxis para usar WebSockets no es diferente de la implementada de forma nativa por JavaScript. Consulta la documentación de [Mozilla Web API](https://developer.mozilla.org/en-US/docs/Web/API/WebSocket) para ver detalles sobre cómo recibir y enviar mensajes por WebSockets.


> [!NOTE]
> **💡 Consejo**: Una biblioteca que simplifica el uso de conexiones WebSocket y que ha demostrado funcionar muy bien con Decentraland es [Colyseus](https://colyseus.io/). Varias otras bibliotecas WebSocket no son compatibles con el SDK de Decentraland.
> 
> Crea una capa de abstracción sobre las conexiones WebSocket que hace que reaccionar a los cambios y almacenar remotamente en el servidor un estado de juego coherente sea muy fácil. Puedes verlo en acción en estos ejemplos:
> 
> * [Cube Jumper](https://github.com/decentraland-scenes/cube-jumper-colyesus-sdk7)
> * [Space Traitor](https://github.com/decentraland-scenes/Space-Traitor)
> * [AI NPC](https://github.com/decentraland-scenes/inworld-ai-sdk7)


## Depuración de solicitudes de red

Puedes depurar solicitudes de red abriendo Debug Panel.

Para abrir el Debug Panel, puedes hacer clic en el ![](/docs/_remote/3a2d6a1437dc36383f2065db.webp) icono en la esquina superior derecha. Luego selecciona la **Web Requests** pestaña y haz clic en **Abrir Chrome DevTools**.

Esto abrirá una nueva ventana de Chrome con la pestaña Network abierta.

Consulta [Depurar en Preview](/docs/creator/content-creator-es/scenes-sdk7/depuracion/debug-in-preview/#web-requests) para más detalles.


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-es/scenes-sdk7/networking/network-connections.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
