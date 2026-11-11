> Enlazar a otras escenas o sitios externos

# Enlaces externos

Puedes añadir enlaces desde tu scene hacia otro contenido, ya sea hacia otras scenes o hacia sitios web externos.

## Usa el Scene Editor en Creator Hub

La forma más sencilla de añadir un enlace externo o un teleport es usar el Scene Editor. Usa el **Teletransportar** [Smart Item](/docs/creator/content-creator-es/scene-editor/interactividad/smart-items/) para añadir un teleport a otra scene en Genesis City, o usa cualquiera de los **Social Links** smart items para añadir enlaces a sitios externos.

![](/docs/_remote/71809788940cd245e36af334.webp)

## Teletransportes

Para teletransportar a un jugador a otra scene, llama a la siguiente función, indicando las coordenadas a las que quieres teletransportar a los jugadores.

```ts
import { teleportTo } from "~system/RestrictedActions"

(...)

teleportTo({ worldCoordinates: { x: -51, y: 1 } })
```

A los jugadores se les muestra una pantalla de confirmación antes de ser teletransportados; esta pantalla muestra información del archivo `scene.json file`, incluyendo la scene `name`, `description` y `navmapThumbnail`. Consulta [metadatos de la escena](/docs/creator/content-creator-es/scenes-sdk7/tipos-de-proyectos/scene-metadata/) para obtener detalles sobre cómo configurar estos datos.

Ten en cuenta que los teleports te llevan a una scene en las coordinates indicadas, pero no necesariamente a esas mismas coordinates. Esto significa que, al viajar a una scene que tiene múltiples parcels, es posible que los jugadores no aterricen en las mismas coordinates especificadas, sino en uno de los spawn points designados por el creador de la scene.

Para mover a un jugador a otro conjunto de coordinates dentro de la scene actual, usa la `movePlayerTo()` función en su lugar. Consulta [Mover a un jugador](/docs/creator/content-creator-es/scenes-sdk7/interactividad/avatares/move-player/).

## Teletransportarse a un WORLD

Para enviar a un jugador a una scene que no está publicada en el mapa abierto de Genesis City, sino a un [Decentraland WORLD](/docs/creator/content-creator-es/scenes-sdk7/publicacion/publishing-options/#decentraland-worlds), pasa un `realm` en `teleportTo()`.

```ts
import { teleportTo } from "~system/RestrictedActions"

(...)

teleportTo({ realm: 'mannakia.dcl.eth' })
```

`realm` acepta tanto un nombre de world como `mannakia.dcl.eth`, o una URL de realm.

Sin `worldCoordinates`, el jugador aterriza en uno de los spawn points del destino. Añade coordinates para elegir un parcel específico dentro de ese world:

```ts
// Aterriza en el parcel 12,34 de un world específico
teleportTo({
	realm: 'mannakia.dcl.eth',
	worldCoordinates: { x: 12, y: 34 },
})
```

A los jugadores se les muestra una pantalla de confirmación antes de cambiar de realm. Pregunta *"¿Seguro que quieres entrar en este World?"* y muestra el destino (el nombre del world, o el host de la URL del realm). El jugador solo se mueve si acepta.

Para enviar a un jugador de vuelta a Genesis City desde un world, no incluyas el `realm` campo, que por defecto apunta a Genesis City.


> [!WARNING]
> **📔 Nota**: El `realm` el campo requiere `@dcl/sdk` la versión 7.28.0 o más reciente.


### changeRealm está obsoleto

Las escenas antiguas usaban `changeRealm()` para llegar a un world. Sigue funcionando, pero usa `teleportTo({ realm })` en su lugar. `changeRealm()` también muestra una pantalla de confirmación; su campo opcional `message` solo reemplaza el texto del aviso predeterminado.

`changeRealm()` se resuelve en cuanto el jugador acepta el aviso, no cuando el nuevo realm está realmente activo. Una `teleportTo()` llamada justo después se ejecutaría contra el **antiguo** realm y dejaría al jugador en el lugar incorrecto. `teleportTo({ realm })` hace ambos pasos en una sola llamada, así que esa brecha no existe.

## Enlaces externos

Para añadir un enlace a un sitio web externo, usa el `openExternalUrl()` comando.

```ts
import { openExternalUrl } de '~system/RestrictedActions'

openExternalUrl({ url: 'google.com' })
```

Para evitar cualquier uso abusivo de esta función para enviar spam a los jugadores, solo es posible llamar a `openExternalUrl` desde un evento explícito de click o de botón en un entity. No es posible llamar a esta función como resultado de un timer, o de un collision area, o de un evento global de click. Consulta [Button events](/docs/creator/content-creator-es/scenes-sdk7/interactividad/eventos-de-button/click-events/) para obtener detalles sobre cómo hacerlo.

Cuando `openExternalUrl` Cuando se llama, se muestra a los jugadores una pantalla de confirmación, donde se les informa de a dónde les llevará el enlace y donde pueden aceptar o rechazar visitarlo.

El enlace se abre en una nueva pestaña, manteniendo la pestaña original en Decentraland.

Si los jugadores marcan la casilla *confiar en este dominio* no se les volverá a pedir durante su sesión, siempre que el enlace provenga de la misma scene y sea al mismo dominio.

## Copiar al portapapeles

Para copiar una cadena al portapapeles del jugador, usa `copyToClipboard()`. Después de esto, cuando el jugador haga *pegar* en el chat de Decentraland o en cualquier otra aplicación de su máquina, pegará tu cadena.

```ts
import { copyToClipboard } from "~system/RestrictedActions"

copyToClipboard( { text: 'My text to copy' } )
```


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-es/scenes-sdk7/interactividad/external-links.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
