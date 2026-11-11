# Padrões de teste

Esta seção descreve a forma como os testes são realizados nos diferentes serviços e UIs em todos os projetos da Decentraland.

As palavras-chave "MUST", "MUST NOT", "REQUIRED", "SHALL", "SHALL NOT", "SHOULD", "SHOULD NOT", "RECOMMENDED", "MAY" e "OPTIONAL" neste documento devem ser interpretadas conforme descrito em [RFC 2119](https://datatracker.ietf.org/doc/html/rfc2119).

## Visão geral

Os testes são uma parte crítica do nosso processo de desenvolvimento. Mantemos altos padrões de cobertura e qualidade de testes em todos os nossos projetos. Esta documentação fornece diretrizes abrangentes para:

* [**Escrever Testes**](/docs/contributor/contributor-pt/guias-para-colaboradores/testing-standards/writing-tests/) - Princípios gerais e padrões para escrever testes claros e de fácil manutenção
* [**Testar UIs**](/docs/contributor/contributor-pt/guias-para-colaboradores/testing-standards/testing-uis/) - Testar componentes React, sagas do Redux, reducers, selectors e action creators
* [**Serviços de Teste (Well-Known-Component)**](/docs/contributor/contributor-pt/guias-para-colaboradores/testing-standards/testing-services-wkc/) - Testar serviços modulares construídos com a arquitetura well-known-component
* [**Testar Serviços (descontinuado)**](/docs/contributor/contributor-pt/guias-para-colaboradores/testing-standards/testing-services-deprecated/) - Abordagem legada para testar serviços (para referência)
* [**Testes End-to-End**](/docs/contributor/contributor-pt/guias-para-colaboradores/testing-standards/testing-e2e/) - Testes E2E com Cypress (em andamento)

## Filosofia de Testes

Nossa abordagem de testes enfatiza:

* **Clareza** - Os testes devem ser autoexplicativos e fáceis de entender
* **Manutenibilidade** - Os testes devem ser fáceis de atualizar quando os requisitos mudarem
* **Isolamento** - Cada teste deve ser executado independentemente, sem afetar os outros
* **Cobertura** - Todos os caminhos críticos e casos de borda devem ser testados
* **Valor** - Os testes devem transmitir confiança de que o código funciona como pretendido

## Links Rápidos

* [Como descrever contextos com `describe`](/docs/contributor/contributor-pt/guias-para-colaboradores/testing-standards/writing-tests/#describing-and-building-context)
* [Como escrever expectativas com `it`](/docs/contributor/contributor-pt/guias-para-colaboradores/testing-standards/writing-tests/#describing-expectations-and-executing-code)
* [O que testar e quando](/docs/contributor/contributor-pt/guias-para-colaboradores/testing-standards/writing-tests/#what-to-test)
* [Quando e como simular](/docs/contributor/contributor-pt/guias-para-colaboradores/testing-standards/writing-tests/#when-to-mock)

## Padrões relacionados

* [Gerenciamento de dependências](/docs/contributor/contributor-pt/guias-para-colaboradores/dependency-management/) - Gerenciar dependências npm e peerDependencies
* [Padrões de UI](https://github.com/decentraland/docs/tree/main/contributor/contributor-guides/ui-standards/README.md) - padrões do Redux e do RTK Query
* [Documentação da API](/docs/contributor/contributor-pt/guias-para-colaboradores/api-documentation/) - padrões de documentação OpenAPI


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/contributor/contributor-pt/guias-para-colaboradores/testing-standards.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
