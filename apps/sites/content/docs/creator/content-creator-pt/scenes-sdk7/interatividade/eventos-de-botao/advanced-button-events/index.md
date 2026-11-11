> Saiba como lidar com cliques de usuário na sua scene.

# Eventos Avançados de Botão

Se precisar que a interação na sua Scene siga uma lógica personalizada que não seja compatível com a [Registar callback](/docs/creator/content-creator-pt/scenes-sdk7/interatividade/eventos-de-botao/register-callback/) ou no [Baseado em System](/docs/creator/content-creator-pt/scenes-sdk7/interatividade/eventos-de-botao/system-based-events/) abordagens, pode lidar diretamente com os dados brutos do evento de input. Esta abordagem é a mais difícil, mas também a mais flexível.

## O componente PointerEventsResult

Quando um pointer event é detetado pelo engine, a Entity que foi clicada recebe um `PointerEventsResult` componente. Este componente contém todos os dados brutos sobre o hit event e armazena dados históricos sobre eventos anteriores.

Os helpers em `inputSystem`, como `inputSystem.isTriggered` ou `inputSystem.getInputCommand` são bons para a maioria dos cenários simples, mas, se precisar de obter mais detalhes sobre o hit event, verifique os dados brutos no `PointerEventsResult`.

O `PointerEventsResult` Este componente contém uma lista de pointer events, contendo um objeto para cada evento. Armazena uma lista de até 100 eventos; os eventos mais recentes são armazenados no final da lista. Quando a lista atinge um comprimento de 100, começa a descartar os eventos antigos a cada novo que chega.

Cada evento na lista tem os seguintes dados:

* `analógico`: Número opcional, presente apenas em eventos de uma entrada analógica (como um joystick), armazenando o valor analógico da entrada.
* `button`: Que button id foi pressionado. O número corresponde ao `InputAction` enum, que lista todos os botões disponíveis.
* `state`: Tipo de pointer event, da enum `PointerEventType`. *0* refere-se a `PointerEventType.PET_UP`, *1* como `PointerEventType.PET_DOWN`, *2* como `PointerEventType.PET_HOVER_ENTER`, *3* como `PointerEventType.PET_HOVER_LEAVE`, *4* como `PointerEventType.PET_PROXIMITY_ENTER`, *5* como `PointerEventType.PET_PROXIMITY_LEAVE`
* `timestamp`: Um [timestamp de Lamport](https://en.wikipedia.org/wiki/Lamport_timestamp) para identificar cada evento de button.

  > Nota: Este timestamp não é numerado com base no tempo atual. Pense nele como um contador que começa em 0 e é incrementado de 1 em 1 por cada evento.
* `impacto`: Um objeto que contém os seguintes dados sobre o hit event:
  * `entityId`: Número de Id da entidade que foi atingida pelo ray.
  * `meshName`: *String* com o nome interno da malha específica no modelo 3D que foi atingida. Isto é útil quando um modelo 3D é composto por várias malhas.
  * `globalOrigin`: *Vector3* para a posição onde o ray se origina (relativa à cena)
  * `direction`: *Vector3* com o vetor de direção do ray, em coordenadas globais
  * `posição`: *Vector3* para a posição onde o ray intersectou a entidade atingida (relativa à cena)
  * `length`: Comprimento do ray desde a sua origem até à posição onde ocorreu o impacto na entidade.
  * `normalHit`: *Vector3* com um vetor de direção normalizado, descrevendo o ângulo da normal do impacto no world space.

```ts
import { engine, PointerEventsResult } from '@dcl/sdk/ecs'

function PointerReadingSystem() {
  for (const [entity] of engine.getEntitiesWith(PointerEventsResult)) {
    const events = PointerEventsResult.get(entity)
    for (const event of events) {
      console.log('POINTER EVENT DATA:', event)
    }
  }
}

engine.addSystem(PointerReadingSystem)
```

## Acompanhar movimentos do jogador

Em jogos multijogador em tempo real, em que o timing dos movimentos do jogador é crítico, pode querer acompanhar a posição de cada jogador usando um Server de terceiros como fonte da verdade. Pode melhorar o tempo de resposta ao escutar o botão com antecedência e prever os seus efeitos no seu Server antes de o avatar ter mudado de posição.

Esta abordagem ajuda a compensar atrasos de rede, mas acabará por resultar em discrepâncias, por isso também deve consultar regularmente a posição atual do jogador para fazer correções. Equilibrar estas previsões e correções pode exigir bastante ajuste fino.


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-pt/scenes-sdk7/interatividade/eventos-de-botao/advanced-button-events.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
