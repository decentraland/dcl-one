# API do Engine

O `EngineApi` o módulo fornece acesso ao framework Entity-Component-System compartilhado entre o World Explorer (que executa o loop do jogo) e as cenas individuais (que gerenciam suas próprias entities), além de utilitários relacionados.

```ts
const engine = require("~system/EngineApi");
```

Este módulo é o mais complexo e rico em funcionalidades, e o mais propenso a receber atualizações e extensões. É nele que reside a maior parte do poder de Decentraland, já que determina o tipo de experiências que as pessoas podem criar.

Este módulo contém os seguintes métodos:

* [`função crdtSendToRenderer`](#crdtsendtorenderer)
* [`função crdtGetState`](#crdtgetstate)

## Introdução

O `EngineApi` o módulo foi projetado para sincronizar o estado do mundo entre o Explorer e a scene trocando atualizações, eventos e comandos. Ele implementa um protocolo de mensagens extensível que permite às scenes, entre outras coisas:

* Criar e destruir entities.
* Anexar, atualizar e remover Components.
* Lançar raios no ambiente 3D e detetar impactos.
* Receber eventos como entrada do jogador.

```
.------------------------------------------------------.
| World Explorer                                       |
|                                                      |
|                   [Engine API]                       |
|                        |                             |
|  .--------.            |              .------------. |
|  |        |<-----------+<-------------+  Runtime   | |
|  |  Jogo  |            |   Comandos   |  .------.  | |
|  |        |   Eventos  |              |  | Cena | | |
|  | Motor +----------->+------------->|  |       | | |
|  |        |            |              |  '-------' | |
|  '--------'            |              '------------' |
|                                                      |
'------------------------------------------------------'

```


> [!NOTE]
> O `EngineApi` o módulo está passando por renovações. Se você for às definições de origem, encontrará métodos legados que não são mais usados ou podem ser implementados como no-ops na versão mais recente (mais sobre isso abaixo).


## Framework ECS

Com o `EngineApi` módulo vem uma implementação genérica e extensível de um framework ECS compartilhado, que permite tanto às cenas quanto ao próprio motor do jogo criar entities, anexar components e atualizar seus estados.

As alterações feitas de qualquer dos lados são refletidas no outro por meio da troca de mensagens. O mecanismo utilizado ([veja abaixo](#synchronization)) garante que ambas as partes concordem com a ordem de quaisquer atualizações e alcancem consistência eventual sobre o estado do mundo.

O World Explorer implementa um conjunto conhecido de components básicos (posições, formas, texturas, mídia e mais) e sabe como desserializar e aplicar mudanças ao seu estado quando recebe uma atualização. As cenas que usam o [SDK](https://docs.decentraland.org/creator) também têm utilitários para criar components personalizados, mas estes são inteiramente gerenciados pela cena (não sincronizados) e, portanto, fora do escopo do protocolo.


> [!NOTE]
> A maioria das cenas usa o Decentraland SDK, que encapsula o `EngineApi` módulo e oferece uma interface muito mais agradável e de nível mais alto para desenvolvedores de conteúdo. Cenas que acessam diretamente o protocolo de mensagens neste módulo são extremamente raras.


### Identificando Entities <a href="#identifying" id="identifying"></a>

Os IDs de Entity são inteiros simples que começam com `0`.

Por protocolo, IDs abaixo de `512` são reservados para o World Explorer e, portanto, inválidos para entities criadas pela scene. Números `0`, `1` e `2` estão efetivamente em uso (veja [entidades básicas](/docs/contributor/contributor-pt/runtime-da-scene/basic-entities/)), com o restante do intervalo disponível para futuras extensões.

Cada cena tem seu próprio intervalo privado de IDs, que o Explorer pode mapear de forma transparente para um identificador global entre todas as entities no motor do jogo.

Com o tempo, à medida que entities são criadas e deletadas, é perfeitamente válido reutilizar IDs que foram liberados anteriormente — de fato, isso pode ser necessário por razões de desempenho ([veja abaixo](#crdtstatedeleteentities)).

## Sincronização <a href="#synchronization" id="synchronization"></a>

Tanto o World Explorer quanto a cena devem gerenciar um conjunto de entities e components compartilhados, onde as atualizações vêm de ambos os lados de forma assíncrona. Sem medidas adicionais, suas versões do estado do mundo podem (e vão) acabar sendo diferentes.

Para evitar isso, um CRDT ([tipo de dado replicado sem conflitos](https://en.wikipedia.org/wiki/Conflict-free_replicated_data_type)) é usado para chegar a um acordo entre ambas as partes trocando mensagens com atualizações. Isso oferece várias vantagens:

* Ambas as partes podem atualizar o estado de forma independente e concorrente.
* Conflitos são gerenciados por uma estratégia de resolução compartilhada aplicada localmente por ambas as partes.
* Ambas as partes têm garantia de convergir para um estado compartilhado e idêntico.
* Além de uma pequena sobrecarga no tamanho da mensagem, não são necessárias coordenação adicional nem idas e voltas extras de mensagens.


> [!NOTE]
> Esta página explica o uso do CRDT para sincronizar o estado entre uma scene em execução localmente e o motor do jogo, mas o verdadeiro potencial desse mecanismo está em cenários multiplayer. Usando esse protocolo, os jogadores podem coordenar o estado do jogo e compartilhar a mesma experiência.


Para que isso funcione, todas as mensagens devem ser comutativas e idempotentes. Mensagens fora de ordem podem ser aplicadas mesmo que uma atualização supostamente intermediária ainda não tenha sido recebida, e mensagens iguais podem ser reprocessadas sem quebrar a consistência.

Vamos revisitar o diagrama arquitetural acima, agora focando no CRDT:

```
.-------------------------------------------------------.
| World Explorer                                        |
|                                                       |
| .-------------.                       .------------.  |
| | Motor do Jogo |                       |   Cena     | |
| |  .-------.  |                       |  .-------.  | |
| |  |       +--+-----------------------+->|       |  | |
| |  | CRDT  |  |   Protocolo de Sync   |  | CRDT  |  | |
| |  |       |<-+-----------------------+--+       |  | |
| |  '-------'  |                       |  '-------'  | |
| |             |                       |             | |
| '-------------'                       '-------------' |
'-------------------------------------------------------'

```

A implementação do mecanismo de sincronização CRDT consiste em três partes:

1. Uma representação interna do estado de todas as entities e components.
2. Uma estratégia de resolução de conflitos para escolher entre estados concorrentes.
3. Um protocolo de mensagens para comunicar e processar atualizações do estado.

### Estado do CRDT <a href="#crdtstate" id="crdtstate"></a>

O CRDT contém o estado de todos os components para todas as suas entities anexadas e pode ser armazenado em um mapeamento de components para estados com carimbo de data/hora para cada entity. Em pseudocódigo:

```ts
// O estado (completamente genérico) de um component para uma entity específica, em um ponto no tempo:
type EntityComponentState = { timestamp: number; state: byte[] };

// O estado de um component para todas as entities anexadas:
type ComponentState = Map<EntityId, EntityComponentState>;

// O estado completo do CRDT:
type CRDTState = Map<ComponentId, ComponentState>;
```

Os timestamps não são, na verdade, timestamps no estilo Unix. Eles são um tipo de contador incremental para sequenciar atualizações, sem relação com o tempo do relógio. Mais sobre isso [abaixo](#timestamps).

#### Criando entradas de estado <a href="#crdtstateautocreate" id="crdtstateautocreate"></a>

Como a camada CRDT não conhece (nem precisa conhecer) todos os components disponíveis, o estado inicial é um mapa vazio. As entradas são criadas dinamicamente quando as operações envolvem um `ComponentId` ou `EntityId` que ainda não estava mapeado. Quando uma entity é removida, o estado associado é excluído de todos os components.

Vamos ilustrar isso com mais pseudocódigo.

```ts
const crdtState = new Map(); // vazio, vamos abrir espaço automaticamente para novos components e entities
```

```ts
function putEntityComponentState(componentId, entityId, entityComponentState) {
  // Se este component ainda não for conhecido, adicione-o:
  if (!crdtState[componentId]) {
    crdtState[componentId] = new Map();
  }

  // Se esta entity ainda não tiver estado para este component, adicione-o e pronto:
  if (!crdtState[componentId][entityId] && shouldCreate(entityId)) {
    crdtState[componentId][entityId] = entityComponentState;
    return;
  }

  // Neste ponto, temos dois estados concorrentes. O que acabamos de receber não é necessariamente o
  // que deve ser mantido. Precisamos decidir.
  const existingEntityComponentState = crdtState[componentId][entityId];

  // Somente se as regras de resolução de conflitos assim o disserem (mais sobre isso mais tarde), substitua o estado:
  if (shouldReplace(entityComponentState, existingEntityComponentState)) {
    crdtState[componentId][entityId] = entityComponentState;
  }
}
```

O pseudocódigo acima (se traduzido em código real) está obviamente incompleto e abaixo do ideal. Em particular, ele carece da definição de `shouldCreate` e `shouldReplace`, que encapsulam a estratégia de resolução de conflitos. Estas têm requisitos:

* `shouldCreate` devem adicionar novas entities, mas recusar recriar entities deletadas.
* `shouldReplace` devem preferir manter estados que tenham timestamps maiores (com alguns casos de borda).

#### Rastreando entities deletadas <a href="#crdtstatedeleteentities" id="crdtstatedeleteentities"></a>

Para dar suporte ao `shouldCreate` requisito, você vai querer acompanhar os IDs de entity deletadas, para ignorar quaisquer futuras atualizações de estado para elas. Vamos ver isso em mais pseudocódigo:

```ts
const crdtDeletedEntities = new Set();

function shouldCreate(entityId) {
  return !crdtDeletedEntities.contains(entityId);
}

function deleteEntity(entityId) {
  // Marque esta entity como deletada:
  crdtDeletedEntities.add(entityId);

  // Exclua o estado desta entity em cada component:
  for (componentState of crdtState) {
    delete componentState[entityId];
  }
}
```


> [!NOTE]
> Observe que, na implementação ingênua acima, o conjunto de entities deletadas só vai crescer. Em scenes que reciclarem entities rapidamente, isso pode levar a uma explosão no uso de memória.
> 
> Para evitar isso, o explorer da Foundation emprega um índice geracional para reutilizar IDs em um espaço numérico finito, limitando a quantidade de memória necessária para acompanhar tanto entities ativas quanto deletadas.


Quanto ao `shouldReplace` requisito, veja [resolução de conflitos](#crdtconflicts) abaixo.

### Timestamps <a href="#timestamps" id="timestamps"></a>

Como mencionado acima, quando falamos de timestamps do CRDT, não estamos nos referindo a timestamps Unix reais para um ponto no tempo. Em vez disso, um [timestamp de Lamport](https://en.wikipedia.org/wiki/Lamport_timestamp) (um tipo especial de contador incremental compartilhado) é usado para acompanhar a sequência de eventos por ambas as partes.

O valor desse contador é atualizado de acordo com estas regras:

1. Inicialize o contador local em `0`.
2. Antes de enviar uma mensagem para partes remotas, incremente o contador local.
3. Após receber uma mensagem de uma parte remota, defina o contador local para o máximo entre seu valor e o valor recebido, mais `1`.

Em pseudocódigo:

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

### Resolução de Conflitos <a href="#crdtconflicts" id="crdtconflicts"></a>

Quando o CRDT encontra dois estados concorrentes, ele precisa de uma estratégia de resolução compartilhada que todas as partes apliquem de forma idêntica, garantindo que a mesma decisão seja alcançada por todos. O protocolo de Decentraland usa regras muito simples:

1. Se a entity foi deletada (e o ID nunca foi reutilizado), ignore o novo estado.
2. Se não havia estado anterior, mantenha o novo estado.
3. Se o novo estado tiver um timestamp maior, mantenha o novo estado.
4. Se o novo estado tiver um timestamp menor, mantenha o estado antigo.
5. Se ambos os timestamps forem iguais, compare os estados byte a byte e mantenha o menor valor.

A maioria dos casos será resolvida pelas regras `1`, `2` e `3`.

### Sincronização Inicial

Antes de uma scene começar a executar seu próprio código, o runtime preenche o CRDT com o estado de todas as [entidades básicas](/docs/contributor/contributor-pt/runtime-da-scene/basic-entities/) e seus components.

Durante essa sincronização inicial, somente o runtime pode definir o estado compartilhado. A scene não tem permissão para fazer modificações até que o processo seja concluído.

### Protocolo de Mensagens <a href="#messages" id="messages"></a>

As mensagens entre o World Explorer e o runtime da scene são estruturas em uma representação binária serializada. Cada mensagem carrega um cabeçalho indicando o tipo e o comprimento, seguido por uma carga útil específica para cada tipo de mensagem.

```
.----------------.--------------.---------------------------------.
| length: uint32 | type: uint32 |     payload: byte[length]       |
'----------------'--------------'---------------------------------'
╵         campos comuns         ╵         dependente do tipo         ╵

```

Há três tipos de mensagem no protocolo CRDT:

* [`PutComponent`](#putcomponent)
* [`DeleteComponent`](#deletecomponent)
* [`DeleteEntity`](#deleteentity)

Observe que não há `CreateComponent` ou `CreateEntity` . As regras do CRDT criam automaticamente entities e components que antes não eram conhecidos, conforme [explicado acima](#crdtstateautocreate).

#### `PutComponentMessage` <a href="#putcomponent" id="putcomponent"></a>

Atualize o `estado` de um `component` para uma `entity`, criando components e entities desconhecidos no CRDT se necessário. [Resolva conflitos](#crdtconflicts) de acordo com `timestamp`.

```
.----------------.-------------------.-------------------------------------------.
| entity: uint32 | component: uint32 | timestamp: uint32 |     state: byte[]     |
'----------------'-------------------'-------------------------------------------'
╵                       campos comuns                    ╵  dependente do component  ╵

```

O `estado` campo é uma serialização binária definida pelo component. A maioria dos components usa [protocol buffers](https://protobuf.dev) para codificar mensagens conforme especificado nos `.proto` arquivos do [pacote do protocolo de Decentraland](https://github.com/decentraland/protocol). A exceção notável a essa regra é o `Transform` component, que (sendo de longe o mais comum) tem um formato de serialização otimizado.

Essa camada adicional de serialização tem uma vantagem importante: o protocolo CRDT é agnóstico a qualquer implementação de component presente ou futura.

Se a atualização contida nesta mensagem for aplicada ao CRDT, o `estado` campo é copiado tal como está para a estrutura.

#### `DeleteComponentMessage` <a href="#deletecomponent" id="deletecomponent"></a>

Remova o estado de `component` para um `entity`. [Resolva conflitos](#crdtconflicts) de acordo com `timestamp`.

```
.----------------.-------------------.-------------------.
| entity: uint32 | component: uint32 | timestamp: uint32 |
'----------------'-------------------'-------------------'

```

#### `DeleteEntityMessage` <a href="#deleteentity" id="deleteentity"></a>

Delete `entity` (ou seja, todo o estado de components associado), esperando que o identificador nunca seja reutilizado para uma entity diferente.

```
.----------------.
| entity: uint32 |
'----------------'

```

Observe que não há `timestamp` campo. Como a vida útil de `entity` chegou ao fim, a estratégia de resolução de conflitos para quaisquer atualizações fora de ordem é simplesmente ignorá-las.

## Métodos

O conjunto de métodos em `EngineApi` fornece a interface para trocar mensagens e reconstruir o estado do mundo do zero.

### `crdtSendToRenderer` <a href="#crdtsendtorenderer" id="crdtsendtorenderer"></a>

Envie uma mensagem serializada da scene para o renderer; retorne um array de mensagens serializadas que o renderer tem para a scene.

```ts
interface Request {
  // A mensagem serializada para esta solicitação:
  data: byte[];
}

interface Response {
  // Um array de mensagens serializadas do renderer (se houver):
  data: byte[][];
}

function crdtSendToRenderer(Request): Promise<Response>;
```

### `crdtGetState` <a href="#crdtgetstate" id="crdtgetstate"></a>

```ts
interface Request {}

interface Response {
  // Se o estado tiver entities criadas pela scene:
  hasEntities: boolean;

  // Um array de mensagens que pode reconstruir o estado do CRDT:
  data: byte[][];
}

function crdtGetState(Request): Promise<Response>;
```

## Scenes Legadas

Scenes no estilo antigo (ou seja, aquelas construídas usando a versão 6 do SDK ou anterior) não usam o mecanismo moderno CRDT; em vez disso, chamam métodos em `EngineApi` que agora estão obsoletos.

O suporte para essas scenes não é um requisito para uma aplicação Decentraland em conformidade com o protocolo.


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/contributor/contributor-pt/runtime-da-scene/modulos-de-runtime/engine-api.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
