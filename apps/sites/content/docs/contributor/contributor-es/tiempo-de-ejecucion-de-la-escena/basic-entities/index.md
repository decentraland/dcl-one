# Entidades básicas

El World Explorer [reserva un rango](/docs/contributor/contributor-es/tiempo-de-ejecucion-de-la-escena/modulos-de-tiempo-de-ejecucion/engine-api/#identifying) de IDs de entidades para su propio uso. De este rango, los identificadores `0`, `1` y `2` se preasignan a un conjunto de entidades básicas cuya existencia está garantizada.

Estos son:

* `0`: `RootEntity`
* `1`: `PlayerEntity`
* `2`: `CameraEntity`

Estas entidades tienen un conjunto conocido de componentes adjuntos que son gestionados por el runtime, es decir, su estado es de solo lectura para la escena. Obtener el estado inicial de estos componentes forma parte de la [inicialización de la escena](/docs/contributor/contributor-es/tiempo-de-ejecucion-de-la-escena/execution/#onstart).


> [!NOTE]
> Se están desarrollando componentes adicionales para estas entidades. El [mecanismo de sincronización CRDT](/docs/contributor/contributor-es/tiempo-de-ejecucion-de-la-escena/modulos-de-tiempo-de-ejecucion/engine-api/#synchronization) se está ampliando para proporcionar parte de la funcionalidad ofrecida por diferentes módulos.
> 
> Por ejemplo, las escenas podrán detectar el estado de la ventana del Explorer para optimizar el uso de red y CPU, o acceder a información más rica sobre el jugador, el sistema de comunicaciones y la cámara del juego.


#### `RootEntity` (`0`) <a href="#rootentity" id="rootentity"></a>

Contiene información general sobre el contexto de la escena.

* [`Transform`](/docs/contributor/contributor-es/tiempo-de-ejecucion-de-la-escena/basic-components/#transform): información posicional del punto de origen de la escena, con respecto al cual son relativos todos los demás transforms sin un padre explícito.

El `Transform` componente de la `RootEntity` se inicializa con los siguientes valores:

```yml
scale: [1, 1, 1] # XYZ
position: [0, 0, 0] # XYZ
rotation: [0, 0, 0, 1] # XYZW
```

#### `PlayerEntity` (`1`) <a href="#playerentity" id="playerentity"></a>

Contiene datos sobre el jugador, su identidad, su avatar y su ubicación.

* [`Transform`](/docs/contributor/contributor-es/tiempo-de-ejecucion-de-la-escena/basic-components/#transform): la información posicional del avatar del jugador.

#### `CameraEntity` (`2`) <a href="#cameraentity" id="cameraentity"></a>

Contiene información sobre la cámara del juego. Todos los componentes son gestionados por el renderizador.

* [`Transform`](/docs/contributor/contributor-es/tiempo-de-ejecucion-de-la-escena/basic-components/#transform): la información posicional de la propia cámara.
* [`PointerLocked`](/docs/contributor/contributor-es/tiempo-de-ejecucion-de-la-escena/basic-components/#pointerlock): si el puntero sigue la dirección de la cámara o se mueve de forma independiente.


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/contributor/contributor-es/tiempo-de-ejecucion-de-la-escena/basic-entities.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
