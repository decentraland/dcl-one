> Obtén datos de los jugadores a medida que interactúan con tu escena.

# Datos del usuario

## Posición y rotación del jugador

Usa la `PlayerEntity` y las `CameraEntity` para conocer la posición y rotación del jugador, comprobando sus `Transform` componentes.

```ts
function getPlayerPosition() {
	if (!Transform.has(engine.PlayerEntity)) return
	if (!Transform.has(engine.CameraEntity)) return

	//posición del jugador
	const playerPos = Transform.get(engine.PlayerEntity).position

	//rotación del jugador
	const playerRot = Transform.get(engine.PlayerEntity).rotation

	//posición de la cámara
	const CameraPos = Transform.get(engine.CameraEntity).position

	//rotación de la cámara
	const CameraRot = Transform.get(engine.CameraEntity).rotation

	console.log('playerPos: ', playerPos)
	console.log('playerRot: ', playerRot)
	console.log('cameraPos: ', CameraPos)
	console.log('cameraRot: ', CameraRot)
}

engine.addSystem(getPlayerPosition)
```

* **posición de PlayerEntity**: La posición del avatar, a la altura del pecho. Aproximadamente a 0,88 metros sobre el suelo.
* **rotación de PlayerEntity**: La dirección en la que mira el avatar, expresada como un quaternion.
* **posición de CameraEntity**:
  * En 1.ª persona: igual a la posición del avatar, pero a la altura de los ojos. Aproximadamente a 1,75 metros sobre el suelo.
  * En 3.ª persona: puede variar según los movimientos de la cámara.
* **rotación de PlayerEntity**:
  * En 1.ª persona: similar a la dirección en la que mira el avatar, expresada como un quaternion. Puede redondearse ligeramente de forma distinta que la rotación del jugador.
  * En 3.ª persona: puede variar según los movimientos de la cámara.


> [!WARNING]
> **📔 Nota**: Evita hacer referencia al `engine.PlayerEntity` o los `engine.CameraEntity` al cargar la escena inicial, porque eso puede provocar errores si las entities aún no se han inicializado. Para evitar este problema, úsalo dentro de la `main()` función, o en una función llamada indirectamente por `main()`. También puedes encapsular el comportamiento en un bloque async [`executeTask` block](/docs/creator/content-creator-es/scenes-sdk7/patrones-de-programacion/async-functions/#the-executetask-function).
> 
> Otra opción es hacer referencia a estas entidades dentro de un system. Allí siempre estarán disponibles, porque la primera ejecución del system se llama una vez que la escena ya está correctamente inicializada.


## Obtener todos los jugadores

Todos los jugadores en la escena tienen un `Transform` component. Este component es de solo lectura en los avatars. Para obtener las posiciones de todos los jugadores, [itera sobre todas las entities con](/docs/creator/content-creator-es/scenes-sdk7/interactividad/user-data/) un `PlayerIdentityData` componente.

```ts
import { PlayerIdentityData } from '@dcl/sdk/ecs'

for (const [entity, data, transform] of engine.getEntitiesWith(
	PlayerIdentityData,
	Transform
)) {
	console.log('Datos del jugador: ', { entity, data, transform })
}
```

El código anterior itera sobre todas las entities con un `Transform` y una `PlayerIdentityData` component, y registra sus datos. Puedes usar este mismo método para obtener cualquiera de los datos disponibles de todos los jugadores.

Consulta [Listeners de eventos](/docs/creator/content-creator-es/scenes-sdk7/interactividad/event-listeners/#player-enters-or-leaves-scene) para aprender a detectar y reaccionar cuando nuevos players se unan a la escena.

## Obtener datos del jugador

Usa `getPlayer()` para obtener datos del jugador actual, o de cualquier otro jugador en la escena.

```ts
import { getPlayer } from '@dcl/sdk/src/players'

export function main() {
	createCube(5, 1, 5)

	let myPlayer = getPlayer()

	if (myPlayer) {
		console.log('Nombre : ', myPlayer.name)
		console.log('Id de usuario : ', myPlayer.userId)
	}
}
```

`getPlayer()` devuelve lo siguiente:

* `name`: *(string)* El nombre de usuario del jugador, tal como lo ven los demás en el mundo
* `userId`: *(string)* Una cadena que identifica al jugador. Para los jugadores conectados con una wallet, esta es la dirección de la wallet, escrita en minúsculas. Para las cuentas de invitado, es un identificador generado localmente.
* `isGuest`: *(boolean)* Indica si el jugador tiene una clave pública. *Verdadero* si el jugador es una cuenta de invitado sin clave pública.
* `posición`: *(Vector3)* La posición del avatar en la escena.
* `avatar`: Un objeto anidado con datos sobre el avatar base y la apariencia del jugador.
* `wearables`: Un array de identificadores de cada uno de los wearables que el jugador lleva puestos actualmente. Por ejemplo `urn:decentraland:off-chain:base-avatars:green_hoodie`. Todos los wearables tienen un identificador similar, aunque sean NFTs.
* `emotes`: Un array de identificadores de cada uno de los emotes que el jugador tiene equipados actualmente en la rueda de acceso rápido.
* `entity`: Una referencia a la entity del jugador. Esto puede ser útil para pasarla a otras funciones, o para añadirle components personalizados.

El `avatar` El objeto tiene la siguiente información anidada:

* `bodyShapeUrn`: Un identificador de la forma corporal general del avatar. Puede ser `urn:decentraland:off-chain:base-avatars:BaseFemale` para femenino o `urn:decentraland:off-chain:base-avatars:BaseMale` para masculino.
* `skinColor`: El color de piel del jugador como un `Color3`
* `eyesColor`: El color de ojos del jugador como un `Color3`
* `hairColor`: El color de cabello del jugador como un `Color3`
* `name`: El nombre del jugador.


> [!WARNING]
> **📔 Nota**: Es posible que los datos del jugador no estén disponibles en el primer frame de la escena, según los tiempos de carga. Debes verificar que los datos se hayan devuelto y, de lo contrario, intentar de nuevo unos milisegundos más tarde.


```ts
import { getPlayer } from '@dcl/sdk/src/players'

export function main() {
	createCube(5, 1, 5)

	let myPlayer = getPlayer()

	if (myPlayer) {
		console.log('Es invitado: ', myPlayer.isGuest)
		console.log('Nombre : ', myPlayer.name)
		console.log('Id de usuario : ', myPlayer.userId)
		console.log('Forma del avatar : ', myPlayer.position)
		console.log('Forma del avatar : ', myPlayer.avatar?.bodyShapeUrn)
		console.log('Color de ojos del avatar : ', myPlayer.avatar?.eyesColor)
		console.log('Color del cabello del avatar : ', myPlayer.avatar?.hairColor)
		console.log('Wearables activos : ', myPlayer.wearables)
		console.log('Emotes disponibles : ', myPlayer.emotes)
	}
}
```

Para obtener los datos de un jugador específico en la escena, diferente del jugador actual, ejecuta `getPlayer()` con un objeto con un `userId` propiedad.

```ts
import { getPlayer } from '@dcl/sdk/src/players'

for (const [entity, data, transform] of engine.getEntitiesWith(
	PlayerIdentityData,
	Transform
)) {
	let player = getPlayer({ userId: data.address })
	console.log('JUGADOR : ', player?.name)
}
```

El fragmento de código anterior itera sobre todas las entities con un `PlayerIdentityData` component, lo que significa todas las entities de avatar en la escena. Luego ejecuta el `getPlayer()` para esa entity.

`getPlayer()` solo puede obtener datos de los jugadores que están actualmente en la misma escena, no tienen por qué estar necesariamente dentro del rango visual, pero sí deben estar conectados a la misma isla de comunicaciones. Para probar esto en preview, abre una segunda pestaña e inicia sesión con una cuenta diferente, y haz que ambos jugadores permanezcan dentro de la escena.


> [!WARNING]
> **📔 Nota**: Los User IDs siempre deben ir en minúsculas. Si copias una dirección de wallet, asegúrate de que todos los caracteres estén en minúsculas.


## Datos de cualquier jugador

Para obtener información de cualquier jugador, haz una [llamada a la REST API](/docs/creator/content-creator-es/scenes-sdk7/networking/network-connections/#call-a-rest-api) a los servidores de contenido.

Esta información se expone en la siguiente URL, añadiendo el user id del jugador al parámetro de la URL.

`https://peer.decentraland.org/lambdas/profile/<player user id>`


> [!NOTE]
> **💡 Consejo**: Prueba la URL en un navegador para ver cómo está estructurada la respuesta.


La siguiente información está disponible desde esta API:

* `displayName`: *(string)* El nombre de usuario del jugador, tal como lo ven los demás en el mundo
* `userId`: *(string)* Una cadena UUID que identifica al jugador. Si el jugador tiene una clave pública, este campo tendrá el mismo valor que la clave pública.
* `hasConnectedWeb3`: *(boolean)* Indica si el jugador tiene una clave pública. *Verdadero* si el jugador tiene una.
* `publicKey`: *(string)* La clave pública de la wallet Ethereum del jugador. Si el jugador inicia sesión como invitado, sin wallet vinculada, este campo será `null`.
* `avatar`: Un objeto anidado con datos sobre la apariencia del jugador.
* `version`: *(number)* Un número de versión que aumenta en uno cada vez que el jugador cambia cualquiera de sus ajustes. Úsalo si encuentras datos en conflicto, para saber qué versión es la más reciente.


> [!WARNING]
> **📔 Nota**: Para cualquier transacción de Ethereum con el jugador, usa siempre el `publicKey` campo, en lugar del `userId`, para evitar tratar con wallets inexistentes.


El `avatar` El objeto tiene la siguiente información anidada:

* `wearables`: `WearableId[]` Un array de identificadores de cada uno de los wearables que el jugador lleva puestos actualmente. Por ejemplo `urn:decentraland:off-chain:base-avatars:green_hoodie`. Todos los wearables tienen un identificador similar, aunque sean NFTs.
* `bodyShape`: Un identificador de la forma corporal general del avatar. Puede ser `urn:decentraland:off-chain:base-avatars:BaseFemale` para femenino o `urn:decentraland:off-chain:base-avatars:BaseMale` para masculino.
* `skin`: Un objeto con un `color` campo, que contiene el color de la piel del jugador como `{ r, g, b, a }` números de 0 a 1.
* `hair`: Un objeto con un `color` campo, que contiene el color del cabello del jugador como `{ r, g, b, a }` números de 0 a 1.
* `eyes`: Un objeto con un `color` campo, que contiene el color de los ojos del jugador como `{ r, g, b, a }` números de 0 a 1.
* `snapshots`: Un objeto anidado con representaciones en base64 de imágenes .jpg del jugador en varias resoluciones.
  * `face256`: *string* El rostro del jugador como una imagen de 256x256 píxeles.
  * `body`: *string* La imagen en resolución completa del jugador de pie, con 512x1024 píxeles.


> [!CAUTION]
> **❗Advertencia** Las snapshots del avatar quedarán obsoletas en el futuro y dejarán de devolverse como parte de los datos de un avatar. El enfoque recomendado es usar `AvatarTexture` en su lugar, consulta [Retratos de Avatar](/docs/creator/content-creator-es/scenes-sdk7/fundamentos-del-contenido-3d/materials/#avatar-portraits).


A diferencia de `getPlayer()`, esta opción no se limita solo a los jugadores que están actualmente en la misma escena, ni siquiera en el mismo servidor. Con este enfoque puedes obtener datos de cualquier jugador que se haya conectado a los servidores en el pasado.

Si sabes a qué servidor está conectado el jugador que quieres consultar, puedes obtener datos más actualizados enviando tus requests a ese servidor específico. Por ejemplo, si el jugador cambia de ropa, esta información estará disponible al instante en el servidor del jugador, pero probablemente tarde un par de minutos en propagarse al `peer.decentraland.org` servidor.

`https://<player server>/lambdas/profile/<player user id>`


> [!NOTE]
> **💡 Consejo**: Puedes obtener el servidor del jugador actual llamando a `getRealm()` y leyendo el `realmInfo.baseUrl` campo de la respuesta.


Este ejemplo combina `myProfile.userId` y `getRealm()` para obtener los datos del jugador directamente desde el servidor en el que se encuentra el jugador:

```ts
import { getRealm } from '~system/Runtime'
import { myProfile } from '@dcl/sdk/network'

async function fetchPlayerData() {
	const { realmInfo } = await getRealm({})
	if (!realmInfo) return

	const url = `${realmInfo.baseUrl}/lambdas/profile/${myProfile.userId}`
	console.log('usando URL: ', url)

	try {
		const response = await fetch(url)
		const json = await response.json()

		console.log('respuesta completa: ', json)
		console.log('el jugador lleva puestos: ', json.avatars[0].avatar.wearables)
	} catch {
		console.log('se produjo un error al obtener los datos del jugador')
	}
}

fetchPlayerData()
```

## Componentes de datos del jugador

En lugar de usar `getPlayer()`, puedes leer los datos directamente desde una serie de components que almacenan los datos en la entity de cada jugador. Existen los siguientes components:

* `PlayerIdentityData`: Almacena la dirección del jugador y una `isGuest` propiedad para marcar las cuentas de invitado.
* `AvatarBase`: Almacena datos sobre el avatar base, incluyendo:
  * `name`: El nombre del jugador.
  * `bodyShapeUrn`: Los IDs correspondientes al tipo de cuerpo masculino o femenino.
  * `skinColor`: El color de piel del jugador como un `Color3`
  * `eyesColor`: El color de ojos del jugador como un `Color3`
  * `hairColor`: El color de cabello del jugador como un `Color3`
  * `talking`: *true* mientras el jugador habla en el chat de voz, *false* cuando está en silencio, *undefined* cuando el client no informa actividad de voz para ellos. Consulta [Detectar cuando un jugador está hablando](#detect-when-a-player-is-speaking).
* `AvatarEquippedData`: La lista de wearables y emotes equipados.
  * `wearableUrns`: La lista de wearables que el jugador tiene equipados actualmente.
  * `emoteUrns`: La lista de emotes que el jugador tiene equipados actualmente en la rueda de acceso rápido.
* `AvatarEmoteCommand`: Información sobre los emotes que el jugador está reproduciendo actualmente. Incluye:
  * `emoteUrn`: El URN del último emote reproducido por el jugador desde que entró en la escena
  * `loop`: Verdadero si el emote se está reproduciendo en bucle
  * `timestamp`: El momento en que se activó este emote

```ts
for (const [entity, data, base, attach, transform] of engine.getEntitiesWith(
	PlayerIdentityData,
	AvatarBase,
	AvatarEquippedData,
	Transform
)) {
	console.log('DATOS DEL JUGADOR: ', { entity, data, transform, base, attach })
}
```


> [!WARNING]
> **📔 Nota**: Todos estos components son de solo lectura. No puedes cambiar sus valores desde la escena.


## Detectar cuando un jugador está hablando

Leer `talking` en el `AvatarBase` component para saber si alguien está hablando en el chat de voz ahora mismo. Funciona para el jugador que ejecuta la escena y para cualquier otro jugador cercano.

```ts
import { AvatarBase, engine } from '@dcl/sdk/ecs'

// registra cada vez que el jugador local empieza o deja de hablar
AvatarBase.onChange(engine.PlayerEntity, (base) => {
	console.log('hablando: ', base?.talking)
})
```

`talking` tiene tres estados, no dos:

| Valor       | Significado                                                                                                          |
| ----------- | -------------------------------------------------------------------------------------------------------------------- |
| `true`      | El jugador está hablando ahora mismo.                                                                                |
| `false`     | El jugador no está hablando.                                                                                         |
| `undefined` | El client no informa actividad de voz para este jugador, por ejemplo porque el chat de voz cercano está desactivado. |

Ese tercer estado importa: `undefined` significa "sin datos", no "silencioso". Compruébalo explícitamente en lugar de tratar cualquier valor no-`true` como silencio.

```ts
import { AvatarBase, engine, PlayerIdentityData } from '@dcl/sdk/ecs'

function whoIsSpeaking() {
	for (const [player, base] of engine.getEntitiesWith(AvatarBase)) {
		if (base.talking === undefined) continue // no hay datos de voz para este jugador
		if (base.talking) console.log(`${base.name} está hablando`)
	}
}

engine.addSystem(whoIsSpeaking)
```

Para poner un marcador sobre la cabeza de un jugador que habla, adjúntalo con [`AvatarAttach`](/docs/creator/content-creator-es/scenes-sdk7/fundamentos-del-contenido-3d/entity-positioning/#attach-to-other-players) usando la dirección de `PlayerIdentityData`. No parentices tu entity a la entity del jugador: eso no funciona, y el marcador termina en el origen de la escena.


> [!WARNING]
> **📔 Nota**: `AvatarBase.talking` informa de la actividad real del chat de voz y no puedes escribir en él. Es algo distinto de `AvatarShape.talking`, que es un flag que tu escena establece para dibujar barras verdes de voz junto a un [avatar NPC](/docs/creator/content-creator-es/scenes-sdk7/interactividad/avatares/npc-avatars/) que creaste.


## Obtener Portable Experiences

Las Portable Experiences son, en esencia, escenas que no están limitadas a parcelas de terreno. Los jugadores pueden llevarlas consigo a donde vayan en Decentraland, añadiendo una nueva capa de contenido sobre el mundo. Los Smart Wearables son ejemplos de portable experiences. Quizá quieras saber si un jugador lleva una de estas, ya que un smart wearable podría permitir a los jugadores tener habilidades que podrían considerarse trampa en un juego competitivo. Por ejemplo, en un juego de plataformas, un jugador que lleva un jetpack tiene una ventaja muy injusta sobre los demás.

Como creador de una escena, quizá quieras limitar lo que los jugadores que llevan Portable Experiences pueden hacer en tu escena. Usa `getPortableExperiencesLoaded()` para comprobar si el jugador tiene actualmente activada alguna Portable Experience.

```ts
import { getPortableExperiencesLoaded } from '~system/PortableExperiences'

executeTask(async () => {
	let portableExperiences = await getPortableExperiencesLoaded({})
	console.log(portableExperiences.loaded)
})
```

`getPortableExperiencesLoaded()` devuelve un array de objetos, y cada uno de estos objetos incluye un atributo `id` En el caso de los wearables, el id es el URN del wearable.

## Obtener información detallada sobre los wearables de un jugador

El `getPlayer()` La función devuelve solo una lista de ids de wearables, sin información sobre cada wearable. Quizá quieras comprobar si hay algún wearable de una categoría específica (por ejemplo: sombreros), o cualquier wearable de una rareza específica (por ejemplo: Mythic); para eso necesitarás obtener información más detallada sobre los wearables del jugador.

Haz una [llamada a la REST API](/docs/creator/content-creator-es/scenes-sdk7/networking/network-connections/#call-a-rest-api) a la siguiente URL, para obtener una lista completa y actualizada de todos los wearables que se pueden usar actualmente, con detalles sobre cada uno.

`${playerRealm.realmInfo.baseUrl}/lambdas/collections/wearables-by-owner/${userData.userId}?includeDefinitions`


> [!WARNING]
> **📔 Nota**: Para construir esta URL, debes obtener el realm (probablemente con `getRealm()`) y el id del jugador (probablemente con `getPlayer()`)


Esta función podría usarse junto con la obtención de información sobre el jugador, por ejemplo para permitir que los jugadores entren en un lugar solo si llevan cualquier wearable de la colección de Halloween, o cualquier wearable que sea de rareza *legendaria* .


> [!NOTE]
> **💡 Consejo**: Prueba la URL en un navegador para ver cómo está estructurada la respuesta.


```ts
import { getPlayer } from '@dcl/sdk/src/players'
import { getRealm } from '~system/Runtime'

async function fetchWearablesData() {
	try {
		let userData = getPlayer()
		const realm = await getRealm({})
		if (!userData || !realm.realmInfo) return

		const url =
			`${realm.realmInfo?.baseUrl}/lambdas/collections/wearables-by-owner/${userData.userId}?includeDefinitions`.toString()
		console.log('usando URL: ', url)

		let response = await fetch(url)
		let json = await response.json()

		console.log('respuesta completa: ', json)
	} catch {
		se produjo un error al obtener los datos de wearables
	}
}

executeTask(fetchWearablesData)
```


> [!NOTE]
> **💡 Consejo**: Puedes obtener aún más información sobre wearables específicos desde la [siguiente API](https://decentraland.github.io/catalyst-api-specs/#tag/Lambdas/operation/searchWearables).


## Comprobar el modo de cámara del jugador

Los jugadores pueden usar una cámara en primera o tercera persona al explorar Decentraland. Comprueba cuál de estas está usando el jugador comprobando el valor `CameraMode` component del `engine.CameraEntity` Entity.

```ts
function checkCameraMode() {
	if (!CameraMode.has(engine.CameraEntity)) return

	let cameraEntity = CameraMode.get(engine.CameraEntity)

	if (cameraEntity.mode == CameraType.CT_THIRD_PERSON) {
		console.log('El jugador está usando la cámara en 3.ª persona')
	} else {
		console.log('El jugador está usando la cámara en 1.ª persona')
	}
}

engine.addSystem(checkCameraMode)
```


> [!WARNING]
> **📔 Nota**La información de la cámara solo está disponible para el jugador actual que ejecuta la escena. No puedes consultar los datos de cámara de ningún otro jugador.


El modo de cámara usa un valor del `CameraType` enum. Los siguientes valores son posibles:

* `CameraType.CT_FIRST_PERSON`
* `CameraType.CT_THIRD_PERSON`

El `CameraMode` component del `engine.CameraEntity` es de solo lectura, no puedes obligar al jugador a cambiar de modo de cámara a través de esto.


> [!NOTE]
> **💡 Consejo**: Para cambiar el modo de cámara del jugador, usa un [Área modificadora de cámara](/docs/creator/content-creator-es/scenes-sdk7/fundamentos-del-contenido-3d/camera/#1st-and-3rd-person-camera-modes).


Conocer el modo de cámara puede ser muy útil para ajustar finamente las mecánicas de tu escena y adaptarlas mejor a lo que resulte más cómodo usar con este modo. Por ejemplo, los objetivos pequeños son más difíciles de pulsar en tercera persona.


> [!WARNING]
> **📔 Nota**: Evita hacer referencia al `engine.CameraEntity` al cargar la escena inicial, porque eso puede provocar errores si las entities aún no se han inicializado. Para evitar este problema, úsalo dentro de la `main()` función, o en una función llamada indirectamente por `main()`. También puedes encapsular el comportamiento en un bloque async [`executeTask` block](/docs/creator/content-creator-es/scenes-sdk7/patrones-de-programacion/async-functions/#the-executetask-function).
> 
> Otra opción es hacer referencia a esta entity dentro de un system. Siempre estará disponible, porque la primera ejecución del system se llama una vez que la escena ya está correctamente inicializada.


## Comprobar si el jugador tiene el cursor bloqueado

Los jugadores pueden alternar entre dos modos de cursor: *cursor bloqueado* para controlar la cámara o *cursor desbloqueado* para mover el cursor libremente por la UI.

Los jugadores desbloquean el cursor al hacer clic en el *botón derecho del mouse* o al presionar la *Esc* tecla, y vuelven a bloquear el cursor al hacer clic en cualquier parte de la pantalla.

Comprueba el `PointerLock` component de la [camera entity](/docs/creator/content-creator-es/scenes-sdk7/arquitectura/entities-components/#reserved-entities) de la escena para averiguar cuál es el modo actual del cursor.

```ts
export function main() {
	const isLocked = PointerLock.get(engine.CameraEntity).isPointerLocked
	console.log(isLocked)
}
```

Consulta [Listeners de eventos](/docs/creator/content-creator-es/scenes-sdk7/interactividad/event-listeners/#player-locks-or-unlocks-cursor) para ver cómo reaccionar fácilmente a los cambios en el estado del cursor.

También puedes forzar el estado del cursor del jugador escribiendo en el `isPointerLocked` campo del `PointerLock` component en `engine.CameraEntity`. Consulta [Bloquear o desbloquear el cursor](/docs/creator/content-creator-es/scenes-sdk7/interactividad/eventos-de-button/click-events/#lock-or-unlock-the-cursor) para ver ejemplos.


> [!WARNING]
> **📔 Nota**: Evita hacer referencia al `engine.CameraEntity` al cargar la escena inicial, porque eso puede provocar errores si las entities aún no se han inicializado. Para evitar este problema, úsalo dentro de la `main()` función, o en una función llamada indirectamente por `main()`. También puedes encapsular el comportamiento en un bloque async [`executeTask` block](/docs/creator/content-creator-es/scenes-sdk7/patrones-de-programacion/async-functions/#the-executetask-function).
> 
> Otra opción es hacer referencia a la entity dentro de un system. Siempre estará disponible, porque la primera ejecución del system se llama una vez que la escena ya está correctamente inicializada.


## Comprobar la posición del cursor del jugador

Usa la `PrimaryPointerInfo` component en el `engine.RootEntity` para obtener la posición del cursor del jugador. Esto puede usarse para mecánicas como interacciones de arrastrar y soltar, gestos de deslizamiento, etc.

```ts
import { PrimaryPointerInfo } from '@dcl/sdk/ecs'

function CursorSystem() {
	const pointerInfo = PrimaryPointerInfo.get(engine.RootEntity)
	console.log(pointerInfo)
}

engine.addSystem(CursorSystem)
```


> [!WARNING]
> **📔 Nota**: Evita hacer referencia al `engine.RootEntity` al cargar la escena inicial, porque eso puede provocar errores si las entities aún no se han inicializado. Para evitar este problema, haz siempre referencia a la entity dentro de un system. Siempre estará disponible, porque la primera ejecución del system se llama una vez que la escena ya está correctamente inicializada.


El `PrimaryPointerInfo` component devuelve un objeto con las siguientes propiedades:

* `screenCoordinates`: *(Vector2)* La posición del cursor en la escena, expresada en píxeles. El origen es la esquina superior izquierda de la pantalla, con `y` aumentando hacia abajo, en el mismo espacio de coordenadas que `UiTransform` y `UiCanvasInformation`. Cuando el cursor está bloqueado, esto informa del centro de la pantalla.
* `screenDelta`: *(Vector2)* El cambio en la posición del cursor desde el último frame, expresado en píxeles. Consulta [Movimiento del mouse](/docs/creator/content-creator-es/scenes-sdk7/interactividad/mouse-movement/) para obtener detalles y ejemplos.
* `worldRayDirection`: *(Vector3)* Un vector que representa la dirección del rayo desde la cámara hasta el cursor. El origen es la posición de la cámara. Úsalo para calcular la posición del cursor en el mundo.
* `pointerType`: 0 para `none`, 1 para `mouse`


> [!NOTE]
> **💡 Consejo**: A diferencia de las demás propiedades, `screenDelta` sigue informando del movimiento del ratón incluso mientras el cursor está [bloqueado](/docs/creator/content-creator-es/scenes-sdk7/interactividad/eventos-de-button/click-events/#lock-or-unlock-the-cursor). Esto lo hace ideal para interacciones en tiempo real como gestos de arrastre y controles de cámara personalizados, consulta [Movimiento del mouse](/docs/creator/content-creator-es/scenes-sdk7/interactividad/mouse-movement/).



> [!NOTE]
> **Consejo:** Para reaccionar a eventos simples de hover en elementos de UI, puede que te resulte más fácil usar los `onMouseEnter` y `onMouseLeave` eventos, consulta [Eventos de botón de UI](/docs/creator/content-creator-es/scenes-sdk7/ui-2d/ui_button_events/#hover-feedback).


El `PrimaryPointerInfo` el componente es de solo lectura, no puedes forzar al jugador a cambiar la posición del cursor.

El siguiente ejemplo muestra cómo mostrar la posición del cursor en un elemento de UI.

***archivo ui.tsx:***

```tsx
import { UiEntity, ReactEcs } from '@dcl/sdk/react-ecs'
import { Color4 } from '@dcl/sdk/math'
import {cursorXpos, cursorYpos} from './index'

export const uiMenu = () => (
  <UiEntity
    uiTransform={{
			width: '100%',
			height: '100px',
			justifyContent: 'center',
			alignItems: 'center',
    }}
    uiText={{ value: `Cursor pos: `+  cursorXpos + `,` + cursorYpos, fontSize: 40 }}
    uiBackground={{ color: Color4.create(0.5, 0.8, 0.1, 0.6) }}
  />
)
```

***archivo index.ts:***

```ts
import { engine, PrimaryPointerInfo } from '@dcl/sdk/ecs'
import { ReactEcsRenderer } from '@dcl/sdk/react-ecs'
import { uiMenu } from './ui'

export function main() {
  ReactEcsRenderer.setUiRenderer(uiMenu)
}

export let cursorXpos: number | undefined = undefined
export let cursorYpos: number | undefined = undefined

function CursorSystem() {
  const pointerInfo = PrimaryPointerInfo.get(engine.RootEntity)
  console.log(pointerInfo)

  cursorXpos = pointerInfo.screenCoordinates?.x
  cursorYpos = pointerInfo.screenCoordinates?.y
}

engine.addSystem(CursorSystem)
```

Puedes usar el `worldRayDirection` para establecer el `dirección` campo de un raycast para saber si una Entity está en la línea de visión del cursor. Ver [Raycasting](/docs/creator/content-creator-es/scenes-sdk7/interactividad/raycasting/) para más detalles.


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-es/scenes-sdk7/interactividad/user-data.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
