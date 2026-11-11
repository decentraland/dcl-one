> Mecânicas centrais de jogabilidade e interações do jogador.

# Jogabilidade e interação

Mecânicas centrais de jogabilidade e interações do jogador.

| Exemplo                                                                                                                                 | O que demonstra                                                                        | Mantido por  | Tags                                 | Fonte                                                                                                       |
| --------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------- | ------------ | ------------------------------------ | ----------------------------------------------------------------------------------------------------------- |
| [Input modifier](/docs/creator/content-creator-pt/tutoriais-e-exemplos/gameplay-interaction/input-modifier/)                               | Alterar ou restringir o input de movimento do jogador numa parte de uma scene.         | Decentraland | `movimento`, `input`, `jogador`      | [Abrir Scene](https://github.com/decentraland/sdk7-test-scenes/tree/main/scenes/0,1-input-modifier)         |
| [Particle system](/docs/creator/content-creator-pt/tutoriais-e-exemplos/gameplay-interaction/particle-system/)                             | Fogo, fumo, faíscas, neve e outros efeitos de partículas.                              | Decentraland | `partículas`, `efeitos`, `visuais`   | [Abrir Scene](https://github.com/decentraland/sdk7-test-scenes/tree/main/scenes/0,7-particle-system)        |
| [Pointer lock control](/docs/creator/content-creator-pt/tutoriais-e-exemplos/gameplay-interaction/pointer-lock-control/)                   | Capturar o input do rato para controlos personalizados.                                | Decentraland | `input`, `rato`, `camera`            | [Abrir Scene](https://github.com/decentraland/sdk7-test-scenes/tree/main/scenes/31,20-pointer-lock-control) |
| [Raycasting e collider layers](/docs/creator/content-creator-pt/tutoriais-e-exemplos/gameplay-interaction/raycasting-and-collider-layers/) | Apontar para a geometria da scene e controlar quais colliders reagem a um raycast.     | Decentraland | `interação`, `raycast`, `colisão`    | [Abrir Scene](https://github.com/decentraland/sdk7-test-scenes/tree/main/scenes/77,-1-raycast-unit-tests)   |
| [Trigger areas](/docs/creator/content-creator-pt/tutoriais-e-exemplos/gameplay-interaction/trigger-areas/)                                 | Detetar jogadores a entrar e a sair de áreas, incluindo triggers baseados em collider. | Decentraland | `interação`, `triggers`, `colisão`   | [Abrir Scene](https://github.com/decentraland/sdk7-test-scenes/tree/main/scenes/75,-9-trigger-areas)        |
| [Zombie Attack](/docs/creator/content-creator-pt/tutoriais-e-exemplos/gameplay-interaction/zombie-attack/)                                 | Uma scene de jogo completa e jogável, com inimigos e pontuação.                        | Decentraland | `game`, `cena-completa`, `pontuação` | [Abrir Scene](https://github.com/decentraland/sdk7-goerli-plaza/tree/main/zombie-attack)                    |

[Navegar por todas as cenas de exemplo](/docs/creator/content-creator-pt/tutoriais-e-exemplos/examples/)


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-pt/tutoriais-e-exemplos/gameplay-interaction.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
