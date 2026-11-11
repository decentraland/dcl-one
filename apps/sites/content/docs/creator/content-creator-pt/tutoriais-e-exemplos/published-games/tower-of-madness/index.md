> Platforming cronometrado por seções da torre em mudança, com pré-carregamento de assets e um tour de câmera pelo nível que pode ser ignorado.

# Tower of Madness

![Pré-visualização de Tower of Madness](/docs/_remote/ec24d5b235cc62f71cd1eae3.webp)

Um jogo de plataforma cronometrado em que os jogadores correm para chegar ao topo.

* [Jogue no Decentraland](https://decentraland.org/jump?realm=towerofmadness.dcl.eth)

## Informações do projeto

**Mantido por:** Decentraland DAO

[Ver código-fonte](https://github.com/dcl-regenesislabs/towerofmadness)

**Licença:** Não identificado neste catálogo. Consulte o repositório upstream e o mantenedor para os termos aplicáveis antes de reutilizar.

## Explorar o código

Estes links apontam para um snapshot específico da source para que as explicações continuem úteis à medida que o desenvolvimento upstream avança. São referências de aprendizagem, não uma avaliação de segurança.

### Pré-carregue as partes de um nível em mudança

`setupChunkPreload` deduplica os caminhos dos assets dos chunks e cria entidades GLTF temporárias. Um system monitora seus estados de carregamento, remove as entidades após o carregamento ser concluído ou falhar e se desregistra quando não resta nenhuma.

**Contexto do projeto:** Isto ilustra o gerenciamento de um ciclo de vida de pré-carregamento; a lista de assets vem de `shared/chunks` e é específica da torre.

**Referências de origem:**

* [`setupChunkPreload`](https://github.com/dcl-regenesislabs/towerofmadness/blob/be318c6f0a287c15f3771f12f629b84de6cc18af/src/chunkPreload.ts#L22-L61)

### Crie um tour de câmera ignorável em torno de geometria em mudança

`playCinematic` ajusta os keyframes da câmera em torno dos chunks inicial e final atuais e, em seguida, seleciona uma virtual camera. O update system interpola a posição e os alvos de look-at e permite manter um botão pressionado para ignorar.

**Contexto do projeto:** Siga os helpers de busca de chunks e `skipCinematic` para ver como o tour encontra seus pontos âncora e devolve o controle à câmera do player.

**Referências de origem:**

* [`playCinematic`](https://github.com/dcl-regenesislabs/towerofmadness/blob/be318c6f0a287c15f3771f12f629b84de6cc18af/src/cinematicCamera.ts#L110-L209)
* [sistema de atualização](https://github.com/dcl-regenesislabs/towerofmadness/blob/be318c6f0a287c15f3771f12f629b84de6cc18af/src/cinematicCamera.ts#L286-L334)


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-pt/tutoriais-e-exemplos/published-games/tower-of-madness.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
