> Detecte quando um jogador entra ou sai da proximidade de uma entity.

# Eventos de Proximidade

As interações por proximidade permitem que entities reajam a eventos de botão quando um jogador entra, está perto e está aproximadamente de frente para elas, sem exigir que aponte o cursor para a entity.

Os eventos de ponteiro tradicionais usam ray-casting: o jogador precisa apontar o cursor para o collider de uma entity antes de poder pressionar um botão para disparar o evento. Os eventos de proximidade, por outro lado, verificam entities que estão dentro de uma área dada ao redor do avatar, independentemente de para onde o cursor do jogador está apontando.

A área interativa considerada para interações por proximidade é uma ampla fatia triangular de uma esfera, projetada para a frente a partir da posição do jogador. Ao contrário dos eventos de ponteiro, a direção para a qual o avatar está virado é importante, independentemente da direção para a qual a câmera está virada.

Para definir uma lógica customizada mais específica, talvez você queira lidar com os dados brutos e usar o [Avançado](/docs/creator/content-creator-pt/scenes-sdk7/interatividade/eventos-de-botao/advanced-button-events/) abordagem.

Para uma entity ser interativa, ela precisa ter um [collider](/docs/creator/content-creator-pt/scenes-sdk7/fundamentos-de-conteudo-3d/colliders/). Veja [obstáculos](/docs/creator/content-creator-pt/scenes-sdk7/interatividade/eventos-de-botao/click-events/#obstacles) para mais detalhes.

## Registre callbacks de proximidade

A `pointerEventsSystem` inclui funções auxiliares para eventos de proximidade, seguindo o mesmo padrão que [callbacks de eventos de ponteiro](/docs/creator/content-creator-pt/scenes-sdk7/interatividade/eventos-de-botao/register-callback/).

### Pressões de botão por proximidade

Usa `pointerEventsSystem.onProximityDown` para detectar pressões de botão enquanto o jogador estiver ao alcance. Ao contrário de `onPointerDown`, isso não exige que o jogador aponte o cursor para a entity.

```ts
pointerEventsSystem.onProximityDown(
    {
        entity: myEntity,
        opts: {
            button: InputAction.IA_PRIMARY,
            hoverText: 'Press E',
            maxDistance: 5,
        },
    },
    function () {
        console.log('Player pressed button near entity')
    }
)
```

Usa `pointerEventsSystem.onProximityUp` para detectar quando o jogador solta um botão enquanto estiver ao alcance.

```ts
pointerEventsSystem.onProximityUp(
    {
        entity: myEntity,
        opts: {
            button: InputAction.IA_PRIMARY,
            hoverText: 'Solte E',
            maxDistance: 5,
        },
    },
    function () {
        console.log('Jogador soltou o botão perto da entity')
    }
)
```


> [!WARNING]
> **📔 Nota**: Apenas um `onProximityDown` e um `onProximityUp` podem ser registrados por entity. Depois de adicionados, eles continuam ouvindo até serem removidos. Não chame isso dentro de um loop de system, pois isso ficaria reescrevendo o comportamento.


## Tipos de evento de proximidade

Dois novos valores foram adicionados ao `PointerEventType` enum:

* `PET_PROXIMITY_ENTER`: Disparado quando o jogador entra no alcance de proximidade da entity.
* `PET_PROXIMITY_LEAVE`: Disparado quando o jogador sai do alcance de proximidade da entity.

Estes podem ser usados com um `PointerEvents` componente e o `inputSystem` para a [abordagem baseada em system](/docs/creator/content-creator-pt/scenes-sdk7/interatividade/eventos-de-botao/system-based-events/#proximity-events). Essa abordagem também suporta pressões de botão por proximidade usando o `InteractionType.PROXIMITY` field.

## Prioridade

Quando várias entities estão ao alcance e podem responder ao mesmo input, apenas uma é ativada, a mais próxima por padrão. Use o `priority` campo para controlar qual tem precedência. Entities com um **número maior** como valor de prioridade respondem primeiro.

Se o jogador estiver tanto no alcance de uma entity com uma interação por proximidade quanto se o cursor do jogador estiver apontando para uma entity que tenha uma interação de ponteiro, a entity com a interação de ponteiro sempre recebe prioridade. Mais uma vez, apenas uma entity é ativada.

```ts
pointerEventsSystem.onProximityDown(
    {
        entity: doorEntity,
        opts: {
            button: InputAction.IA_PRIMARY,
            hoverText: 'Open door',
            maxDistance: 5,
            priority: 2,
        },
    },
    function () {
        console.log('Porta ativada')
    }
)

pointerEventsSystem.onProximityDown(
    {
        entity: floorEntity,
        opts: {
            button: InputAction.IA_PRIMARY,
            hoverText: 'Pise aqui',
            maxDistance: 5,
            priority: 1,
        },
    },
    function () {
        console.log('Chão ativado')
    }
)
```

No exemplo acima, se ambas as entities estiverem ao alcance, a entity da porta responde porque tem a prioridade mais alta.

A `priority` campo também pode ser definido nas propriedades de um `PointerEvents` component.

## Opções

As funções auxiliares de proximidade aceitam as mesmas opções que seus equivalentes de ponteiro:

* `botão`: Qual botão ouvir. Veja [Botões do pointer](/docs/creator/content-creator-pt/scenes-sdk7/interatividade/eventos-de-botao/click-events/#pointer-buttons) para opções suportadas.
* `maxDistance`: Distância máxima da **avatar** do jogador até a entity, em metros. 10 por padrão.
* `hoverText`: Texto para exibir na UI quando o jogador estiver perto da entity.
* `showHighlight`: Se verdadeiro, mostra um destaque na borda da entity quando o jogador estiver ao alcance. *true* por padrão.
* `showFeedback`: Se verdadeiro, mostra feedback de hover, circulando em torno do centro da entity. *true* por padrão.
* `priority`: Resolve conflitos quando várias entities estão em proximidade. Valores mais altos têm precedência. Se várias entities tiverem o mesmo valor de prioridade, a mais próxima é escolhida.

`maxPlayerDistance` é um alias obsoleto de `maxDistance`. Ambos medem a distância a partir do avatar, então use `maxDistance` em scenes novas.

### O que conta como estar ao alcance

Os eventos de proximidade medem a partir do **centro da cápsula do jogador**, aproximadamente na altura do peito, até o ponto mais próximo no collider da entity. Isso fica cerca de um metro acima de onde um clique de cursor mede a partir de, então o mesmo ponto pode registrar uma distância ligeiramente diferente em cada caminho.

A distância por si só não é suficiente. Para disparar um evento de proximidade, o jogador também deve:

* Estar **virado para** a entity, dentro de um cone de cerca de 120 graus à sua frente.
* Ter uma **linha de visão desobstruída** até ela, sem nada no meio.


> [!WARNING]
> **📔 Nota**: Os eventos de proximidade ignoram `maxCameraDistance`. Onde a câmera está posicionada não faz diferença. Apenas a distância, a orientação e a linha de visão do avatar são consideradas. Veja [Limites de distância](/docs/creator/content-creator-pt/scenes-sdk7/interatividade/eventos-de-botao/register-callback/#distance-limits) para ver como a limitação da câmera funciona em eventos de cursor.


## Remover callbacks

Para remover um callback de proximidade, use a função de remoção correspondente:

```ts
pointerEventsSystem.removeOnProximityDown(myEntity)
pointerEventsSystem.removeOnProximityUp(myEntity)
pointerEventsSystem.removeOnProximityEnter(myEntity)
pointerEventsSystem.removeOnProximityLeave(myEntity)
```

Depois de removida, a entity deixa de reagir a esse evento de proximidade.

## Entrada e saída por proximidade

Usa `pointerEventsSystem.onProximityEnter` para executar um callback quando o jogador entrar no alcance de proximidade da entity, e `pointerEventsSystem.onProximityLeave` quando sair. Isso pode ser usado para exibir dicas extras de feedback ao jogador, por exemplo um som ou uma animação quando um item pode ser interagido.

```ts
pointerEventsSystem.onProximityEnter(
    {
        entity: myEntity,
        opts: {
            button: InputAction.IA_POINTER,
            hoverText: 'Perto',
            maxDistance: 5,
        },
    },
    function () {
        console.log('Player entrou na proximidade')
    }
)

pointerEventsSystem.onProximityLeave(
    {
        entity: myEntity,
        opts: {
            button: InputAction.IA_POINTER,
            hoverText: 'Perto',
            maxDistance: 5,
        },
    },
    function () {
        console.log('Player saiu da proximidade')
    }
)
```

## Exemplo: porta por proximidade

O exemplo a seguir abre ou fecha uma porta quando o jogador pressiona um botão enquanto está perto, sem precisar apontar para a porta.

```ts
const doorPivot = engine.addEntity()
Transform.create(doorPivot, { position: Vector3.create(3, 0, 4) })

const door = engine.addEntity()
GltfContainer.create(door, { src: 'assets/door.glb' })
Transform.create(door, { position: Vector3.create(-1, 0, 0), parent: doorPivot })

let isDoorOpen = false
const closedRot = Quaternion.fromEulerDegrees(0, 0, 0)
const openRot = Quaternion.fromEulerDegrees(0, 90, 0)

pointerEventsSystem.onProximityDown(
    {
        entity: door,
        opts: {
            button: InputAction.IA_PRIMARY,
            hoverText: 'Abrir / Fechar',
            maxDistance: 5,
            priority: 1,
        },
    },
    function () {
        if (isDoorOpen) {
            Tween.setRotate(doorPivot, openRot, closedRot, 700)
            isDoorOpen = false
        } else {
            Tween.setRotate(doorPivot, closedRot, openRot, 700)
            isDoorOpen = true
        }
    }
)
```


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-pt/scenes-sdk7/interatividade/eventos-de-botao/proximity-events.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
