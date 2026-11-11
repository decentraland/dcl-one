# Identidad de usuario

El `UserIdentity` módulo permite [escenas](/docs/contributor/contributor-es/contenido/tipos-de-entidad/scenes/) acceder al [perfil](/docs/contributor/contributor-es/contenido/tipos-de-entidad/profiles/) del usuario del jugador.

```ts
const UserIdentity = require("~system/UserIdentity");
```

Contiene los siguientes métodos y tipos:

* [`function getUserData`](#getuserdata)
* [`interfaz UserData`](#userdata)

### Métodos

Los dos métodos de este módulo obtienen fragmentos de información del usuario del jugador.

#### `getUserData` <a href="#getuserdata" id="getuserdata"></a>

Devuelve la [`UserData`](#userdata) del usuario del jugador.

```ts
interface Request {}

interface Response {
  // La información del perfil del usuario, si está disponible.
  data?: UserData;
}

function getUserData(Request): Promise<Response>;
```

### Tipos

Solo hay un tipo en este módulo: `UserData`.

#### `UserData` <a href="#userdata" id="userdata"></a>

Contiene información (posiblemente parcial) sobre un usuario, su identidad y avatar.

Los World Explorers pueden obtener esto a través del [sistema de contenido](/docs/contributor/contributor-es/contenido/overview/).

El [definición de entidad de perfil](/docs/contributor/contributor-es/contenido/tipos-de-entidad/profiles/) detalla todos estos campos y más. Aunque la estructura y las claves de `UserData` son ligeramente diferentes, el significado de cada campo es el mismo.

```ts
export interface UserData {
  // La dirección de Ethereum del usuario.
  userId: string;

  // Un nombre para llamarlo en la UI.
  displayName: string;

  // La clave pública de Ethereum con la que firman.
  publicKey?: string;

  // Si tienen habilitada la funcionalidad web3.
  hasConnectedWeb3: boolean;

  // La versión secuencial de esta información, incrementada en cada actualización del usuario.
  version: number;

  // Información sobre su avatar, si está disponible.
  avatar?: {
    // Referencias a los assets necesarios para renderizar este avatar.
    bodyShape: string;
    wearables: string[];

    // Colores RGB/RGBA codificados en hexadecimal para diferentes partes del cuerpo (#aabbcc o #aabbccdd).
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
GET https://docs.decentraland.org/contributor/contributor-es/tiempo-de-ejecucion-de-la-escena/modulos-de-tiempo-de-ejecucion/user-identity.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
