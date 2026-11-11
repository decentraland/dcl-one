# Jugadores

El `Jugadores` el módulo permite [las escenas](/docs/contributor/contributor-es/contenido/tipos-de-entidad/scenes/) buscar jugadores en su entorno y obtener información sobre su identidad y [perfiles](/docs/contributor/contributor-es/contenido/tipos-de-entidad/profiles/).

```ts
const Players = require("~system/Players");
```

Contiene los siguientes métodos y tipos:

* [`función getPlayersInScene`](#getplayersinscene)
* [`función getConnectedPlayers`](#getplayersinscene)
* [`función getPlayerData`](#getplayerdata)
* [`interface Player`](#player)
* [`interface UserData`](#userdata)

### Métodos

Hay dos métodos en este módulo para descubrir jugadores en la escena y uno para obtener sus perfiles.

#### `getPlayersInScene` <a href="#getplayersinscene" id="getplayersinscene"></a>

Devuelve un [`Player`](#player) array, cada uno con un `userId` que se puede usar en [getPlayerData](#getplayerdata).

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

Devuelve los [`UserData`](#userdata) de un usuario, si se conocen para el `userId`.

```ts
interface Request {
  // El identificador del usuario (para los no invitados, su dirección de Ethereum).
  userId: string;
}

interface Response {
  // La información de perfil de este usuario, si está disponible.
  data?: UserData;
}

function getPlayerData(Request): Promise<Response>;
```

### Tipos

Solo hay dos tipos en este módulo: [`Player`](#player) y [`UserData`](#userdata).

#### `Player` <a href="#player" id="player"></a>

Contiene una referencia a un `userId`.

```ts
interface Player {
  // El identificador del usuario (para los no invitados, su dirección de Ethereum).
  userId: string;
}
```

#### `UserData` <a href="#userdata" id="userdata"></a>

Contiene información (posiblemente parcial) sobre un usuario, su identidad y su avatar.

Los World Explorers obtienen esta información a través de la [sistema de contenido](/docs/contributor/contributor-es/contenido/overview/).

El [definición de entidad de perfil](/docs/contributor/contributor-es/contenido/tipos-de-entidad/profiles/) detalla todos estos campos y más. Aunque la estructura y las claves de `UserData` son ligeramente diferentes, el significado de cada campo es el mismo.

```ts
export interface UserData {
  // El identificador del usuario (es decir, su dirección de Ethereum).
  userId: string;

  // Un nombre para llamarlos en la UI.
  displayName: string;

  // La clave pública de Ethereum con la que firman.
  publicKey?: string;

  // Si tienen habilitada la funcionalidad web3.
  hasConnectedWeb3: boolean;

  // La versión secuencial de esta información, incrementada en cada actualización del usuario.
  version: number;

  // Información sobre su avatar, si está disponible.
  avatar?: {
    // Punteros a los assets necesarios para renderizar este avatar.
    bodyShape: string;
    wearables: string[];

    // Tintes de color RGB/RGBA codificados en hexadecimal para diferentes partes del cuerpo.
    skinColor: string;
    hairColor: string;
    eyeColor: string;

    // Identificadores de archivo para las "fotos" de este avatar.
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
GET https://docs.decentraland.org/contributor/contributor-es/tiempo-de-ejecucion-de-la-escena/modulos-de-tiempo-de-ejecucion/players.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
