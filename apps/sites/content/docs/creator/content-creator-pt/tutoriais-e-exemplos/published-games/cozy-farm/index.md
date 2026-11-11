> Um jogo social de fazenda com estágios de crescimento de culturas cronometrados, modificadores de fertilizante e fazendas repetidas construídas a partir de uma hierarquia de Entity.

# Cozy Farm

![Pré-visualização de Cozy Farm](/docs/_remote/f005f5c4d6dce02d9535e094.webp)

Um jogo social de agricultura focado em cultivar plantações, expandir uma fazenda e conhecer outros jogadores.

* [Jogue no Decentraland](https://decentraland.org/jump?realm=cozyfarm.dcl.eth)

## Informações do projeto

**Mantido por:** Decentraland DAO

[Ver código-fonte](https://github.com/dcl-regenesislabs/cozy-farm)

**Licença:** Não identificado neste catálogo. Consulte o repositório upstream e o mantenedor para os termos aplicáveis antes de reutilizar.

## Explorar o código

Estes links apontam para um snapshot específico da source para que as explicações continuem úteis à medida que o desenvolvimento upstream avança. São referências de aprendizagem, não uma avaliação de segurança.

### Transforme o tempo decorrido em etapas de crescimento das culturas

`growthSystem` deriva o progresso de um carimbo de data/hora de plantio, ajusta a duração para culturas de tutorial e fertilizante, e troca os modelos apenas quando uma etapa muda.

**Contexto do projeto:** Siga-o juntamente com `PlotState`, `CROP_DATA`, e os auxiliares de modelo em `game/actions` para ver como o estado da simulação impulsiona o feedback visual.

**Referências de origem:**

* [`growthSystem`](https://github.com/dcl-regenesislabs/cozy-farm/blob/440805a68d4d93a665c42cf17830735804b54a71/src/systems/growthSystem.ts#L62-L142)

### Crie várias fazendas a partir de uma única hierarquia de entities

`cloneEntityTree` recria recursivamente os filhos de um template, preserva os Transforms locais, copia os Components de rendering selecionados e registra cada clone pelo nome. Isso mostra como áreas repetidas podem compartilhar um layout criado.

**Contexto do projeto:** O helper espera o mapa de filhos da Scene e copia uma lista explícita de Components, então novos tipos de Components precisam de tratamento deliberado.

**Referências de origem:**

* [`cloneEntityTree`](https://github.com/dcl-regenesislabs/cozy-farm/blob/440805a68d4d93a665c42cf17830735804b54a71/src/systems/farmInstances.ts#L127-L168)


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-pt/tutoriais-e-exemplos/published-games/cozy-farm.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
