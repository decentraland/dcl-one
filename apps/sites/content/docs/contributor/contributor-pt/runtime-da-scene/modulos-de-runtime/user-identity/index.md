# Identidade do utilizador

O `UserIdentity` módulo permite [scenes](/docs/contributor/contributor-pt/conteudo/tipos-de-entity/scenes/) acessar o [perfil](/docs/contributor/contributor-pt/conteudo/tipos-de-entity/profiles/) do usuário do player.

```ts
const UserIdentity = require("~system/UserIdentity");
```

Ele contém os seguintes métodos e tipos:

* [`function getUserData`](#getuserdata)
* [`interface UserData`](#userdata)

### Métodos

Os dois métodos neste módulo obtêm partes de informação do usuário do player.

#### `getUserData` <a href="#getuserdata" id="getuserdata"></a>

Retorna a [`UserData`](#userdata) do usuário do player.

```ts
interface Request {}

interface Response {
  // As informações de perfil do usuário, se disponíveis.
  data?: UserData;
}

function getUserData(Request): Promise<Response>;
```

### Tipos

Há apenas um tipo neste módulo: `UserData`.

#### `UserData` <a href="#userdata" id="userdata"></a>

Contém informações (possivelmente parciais) sobre um usuário, sua identidade e avatar.

World Explorers podem obter isso por meio do [sistema de conteúdo](/docs/contributor/contributor-pt/conteudo/overview/).

O [definição de entidade de perfil](/docs/contributor/contributor-pt/conteudo/tipos-de-entity/profiles/) detalha todos esses campos e mais. Embora a estrutura e as chaves de `UserData` sejam ligeiramente diferentes, o significado de cada campo é o mesmo.

```ts
export interface UserData {
  // O endereço Ethereum do usuário.
  userId: string;

  // Um nome para chamá-lo na UI.
  displayName: string;

  // A chave pública Ethereum com a qual assinam.
  publicKey?: string;

  // Se eles têm a funcionalidade web3 ativada.
  hasConnectedWeb3: boolean;

  // A versão sequencial dessas informações, incrementada a cada atualização do usuário.
  version: number;

  // Informações sobre o avatar deles, se disponíveis.
  avatar?: {
    // Referências aos recursos necessários para renderizar este avatar.
    bodyShape: string;
    wearables: string[];

    // Cores RGB/RGBA codificadas em hexadecimal para diferentes partes do corpo (#aabbcc ou #aabbccdd).
    skinColor: string;
    hairColor: string;
    eyeColor: string;

    // Identificadores de arquivo para as "fotos" deste avatar.
    snapshots?: {
      face256: string;
      body: string;
    };
  };
}
```


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/contributor/contributor-pt/runtime-da-scene/modulos-de-runtime/user-identity.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
