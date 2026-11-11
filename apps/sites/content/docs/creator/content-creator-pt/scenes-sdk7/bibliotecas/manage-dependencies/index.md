> Como adicionar bibliotecas a uma scene

# Gerenciar Dependências

Muitas vezes, sua scene pode precisar de funcionalidades que já estão encapsuladas em uma biblioteca reutilizável. Usar bibliotecas externas pode facilitar muito o seu trabalho.

Você pode importar bibliotecas externas para um projeto Decentraland e então referenciar os systems, components ou functions descritos nessas bibliotecas.

Confira a [página de Examples](https://studios.decentraland.org/resources?sdk_version=SDK7&resource_type=Library) para encontrar uma série de bibliotecas, criadas tanto pela Decentraland Foundation quanto por membros da comunidade, que resolvem problemas comuns.

## Via a CLI

### Instalar

Para instalar uma biblioteca na pasta do seu projeto, execute `npm i` e o nome da biblioteca no caminho raiz do seu projeto. Por exemplo:

`npm i @dcl-sdk/utils`

> **📔 Nota**: Talvez seja necessário executar `npm run start` ou `npm run build` após instalar a biblioteca para que as dependências da biblioteca sejam instaladas.

### Atualizar

Se o seu `package.json` arquivo listar a versão da biblioteca como `@latest`, então execute `npm i` para atualizar todas as bibliotecas para suas respectivas versões mais recentes.

Se o seu `package.json` especifica um número de versão, você pode atualizá-la executando o comando para instalar a biblioteca, adicionando `@` e o número da versão no final. Por exemplo:

`npm i @dcl-sdk/utils@1.7.5`

### Desinstalar

Para excluir uma biblioteca das dependências da sua scene, execute `npm rm` e o nome da biblioteca. Por exemplo:

`npm rm @dcl-sdk/utils`

É uma boa prática remover quaisquer bibliotecas que você não esteja usando. Bibliotecas não usadas ainda ocupam espaço na scene, afetando a experiência do jogador ao baixar e executar sua scene.


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-pt/scenes-sdk7/bibliotecas/manage-dependencies.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
