> Uma cena de previsão da Copa do Mundo com chutes de bola carregados, feedback local imediato de movimento e painéis de calendário responsivos.

# Kick Off

![Pré-visualização de Kick Off](/docs/_remote/e381e39b01875ac25f3f33f9.webp)

Um jogo sazonal de previsão da Copa do Mundo.

* [Jogue no Decentraland](https://decentraland.org/jump?realm=kickoff.dcl.eth)

## Informações do projeto

**Mantido por:** Decentraland DAO

[Ver código-fonte](https://github.com/dcl-regenesislabs/kickoff-2026)

**Licença:** Não identificado neste catálogo. Consulte o repositório upstream e o mantenedor para os termos aplicáveis antes de reutilizar.

## Explorar o código

Estes links apontam para um snapshot específico da source para que as explicações continuem úteis à medida que o desenvolvimento upstream avança. São referências de aprendizagem, não uma avaliação de segurança.

### Conecta um chute carregado a um feedback visual imediato

O ciclo de carregar e soltar combina entrada de teclado e mobile, aumenta e recolore uma barra de carga, envia uma mensagem de chute e inicia o movimento local da bola. `releaseOptimistic` inicializa esse movimento.

**Contexto do projeto:** Estude-o com os `prodeNet` handlers de mensagens da sala e da bola para entender a relação entre entrada, feedback e mensagens de multiplayer.

**Referências de origem:**

* [ciclo de carregar e soltar](https://github.com/dcl-regenesislabs/kickoff-2026/blob/1813434548c1c56d9eec46575cc001d2bbdcebfd/client/ball.ts#L491-L536)
* [`releaseOptimistic`](https://github.com/dcl-regenesislabs/kickoff-2026/blob/1813434548c1c56d9eec46575cc001d2bbdcebfd/client/ball.ts#L155-L178)

### Escalar a UI a partir de um canvas de referência

`layoutScale` lê `UiCanvasInformation`, compara ambas as dimensões com um espaço de design de 1920×1080, limita o resultado e aplica um fator adicional para mobile.

**Contexto do projeto:** É um exemplo compacto de uma função auxiliar de layout partilhada; os limites e o fator mobile são ajustados para os painéis deste projeto.

**Referências de origem:**

* [`layoutScale`](https://github.com/dcl-regenesislabs/kickoff-2026/blob/1813434548c1c56d9eec46575cc001d2bbdcebfd/schedule/responsive.ts#L1-L21)


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-pt/tutoriais-e-exemplos/published-games/kick-off.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
