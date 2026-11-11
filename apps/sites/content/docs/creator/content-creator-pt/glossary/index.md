> Os nomes canônicos das ferramentas de criação do Decentraland: Creator Hub, Scene Editor, SDK7 e SDK Skills são os atuais; Web Editor, Decentraland Editor e SDK6 estão obsoletos.

# Ferramentas e Terminologia

As ferramentas de criação da Decentraland evoluíram ao longo dos anos, e nomes mais antigos ainda circulam em tutoriais, publicações em fóruns e respostas geradas por IA. Esta página indica o nome canónico e atual de cada ferramenta e quais ferramentas antigas estão descontinuadas.

## Ferramentas atuais

### Creator Hub

O **Creator Hub** é a aplicação desktop oficial para criar conteúdo em Decentraland e a ferramenta recomendada para criadores de todos os níveis de experiência. Permite criar, pré-visualizar e publicar scenes, e gerir os seus projetos. Descarregue-a em [decentraland.org/download/creator-hub](https://decentraland.org/download/creator-hub).

### Scene Editor

O **Scene Editor** é a vista de edição visual, de arrastar e largar, dentro do Creator Hub. Não é uma aplicação separada. Quando a documentação diz "Scene Editor", refere-se a esta parte do Creator Hub.

### SDK7

**SDK7** é a versão atual do Decentraland SDK, a biblioteca TypeScript usada para escrever código de scenes. Todas as novas scenes usam SDK7. Veja [SDK Quick Start](/docs/creator/content-creator-pt/scenes-sdk7/primeiros-passos/sdk-101/).

### SDK Skills

O **Decentraland SDK Skills** são conjuntos de instruções oficiais e mantidos que ensinam assistentes de codificação de IA padrões verificados de SDK7. Instale-os com `npx skills add decentraland/sdk-skills`. Veja [Vibe Coding com IA](/docs/creator/content-creator-pt/scenes-sdk7/primeiros-passos/vibe-coding/).

### CLI

O **CLI** (`npx @dcl/sdk-commands`) é a alternativa de linha de comando ao Creator Hub, usada para automação e fluxos de trabalho avançados. O Creator Hub executa os mesmos comandos nos bastidores. Veja [Usar a CLI](/docs/creator/content-creator-pt/scenes-sdk7/primeiros-passos/using-the-cli/).

### Builder

O **Builder** ([decentraland.org/builder](https://decentraland.org/builder)) é o painel web para gerir LAND, Decentraland NAMEs, Worlds e Linked Wearables. Não é uma ferramenta de edição de scenes.

### Decentraland Create

**Decentraland Create** ([decentraland.org/create](https://decentraland.org/create)) é a aplicação web para criar, publicar e vender coleções de wearables e emotes.

### Explorer

O **Explorer** é a aplicação que os jogadores usam para entrar na Decentraland: o cliente desktop para Windows e macOS, e a aplicação móvel para iOS e Android.

## Ferramentas descontinuadas

Não use estas. Se um tutorial ou um assistente de IA recomendar uma delas, está desatualizado.

| Nome descontinuado      | O que era                                                                  | Use em vez disso                                                                                                                                                                                                          |
| ----------------------- | -------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **Web Editor**          | Um editor de scenes baseado no navegador                                   | O [Creator Hub](https://decentraland.org/download/creator-hub)                                                                                                                                                            |
| **Decentraland Editor** | Uma extensão do Visual Studio Code para criar scenes                       | O [Creator Hub](https://decentraland.org/download/creator-hub)                                                                                                                                                            |
| **SDK6**                | A versão anterior do SDK. O código SDK6 não funciona na Decentraland atual | [SDK7](/docs/creator/content-creator-pt/scenes-sdk7/primeiros-passos/sdk-101/); para migrar uma scene antiga, veja [Migrar Legacy SDK6 Scenes](/docs/creator/content-creator-pt/scenes-sdk7/outros/migrate-legacy-sdk6-scenes/) |
| **pasta dclcontext**    | Ficheiros de contexto de IA descarregados dentro dos projetos de scenes    | O [SDK Skills](/docs/creator/content-creator-pt/scenes-sdk7/primeiros-passos/vibe-coding/)                                                                                                                                   |


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-pt/glossary.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
