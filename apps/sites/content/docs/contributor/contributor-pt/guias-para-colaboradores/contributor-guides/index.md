# Visão geral

Esta seção contém os guias de contribuição e as melhores práticas para os serviços da Decentraland. Esses guias garantem consistência, qualidade e manutenibilidade em todos os projetos.

## Seções

* [Documentação da API](/docs/contributor/contributor-pt/guias-para-colaboradores/api-documentation/) - Padrões OpenAPI para documentar e publicar APIs de serviço
* [Padrões de Teste](/docs/contributor/contributor-pt/guias-para-colaboradores/testing-standards/) - Diretrizes abrangentes para escrever testes em diferentes tipos de aplicações e serviços
* [Componentes Bem Conhecidos](https://github.com/decentraland/docs/tree/main/contributor/contributor-guides/well-known-components/README.md) - Padrões para construir serviços modulares e testáveis usando a arquitetura WKC
* [Padrões de UI](https://github.com/decentraland/docs/tree/main/contributor/contributor-guides/ui-standards/README.md) - Padrões de Redux Toolkit e RTK Query para construir aplicações React modernas
* [Padrões de UI Web](https://github.com/decentraland/docs/tree/main/contributor/contributor-guides/web-ui-standards/README.md) - Padrões de biblioteca de componentes usando UI2, Material UI e o sistema de design da Decentraland

## Sobre Estes Padrões

As palavras-chave "MUST", "MUST NOT", "REQUIRED", "SHALL", "SHALL NOT", "SHOULD", "SHOULD NOT", "RECOMMENDED", "MAY" e "OPTIONAL" nestes padrões devem ser interpretadas conforme descrito em [RFC 2119](https://datatracker.ietf.org/doc/html/rfc2119).


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/contributor/contributor-pt/guias-para-colaboradores/contributor-guides.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
