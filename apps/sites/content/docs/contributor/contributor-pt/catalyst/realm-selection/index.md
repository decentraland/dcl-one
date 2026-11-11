# Seleção de Realm

Um [Catalyst](https://github.com/decentraland/catalyst) é um Server que agrupa diferentes Services necessários para o Decentraland World. Algumas das principais responsabilidades do server são o gerenciamento do armazenamento descentralizado da maior parte do conteúdo necessário ao client e a orquestração das comunicações entre peers.

Ao entrar em um Decentraland Client, você precisa se conectar a um Catalyst específico para o suporte dos serviços de backend Content, Lambdas e Comms. Um dos Catalysts da DAO precisa ser usado; ele pode ser escolhido aleatoriamente, mas isso pode levar o usuário a acessar um serviço de backend com grande latência ou um realm com uma baixa quantidade de usuários enquanto eles estão conectados a outro.

Para mitigar isso, um algoritmo de seleção de realm é implementado no Kernel, כפי descrito em [ADR-86](https://rfc.decentraland.org/adr/ADR-86).

É um algoritmo configurável que usa diferentes critérios em uma lista ordenada; se uma das condições for forte o suficiente para tomar uma decisão, então ele escolhe um realm; caso contrário, delega a decisão aos critérios seguintes. Se nenhuma condição for forte o suficiente, então uma decisão padrão é tomada (aleatória).

A configuração padrão é:

1. Prioriza Catalyst Nodes com a maior quantidade de usuários
2. Prioriza Catalyst Nodes com mais usuários na localização visitada

Outros critérios que podem ser usados são:

* Grande Latência: Prioriza os Catalysts com a menor latência
* Balanceamento de Carga: É um mecanismo Round Robin para distribuir todos os peers entre todos os Catalysts.


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/contributor/contributor-pt/catalyst/realm-selection.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
