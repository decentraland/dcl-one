# Áreas de Trigger

Saiba como usar áreas de trigger na sua scene

Trigger areas permitem que você reaja ao evento de um player entrando ou saindo de uma área, ou de qualquer outra entity entrando ou saindo de uma área. Esta é uma ferramenta fundamental para criar scenes interativas. Use-as para coisas como abrir uma porta quando o player se aproxima, ou marcar um ponto quando uma bola entra no gol.

## Usando trigger areas

Para usar trigger areas, você precisa adicionar um `TriggerArea` component a uma entity, depois use um `triggerAreaEventsSystem` para reagir aos eventos.

```ts
import { engine, Transform, TriggerArea, triggerAreaEventsSystem } from '@dcl/sdk/ecs'

// criar entity
const triggerEntity = engine.addEntity()

// definir Transform
Transform.create(triggerEntity, {
  position: Vector3.create(8, 0, 8)
  })

// Área de trigger
TriggerArea.setBox(triggerEntity)

// Evento quando a trigger area é ativada
triggerAreaEventsSystem.onTriggerEnter(triggerEntity, function(result) {
  if (result.trigger?.entity !== engine.PlayerEntity) return;
  console.log('Player entrou na trigger area!')
})
```

Por padrão, o `TriggerArea` component reage ao evento de *qualquer* player entrando na área. O código acima adiciona `if (result.trigger?.entity !== engine.PlayerEntity) return` para verificar se a entity que causou o evento é o player atual, e não o avatar de outra pessoa

## Formas da trigger area

Trigger areas podem ser uma box ou uma sphere.

```ts
import { engine, Transform, TriggerArea } from '@dcl/sdk/ecs'

// Box
TriggerArea.setBox(triggerEntity)

// Sphere
TriggerArea.setSphere(triggerEntity)
```


> [!NOTE]
> **💡 Dica**: A sphere é a forma mais fácil de calcular para o engine, pois isso é feito verificando a distância a partir do centro da sphere. Se estiver em dúvida, use uma sphere.


Para alterar o tamanho da trigger area, você pode usar a `scale` propriedade do `Transform` component na entity que contém a `TriggerArea`.

```ts
import { engine, Transform, TriggerArea } from '@dcl/sdk/ecs'

const triggerEntity = engine.addEntity()

TriggerArea.setBox(triggerEntity)

Transform.create(triggerEntity, {
  scale: Vector3.create(4, 2, 4),
})
```

### Depuração

Para depurar sua scene e ver a área coberta pela trigger area, você pode adicionar um `MeshRenderer` component à entity com a trigger area, e definir a shape para a que você quer depurar. As dimensões do mesh padrão corresponderão às dimensões da trigger area.

```ts
import { engine, Transform, TriggerArea } from '@dcl/sdk/ecs'

const triggerEntity = engine.addEntity()

TriggerArea.setBox(triggerEntity)

MeshRenderer.setBox(triggerEntity)

Transform.create(triggerEntity, {
  position: Vector3.create(8, 0, 8),
})
```

## Eventos da trigger area

Você pode usar o `triggerAreaEventsSystem` para reagir aos diferentes eventos de uma trigger area:

* `onTriggerEnter`: Disparado quando uma entity entra na trigger area.
* `onTriggerExit`: Disparado quando uma entity sai da trigger area.
* `onTriggerStay`: Disparado enquanto uma entity está na trigger area, a cada frame.

```ts
import { engine, Transform, TriggerArea, triggerAreaEventsSystem } from '@dcl/sdk/ecs'

const triggerEntity = engine.addEntity()

TriggerArea.setBox(triggerEntity)

// Ao entrar
triggerAreaEventsSystem.onTriggerEnter(triggerEntity, function(result) {
  console.log('Player entrou na trigger area!')
})

// Ao sair
triggerAreaEventsSystem.onTriggerExit(triggerEntity, function(result) {
  console.log('Player saiu da trigger area!')
})

// Ao permanecer
triggerAreaEventsSystem.onTriggerStay(triggerEntity, function(result) {
  console.log('Player está na trigger area!')
})
```

## Respostas aos eventos de trigger

Quando um evento de trigger area é disparado, você pode usar o `result` parâmetro para obter informações tanto sobre a entity que foi disparada quanto sobre a entity que disparou o evento.

As seguintes propriedades estão disponíveis no `result` parâmetro:

* `triggeredEntity`: O ID da entity que foi disparada (esta é a entity que possui a trigger area)
* `triggeredEntityPosition`: A posição da entity que foi disparada
* `triggeredEntityRotation`: A rotação da entity que foi disparada
* `eventType`: O tipo de evento de trigger (ENTER, EXIT, STAY)
* `timestamp`: O timestamp do evento de trigger
* `trigger`: Um objeto com os seguintes campos:
  * `entity`: O ID da entity que disparou o trigger (a entity que entrou na trigger area)
  * `layers`: As collision layers da entity que disparou o trigger
  * `position`: A posição da entity que disparou o trigger
  * `rotation`: A rotação da entity que disparou o trigger
  * `scale`: A scale da entity que disparou o trigger


> [!WARNING]
> **Confusão comum**: apesar do nome, `result.triggeredEntity` faz **não** se refere à entity que disparou o evento. Esses dois campos são facilmente confundidos:
> 
> * `result.trigger?.entity`: a entity que entrou na trigger area. Normalmente é isso que você quer verificar, por exemplo para identificar qual player ou objeto entrou.
> * `result.triggeredEntity`: a entity que possui o `TriggerArea` component (a própria zona de trigger, não a entity que entrou nela).
> 
> Use `result.trigger?.entity` quando precisar identificar a entity que entrou.


```ts
import { engine, Transform, TriggerArea, triggerAreaEventsSystem } from '@dcl/sdk/ecs'

// Entity com a trigger area
const triggerEntity = engine.addEntity()

TriggerArea.setBox(triggerEntity)

Transform.create(triggerEntity, {
  position: Vector3.create(8, 0, 8),
})

// Ao entrar
triggerAreaEventsSystem.onTriggerEnter(triggerEntity, function(result) {
  console.log('Uma entity entrou na trigger area!', result.triggeredEntity)
  console.log('Posição da entity disparada: ', result.triggeredEntityPosition)
  console.log('Rotação da entity disparada: ', result.triggeredEntityRotation)
  console.log('Tipo de evento: ', result.eventType)
  console.log('Timestamp: ', result.timestamp)
  console.log('Trigger entity: ', result.trigger.entity)
  console.log('Trigger layers: ', result.trigger.layers)
  console.log('Trigger position: ', result.trigger.position)
  console.log('Trigger rotation: ', result.trigger.rotation)
  console.log('Trigger scale: ', result.trigger.scale)
})
```

## Detectar players entrando em uma área

O uso mais comum de trigger areas é reagir a players entrando ou saindo. Duas collision layers detectam avatars, e qual usar depende de com quem a scene deve reagir:

* `ColliderLayer.CL_MAIN_PLAYER`: Detecta **apenas o player usando a máquina local**. Use isso quando a resposta for destinada apenas a esse player, por exemplo para teleportá-lo, reproduzir um som para ele ou acompanhar seu progresso em uma quest. Esta também é a opção mais barata em termos de performance.
* `ColliderLayer.CL_PLAYER`: Detecta **todos os avatars**, tanto o player local quanto quaisquer outros players sendo renderizados na scene. Use isso quando importar quem entra, por exemplo uma porta que se abre quando alguém se aproxima, ou para contar quantas pessoas estão em uma sala. Esta é a layer padrão se nenhuma for especificada.

O exemplo a seguir reage apenas ao player local:

```ts
import { engine, Transform, TriggerArea, ColliderLayer, triggerAreaEventsSystem } from '@dcl/sdk/ecs'

// criar entity
const triggerEntity = engine.addEntity()

// definir Transform
Transform.create(triggerEntity, {
  position: Vector3.create(8, 0, 8),
})

// Trigger area que detecta apenas o player local
TriggerArea.setBox(triggerEntity, ColliderLayer.CL_MAIN_PLAYER)

// Evento quando a trigger area é ativada
triggerAreaEventsSystem.onTriggerEnter(triggerEntity, function (result) {
  console.log('Você entrou na trigger area!')
})
```

Ao usar `CL_PLAYER`, todos os avatars ativam a trigger area. Para saber se o avatar que a acionou é o player local, compare a entity no result com `engine.PlayerEntity`:

```ts
import { engine, Transform, TriggerArea, triggerAreaEventsSystem } from '@dcl/sdk/ecs'

// criar entity
const triggerEntity = engine.addEntity()

// definir Transform
Transform.create(triggerEntity, {
  position: Vector3.create(8, 0, 8),
})

// Trigger area, usando a layer padrão CL_PLAYER
TriggerArea.setBox(triggerEntity)

// Evento quando a trigger area é ativada
triggerAreaEventsSystem.onTriggerEnter(triggerEntity, function (result) {
  if (result.trigger?.entity === engine.PlayerEntity) {
    console.log('Você entrou na trigger area!')
  } else {
    console.log('Outro player entrou na trigger area!')
  }
})
```

Na maioria dos casos, em vez de tentar detectar todos os players, a abordagem ideal é detectar apenas o player atual (via `CL_MAIN_PLAYER`) e então sincronizar os efeitos desse trigger entre todos os players. Por exemplo, uma porta deslizante abre apenas para o seu avatar, mas depois o estado aberto dessa porta é compartilhado com todos os outros players.


> [!NOTE]
> **💡 Dica**: Trigger areas cuja mask inclui apenas avatar layers (`CL_PLAYER`, `CL_MAIN_PLAYER`, ou ambos) são otimizadas: o engine as coloca em uma physics layer apenas para avatars, então overlaps com qualquer coisa diferente de um avatar são descartados pelo physics engine antes de chegarem ao código da sua scene. Uma trigger area com uma mask exatamente igual a `CL_MAIN_PLAYER` é otimizada ainda mais: ela interrompe qualquer overlap com colliders que não sejam do player local antes de chegar ao handler, então detectar apenas o player local é essencialmente gratuito.


## Layers da trigger area

Use o segundo argumento opcional do `TriggerArea` component para definir as layers que ativarão a trigger area. Por padrão, a trigger area é ativada apenas pela layer `ColliderLayer.CL_PLAYER`, veja [Detectar players entrando em uma área](#detect-players-entering-an-area).

Você pode alterar a collision layer para detectar qualquer outra entity passando-a como o segundo argumento do `TriggerArea` componente.

```ts
import { engine, Transform, TriggerArea, MeshCollider, ColliderLayer, triggerAreaEventsSystem } from '@dcl/sdk/ecs'

// Área de trigger
const triggerEntity = engine.addEntity()

TriggerArea.setBox(triggerEntity, ColliderLayer.CL_CUSTOM1)

Transform.create(triggerEntity, {
  position: Vector3.create(8, 0, 8),
})

// Entity que ativará a trigger area
const movingEntity = engine.addEntity()

Transform.create(movingEntity, {
  position: Vector3.create(8, 0, 8),
})

MeshCollider.setBox(movingEntity, ColliderLayer.CL_CUSTOM1)
```

Os valores permitidos são os mesmos que os do `MeshCollider` componente. Veja [Collision layers](/docs/creator/content-creator-pt/scenes-sdk7/fundamentos-de-conteudo-3d/colliders/#collision-layers) para mais detalhes.

* `ColliderLayer.CL_PLAYER`: qualquer avatar (local + remoto)
* `ColliderLayer.CL_MAIN_PLAYER`: apenas o player local
* `ColliderLayer.CL_PHYSICS`
* `ColliderLayer.CL_POINTER`
* `ColliderLayer.CL_CUSTOM1` até `CL_CUSTOM8`
* `ColliderLayer.CL_NONE`


> [!NOTE]
> **💡 Dica**: As layers `CL_CUSTOM1` até `CL_CUSTOM8` não têm nenhum comportamento especial por si só; você pode usá-las para o que melhor se adequar à sua scene.


Você também pode configurar uma trigger area para detectar várias layers de uma vez.

```ts
import { engine, Transform, TriggerArea, ColliderLayer, triggerAreaEventsSystem } from '@dcl/sdk/ecs'

const triggerEntity = engine.addEntity()

TriggerArea.setBox(triggerEntity, ColliderLayer.CL_CUSTOM1 | ColliderLayer.CL_CUSTOM2)

Transform.create(triggerEntity, {
  position: Vector3.create(8, 0, 8),
})
```

Isso ativará a trigger area quando qualquer entity com as layers `CL_CUSTOM1` ou `CL_CUSTOM2` entrar na trigger area.


> [!NOTE]
> **💡 Dica**: Para um exemplo funcional que utiliza `TriggerArea` end-to-end, veja a [`75,-9-trigger-areas`](https://github.com/decentraland/sdk7-test-scenes/tree/main/scenes/75,-9-trigger-areas) scene de teste, que cobre o wire contract de ENTER/EXIT e a synthesis por tick do SDK, `onTriggerStay` áreas rotacionadas, triggerers em movimento em custom collider layers e áreas anexadas a entities em queda. Para ver como as próprias collider layers se comportam, veja [`5,5-collider-layers`](https://github.com/decentraland/sdk7-test-scenes/tree/main/scenes/5,5-collider-layers).



---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-pt/scenes-sdk7/fundamentos-de-conteudo-3d/trigger-areas.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
