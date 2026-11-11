# Como criar um novo tipo de Entity

#### Quando é necessário criar uma nova entity?

Primeiro, verifique se uma entity existente pode resolver o caso de uso necessário para evitar sobreengenharia de uma solução. Se nenhum dos tipos de entity existentes definidos em <https://github.com/decentraland/common-schemas/blob/main/src/platform/entity.ts#L13> satisfizer os requisitos, então poderá ser necessário criar um novo tipo de entity.

#### Primeira resposta:

* A nova entity é uma NFT Collection?
* Como será validada a ownership?
* Qual será o seu tamanho?
* Qual é a quantidade de entities que serão criadas?

#### Etapas

Primeiro, crie um ADR em <https://github.com/decentraland/adr> onde definirá o schema e a data para começar a validar a nova entity.

**Repositórios**

**@dcl/schemas**

1. Crie o novo tipo de entity no enum: <https://github.com/decentraland/common-schemas/blob/main/src/platform/entity.ts#L19>
2. Crie a pasta e os ficheiros em <https://github.com/decentraland/common-schemas/tree/main/src/platform> gerando o tipo e o JSON schema, por exemplo: <https://github.com/decentraland/common-schemas/blob/main/src/platform/profile/profile.ts#L20>

**@dcl/content-validator**

Implemente as validações definidas no ADR, tendo em conta a data desse ADR em <https://github.com/decentraland/content-validator/blob/main/src/validations/timestamps.ts>. Por exemplo: <https://github.com/decentraland/content-validator/blob/main/src/validations/items/emotes.ts#L8>

1. Adicione o código para verificar a ownership do novo tipo de entity em <https://github.com/decentraland/content-validator/tree/main/src/validations/access-checker>
2. Implemente o validationForType para o novo tipo, por exemplo: <https://github.com/decentraland/content-validator/blob/main/src/validations/profile.ts#L112>
3. Aqui terá de definir o tamanho máximo por entity em <https://github.com/decentraland/content-validator/blob/main/src/validations/ADR51.ts>.

**@dcl/urn-resolver**

1. Se a nova entity corresponder a uma Collection, então não é necessário adicionar nada.
2. Se não, então crie um novo urn correspondente à nova entity e defina-o em <https://github.com/decentraland/urn-resolver> adicionando um resolver em <https://github.com/decentraland/urn-resolver/blob/main/src/resolvers.ts#L23>

**Catalyst: Content Server**

1. Não é necessária nenhuma alteração na base de dados.
2. Atualize todas as libraries modificadas detalhadas acima.

**Catalyst: Lambdas Server**

1. Atualize todas as libraries modificadas detalhadas acima.
2. Verifique se a sanitização do profile precisa de ser atualizada (`/lambdas/profiles` endpoint)
3. Verifique se o endpoint das coleções ERC precisa de ser atualizado (`/lambdas/collections` endpoint)
4. Crie um endpoint para mostrar essa entity, por exemplo `/lambdas/wearables-by-owner`


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/contributor/contributor-pt/tutoriais/how-to-create-new-entity-type.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
