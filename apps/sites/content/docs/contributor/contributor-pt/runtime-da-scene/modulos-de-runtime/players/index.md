# Jogadores

O `Jogadores` módulo permite [scenes](/docs/contributor/contributor-pt/conteudo/tipos-de-entity/scenes/) para procurar jogadores no seu ambiente e obter informações sobre a sua identidade e [perfis](/docs/contributor/contributor-pt/conteudo/tipos-de-entity/profiles/).

```ts
const Players = require("~system/Players");
```

Ele contém os seguintes métodos e tipos:

* [`function getPlayersInScene`](#getplayersinscene)
* [`function getConnectedPlayers`](#getplayersinscene)
* [`function getPlayerData`](#getplayerdata)
* [`interface Player`](#player)
* [`interface UserData`](#userdata)

### Métodos

Existem dois métodos neste módulo para descobrir jogadores na cena e um para obter os seus perfis.

#### `getPlayersInScene` <a href="#getplayersinscene" id="getplayersinscene"></a>

Retorna um [`Player`](#player) array, cada um com um `userId` que pode ser usado em [getPlayerData](#getplayerdata).

```ts
interface Request {}

interface Response {
  players: Player[];
}

function getPlayersInScene(Request): Promise<Response>;
```

#### `getConnectedPlayers` <a href="#getconnectedplayers" id="getconnectedplayers"></a>

```ts
interface Request {}

interface Response {
  players: Player[];
}

function getConnectedPlayers(Request): Promise<Response>;
```

#### `getPlayerData` <a href="#getplayerdata" id="getplayerdata"></a>

Retorna a [`UserData`](#userdata) para um utilizador, se conhecido para o dado `userId`.

```ts
interface Request {
  // O identificador do utilizador (para não-convidados, o respetivo endereço Ethereum).
  userId: string;
}

interface Response {
  // As informações do perfil para este utilizador, se disponíveis.
  data?: UserData;
}

function getPlayerData(Request): Promise<Response>;
```

### Tipos

Existem apenas dois tipos neste módulo: [`Player`](#player) e [`UserData`](#userdata).

#### `Player` <a href="#player" id="player"></a>

Contém uma referência a um `userId`.

```ts
interface Player {
  // O identificador do utilizador (para não-convidados, o respetivo endereço Ethereum).
  userId: string;
}
```

#### `UserData` <a href="#userdata" id="userdata"></a>

Contém informações (possivelmente parciais) sobre um usuário, sua identidade e avatar.

Os World Explorers obtêm esta informação através do [sistema de conteúdo](/docs/contributor/contributor-pt/conteudo/overview/).

O [definição de entidade de perfil](/docs/contributor/contributor-pt/conteudo/tipos-de-entity/profiles/) detalha todos esses campos e mais. Embora a estrutura e as chaves de `UserData` sejam ligeiramente diferentes, o significado de cada campo é o mesmo.

```ts
export interface UserData {
  // O identificador do utilizador (ou seja, o respetivo endereço Ethereum).
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

    // Tonalidades de cor codificadas em hexadecimal RGB/RGBA para diferentes partes do corpo.
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
GET https://docs.decentraland.org/contributor/contributor-pt/runtime-da-scene/modulos-de-runtime/players.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
