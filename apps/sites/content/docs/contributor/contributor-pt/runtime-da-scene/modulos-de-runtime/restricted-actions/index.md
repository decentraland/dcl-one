# Ações restritas

O `RestrictedActions` módulo permite que as scenes acedam a funcionalidades sensíveis (e, por isso, restritas). Está ligado ao [`permissão`](/docs/contributor/contributor-pt/conteudo/tipos-de-entity/scenes/#permissions) sistema de permissões, que as scenes usam para solicitar a utilização de métodos individuais.

```ts
const RestrictedActions = require("~system/RestrictedActions");
```


> [!NOTE]
> Como implementador do World Explorer, o seu runtime pode não impor permissões. Nós **fortemente** desaconselhamos isso, pois coloca os jogadores em perigo.


A maior parte da funcionalidade restrita é fornecida por este módulo, mas também existem [funções globais](/docs/contributor/contributor-pt/runtime-da-scene/globals/).

O módulo contém os seguintes métodos e tipos:

* [`function movePlayerTo`](#moveplayerto)
* [`function teleportTo`](#teleportto)
* [`function triggerEmote`](#triggeremote)
* [`function changeRealm`](#changerealm)
* [`function openExternalUrl`](#openexternalurl)
* [`function openNftDialog`](#opennftdialog)
* [`function openExplorerUi`](#openexplorerui)
* [`function stopEmote`](#stopemote)
* [`function triggerSceneEmote`](#triggersceneemote)
* [`function copyToClipboard`](#copytoclipboard)
* [`function setCommunicationsAdapter`](#setcommunicationsadapter)
* [`interface Vector3`](#vector3)

### Métodos

Cada um dos métodos abaixo está associado a uma [permissão](/docs/contributor/contributor-pt/conteudo/tipos-de-entity/scenes/#permissions) que pode ser solicitada no manifest da scene.

### `movePlayerTo` <a href="#moveplayerto" id="moveplayerto"></a>

Desloca o jogador para uma nova posição relativa à atual e, opcionalmente, define o alvo da câmara com [vetores](#vector3).

Requer a [`ALLOW_TO_MOVE_PLAYER_INSIDE_SCENE`](/docs/contributor/contributor-pt/conteudo/tipos-de-entity/scenes/#permissions) permissão.

```ts
interface Request {
  newRelativePosition: Vector3;
  cameraTarget?: Vector3;
}

interface Response {
    // Assim que o jogador tiver sido movido para a nova posição, e se o jogador não tiver interrompido o movimento, isto será true.
  success: boolean;
}

function movePlayerTo(Request): Promise<Response>;
```

### `teleportTo` <a href="#teleportto" id="teleportto"></a>

Reposiciona o jogador para um parcel, opcionalmente noutro realm.

Em vez de exigir uma permissão previamente aprovada, cada chamada a `teleportTo` tem de ser aprovada pelo jogador.

```ts
interface Request {
  // O parcel onde aterrar. Omitido: o spawn predefinido do realm
  // (só faz sentido juntamente com `realm`).
  worldCoordinates?: Vector2;

  // O realm ao qual o parcel pertence: um nome de world (`foo.dcl.eth`) ou um URL de realm.
  // Quando definido, o client muda de realm (uma reconexão completa, mesmo para o realm em que o
  // jogador já está) e depois aterra no parcel lá.
  // Omitido: o parcel está no realm atual do jogador.
  realm?: string;
}

interface Response {}

function teleportTo(Request): Promise<Response>;
```

### `triggerEmote` <a href="#triggeremote" id="triggeremote"></a>

Faz com que o avatar do jogador exiba uma animação de emote, usando um dos nomes predefinidos.

Requer a [`ALLOW_TO_TRIGGER_AVATAR_EMOTE`](/docs/contributor/contributor-pt/conteudo/tipos-de-entity/scenes/#permissions) permissão.

```ts
interface Request {
  predefinedEmote: string;
}

interface Response {}

function triggerEmote(Request): Promise<Response>;
```

### `openExternalUrl` <a href="#openexternalurl" id="openexternalurl"></a>

Oferece mostrar um website ao jogador, usando uma UI adequada (que pode ser outra aplicação).

Requer a [`OPEN_EXTERNAL_LINK`](/docs/contributor/contributor-pt/conteudo/tipos-de-entity/scenes/#permissions) permissão.

```ts
interface Request {
  url: string;
}

interface Response {
  // Se o jogador autorizou a abertura do link.
  success: boolean;
}

function openExternalUrl(Request): Promise<Response>;
```

### `openNftDialog` <a href="#opennftdialog" id="opennftdialog"></a>

Mostra informação sobre um NFT ao jogador, usando uma UI adequada.

```ts
interface Request {
  // O URN do NFT.
  urn: string;
}

interface Response {
  // Se o NFT foi localizado com sucesso e associado a uma imagem ou vídeo.
  success: boolean;
}

function openNftDialog(Request): Promise<Response>;
```

### `openExplorerUi` <a href="#openexplorerui" id="openexplorerui"></a>

Abre um painel do Explorer, em ecrã inteiro ou modal, e devolve o veredito de abertura.

```ts
interface Request {
  ui: ExplorerUi;

  // Devolvido nos eventos que este pedido produz, para que possam ser associados à respetiva chamada.
  // Não definido significa sem correlação, e 0 está reservado para o mesmo
  // significado no resultado, pelo que os ids começam em 1.
  requestId?: number;

  // Apenas para `EU_ITEM_PURCHASE`.
  itemPurchase?: { urn: string };
}

interface Response {
  // Transporta apenas o veredito da ação de abertura.
  openResult: OpenExplorerUiResult;
}

function openExplorerUi(Request): Promise<Response>;
```

### `stopEmote` <a href="#stopemote" id="stopemote"></a>

Pára o emote que o avatar do jogador está atualmente a reproduzir.

```ts
interface Request {}

interface Response {
  success: boolean;
}

function stopEmote(Request): Promise<Response>;
```

### `triggerSceneEmote` <a href="#triggersceneemote" id="triggersceneemote"></a>

Faz com que o avatar do jogador exiba uma animação de emote a partir de um ficheiro na scene.

```ts
interface Request {
  // O caminho para o ficheiro de emote dentro da scene.
  src: string;

  loop?: boolean;
  mask?: AvatarMask;
}

interface Response {
  success: boolean;
}

function triggerSceneEmote(Request): Promise<Response>;
```

### `copyToClipboard` <a href="#copytoclipboard" id="copytoclipboard"></a>

Copia o texto fornecido para a área de transferência do jogador.

```ts
interface Request {
  text: string;
}

interface Response {}

function copyToClipboard(Request): Promise<Response>;
```

### `setCommunicationsAdapter` <a href="#setcommunicationsadapter" id="setcommunicationsadapter"></a>

Pede ao World Explorer que se ligue a outro adaptador de comunicações, que as scenes usam para entrar em servidores de jogo privados.

```ts
interface Request {
  connectionString: string;
}

interface Response {
  success: boolean;
}

function setCommunicationsAdapter(Request): Promise<Response>;
```

### `changeRealm` (obsoleto) <a href="#changerealm" id="changerealm"></a>

Muda o World Explorer para outro servidor de conteúdo, usando o respetivo URL base.

Obsoleto em favor de [`teleportTo`](#teleportto) com um `realm` (e sem `worldCoordinates` para o spawn predefinido do realm). `changeRealm` é resolvido assim que a alteração é aceite, e não quando o novo realm fica ativo, pelo que um `teleportTo` emitido imediatamente a seguir é executado contra o realm anterior.

```ts
interface Request {
  // O URL do novo realm.
  realm: string;

  // Uma mensagem opcional para mostrar aos utilizadores quando tiverem de aprovar a alteração.
  message?: string;
}

interface Response {
  // Se a alteração de realm foi autorizada e bem-sucedida.
  success: boolean;
}

function changeRealm(Request): Promise<Response>;
```

### Tipos

O único tipo adicional utilizado pelos métodos neste módulo é o `Vector3`.

### `Vector3` <a href="#vector3" id="vector3"></a>

Contém uma posição 3D relativa ou absoluta.

```ts
interface Vector3 {
  x: number;
  y: number;
  z: number;
}
```


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/contributor/contributor-pt/runtime-da-scene/modulos-de-runtime/restricted-actions.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
