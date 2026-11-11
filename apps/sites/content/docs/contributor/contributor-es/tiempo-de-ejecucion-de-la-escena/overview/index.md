# Resumen

El runtime de escenas de Decentraland es el entorno de ejecución aislado donde una [escena](/docs/contributor/contributor-es/contenido/tipos-de-entidad/scenes/) ejecuta su script principal.

Cada escena se ejecuta dentro de su propio runtime aislado. Dentro de él, tiene acceso a ciertos [objetos globales](/docs/contributor/contributor-es/tiempo-de-ejecucion-de-la-escena/globals/) y puede importar [módulos](https://github.com/decentraland/docs/tree/main/contributor/runtime/modules/README.md) desde la biblioteca del runtime.

```mermaid
diagrama de flujo TB
    subgrafo WorldExplorer["World Explorer"]
        subgrafo Runtime1["Runtime 1"]
            Scene1["Scene"]
        fin
        
        subgrafo Runtime2["Runtime 2"]
            Scene2["Scene"]
        fin
        
        subgrafo Runtime3["Runtime 3"]
            Scene3["Scene"]
        fin
        
        Scene1 --> Library["Biblioteca de módulos del runtime"]
        Scene2 --> Library
        Scene3 --> Library
    fin
```

Al importar módulos desde la biblioteca del runtime, las escenas tienen acceso a una amplia variedad de funcionalidades, entre ellas:

* Crear Entities y controlar su comportamiento
* Interactuar con el jugador
* Comunicarse con otros jugadores
* Inspeccionar el estado del mundo
* Conectarse a servicios externos
* Realizar pagos web3

En la práctica, las escenas incluyen el [SDK de Decentraland](https://github.com/decentraland/docs/tree/main/creator/sdk7/getting-started/sdk-101.md) con su código, que encapsula la interfaz de estilo RPC de la biblioteca del runtime y proporciona una API más agradable y cómoda.


> [!NOTE]
> Ten en cuenta que el SDK *no* forma parte del protocolo de Decentraland; solo lo son las interfaces de nivel inferior. Las escenas pueden decidir si incluir el SDK o usar bibliotecas alternativas.


### Objetos globales

Dentro del entorno aislado, [escenas](/docs/contributor/contributor-es/contenido/tipos-de-entidad/scenes/) se asume que ciertos objetos y Functions están disponibles globalmente. Algunos son objetos estándar de ES2020 (como `Date` o `Math`), y otros son inyectados por el runtime para implementar funcionalidades como requerir módulos, registrar mensajes y programar callbacks.

Ver [globales](/docs/contributor/contributor-es/tiempo-de-ejecucion-de-la-escena/globals/) para obtener más detalles.

### Módulos

El [`require`](/docs/contributor/contributor-es/tiempo-de-ejecucion-de-la-escena/globals/#module) La Function global puede importar módulos proporcionados por la [biblioteca del runtime](/docs/contributor/contributor-es/tiempo-de-ejecucion-de-la-escena/execution/). Estos módulos permiten importar selectivamente utilidades como los Systems ECS o Communications.

Consulta la sección de módulos del runtime para ver una lista completa.

### Ejecución

El runtime requiere que las escenas implementen un conjunto específico de métodos y garantiza que se llamarán de acuerdo con ciertas reglas.

Consulta [ejecución](/docs/contributor/contributor-es/tiempo-de-ejecucion-de-la-escena/execution/) para obtener más información sobre esto.

### Permisos

La funcionalidad sensible, como realizar requests HTTP a servicios de terceros, está restringida por el runtime a menos que se soliciten ciertos permisos mediante el [escena](/docs/contributor/contributor-es/contenido/tipos-de-entidad/scenes/), y sean concedidos por el jugador.

Consulta [permisos](/docs/contributor/contributor-es/tiempo-de-ejecucion-de-la-escena/permissions/) para obtener más información.


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/contributor/contributor-es/tiempo-de-ejecucion-de-la-escena/overview.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
