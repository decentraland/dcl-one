# Acciones restringidas

El `RestrictedActions` módulo permite que las escenas accedan a funcionalidades sensibles (y, por lo tanto, restringidas). Está vinculado al [`permiso`](/docs/contributor/contributor-es/contenido/tipos-de-entidad/scenes/#permissions) sistema, que las escenas usan para solicitar el uso de métodos individuales.

```ts
const RestrictedActions = require("~system/RestrictedActions");
```


> [!NOTE]
> Como implementador de World Explorer, tu runtime podría no aplicar permisos. Nosotros **enérgicamente** desaconsejamos esto, ya que pone a los jugadores en peligro.


La mayor parte de la funcionalidad restringida es proporcionada por este módulo, pero también hay [funciones globales](/docs/contributor/contributor-es/tiempo-de-ejecucion-de-la-escena/globals/).

El módulo contiene los siguientes métodos y tipos:

* [`función movePlayerTo`](#moveplayerto)
* [`función teleportTo`](#teleportto)
* [`función triggerEmote`](#triggeremote)
* [`función changeRealm`](#changerealm)
* [`función openExternalUrl`](#openexternalurl)
* [`función openNftDialog`](#opennftdialog)
* [`función openExplorerUi`](#openexplorerui)
* [`función stopEmote`](#stopemote)
* [`función triggerSceneEmote`](#triggersceneemote)
* [`función copyToClipboard`](#copytoclipboard)
* [`función setCommunicationsAdapter`](#setcommunicationsadapter)
* [`interfaz Vector3`](#vector3)

### Métodos

Cada uno de los métodos siguientes está asociado con un [permiso](/docs/contributor/contributor-es/contenido/tipos-de-entidad/scenes/#permissions) que puede solicitarse en el manifiesto de la escena.

### `movePlayerTo` <a href="#moveplayerto" id="moveplayerto"></a>

Desplaza al jugador a una nueva posición relativa a la actual y, opcionalmente, establece el objetivo de la cámara con [vectores](#vector3).

Requiere el [`ALLOW_TO_MOVE_PLAYER_INSIDE_SCENE`](/docs/contributor/contributor-es/contenido/tipos-de-entidad/scenes/#permissions) permiso.

```ts
interfaz Request {
  newRelativePosition: Vector3;
  cameraTarget?: Vector3;
}

interfaz Response {
    // Una vez que el jugador se haya movido a la nueva posición, y si el jugador no interrumpió el movimiento, esto será true.
  success: boolean;
}

función movePlayerTo(Request): Promise<Response>;
```

### `teleportTo` <a href="#teleportto" id="teleportto"></a>

Reubica al jugador en un parcel, opcionalmente en otro realm.

En lugar de requerir un permiso preaprobado, cada llamada a `teleportTo` debe ser aprobada por el jugador.

```ts
interfaz Request {
  // El parcel en el que aterrizar. Omitido: el spawn predeterminado del realm
  // (solo tiene sentido junto con `realm`).
  worldCoordinates?: Vector2;

  // El realm al que pertenece el parcel: un nombre de world (`foo.dcl.eth`) o una URL de realm.
  // Cuando se establece, el client cambia de realm (una reconexión completa, incluso al realm en el que
  // el jugador ya está) y luego aterriza en el parcel de allí.
  // Omitido: el parcel está en el realm actual del jugador.
  realm?: string;
}

interfaz Response {}

función teleportTo(Request): Promise<Response>;
```

### `triggerEmote` <a href="#triggeremote" id="triggeremote"></a>

Haz que el avatar del jugador muestre una animación de emote, usando uno de los nombres predefinidos.

Requiere el [`ALLOW_TO_TRIGGER_AVATAR_EMOTE`](/docs/contributor/contributor-es/contenido/tipos-de-entidad/scenes/#permissions) permiso.

```ts
interfaz Request {
  predefinedEmote: string;
}

interfaz Response {}

función triggerEmote(Request): Promise<Response>;
```

### `openExternalUrl` <a href="#openexternalurl" id="openexternalurl"></a>

Ofrece mostrar un sitio web al jugador, usando una UI adecuada (que puede ser otra aplicación).

Requiere el [`OPEN_EXTERNAL_LINK`](/docs/contributor/contributor-es/contenido/tipos-de-entidad/scenes/#permissions) permiso.

```ts
interfaz Request {
  url: string;
}

interfaz Response {
  // Si el jugador autorizó abrir el enlace.
  success: boolean;
}

función openExternalUrl(Request): Promise<Response>;
```

### `openNftDialog` <a href="#opennftdialog" id="opennftdialog"></a>

Muestra información sobre un NFT al jugador, usando una UI adecuada.

```ts
interfaz Request {
  // El URN del NFT.
  urn: string;
}

interfaz Response {
  // Si el NFT se localizó correctamente y se asoció a una imagen o video.
  success: boolean;
}

función openNftDialog(Request): Promise<Response>;
```

### `openExplorerUi` <a href="#openexplorerui" id="openexplorerui"></a>

Abre un panel de explorer, en pantalla completa o modal, y devuelve el veredicto de apertura.

```ts
interfaz Request {
  ui: ExplorerUi;

  // Se repite en los eventos que produce esta solicitud, para que puedan coincidir con
  // su llamada. Si no se establece, no hay correlación, y 0 está reservado para el mismo
  // significado en el resultado, así que los ids comienzan en 1.
  requestId?: number;

  // Solo para `EU_ITEM_PURCHASE`.
  itemPurchase?: { urn: string };
}

interfaz Response {
  // Solo lleva el veredicto de la acción de apertura.
  openResult: OpenExplorerUiResult;
}

función openExplorerUi(Request): Promise<Response>;
```

### `stopEmote` <a href="#stopemote" id="stopemote"></a>

Detén el emote que el avatar del jugador está reproduciendo actualmente.

```ts
interfaz Request {}

interfaz Response {
  success: boolean;
}

función stopEmote(Request): Promise<Response>;
```

### `triggerSceneEmote` <a href="#triggersceneemote" id="triggersceneemote"></a>

Haz que el avatar del jugador muestre una animación de emote desde un archivo en la escena.

```ts
interfaz Request {
  // La ruta al archivo de emote dentro de la escena.
  src: string;

  loop?: boolean;
  mask?: AvatarMask;
}

interfaz Response {
  success: boolean;
}

función triggerSceneEmote(Request): Promise<Response>;
```

### `copyToClipboard` <a href="#copytoclipboard" id="copytoclipboard"></a>

Copia el texto proporcionado en el portapapeles del jugador.

```ts
interfaz Request {
  text: string;
}

interfaz Response {}

función copyToClipboard(Request): Promise<Response>;
```

### `setCommunicationsAdapter` <a href="#setcommunicationsadapter" id="setcommunicationsadapter"></a>

Pide a World Explorer que se conecte a otro adaptador de comunicaciones, que las escenas usan para unirse a servidores privados de juego.

```ts
interfaz Request {
  connectionString: string;
}

interfaz Response {
  success: boolean;
}

función setCommunicationsAdapter(Request): Promise<Response>;
```

### `changeRealm` (deprecated) <a href="#changerealm" id="changerealm"></a>

Cambia World Explorer a otro servidor de contenido, usando su URL base.

Obsoleto en favor de [`teleportTo`](#teleportto) con un `realm` (y sin `worldCoordinates` para el spawn predeterminado del realm). `changeRealm` se resuelve una vez que el cambio es aceptado, en lugar de cuando el nuevo realm está activo, así que un `teleportTo` emitido inmediatamente después se ejecuta contra el realm anterior.

```ts
interfaz Request {
  // La URL del nuevo realm.
  realm: string;

  // Un mensaje opcional para mostrar a los usuarios cuando tengan que aprobar el cambio.
  message?: string;
}

interfaz Response {
  // Si el cambio de realm fue autorizado y exitoso.
  success: boolean;
}

función changeRealm(Request): Promise<Response>;
```

### Tipos

El único tipo adicional usado por los métodos de este módulo es el `Vector3`.

### `Vector3` <a href="#vector3" id="vector3"></a>

Contiene una posición 3D relativa o absoluta.

```ts
interfaz Vector3 {
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
GET https://docs.decentraland.org/contributor/contributor-es/tiempo-de-ejecucion-de-la-escena/modulos-de-tiempo-de-ejecucion/restricted-actions.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
