> Conectando scenes a APIs externas, fontes de dados e servidores de terceiros.

# Serviços externos

Conectando scenes a APIs externas, fontes de dados e servidores de terceiros.

| Exemplo                                                                                                                                         | O que demonstra                                                                  | Mantido por  | Tags                                              | Fonte                                                                                                   |
| ----------------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------- | ------------ | ------------------------------------------------- | ------------------------------------------------------------------------------------------------------- |
| [Dados de uma planilha](/docs/creator/content-creator-pt/tutoriais-e-exemplos/external-services/spreadsheet-data/)                                 | Conteúdo da scene conduzido por uma Google Sheet, atualizado sem redeploy.       | Decentraland | `API`, `planilha`, `vídeo`, `wearables`           | [Abrir repositório](https://github.com/decentraland/Read_Spreadsheet_Scene)                             |
| [Livro de visitas de terceiros](/docs/creator/content-creator-pt/tutoriais-e-exemplos/external-services/third-party-guest-book/)                   | Um livro de visitas suportado por um serviço HTTP externo.                       | Decentraland | `API`, `armazenamento`                            | [Abrir scene](https://github.com/decentraland/sdk7-goerli-plaza/tree/main/guest-book-api)               |
| [Tabela de classificação de terceiros](/docs/creator/content-creator-pt/tutoriais-e-exemplos/external-services/third-party-leaderboard/)           | Uma tabela de classificação persistente suportada por um serviço HTTP externo.   | Decentraland | `API`, `armazenamento`, `tabela de classificação` | [Abrir scene](https://github.com/decentraland/sdk7-goerli-plaza/tree/main/leader-board)                 |
| [Autenticação de jogador de terceiros](/docs/creator/content-creator-pt/tutoriais-e-exemplos/external-services/third-party-player-authentication/) | Verificando a identidade do jogador para requests enviados a um serviço externo. | Decentraland | `API`, `autenticação`, `jogador`                  | [Abrir scene](https://github.com/decentraland/sdk7-goerli-plaza/tree/main/validate-player-authenticity) |

[Navegar por todas as scenes de exemplo](/docs/creator/content-creator-pt/tutoriais-e-exemplos/examples/)


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-pt/tutoriais-e-exemplos/external-services.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
