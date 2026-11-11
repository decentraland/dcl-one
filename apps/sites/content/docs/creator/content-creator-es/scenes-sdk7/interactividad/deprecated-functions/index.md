> Funciones heredadas

# Funciones obsoletas

Las siguientes funciones son todas heredadas y deben evitarse. Siguen funcionando, pero en el futuro podrían dejar de ser compatibles. Todos estos ejemplos incluyen enlaces a formas alternativas de obtener la misma información o lograr los mismos resultados.

## El jugador entra o sale de la escena


> [!WARNING]
> **📔 Nota**: Los `onEnterSceneObservable` y `onLeaveSceneObservable` events están obsoletos en SDK 7.x. Usa `onEnterScene` en su lugar, vea [El jugador entra o sale de la escena](/docs/creator/content-creator-es/scenes-sdk7/interactividad/event-listeners/#player-enters-or-leaves-scene).


Cada vez que un avatar entra o sale de las parcelas de terreno que conforman tu scene, o se teletransporta dentro o fuera, esto crea un evento al que puedes suscribirte. Este evento se activa con todos los avatares, incluido el del jugador.

```ts
import {
	onEnterSceneObservable,
	onLeaveSceneObservable,
} from '@dcl/sdk/observables'

onEnterSceneObservable.add((player) => {
	console.log('el jugador entró en la escena: ', player.userId)
})

onLeaveSceneObservable.add((player) => {
	console.log('el jugador salió de la escena: ', player.userId)
})
```


> [!WARNING]
> **📔 Nota**: Este evento solo responde a los jugadores que actualmente se están renderizando localmente. En escenas grandes donde el tamaño de la escena supera el rango visual, es posible que los jugadores que entren por la esquina opuesta no se registren. Si el número de jugadores en la región supera las capacidades de una island en los servidores de Decentraland, los jugadores que no compartan la misma island no serán visibles y tampoco serán rastreados por estos eventos.


Los observables `onEnterScene` y `onLeaveScene` importados desde `'@dcl/sdk/observables'` también están obsoletos. Las funciones correctas tienen el mismo nombre, pero se importan desde `'@dcl/sdk/src/players'` en su lugar. En el ejemplo siguiente puedes ver ambas variantes, primero la versión obsoleta y luego la correcta.

```ts
// OBSOLETO - importado desde observables
import { onEnterScene, onLeaveScene } from '@dcl/sdk/observables'

onEnterScene.add((player) => {
	console.log('el jugador entró en la escena: ', player.userId)
})

onLeaveScene.add((player) => {
	console.log('el jugador salió de la escena: ', player.userId)
})

// ACTUAL - importado como una función de jugador
import { onEnterScene, onLeaveScene } from '@dcl/sdk/src/players'

export function main() {
	onEnterScene((player) => {
		if (!player) return
		console.log('ENTRÓ EN LA ESCENA', player)
	})

	onLeaveScene((userId) => {
		if (!userId) return
		console.log('SALIÓ DE LA ESCENA', userId)
	})
}
```

## El jugador se conecta o se desconecta


> [!WARNING]
> **📔 Nota**: Los `getConnectedPlayers` función y las `onPlayerConnectedObservable` y `onPlayerDisconnectedObservable` events están obsoletos en SDK 7.x. Usa `onEnterScene` en su lugar, vea [El jugador entra o sale de la escena](/docs/creator/content-creator-es/scenes-sdk7/interactividad/event-listeners/#player-enters-or-leaves-scene)Cada scene ahora es una isla de comunicaciones distinta, por lo que estar conectado o estar en la misma scene es lo mismo.


Obtén la lista completa de jugadores actualmente conectados desde `getConnectedPlayers`.

```ts
import { getConnectedPlayers } from '~system/Players'

executeTask(async () => {
	let connectedPlayers = await getConnectedPlayers({})
	connectedPlayers.players.forEach((player) => {
		console.log('el jugador ya estaba aquí: ', player.userId)
	})
})
```

Cada vez que otro jugador comienza o deja de ser renderizado por el engine local, esto crea un evento al que puedes suscribirte. Los jugadores pueden o no estar en la misma scene que tú, pero deben estar dentro del rango visual (no necesariamente a la vista). El `onPlayerConnectedObservable` detecta tanto cuando un jugador se conecta cerca por primera vez o se acerca lo suficiente para estar dentro del rango visual; asimismo, el `onPlayerDisconnectedObservable` detecta cuando un jugador termina su sesión o se aleja demasiado.

```ts
import {
	onPlayerConnectedObservable,
	onPlayerDisconnectedObservable,
} from '@dcl/sdk/observables'

onPlayerConnectedObservable.add((player) => {
	console.log('el jugador entró: ', player.userId)
})

onPlayerDisconnectedObservable.add((player) => {
	console.log('el jugador salió: ', player.userId)
})
```

Ten en cuenta que si otros jugadores ya se están renderizando en los alrededores antes de que el jugador haya cargado tu escena, este evento no notificará a la escena recién cargada sobre los jugadores ya existentes. Si necesitas hacer un seguimiento de todos los jugadores actuales, puedes consultar los jugadores existentes al cargar la escena y luego escuchar este evento para las actualizaciones.

## Consultar todos los jugadores en la escena


> [!WARNING]
> **📔 Nota**: Los `getPlayersInScene` la función está obsoleta en SDK 7.x. En su lugar, itera sobre todos los jugadores con un `PlayerIdentityData` componente. Vea [Obtener todos los jugadores](/docs/creator/content-creator-es/scenes-sdk7/interactividad/user-data/#fetch-all-players).


También puedes obtener la lista completa de jugadores que actualmente están en tu escena y se están renderizando llamando a `getPlayersInScene()`.

```ts
import { getPlayersInScene } from '~system/Players'

executeTask(async () => {
	let connectedPlayers = await getPlayersInScene({})
	connectedPlayers.players.forEach((player) => {
		console.log('el jugador ya estaba aquí: ', player.userId)
	})
})
```

## El jugador reproduce una animación


> [!WARNING]
> **📔 Nota**: Los `onPlayerExpressionObservable` el evento está obsoleto en SDK 7.x. Usa el `AvatarEmoteCommand` componente en su lugar, vea [El jugador reproduce una animación](/docs/creator/content-creator-es/scenes-sdk7/interactividad/event-listeners/#player-plays-animation).


Cada vez que el jugador realiza un emote (dance, clap, wave, etc.), puedes detectar este evento.

```ts
import { onPlayerExpressionObservable } from '@dcl/sdk/observables'

onPlayerExpressionObservable.add(({ expressionId }) => {
	console.log('Emote: ', expressionId)
})
```

El evento incluye la siguiente información:

* expressionId: Nombre del emote realizado (es decir: *wave*, *clap*, *kiss*)


> [!WARNING]
> **📔 Nota**Este evento se activa cada vez que el jugador hace un emote y la escena está cargada. El jugador podría estar en una escena cercana cuando esto ocurra.


## El jugador cambia su perfil


> [!WARNING]
> **📔 Nota**: Los `onProfileChanged` el evento está obsoleto en SDK 7.x. Usa el `AvatarEquippedData` componente en su lugar, vea [El jugador cambia su perfil](/docs/creator/content-creator-es/scenes-sdk7/interactividad/event-listeners/#player-changes-profile).


Cada vez que el jugador realiza un cambio en su perfil, se llama al `onProfileChanged` evento. Estos cambios pueden incluir ponerse Wearables diferentes, cambiar el nombre, la descripción, activar experiencias portables, etc.

```ts
import { onProfileChanged } from '@dcl/sdk/observables'

onProfileChanged.add((profileData) => {
	console.log('Los datos de mi perfil son ', profileData)
})
```

Los datos del evento incluyen solo el ID del jugador y un número de versión para el perfil de ese avatar, según el servidor catalyst. Cada vez que se propaga un cambio, el número de versión aumenta en 1.

Cuando se activa este evento, puedes usar luego la [getUserData()](/docs/creator/content-creator-es/scenes-sdk7/interactividad/user-data/#get-player-data) función para obtener la última versión de esta información, incluida la lista de Wearables que lleva puesto el jugador. Es posible que tengas que añadir un pequeño retraso antes de llamar a `getUserData()` para asegurarte de que la versión que devuelve esta función esté actualizada.


> [!NOTE]
> **💡 Consejo**: Cuando pruebas en preview con el legacy web explorer, para evitar usar un avatar aleatorio, ejecuta la escena en el navegador conectado con tu wallet de Metamask.



> [!WARNING]
> **📔 Nota**: Este evento solo se activa por cambios en el jugador actual, no por cambios en los perfiles de otros jugadores cercanos.


## La escena terminó de cargar


> [!WARNING]
> **📔 Nota**: Los `onSceneReadyObservable` el evento está obsoleto desde SDK v7.x. Esta función ya no es relevante. Puedes asegurarte de que algo se ejecute después de que la escena termine de cargar ejecutándolo dentro de la `Main()` función. Vea [Ciclo de vida de la escena](/docs/creator/content-creator-es/scenes-sdk7/primeros-pasos/coding-scenes/#scene-lifecycle)


Cuando la escena termina de cargar, se llama al `onSceneReadyObservable` Esto funciona tanto si el jugador entra directamente a la escena, como si camina hasta la escena desde otro lugar. Cuando todo el contenido de la escena ha terminado su carga inicial, incluidos modelos pesados, etc., se llama a este evento.

```ts
import { onSceneReadyObservable } from '@dcl/sdk/observables'

onSceneReadyObservable.add(() => {
	console.log('ESCENA CARGADA')
})
```

## Métodos de datos del jugador obsoletos


> [!WARNING]
> **📔 Nota**: Los `getUserData()` y `getPlayerData()` las funciones están obsoletas desde SDK v7.4.x. Usa `getPlayer()` en su lugar. Vea [Datos del usuario](/docs/creator/content-creator-es/scenes-sdk7/interactividad/user-data/#get-player-data).


Para obtener información del jugador actual que ejecuta la escena, usa `getUserData()`.

El ejemplo siguiente importa el `~system/UserIdentity` espacio de nombres y ejecuta `getUserData()`.

```ts
import { getUserData } from '~system/UserIdentity'

executeTask(async () => {
	let userData = await getUserData({})
	console.log(userData.data)
})
```

También puedes obtener datos de otros jugadores que estén cerca, llamando a `getPlayerData()`, pasando el id de una cuenta de Decentraland.

```ts
import { getPlayerData } from '~system/Players'

executeTask(async () => {
	let userData = await getPlayerData({ userId: '0x….' })
	console.log(userData)
})
```

Ambos `getUserData()` y `getPlayerData()` devuelven la misma estructura de datos disponible a través de la content API. Vea [Datos de cualquier jugador](/docs/creator/content-creator-es/scenes-sdk7/interactividad/user-data/#data-from-any-player)

`getPlayerData()` solo puede obtener datos de jugadores que estén cerca en ese momento. No necesariamente tienen que estar en la misma scene, pero sí dentro del rango visual, porque esta información se obtiene del engine local que está renderizando estos avatares. Para probar esto en preview, abre una segunda pestaña e inicia sesión con una cuenta diferente.


> [!WARNING]
> **📔 Nota**: Los user IDs siempre deben estar en minúsculas. Si copias una dirección de wallet, asegúrate de que todos los caracteres estén en minúsculas.


El `getUserPublicKey()` y `getUserAccount()` las funciones también están obsoletas. Usa por favor `getPlayer()` en su lugar. Vea [Datos del usuario](/docs/creator/content-creator-es/scenes-sdk7/interactividad/user-data/#get-player-data).

## Obtener la hora del skybox


> [!WARNING]
> **📔 Nota**: Los `getDecentralandTime()` la función está obsoleta desde SDK v7.x.x. Usa `getWorldTime()` en su lugar. Vea [Obtener la hora de Decentraland](/docs/creator/content-creator-es/scenes-sdk7/interactividad/runtime-data/#get-decentraland-time).


```ts
import { getDecentralandTime } from '~system/EnvironmentApi'

executeTask(async () => {
	let time = await getDecentralandTime({})
	console.log(time)
})
```

## Obtener el realm


> [!WARNING]
> **📔 Nota**: Los `getCurrentRealm()` la función está obsoleta desde SDK v7.x.x. Usa `getRealm()` en su lugar. Vea [Obtener datos del realm](/docs/creator/content-creator-es/scenes-sdk7/interactividad/runtime-data/#get-realm-data).


```ts
import { getCurrentRealm } from '~system/EnvironmentApi'

async function fetchPlayerData() {
	const playerRealm = await getCurrentRealm({})

	console.log(playerRealm.currentRealm?.domain)
}

fetchPlayerData()
```

## ¿Está en modo preview?


> [!WARNING]
> **📔 Nota**: Los `isPreviewMode()` la función está obsoleta desde SDK v7.x.x. Usa `getRealm()` en su lugar, que contiene una `preview` propiedad. Vea [Obtener datos del realm](/docs/creator/content-creator-es/scenes-sdk7/interactividad/runtime-data/#get-realm-data).


```ts
import { isPreviewMode } from '~system/EnvironmentApi'

executeTask(async () => {
	const { isPreview } = await isPreviewMode({})

	if (isPreview) {
		console.log('Ejecutándose en preview')
	}
})
```

## El jugador hace clic en otro jugador

Cada vez que el jugador hace clic en otro jugador, puedes detectar un evento.

```ts
import { onPlayerClickedObservable } from '@dcl/sdk/observables'

onPlayerClickedObservable.add((clickEvent) => {
	console.log('Clic en ', clickEvent.userId, ' detalles: ', clickEvent.ray)
})
```


> [!WARNING]
> **📔 Nota**: Los `onPlayerClickedObservable` el evento está obsoleto desde SDK v7.x. Las versiones futuras permitirán un enfoque más [orientado a datos](/docs/creator/content-creator-es/scenes-sdk7/arquitectura/data-oriented-programming/), basado en consultar datos regularmente en lugar de eventos.
> 
> Como alternativa, puedes adjuntar un collider invisible al jugador y detectar clics contra él.



> [!WARNING]
> **📔 Nota**: Tanto el jugador que realiza el clic como el jugador en el que se hace clic deben estar dentro de las parcelas de la escena. Este listener solo detecta eventos del jugador actual haciendo clic sobre otros jugadores, no los clics realizados por otros jugadores.


El evento incluye los siguientes datos:

* `userId`: El id del jugador en el que se hizo clic
* `ray`: Datos sobre el rayo trazado por el clic
  * `direction`: *Vector3* Un Vector3 normalizado que representa la dirección desde el punto de origen del clic hasta el punto de impacto del clic.
  * `distance`: *number* La distancia en metros desde el punto de origen hasta el punto de impacto.
  * `origin`: *Vector3* El punto de origen del clic, la posición del jugador que hizo el clic, relativa a la escena.


> [!NOTE]
> **💡 Consejo**: El comportamiento predeterminado al hacer clic sobre otro jugador es abrir el pasaporte del jugador, donde puedes ver información adicional sobre ese jugador, agregarlo como amigo, etc. Puedes desactivar la apertura de esta UI para que no interfiera con la experiencia que quieres construir añadiendo un [Avatar Modifier Area](/docs/creator/content-creator-es/scenes-sdk7/interactividad/avatares/modifier-areas/).


## El jugador bloquea/desbloquea el cursor


> [!WARNING]
> **📔 Nota**: Los `onPointerLockedStateChange` evento se eliminó en SDK v7.x, ya no está disponible. Vea [Escuchadores de eventos](/docs/creator/content-creator-es/scenes-sdk7/interactividad/event-listeners/#player-locks-or-unlocks-cursor) para la alternativa actual.


Los jugadores pueden alternar entre dos modos de cursor: *modo de cursor bloqueado* para controlar la cámara o *modo de cursor desbloqueado* para mover el cursor libremente sobre la UI.

Los jugadores desbloquean el cursor haciendo clic en el *botón derecho del ratón* o presionando la *Esc* tecla, y vuelven a bloquear el cursor haciendo clic en cualquier parte de la pantalla.

Para reaccionar cada vez que un jugador cambia entre estos dos modos, usa la `onChange` función en el `PointerLock` componente, vea [Escuchadores de eventos](/docs/creator/content-creator-es/scenes-sdk7/interactividad/event-listeners/#player-locks-or-unlocks-cursor).

## El jugador cambia de realm o isla

Los jugadores en Decentraland existen en realms separados *realms*, y en separadas *islands* dentro de cada realm. Los jugadores en diferentes realms o islas no pueden verse, interactuar ni chatear entre sí, incluso si están de pie sobre las mismas parcelas.

Cada vez que el jugador cambia de realm o isla, `onRealmChangedObservable` se llama al evento.

```ts
import { onRealmChangedObservable } from '@dcl/sdk/observables'

onRealmChangedObservable.add((realmChange) => {
	console.log('EL JUGADOR CAMBIÓ A LA ISLA ', realmChange.room)
})
```


> [!WARNING]
> **📔 Nota**: Los `onRealmChangedObservable` el evento está obsoleto desde SDK v7.x. Las versiones futuras permitirán un enfoque más [orientado a datos](/docs/creator/content-creator-es/scenes-sdk7/arquitectura/data-oriented-programming/), basado en consultar datos regularmente en lugar de eventos.


Este evento incluye los siguientes campos:

* **serverName**: *string*; El nombre del servidor catalyst.
* **room**: *string*; El nombre de la isla.
* **displayName**: *string*; El nombre del servidor catalyst seguido de una *-* y el nombre de la isla. Por ejemplo `unicorn-x011`.
* **domain**: *string*; La URL del servidor catalyst que se está usando.

A medida que los jugadores se mueven por el mapa, pueden cambiar de isla para agruparse con aquellos jugadores que ahora están más cerca de ellos. Las islas también ajustan sus límites dinámicamente para acomodar un grupo manejable de personas en cada una. Así que, aunque un jugador se quede quieto, podría cambiar de isla a medida que otros entren y salgan de las escenas cercanas.

Si tu scene depende de un [servidor de terceros](/docs/creator/content-creator-es/scenes-sdk7/networking/third-party-servers/) para sincronizar cambios entre jugadores en tiempo real, entonces quizá quieras compartir datos solo entre jugadores que estén agrupados en el mismo realm+isla, así que es una buena práctica cambiar de room en el servidor de terceros cada vez que los jugadores cambian de isla.

## Funciones de crypto


> [!WARNING]
> **📔 Nota**: Las funciones `requirePayment()`, `signMessage()`, `convertMessageToObject()` están obsoletas. Usa la `sendAsync()` función en su lugar. Vea [Operaciones blockchain de la scene](/docs/creator/content-creator-es/scenes-sdk7/blockchain/scene-blockchain-operations/). También hay bibliotecas que pueden ayudar a simplificar algunos casos de uso comunes con estas funciones.


## Eventos de video


> [!WARNING]
> **📔 Nota**: Los `onVideoEvent` el evento está obsoleto en SDK v7.x. Vea [Escuchadores de eventos](/docs/creator/content-creator-es/scenes-sdk7/media/video-playing/#video-events) para una alternativa no obsoleta.


Cuando un video cambia su estado de reproducción, el `onVideoEvent` observable recibe un evento.

```ts
onVideoEvent.add((data) => {
	console.log('Nuevo evento de video ', data)
})
```

La entrada de un evento de video contiene las siguientes propiedades:

* `videoClipId` ( *string*): El ID de la entidad que cambió de estado.
* `componentId` (*string*): El ID de la entidad que cambió de estado.
* `currentOffset` (*number*): El valor actual de la `seek` propiedad del video. Este valor muestra los segundos transcurridos desde el comienzo original del video. *-1* por defecto.
* `totalVideoLength` (*number* ): La duración en segundos de todo el video. *-1* si se desconoce la duración.
* `videoStatus`: El valor del nuevo estado de video de la `VideoTexture`, expresado como un valor del `VideoStatus` enum. Este enum puede contener los siguientes valores posibles:
* `VideoStatus.NONE` = 0,
* `VideoStatus.ERROR` = 1,
* `VideoStatus.LOADING` = 2,
* `VideoStatus.READY` = 3,
* `VideoStatus.PLAYING` = 4,
* `VideoStatus.BUFFERING` = 5


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-es/scenes-sdk7/interactividad/deprecated-functions.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
