> Sobrevivência zumbi com progressão de armas, texto de recompensa em pool e movimentação local dos inimigos ao redor de outros zumbis e obstáculos.

# Dead Surge

![pré-visualização de Dead Surge](/docs/_remote/fbb6dfc582416446805d17d3.webp)

Jogo de sobrevivência de zombies com combate e progressão de armas.

* [Jogue no Decentraland](https://decentraland.org/jump?realm=deadsurge.dcl.eth)

## Informações do projeto

**Mantido por:** Decentraland DAO

[Ver código-fonte](https://github.com/dcl-regenesislabs/dead-surge)

**Licença:** Não identificado neste catálogo. Consulte o repositório upstream e o mantenedor para os termos aplicáveis antes de reutilizar.

## Explorar o código

Estes links apontam para um snapshot específico da source para que as explicações continuem úteis à medida que o desenvolvimento upstream avança. São referências de aprendizagem, não uma avaliação de segurança.

### Reutilizar entidades para texto de recompensa flutuante

`spawnZcRewardTextAtPosition` e o seu inicializador de pool percorrem um conjunto fixo de entidades de texto. O `rewardTextSystem` faz com que subam, fiquem voltados para a câmara e esbatam antes de os ocultar para reutilização.

**Contexto do projeto:** Leia o `RewardTextComponent` e as atualizações do tempo de jogo com estas funções para compreender o seu acompanhamento de tempo de vida.

**Referências de origem:**

* [`spawnZcRewardTextAtPosition` e o seu inicializador de pool](https://github.com/dcl-regenesislabs/dead-surge/blob/4e967ba0590e0cdf7d6550ebbf56ea3b246a36dd/src/zombie.ts#L592-L640)
* [`rewardTextSystem`](https://github.com/dcl-regenesislabs/dead-surge/blob/4e967ba0590e0cdf7d6550ebbf56ea3b246a36dd/src/zombie.ts#L815-L850)

### Conduzir os inimigos à volta uns dos outros e de obstáculos simples

`getZombieSeparationDirection` combina a repulsão dos zombies próximos. `resolveZombieMovePosition` tenta a direção desejada, uma alternativa e os eixos de movimento individuais contra obstáculos circulares de tijolo.

**Contexto do projeto:** Isto ilustra a orientação local; o comportamento depende das entradas de movimento da cena, das posições dos obstáculos e das constantes de raio.

**Referências de origem:**

* [`getZombieSeparationDirection`](https://github.com/dcl-regenesislabs/dead-surge/blob/4e967ba0590e0cdf7d6550ebbf56ea3b246a36dd/src/zombie.ts#L868-L896)
* [`resolveZombieMovePosition`](https://github.com/dcl-regenesislabs/dead-surge/blob/4e967ba0590e0cdf7d6550ebbf56ea3b246a36dd/src/zombie.ts#L898-L939)


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-pt/tutoriais-e-exemplos/published-games/dead-surge.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
