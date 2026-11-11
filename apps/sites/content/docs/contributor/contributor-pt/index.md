# Bem-vindo, colaborador

Decentraland é um mundo virtual propriedade dos seus utilizadores, desenvolvido de forma aberta e pronto a crescer com as contribuições de pessoas de todo o mundo.

Esta documentação detalha o protocolo aberto da Decentraland, desde a visão geral da arquitetura até às capacidades específicas de cada componente. Aqui encontrará os recursos de que precisa para:

1. Conceber produtos e aplicações no ecossistema
2. Realizar investigação e analisar o conteúdo do mundo
3. Criar o seu próprio World Explorer
4. Prosseguir ideias novas que possam vir a entrar nesta lista

**Vamos construir o metaverso juntos!**

## O que Você Encontrará Aqui

Esta documentação foi concebida para o ajudar a compreender e a contribuir para os projetos da Decentraland. Quer seja um novo contribuinte ou um developer experiente, encontrará:

* **Visão Geral da Arquitetura** - Compreender o protocolo completo da Decentraland
* **Padrões de Código** - Aprender como escrevemos código limpo e de fácil manutenção
* **Práticas de Teste** - Compreender as nossas estratégias abrangentes de teste
* **Diretrizes de Documentação** - Descobrir como documentamos o nosso trabalho
* **Referências da API** - Documentação completa de todos os serviços

## Por Onde Começar?

Se não sabe por onde começar, é uma boa ideia consultar a [Arquitetura](/docs/contributor/contributor-pt/arquitetura/architecture/) secção para aprender sobre os diferentes componentes e a sua finalidade geral antes de avançar para secções mais detalhadas.

Para os contribuidores, comece por explorar os [Guias do Contribuinte](/docs/contributor/contributor-pt/guias-para-colaboradores/contributor-guides/) secção, que cobre tudo, desde os padrões de teste até às diretrizes de qualidade de código.

### Links Rápidos

<table data-view="cards"><thead><tr><th></th><th></th><th></th><th data-hidden data-card-target data-type="content-ref"></th></tr></thead><tbody><tr><td><strong>Arquitetura</strong></td><td>Visão geral da arquitetura do sistema</td><td></td><td><a href="/docs/contributor/contributor-pt/arquitetura/architecture/">Visão geral</a></td></tr><tr><td><strong>Guias do Contribuinte</strong></td><td>Diretrizes e melhores práticas</td><td></td><td><a href="/docs/contributor/contributor-pt/guias-para-colaboradores/contributor-guides/">Visão geral</a></td></tr><tr><td><strong>Referência da API</strong></td><td>Documentação da API para serviços</td><td></td><td><a href="https://github.com/decentraland/docs/tree/main/apis/README.md">https://github.com/decentraland/docs/tree/main/apis/README.md</a></td></tr></tbody></table>

## Recursos Adicionais

Para obter informações adicionais e continuar a aprender, talvez queira consultar:

* O [Sessões de Pergunte-me Qualquer Coisa](https://www.notion.so/decentraland/Decentraland-Foundation-Technical-AMA-230fcf1ca1ec4d49922744fe91e6995d?d=8508f4ca20eb4623b578feb235f86cef#bff4e53f1c664d66802e4770f46b959c), onde membros da comunidade podem apresentar perguntas, comentários ou preocupações às equipas técnicas da Decentraland Foundation. São realizadas mensalmente e pode assistir livremente!
* O [página de estado](https://status.decentraland.org/), para ver o estado atual da plataforma, o histórico de incidentes e algumas métricas.
* O [Registos de Decisão de Arquitetura](https://adr.decentraland.org/), uma coleção de documentos de design que descrevem o estado e a evolução de diferentes aspetos da Decentraland.
* O [organização da Decentraland no GitHub](https://github.com/decentraland), onde estão alojados todos os repositórios de código aberto.

## Contribuir

Ao seguir estes padrões, ajuda a manter a qualidade e a consistência do código em todos os projetos da Decentraland. Estas diretrizes são atualizadas continuamente para refletir as nossas melhores práticas em evolução e as lições aprendidas com a nossa experiência de desenvolvimento.

Se tiver perguntas ou sugestões sobre estes padrões, contacte a comunidade de desenvolvimento da Decentraland.


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/contributor/contributor-pt/readme.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
