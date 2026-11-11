# Basic Entities

O World Explorer [reserva um intervalo](/docs/contributor/contributor-pt/runtime-da-scene/modulos-de-runtime/engine-api/#identifying) de IDs de entidades para seu próprio uso. Desse intervalo, os identificadores `0`, `1` e `2` são pré-atribuídos a um conjunto de entidades básicas garantidas a existir.

São:

* `0`: `RootEntity`
* `1`: `PlayerEntity`
* `2`: `CameraEntity`

Estas entidades têm um conjunto conhecido de Components anexados que são gerenciados pelo runtime, ou seja, o seu estado é somente leitura para a scene. Obter o estado inicial desses Components faz parte da [inicialização da scene](/docs/contributor/contributor-pt/runtime-da-scene/execution/#onstart).


> [!NOTE]
> Componentes adicionais para estas entidades estão em desenvolvimento. O [mecanismo de sincronização CRDT](/docs/contributor/contributor-pt/runtime-da-scene/modulos-de-runtime/engine-api/#synchronization) está a expandir-se para fornecer alguma da funcionalidade oferecida por diferentes módulos.
> 
> Por exemplo, as scenes poderão detetar o estado da janela do Explorer para otimizar o uso de rede e CPU, ou aceder a informações mais ricas sobre o player, o sistema de comms e a game camera.


#### `RootEntity` (`0`) <a href="#rootentity" id="rootentity"></a>

Contém informações gerais sobre o contexto da scene.

* [`Transform`](/docs/contributor/contributor-pt/runtime-da-scene/basic-components/#transform): informação posicional do ponto de origem da scene, em relação ao qual todos os outros transforms sem um pai explícito são relativos.

O `Transform` componente do `RootEntity` é inicializado com os seguintes valores:

```yml
scale: [1, 1, 1] # XYZ
position: [0, 0, 0] # XYZ
rotation: [0, 0, 0, 1] # XYZW
```

#### `PlayerEntity` (`1`) <a href="#playerentity" id="playerentity"></a>

Contém dados sobre o player, a sua identidade, o seu avatar e localização.

* [`Transform`](/docs/contributor/contributor-pt/runtime-da-scene/basic-components/#transform): a informação posicional do avatar do player.

#### `CameraEntity` (`2`) <a href="#cameraentity" id="cameraentity"></a>

Contém informações sobre a game camera. Todos os Components são gerenciados pelo renderer.

* [`Transform`](/docs/contributor/contributor-pt/runtime-da-scene/basic-components/#transform): a informação posicional da própria câmara.
* [`PointerLocked`](/docs/contributor/contributor-pt/runtime-da-scene/basic-components/#pointerlock): se o pointer segue a direção da câmara ou se move de forma independente.


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/contributor/contributor-pt/runtime-da-scene/basic-entities.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
