# API del Engine

El `EngineApi` el módulo proporciona acceso al framework Entity-Component-System compartido entre el World Explorer (que ejecuta el game loop) y las scenes individuales (que gestionan sus propias entities), además de utilidades relacionadas.

```ts
const engine = require("~system/EngineApi");
```

Este módulo es el más complejo y rico en funcionalidad, y el más probable de recibir actualizaciones y extensiones. Es donde reside la mayor parte del poder de Decentraland, ya que determina el tipo de experiencias que las personas pueden crear.

Este módulo contiene los siguientes métodos:

* [`function crdtSendToRenderer`](#crdtsendtorenderer)
* [`function crdtGetState`](#crdtgetstate)

## Introducción

El `EngineApi` el módulo está diseñado para sincronizar el state del world entre el Explorer y la scene mediante el intercambio de updates, events y commands. Implementa un protocolo de mensajes extensible que permite a las scenes, entre otras cosas:

* Crear y destruir entities.
* Adjuntar, actualizar y eliminar components.
* Lanzar rays en el entorno 3D y detectar hits.
* Recibir events como la entrada del player.

```
.------------------------------------------------------.
| World Explorer                                       |
|                                                      |
|                   [Engine API]                       |
|                        |                             |
|  .--------.            |              .------------. |
|  |        |<-----------+<-------------+  Runtime   | |
|  |  Game  |            |   Commands   |  .------.  | |
|  |        |   Events   |              |  | Scene | | |
|  | Engine +----------->+------------->|  |       | | |
|  |        |            |              |  '-------' | |
|  '--------'            |              '------------' |
|                                                      |
'------------------------------------------------------'

```


> [!NOTE]
> El `EngineApi` el módulo está en proceso de renovación. Si vas a las definiciones de origen, encontrarás métodos heredados que ya no se usan o que pueden implementarse como no-ops en la versión más reciente (más sobre esto abajo).


## Framework ECS

Con el `EngineApi` módulo viene una implementación genérica y extensible de un framework ECS compartido, que permite tanto a las scenes como al propio game engine crear entities, adjuntar components y actualizar sus states.

Los cambios realizados desde cualquiera de los lados se reflejan en el otro mediante el intercambio de messages. El mecanismo utilizado ([ver abajo](#synchronization)) garantiza que ambas partes acuerden el orden de cualquier update y alcancen una consistencia eventual sobre el state del world.

El World Explorer implementa un conjunto bien conocido de components básicos (positions, shapes, textures, media y más), y sabe cómo deserializar y aplicar changes a su state cuando recibe un update. Las scenes que usan el [SDK](https://docs.decentraland.org/creator) también tienen utilidades para crear components personalizados, pero estos son gestionados por completo por la scene (no sincronizados) y, por tanto, quedan fuera del scope del protocol.


> [!NOTE]
> La mayoría de las scenes usan el Decentraland SDK, que encapsula el `EngineApi` módulo y ofrece una interfaz mucho más agradable y de mayor nivel para los desarrolladores de contenido. Las scenes que acceden directamente al protocolo de mensajería en este módulo son extremadamente raras.


### Identificación de entities <a href="#identifying" id="identifying"></a>

Los IDs de entity son enteros simples que comienzan con `0`.

Por protocolo, los IDs por debajo de `512` están reservados para el World Explorer, y por tanto no son válidos para entities creadas por la scene. Los números `0`, `1` y `2` están realmente en uso (ver [basic entities](/docs/contributor/contributor-es/tiempo-de-ejecucion-de-la-escena/basic-entities/)) con el resto del rango disponible para futuras extensiones.

Cada scene tiene su propio rango privado de IDs, que el Explorer puede mapear de forma transparente a un identificador global entre todas las entities del game engine.

Con el tiempo, a medida que se crean y eliminan entities, es perfectamente válido reutilizar IDs que antes se habían liberado — de hecho, esto puede ser necesario por razones de rendimiento ([ver abajo](#crdtstatedeleteentities)).

## Sincronización <a href="#synchronization" id="synchronization"></a>

Tanto el World Explorer como la scene deben gestionar un conjunto de entities y components compartidos, donde las updates llegan desde ambos lados de forma asíncrona. Sin medidas adicionales, sus versiones del state del world pueden (y lo harán) acabar siendo diferentes.

Para evitar esto, se utiliza un CRDT ([tipo de dato replicado libre de conflictos](https://en.wikipedia.org/wiki/Conflict-free_replicated_data_type)) para llegar a un acuerdo entre ambas partes intercambiando messages con updates. Esto ofrece una serie de ventajas:

* Ambas partes pueden actualizar el state de forma independiente y concurrente.
* Los conflictos se gestionan mediante una estrategia de resolución compartida aplicada localmente por ambas partes.
* Se garantiza que ambas partes converjan a un state compartido e idéntico.
* Además de una pequeña sobrecarga en el tamaño de los messages, no se requiere coordinación adicional ni más viajes de ida y vuelta de messages.


> [!NOTE]
> Esta página explica el uso del CRDT para sincronizar el state entre una scene que se ejecuta localmente y el game engine, pero el verdadero potencial de este mecanismo reside en escenarios multiplayer. Usando este protocolo, los players pueden coordinar su game state y compartir la misma experiencia.


Para que esto funcione, todos los messages deben ser conmutativos e idempotentes. Los messages fuera de orden pueden aplicarse incluso si aún no se recibió un update supuestamente intermedio, y los messages iguales pueden reprocesarse sin romper la consistencia.

Revisemos de nuevo el diagrama arquitectónico anterior, ahora centrándonos en el CRDT:

```
.-------------------------------------------------------.
| World Explorer                                        |
|                                                       |
| .-------------.                       .------------.  |
| | Game Engine |                       |   Scene     | |
| |  .-------.  |                       |  .-------.  | |
| |  |       +--+-----------------------+->|       |  | |
| |  | CRDT  |  |   Protocolo de sync |  | CRDT  |  | |
| |  |       |<-+-----------------------+--+       |  | |
| |  '-------'  |                       |  '-------'  | |
| |             |                       |             | |
| '-------------'                       '-------------' |
'-------------------------------------------------------'

```

La implementación del mecanismo de sincronización de CRDT consta de tres partes:

1. Una representación interna del state de todas las entities y components.
2. Una estrategia de resolución de conflictos para elegir entre states en competencia.
3. Un protocolo de mensajería para comunicar y procesar updates al state.

### State de CRDT <a href="#crdtstate" id="crdtstate"></a>

El CRDT mantiene el state de todos los components para todas sus entities adjuntas, y puede almacenarse en un mapeo de components a states con timestamp para cada entity. En pseudocode:

```ts
// El state (completamente genérico) de un component para una entity específica, en un punto del tiempo:
type EntityComponentState = { timestamp: number; state: byte[] };

// El state de un component para todas las entities adjuntas:
type ComponentState = Map<EntityId, EntityComponentState>;

// El state completo del CRDT:
type CRDTState = Map<ComponentId, ComponentState>;
```

Los timestamps en realidad no son timestamps de estilo Unix. Son un tipo de contador incremental para secuenciar updates, sin relación con la hora del reloj. Más sobre esto [abajo](#timestamps).

#### Creación de entradas de state <a href="#crdtstateautocreate" id="crdtstateautocreate"></a>

Como la capa de CRDT no conoce (ni necesita conocer) todos los components disponibles, el state inicial es un mapa vacío. Las entradas se crean sobre la marcha cuando las operaciones involucran un `ComponentId` o `EntityId` que aún no estaba mapeado. Cuando se elimina una entity, el state asociado se borra de todos los components.

Ilustremos esto con algo más de pseudocode.

```ts
const crdtState = new Map(); // vacío, haremos espacio automáticamente para nuevos components y entities
```

```ts
function putEntityComponentState(componentId, entityId, entityComponentState) {
  // Si este component aún no se conoce, añádelo:
  if (!crdtState[componentId]) {
    crdtState[componentId] = new Map();
  }

  // Si esta entity no tiene state para este component, añádela y listo:
  if (!crdtState[componentId][entityId] && shouldCreate(entityId)) {
    crdtState[componentId][entityId] = entityComponentState;
    return;
  }

  // En este punto, tenemos dos states en competencia. El que acabamos de recibir no es necesariamente el
  // que debe conservarse. Hay que decidir.
  const existingEntityComponentState = crdtState[componentId][entityId];

  // Solo si las reglas de resolución de conflictos lo indican (más sobre esto más adelante), reemplaza el state:
  if (shouldReplace(entityComponentState, existingEntityComponentState)) {
    crdtState[componentId][entityId] = entityComponentState;
  }
}
```

El pseudocode anterior (si se tradujera a código real) es obviamente incompleto y subóptimo. En particular, le faltan las definiciones de `shouldCreate` y `shouldReplace`, que encapsulan la estrategia de resolución de conflictos. Estas tienen requisitos:

* `shouldCreate` debe añadir nuevas entities, pero rechazar volver a crear entities eliminadas.
* `shouldReplace` debe preferir conservar los states que tengan timestamps mayores (con algunos casos límite).

#### Seguimiento de entities eliminadas <a href="#crdtstatedeleteentities" id="crdtstatedeleteentities"></a>

Para respaldar el `shouldCreate` requisito, querrás llevar un registro de los IDs de entity eliminadas, para ignorar cualquier futura update de state para ellas. Veamos esto en más pseudocode:

```ts
const crdtDeletedEntities = new Set();

function shouldCreate(entityId) {
  return !crdtDeletedEntities.contains(entityId);
}

function deleteEntity(entityId) {
  // Marca esta entity como eliminada:
  crdtDeletedEntities.add(entityId);

  // Borra el state de esta entity en cada component:
  for (componentState of crdtState) {
    delete componentState[entityId];
  }
}
```


> [!NOTE]
> Ten en cuenta que, en la implementación ingenua anterior, el conjunto de entities eliminadas solo crecerá. En scenes que reciclan entities con rapidez, esto puede provocar una explosión en el uso de memoria.
> 
> Para evitar esto, el Explorer de Foundation emplea un índice generacional para reutilizar IDs en un espacio numérico finito, limitando la cantidad de memoria necesaria para llevar un registro tanto de entities activas como eliminadas.


En cuanto al `shouldReplace` requisito, consulta [resolución de conflictos](#crdtconflicts) abajo.

### Timestamps <a href="#timestamps" id="timestamps"></a>

Como se mencionó arriba, cuando hablamos de timestamps de CRDT no nos referimos a timestamps reales de Unix para un punto en el tiempo. En su lugar, una [timestamp de Lamport](https://en.wikipedia.org/wiki/Lamport_timestamp) (un tipo especial de contador incremental compartido) se usa para llevar el control de la secuencia de events por ambas partes.

El valor de este contador se actualiza según estas reglas:

1. Inicializa el contador local en `0`.
2. Antes de enviar un message a partes remotas, incrementa el contador local.
3. Después de recibir un message de una parte remota, establece el contador local al máximo entre su valor y el valor recibido, más `1`.

En pseudocode:

```ts
const localTimestamp = 0;

function sendState(state) {
  send({ state, timestamp: ++localTimestamp });
}

function receiveState() {
  const { state, timestamp } = receive();
  localTimestamp = max(localTimestamp, remoteTimestamp) + 1;
}
```

### Resolución de conflictos <a href="#crdtconflicts" id="crdtconflicts"></a>

Cuando el CRDT encuentra dos states en competencia, necesita una estrategia de resolución compartida que todas las partes apliquen de la misma manera, garantizando que todos lleguen a la misma decisión. El protocolo de Decentraland usa reglas muy simples:

1. Si la entity fue eliminada (y el ID nunca se reutilizó), ignora el nuevo state.
2. Si no había state previo, conserva el nuevo state.
3. Si el nuevo state tiene un timestamp mayor, conserva el nuevo state.
4. Si el nuevo state tiene un timestamp menor, conserva el state antiguo.
5. Si ambos timestamps son iguales, compara los states byte por byte y conserva el valor menor.

La mayoría de los casos se resolverán mediante reglas `1`, `2` y `3`.

### Sincronización inicial

Antes de que una scene empiece a ejecutar su propio code, el runtime rellena el CRDT con el state de todas las [basic entities](/docs/contributor/contributor-es/tiempo-de-ejecucion-de-la-escena/basic-entities/) y sus components.

Durante esta sincronización inicial, solo el runtime puede establecer el state compartido. La scene no puede hacer modificaciones hasta que el proceso se complete.

### Protocolo de mensajería <a href="#messages" id="messages"></a>

Los messages entre el World Explorer y el runtime de la scene son estructuras en una representación binaria serializada. Cada message lleva un encabezado que indica el tipo y la longitud, seguido de un payload particular para cada tipo de message.

```
.----------------.--------------.---------------------------------.
| length: uint32 | type: uint32 |     payload: byte[length]       |
'----------------'--------------'---------------------------------'
╵         campos comunes         ╵         según el tipo          ╵

```

Hay tres tipos de message en el protocolo CRDT:

* [`PutComponent`](#putcomponent)
* [`DeleteComponent`](#deletecomponent)
* [`DeleteEntity`](#deleteentity)

Ten en cuenta que no hay `CreateComponent` o `CreateEntity` messages. Las reglas de CRDT crean automáticamente entities y components que antes no se conocían, [como se explicó arriba](#crdtstateautocreate).

#### `PutComponentMessage` <a href="#putcomponent" id="putcomponent"></a>

Actualiza el `state` de un `component` para una `entity`, creando components y entities desconocidos en el CRDT si es necesario. [Resuelve conflictos](#crdtconflicts) según `timestamp`.

```
.----------------.-------------------.-------------------------------------------.
| entity: uint32 | component: uint32 | timestamp: uint32 |     state: byte[]     |
'----------------'-------------------'-------------------------------------------'
╵                       campos comunes                    ╵  específicos del component  ╵

```

El `state` el campo es una serialización binaria definida por el component. La mayoría de los components usan [protocol buffers](https://protobuf.dev) para codificar messages según lo especificado en los `.proto` files del [paquete de protocolo de Decentraland](https://github.com/decentraland/protocol). La excepción notable a esta regla es el `Transform` component, que (al ser con diferencia el más común) tiene un formato de serialización optimizado.

Esta capa adicional de serialización tiene una ventaja importante: el protocolo CRDT es agnóstico a cualquier implementación de component presente o futura.

Si el update contenido en este message se aplica al CRDT, el `state` campo se copia tal cual en la estructura.

#### `DeleteComponentMessage` <a href="#deletecomponent" id="deletecomponent"></a>

Elimina el state de `component` para un `entity`. [Resuelve conflictos](#crdtconflicts) según `timestamp`.

```
.----------------.-------------------.-------------------.
| entity: uint32 | component: uint32 | timestamp: uint32 |
'----------------'-------------------'-------------------'

```

#### `DeleteEntityMessage` <a href="#deleteentity" id="deleteentity"></a>

Elimina `entity` (es decir, todo el state de components asociado), esperando que el identificador nunca se reutilice para una entity diferente.

```
.----------------.
| entity: uint32 |
'----------------'

```

Ten en cuenta que no hay `timestamp` campo. Dado que la vida útil de `entity` ha terminado, la estrategia de resolución de conflictos para cualquier update fuera de orden es simplemente ignorarlos.

## Métodos

El conjunto de métodos en `EngineApi` proporciona la interfaz para intercambiar messages y reconstruir el state del world desde cero.

### `crdtSendToRenderer` <a href="#crdtsendtorenderer" id="crdtsendtorenderer"></a>

Envía un message serializado desde la scene al renderer y devuelve un array de messages serializados que el renderer tiene para la scene.

```ts
interface Request {
  // El message serializado para esta request:
  data: byte[];
}

interface Response {
  // Un array de messages serializados desde el renderer (si los hay):
  data: byte[][];
}

function crdtSendToRenderer(Request): Promise<Response>;
```

### `crdtGetState` <a href="#crdtgetstate" id="crdtgetstate"></a>

```ts
interface Request {}

interface Response {
  // Si el state tiene entities creadas por la scene:
  hasEntities: boolean;

  // Un array de messages que puede reconstruir el state del CRDT:
  data: byte[][];
}

function crdtGetState(Request): Promise<Response>;
```

## Scenes heredadas

Las scenes de estilo antiguo (es decir, las construidas usando SDK versión 6 o anterior) no usan el mecanismo moderno de CRDT, sino que llaman métodos en `EngineApi` que ahora están obsoletos.

La compatibilidad con estas scenes no es un requisito para una aplicación de Decentraland compatible con el protocolo.


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/contributor/contributor-es/tiempo-de-ejecucion-de-la-escena/modulos-de-tiempo-de-ejecucion/engine-api.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
