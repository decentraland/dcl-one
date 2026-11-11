> Crea escenas multijugador de Decentraland con un Multiplayer Server sin interfaz.

# Multiplayer Server

**📔 Nota**: El Multiplayer Server se llamaba anteriormente el **Servidor Autoritativo**. Solo cambió el nombre; la función es la misma. La rama del SDK que hay que instalar sigue llamándose `auth-server`, consulta [Configuración](#setup).

## Descripción general

Decentraland ejecuta scenes localmente en la máquina de un player. De forma predeterminada, los players pueden verse entre sí e interactuar directamente, pero cada uno interactúa con el entorno de manera independiente. Los cambios en el entorno no se comparten entre players de forma predeterminada.

Permitir que todos los players vean una scene con el mismo contenido en el mismo estado es extremadamente importante para que los players interactúen de formas más significativas. Sin esto, si un player abre una puerta y entra en una casa, los demás players verán esa puerta todavía cerrada, y el primer player parecerá atravesar directamente la puerta cerrada para los demás players.

El **Multiplayer Server** es un proceso de server sin interfaz gráfica que ejecuta el código de tu scene, valida los cambios de estado y difunde el resultado a todos los players conectados. Sigue una arquitectura de server autoritativo: en lugar de confiar en que cada client informe de sus propias acciones, el server actúa como la única fuente de verdad. Esto lo convierte en el enfoque recomendado para sincronizar scenes multijugador.

El Multiplayer Server es ideal siempre que la equidad sea importante para la mecánica del juego, ya que puedes implementar validaciones anticheat elaboradas que se ejecutan del lado del server. También puedes guardar claves privadas y otra información sensible en el server, evitando tener que exponerla directamente al usuario.

Tener un Multiplayer Server también resuelve un problema real: en una configuración peer-to-peer, dos players que controlan algo como una plataforma flotante pueden producir resultados contradictorios. Cada client establece la plataforma a una altura distinta, y nadie tiene la autoridad para decidir cuál es la correcta. El Multiplayer Server resuelve cada cambio en un solo lugar, de modo que todos los clients convergen en el mismo estado.

También te da un lugar para **persistir datos entre sesiones**: tablas de clasificación, progreso del player, logros desbloqueados o cambios en el entorno como puertas abiertas u objetos colocados. Cuando los players regresan, el mundo refleja lo que ocurrió antes.

Decentraland aloja y despliega el server por ti. Publicar tu scene mediante el proceso normal también publica el server de forma fluida, sin pasos extra ni necesidad de pagar por ningún alojamiento.

## Configuración

### 1. Instala la versión auth-server del SDK

Las API nativas del Multiplayer Server (`isServer`, `registerMessages`, `Storage`, `EnvVar`, etc.) están disponibles en una rama separada del SDK. Ejecuta los siguientes comandos para instalarla en tu proyecto en lugar de la rama estándar del SDK:

```bash
npm install @dcl/sdk@auth-server
npm install @dcl/js-runtime@auth-server
```

### 2. Configura scene.json

El Multiplayer Server se habilita con la bandera `"authoritativeMultiplayer": true` en `scene.json`, en el nivel raíz. No necesitas añadirlo manualmente: cuando usas la rama auth-server del SDK, se añade automáticamente la primera vez que compilas o haces preview de la scene. Solo asegúrate de no eliminarlo; sin esta bandera el server nunca se ejecuta y `isServer()` siempre devuelve *false*.

Opcionalmente, también puedes añadir lo siguiente a tu `scene.json` en el nivel raíz:

```json
{
  "logsPermissions": ["0xYourWalletAddress"]
}
```

Añade `logsPermissions` para dar a estas wallets acceso a los logs y métricas del server después del despliegue. Los propietarios de World y los colaboradores de despliegue elegibles también tienen acceso, al igual que los propietarios de Genesis City y los administradores de LAND delegados de la parcela base de la scene. Consulta [Scene Insights](/docs/creator/content-creator-es/scenes-sdk7/depuracion/scene-insights/) para permisos, inicio de sesión y comandos.

### 3. Ejecuta el preview

Usa el comando estándar de preview, no se necesitan pasos extra. Cuando usas la rama auth-server del SDK, el preview inicia automáticamente una versión local del Multiplayer Server en segundo plano.

La sesión local del server no está conectada a la de producción, así que puedes probar cosas libremente sin afectar a los players que están en tu scene publicada.

## Ramificación Server / Client

El código de la scene en la `src` carpeta se ejecuta tanto en Server como en Client. Usa la `isServer()` función para separar las rutas de ejecución:

```typescript
import { isServer } from "@dcl/sdk/network"
import { initServer } from "./server/server"
import { initClient } from "./client/setup"
import { setupUi } from "./client/ui"

function main() {
  if (isServer()) {
    // Solo Server: lógica del juego, validación, gestión del estado
    initServer()
    return
  } else {
    // Solo Client: UI, manejo de entrada, envío de mensajes
    initClient()
    setupUi()
  }
}
```

El server ejecuta tu scene sin interfaz gráfica y sin renderizado. Tiene acceso verificado a todas las posiciones de los players, wearables y otros datos mediante `PlayerIdentityData` y es la única autoridad sobre el estado del juego.

## Componentes sincronizados y validación

### Sincronizar Entities con todos los Clients

Usa `syncEntity` para difundir cualquier cambio en los components indicados de esa Entity:

```typescript
import { isServer, syncEntity } from "@dcl/sdk/network"

if (isServer()) {
  syncEntity(
    entity,
    [Transform.componentId, GameState.componentId],
    /* enumId */ 1
  )
}
```

La sintaxis es idéntica a la que usa la [multijugador sin servidor](/docs/creator/content-creator-es/scenes-sdk7/networking/serverless-multiplayer/) feature, lo que hace trivial actualizar una scene desde el uso de esta arquitectura al Multiplayer Server. Cuando una scene usa el Multiplayer Server, las actualizaciones de estado ya no se envían entre todos los players; en su lugar, todas las actualizaciones de estado ahora se enrutan y validan mediante el server.


> [!WARNING]
> **📔 Nota**: Con el Multiplayer Server, el patrón ideal es que solo el server llame a `syncEntity`. De ese modo no tienes que preocuparte por la consistencia del entity-id. Más bien, el server instancia y comparte la Entity, y todos los clients se sincronizan respecto a esa instancia. Protégelo siempre con `isServer()`. Esto es diferente de [multijugador sin servidor](/docs/creator/content-creator-es/scenes-sdk7/networking/serverless-multiplayer/), donde cada client llama a `syncEntity` por su cuenta.


### Validar cambios

Usa `validateBeforeChange()` para restringir cualquier actualización de estado en un component específico de una Entity. Te permite ejecutar una función de validación personalizada, y los cambios solo se realizan correctamente cuando se cumple la prueba de validación.

Si la validación devuelve el valor *true*true *false*, entonces el cambio se acepta y se propaga a todos los players. Si la validación devuelve el valor

#### false

Validar valores *El caso más simple es validar que el nuevo* value `Transform` que se está escribiendo esté dentro de ciertos parámetros. Por ejemplo, solo acepta cambios en un

```typescript
cuando la nueva posición Y es superior a 0:
import { Vector3 } from "@dcl/sdk/math"
import { isServer } from "@dcl/sdk/network"

const entity = engine.addEntity()
Transform.create(entity, { position: Vector3.create(10, 2, 10) })

if (isServer()) {
  Transform.validateBeforeChange(entity, (value) => {
    // Rechaza cualquier actualización que colocaría la Entity en Y = 0 o por debajo
    return value.newValue.position.y > 0
  })
}
```

Porque `validateBeforeChange()` solo tiene sentido en el server, protégelo siempre con `isServer()`. En el Client, la llamada no hace nada útil.

Puedes usar esto para impedir cambios que vayan en contra de la lógica de tu juego, como mecanismos anticheat.

#### Validar proximidad al player

También puedes combinar `validateBeforeChange()` con posiciones de player verificadas por el server para comprobar que un player está lo bastante cerca de un objeto antes de permitirle interactuar con él. Por ejemplo, cuando un player intenta recoger un objeto cambiando su `Transform`, puedes rechazar el cambio si el objeto está a más de 5 metros del player:

```typescript
import { engine, Transform, PlayerIdentityData } from "@dcl/sdk/ecs"
import { Vector3 } from "@dcl/sdk/math"
import { isServer } from "@dcl/sdk/network"

const pickableEntity = engine.addEntity()
Transform.create(pickableEntity, { position: Vector3.create(8, 1, 8) })

if (isServer()) {
  Transform.validateBeforeChange(pickableEntity, (value) => {
    // Encuentra al player que envió este cambio
    for (const [playerEntity, identity] of engine.getEntitiesWith(
      PlayerIdentityData
    )) {
      if (identity.address.toLowerCase() !== value.senderAddress.toLowerCase())
        continue

      const playerTransform = Transform.getOrNull(playerEntity)
      if (!playerTransform) return false

      // Obtén la posición actual del objeto, antes del cambio
      const objectTransform = Transform.getOrNull(pickableEntity)
      if (!objectTransform) return false

      const distance = Vector3.distance(
        playerTransform.position,
        objectTransform.position
      )

      // Solo permite el cambio si el player está dentro de 5 metros
      return distance <= 5
    }

    // Sender no encontrado entre los players conectados — rechazar
    return false
  })
}
```

Este patrón es útil como mecanismo anticheat: impide que los players alcancen objetos a través de la scene para recogerlos cuando no deberían poder interactuar con ellos.

#### Permitir cambios solo por administradores

También puedes validar en función de *quién* está enviando el cambio. Cada valor entrante incluye un `senderAddress` campo con la dirección de wallet del remitente. Usa esto para permitir cambios solo de ciertos players. Por ejemplo, para permitir solo a los administradores de la scene modificar un `VideoPlayer` component:

```typescript
import { engine, VideoPlayer } from "@dcl/sdk/ecs"
import { isServer } from "@dcl/sdk/network"
import { isPreview } from "@dcl/asset-packs/dist/admin-toolkit-ui/fetch-utils"
import { getSceneAdmins } from "@dcl/asset-packs/dist/admin-toolkit-ui/ModerationControl/api"

// Caché de direcciones de wallet de administradores, actualizada desde la lista de administradores de la scene
let adminAddresses = new Set<string>()

async function updateAdminAddresses() {
  if (isPreview()) return
  try {
    const [error, response] = await getSceneAdmins()
    if (error) {
      console.error("[SERVER] Error al obtener la lista de administradores:", error)
      adminAddresses = new Set()
      return
    }
    adminAddresses = new Set(
      (response ?? []).map((admin) => admin.admin.toLowerCase())
    )
    console.log(
      "[SERVER] Caché de direcciones de administradores actualizada:",
      Array.from(adminAddresses)
    )
  } catch (error) {
    console.error("[SERVER] Error al actualizar las direcciones de administradores:", error)
    adminAddresses = new Set()
  }
}

export async function main() {
  const videoEntity = engine.addEntity()
  VideoPlayer.create(videoEntity, { src: "videos/intro.mp4" })

  if (isServer()) {
    // Rellena la caché antes de conectar la validación
    await updateAdminAddresses()

    VideoPlayer.validateBeforeChange(videoEntity, (value) => {
      // Permite siempre los cambios mientras se ejecuta en preview, para que las pruebas locales sean más fáciles
      if (isPreview()) return true

      const senderAddress = value.senderAddress.toLowerCase()
      if (!adminAddresses.has(senderAddress)) {
        console.log(
          "[SERVER] Cambio no autorizado de VideoPlayer bloqueado desde:",
          senderAddress
        )
        return false
      }
      return true
    })
  }
}
```

Ver [Administrador de la scene](/docs/creator/content-creator-es/scene-editor/operar-en-vivo/scene-admin/) para más contexto sobre cómo los players se convierten en administradores de una scene.

#### Envía la intención, nunca los resultados

Siempre que haya puntuaciones, premios o recursos compartidos en juego, diseña los mensajes de tu client en torno a lo que el player **está intentando hacer**, no en torno al resultado. Un client envía `claimPoint` o `harvest`. Nunca envía "my score is now 40".

El server entonces comprueba el mensaje: quién lo envió, si el juego está en una fase que lo permite, si el player está lo suficientemente cerca y si queda algo por reclamar. Solo entonces calcula el nuevo valor y lo escribe. El server es el único escritor del estado del juego.

Un client que puede nombrar su propia puntuación puede nombrar cualquier puntuación.

#### Permitir cambios solo por el server

El caso más estricto es aceptar solo escrituras que se originen en el propio server, rechazando cualquier cambio que venga de un client. Este es el patrón de referencia para el estado que debe ser totalmente autoritativo: puntuaciones, fase del juego, Entities generadas, etc.

Cada valor entrante incluye un `senderAddress` campo. Cuando el remitente es el server, este campo coincide con la constante `AUTH_SERVER_PEER_ID`, exportado desde `@dcl/sdk/network/message-bus-sync`.

El ejemplo siguiente define un pequeño `protectServerEntity()` helper que aplica esta comprobación a uno o más components en una Entity dada. Es una forma práctica de proteger múltiples components (como `Transform` y `GltfContainer`) en una sola llamada:

```typescript
import { engine, Entity, Transform, GltfContainer } from "@dcl/sdk/ecs"
import { Vector3 } from "@dcl/sdk/math"
import { isServer } from "@dcl/sdk/network"
import { AUTH_SERVER_PEER_ID } from "@dcl/sdk/network/message-bus-sync"

type ComponentWithValidation = {
  validateBeforeChange: (
    entity: Entity,
    cb: (value: { senderAddress: string }) => boolean
  ) => void
}

function protectServerEntity(
  entity: Entity,
  components: ComponentWithValidation[]
) {
  for (const component of components) {
    component.validateBeforeChange(entity, (value) => {
      return value.senderAddress === AUTH_SERVER_PEER_ID
    })
  }
}

if (isServer()) {
  // Después de crear una Entity gestionada por el server:
  const entity = engine.addEntity()
  Transform.create(entity, { position: Vector3.create(10, 5, 10) })
  GltfContainer.create(entity, { src: "assets/model.glb" })
  protectServerEntity(entity, [Transform, GltfContainer])
}
```


> [!WARNING]
> **📔 Nota**: Llama siempre a `protectServerEntity()` dentro de un `isServer()` bloque. Envuelve `validateBeforeChange()`, que solo tiene sentido en el server — llamarlo en un Client produce errores.


#### Componentes personalizados

También puedes aplicar `validateBeforeChange()` a componentes personalizados definidos por la scene.

```typescript
import { engine, Schemas } from "@dcl/sdk/ecs"
import { isServer } from "@dcl/sdk/network"
import { AUTH_SERVER_PEER_ID } from "@dcl/sdk/network/message-bus-sync"

export const GameState = engine.defineComponent("game:State", {
  phase: Schemas.String,
  score: Schemas.Int,
  timeRemaining: Schemas.Int,
})

// Solo el server puede modificar este component
if (isServer()) {
  GameState.validateBeforeChange((value) => {
    return value.senderAddress === AUTH_SERVER_PEER_ID
  })
}
```

## Mensajes

Los components sincronizados son ideales para el estado que todos los players deben ver de forma continua: cosas como posiciones, puntuaciones o la fase del juego. Pero no todo encaja en ese modelo. A veces un player solo necesita decirle al server "hice clic en este botón" o "quiero unirme al juego", y el server necesita responder con una respuesta de una sola vez como "la ronda comenzó" o "aquí están tus estadísticas". Eso son eventos, no estado persistente.

Para eso sirven los mensajes. Usa `registerMessages()` para una comunicación tipada y validada por esquema entre clients y el server. Los mensajes son de tipo fire-and-forget: un client envía uno al server, el server lo procesa y opcionalmente envía uno de vuelta. No crean directamente ningún estado persistente por sí mismos.

### Definir mensajes

Define todos los mensajes en un archivo compartido que importen tanto el server como el client. Así ambas partes siempre coinciden en qué mensajes existen y qué datos transportan. Cada mensaje es un `Schemas.Map` que describe su carga útil:

```typescript
import { Schemas } from "@dcl/sdk/ecs"
import { registerMessages } from "@dcl/sdk/network"

export const Messages = {
  // Client → Server
  playerReady: Schemas.Map({ displayName: Schemas.String }),
  playerAction: Schemas.Map({ action: Schemas.String, targetId: Schemas.Int }),
  giveCoins: Schemas.Map({ to: Schemas.String, amount: Schemas.Int }),

  // Server → Client
  gameStarted: Schemas.Map({ roundNumber: Schemas.Int }),
  gameEnded: Schemas.Map({ winnerId: Schemas.String }),
}

export const room = registerMessages(Messages)
```


> [!WARNING]
> **📔 Nota**: Llama a `registerMessages()` una vez, en el nivel superior de un módulo compartido, como en el ejemplo anterior. De ese modo se ejecuta cuando el módulo se carga por primera vez, tanto en el server como en el client. No lo llames de forma condicional ni dentro de funciones; ambas partes siempre deben registrar los mismos mensajes.


### Enviar mensajes

Los clients solo pueden enviar mensajes al server. No existe mensajería directa entre clients. El server puede difundir a todos los clients o dirigirse a players específicos por dirección.

```typescript
import { room } from "./shared/messages"

// Client → Server (broadcast, el server lo recibe)
room.send("playerReady", { displayName: "Alice" })

// Server → todos los clients
room.send("gameStarted", { roundNumber: 1 })

// Server → un client específico (por dirección de wallet)
room.send("gameEnded", { winnerId: "Alice" }, { to: [playerAddress] })
```


> [!WARNING]
> **📔 Nota**: Los mensajes tienen un tamaño máximo de alrededor de 13 KB. Los mensajes más grandes se descartan silenciosamente por el transporte, sin producir ningún error. Mantén las cargas útiles pequeñas; si necesitas compartir datos más grandes, divídelos en varios mensajes o usa components sincronizados.


### Recibir mensajes

```typescript
import { room } from "./shared/messages"

// Client recibe del server
room.onMessage("gameStarted", (data) => {
  console.log(`¡La ronda ${data.roundNumber} ha comenzado!`)
})

// El server recibe del client
room.onMessage("playerReady", (data, context) => {
  if (!context) return
  const senderAddress = context.from // dirección de wallet verificada
  console.log(`[Server] ${data.displayName} está listo (${senderAddress})`)
})
```

En el server, cada mensaje recibido incluye un `context` objeto con la dirección de wallet verificada del remitente. Usa esto para saber qué player envió el mensaje (nunca confíes en la identidad autodeclarada en la carga útil).

`context.from` llega exactamente como lo informa la red, sin cambios en mayúsculas o minúsculas. Ponlo en minúsculas una sola vez (`context.from.toLowerCase()`) y usa ese valor para cada llamada a map, set y Storage, para que los datos de un player no acaben divididos entre dos claves.

### Espera a que State Sync se complete antes de enviar

Los clients deberían esperar hasta que el estado de la scene esté sincronizado antes de enviar su primer mensaje, para evitar condiciones de carrera al unirse:

```typescript
import { engine } from "@dcl/sdk/ecs"
import { isStateSyncronized } from "@dcl/sdk/network"
import { room } from "./shared/messages"

engine.addSystem(() => {
  if (!isStateSyncronized()) return

  // Ya es seguro enviar mensajes
  room.send("playerReady", { displayName: "Alice" })
})
```


> [!NOTE]
> **💡 Consejo**: `isStateSyncronized()` solo te indica que el estado del client está sincronizado; no garantiza que el server haya terminado de inicializarse. Para una comprobación de disponibilidad más robusta, haz que el server emita un mensaje "ready" o de heartbeat, y haz que los clients esperen a recibirlo antes de enviar mensajes de gameplay.


### Tipos de Schema disponibles

Todas las cargas útiles de los mensajes y los components personalizados usan `Schemas` para serialización binaria. Aquí tienes una referencia rápida de los tipos disponibles:

```typescript
import { Schemas } from "@dcl/sdk/ecs"

// Tipos básicos
Schemas.String // "hello"
Schemas.Int // 42
Schemas.Float // 3.14
Schemas.Boolean // true / false
Schemas.Int64 // Date.now()

// Tipos de vector
Schemas.Vector3 // { x: 1, y: 2, z: 3 }
Schemas.Quaternion // { x, y, z, w }

// Tipos complejos
Schemas.Array(Schemas.String) // ["a", "b", "c"]
Schemas.Entity // referencia de Entity
Schemas.Optional(Schemas.String) // "hello" o undefined
Schemas.Optional(Schemas.Int) // 42 o undefined

// Objetos anidados
Schemas.Map({
  name: Schemas.String,
  health: Schemas.Int,
  position: Schemas.Vector3,
  playerId: Schemas.Optional(Schemas.String),
})
```


> [!WARNING]
> **📔 Nota**: Mensajes *debe* definirse usando `Schemas.Map(...)`. No puedes enviar objetos JavaScript simples; fallarán en la serialización binaria.


## Lectura de posiciones de jugadores por el servidor

El servidor puede leer **verificadas** las posiciones de los jugadores; los clientes no pueden falsearlas. Esta es la base del anti-cheat basado en posiciones:

```typescript
import { engine, PlayerIdentityData, Transform } from "@dcl/sdk/ecs"

engine.addSystem(() => {
  for (const [entity, identity] of engine.getEntitiesWith(PlayerIdentityData)) {
    const transform = Transform.getOrNull(entity)
    if (!transform) continue

    const address = identity.address
    const position = transform.position
    // Esta posición está verificada por el servidor — nunca confíes en la posición reportada por el cliente
  }
})
```


> [!WARNING]
> **📔 Nota**: Usa siempre `PlayerIdentityData` + `Transform` en el servidor para obtener las posiciones de los jugadores. Nunca confíes en los valores reportados por el propio cliente.


## Almacenamiento de datos

Persistir datos entre reinicios del servidor. Storage es **solo del servidor**, protege siempre las llamadas con `isServer()`. El servidor puede tanto escribir como leer estos datos.

```typescript
import { Storage } from "@dcl/sdk/server"
```

Los datos pueden almacenarse en dos niveles:

* **World**: Úsalo para datos relevantes para todos los jugadores, como tablas de clasificación o cambios persistentes en el entorno.
* **Player**: Úsalo para datos específicos del jugador, como guardar el progreso o las preferencias de ese jugador.


> [!NOTE]
> **💡 Consejo**: Los valores de Storage son JSON. Puedes guardar un número, una cadena, un array o un objeto simple tal cual, y leerlo de vuelta como el mismo valor. No necesitas `JSON.stringify()` o `JSON.parse()`. Dos excepciones: un `Date` se almacena como su texto ISO (`"2026-10-27T13:05:12.000Z"`), y las propiedades del objeto establecidas en `undefined` se omiten. Un valor que guardaste antes con `JSON.stringify()` es una cadena, así que se lee de vuelta como esa cadena.
> 
> Durante el desarrollo local, storage se escribe en `node_modules/@dcl/sdk-commands/.runtime-data/server-storage.json`.



> [!WARNING]
> **📔 Nota**: Cómo informan los problemas las llamadas a Storage:
> 
> * `Storage.set()` y `Storage.player.set()` resuelven a un **boolean**. `false` significa que el valor fue **no** persistido, por ejemplo después de un error de red o cuando se ejecutan demasiadas solicitudes a la vez (ver abajo). Comprueba siempre el resultado: un valor descartado `false` es un guardado perdido en silencio.
> * `set()` lanza un `TypeError` cuando el valor no puede almacenarse como JSON sin cambios: `undefined`, `NaN` o `Infinity`, una función, un `Map` o un `Set`, entre otros. Para eliminar una clave, usa `delete()`.
> * `get()` se resuelve en el valor almacenado, o en `null` solo cuando la clave no existe. Cuando la lectura falla, lanza un error, así que un fallo nunca se confunde con una clave inexistente.
> * El `delete()` las variantes lanzan un error cuando la solicitud falla. Se resuelven en `false` solo cuando la clave no existía.


### Guarda en puntos de control, no en cada cambio

Storage es persistencia duradera para datos que deben sobrevivir a reinicios del servidor y redeploys. No es un datastore en vivo. Mantén el estado activo del juego en memoria en el servidor. Eso es más rápido y es el patrón correcto para un servidor. Escribe en Storage solo cuando realmente lo necesites, en puntos de control significativos.


> [!WARNING]
> **⚠️ Advertencia**: El runtime del servidor permite un máximo de **40 llamadas host en curso** a la vez, compartidas entre *todo* lo que la escena le pide al runtime que haga: cada solicitud de storage, `signedFetch`, y otras API del runtime cuentan para el mismo límite. Las llamadas excesivas no se **encolan**. Se rechazan inmediatamente con un `demasiadas llamadas host concurrentes` error. El SDK captura el rechazo y resuelve la `Storage.set` promesa a `false` en lugar de lanzar un error. Si tu código descarta ese booleano, el guardado fallido es invisible y tus datos persistidos terminan obsoletos o perdidos. Comprueba el resultado de cada escritura (ver la Nota arriba).


Buenos momentos para persistir:

* Fin de partida o final de una ronda.
* Un jugador se va.
* Un guardado periódico con debounce (por ejemplo, una vez cada 30 segundos), no una vez por frame.

Persiste solo los datos que deban sobrevivir a un reinicio o redeploy. Todo lo demás puede vivir en memoria.

El ejemplo siguiente mantiene las puntuaciones en memoria y las escribe solo en un punto de control, en lugar de hacerlo en cada punto anotado:

```typescript
import { Storage } from "@dcl/sdk/server"

// El estado de trabajo vive en memoria, actualizado en cada evento
const scores: Record<string, number> = {}

function onPointScored(address: string) {
  scores[address] = (scores[address] ?? 0) + 1
  // No hay llamada a Storage aquí — solo actualiza la memoria
}

// Persiste solo en un punto de control, como cuando un jugador se va
async function onPlayerLeave(address: string) {
  const saved = await Storage.player.set(address, "score", scores[address] ?? 0)
  if (!saved) {
    // La escritura no se persistió — mantiene el valor en memoria y vuelve a intentarlo más tarde
    console.error(`No se pudo guardar la puntuación de ${address}`)
  }
}
```

Para escenas con muchos jugadores concurrentes, un enfoque más robusto rastrea qué claves tienen cambios sin guardar (un "dirty set") y reintenta las escrituras fallidas en el siguiente flush:

```typescript
import { engine } from "@dcl/sdk/ecs"
import { Storage } from "@dcl/sdk/server"

// Estado de trabajo en memoria, actualizado en cada evento
const scores: Record<string, number> = {}
const dirty = new Set<string>()

function onPointScored(address: string) {
  scores[address] = (scores[address] ?? 0) + 1
  dirty.add(address) // Marcar para flush posterior, sin llamada a Storage aquí
}

// Vaciar las claves dirty periódicamente (por ejemplo, cada ~30 s) y en puntos de control
async function flush() {
  for (const address of dirty) {
    const ok = await Storage.player.set(address, "score", scores[address])
    if (ok) {
      dirty.delete(address) // Guardado correctamente
    }
    // Si ok es false, la clave sigue dirty y se reintenta en el siguiente flush
  }
}

// Ejecuta el flush en un temporizador usando un acumulador dt
let flushTimer = 0
engine.addSystem((dt) => {
  flushTimer += dt
  if (flushTimer > 30) {
    flushTimer = 0
    flush()
  }
})
```

### Storage del World — compartido entre todos los jugadores

```typescript
import { Storage } from "@dcl/sdk/server"

type Entry = { name: string; score: number }

// Escribir — set() se resuelve a false si el valor NO se persistió
const ok = await Storage.set("leaderboard", [
  { name: "Alice", score: 100 },
  { name: "Bob", score: 85 },
])
if (!ok) {
  console.error("No se pudo guardar la tabla de clasificación — inténtalo más tarde")
}

// Leer: null cuando la clave no existe
const leaderboard = (await Storage.get<Entry[]>("leaderboard")) ?? []

// Eliminar
await Storage.delete("leaderboard")
```

También puedes gestionar el storage de la escena mediante la línea de comandos, usando `npx sdk-commands storage scene`:

```bash
# Establecer un valor
npx sdk-commands storage scene set high_score --value 100

# Obtener un valor
npx sdk-commands storage scene get high_score

# Eliminar un valor
npx sdk-commands storage scene delete high_score

# Eliminar todos los datos de storage de la escena
npx sdk-commands storage scene clear --confirm
```

### Storage del jugador — por dirección de Wallet

```typescript
import { Storage } from "@dcl/sdk/server"

type Progress = { level: number; coins: number }

// Escribir — set() se resuelve a false si el valor NO se persistió
const ok = await Storage.player.set(playerAddress, "progress", {
  level: 5,
  coins: 250,
})
if (!ok) {
  console.error(`No se pudo guardar el progreso de ${playerAddress} — inténtalo de nuevo más tarde`)
}

// Leer: null cuando este jugador no tiene progreso guardado
const progress = (await Storage.player.get<Progress>(playerAddress, "progress")) ?? { level: 1, coins: 0 }

// Eliminar
await Storage.player.delete(playerAddress, "progress")
```

También puedes gestionar el storage del jugador mediante la línea de comandos, usando `npx sdk-commands storage player`:

```bash
# Establecer un valor para un jugador específico
npx sdk-commands storage player set level --value 10 --address 0x1234...

# Obtener un valor para un jugador específico
npx sdk-commands storage player get level --address 0x1234...

# Eliminar un valor para un jugador específico
npx sdk-commands storage player delete level --address 0x1234...

# Eliminar todos los datos de un jugador específico
npx sdk-commands storage player clear --address 0x1234... --confirm

# Eliminar todos los datos de jugador (todos los jugadores)
npx sdk-commands storage player clear --confirm
```

Cada valor de la escena puede tener hasta 512 KB, y todos los valores de la escena de un World comparten 10 MB. Cada valor de jugador puede tener hasta 100 KB, con 1 MB por jugador.

### Cargar un guardado sin sobrescribirlo

Nunca asignes datos predeterminados cuando un jugador se une y luego los guardes: si sus datos guardados existen pero aún no se han cargado, los valores predeterminados los reemplazan. Lee primero y crea valores predeterminados solo cuando `get()` se resuelve en `null`. Si la lectura lanza un error, deja al jugador sin cargar e inténtalo de nuevo más tarde.

```typescript
import { Storage } from "@dcl/sdk/server"

type Save = { coins: number }
const saves: Record<string, Save> = {}

async function loadOrCreate(address: string): Promise<Save> {
  if (saves[address]) return saves[address] // Ya cargado
  // Lanza un error si la lectura falla, así un fallo nunca se convierte en un guardado nuevo
  const stored = await Storage.player.get<Save>(address, "save")
  saves[address] = stored ?? { coins: 0 } // Valores predeterminados solo cuando no existe ningún guardado
  return saves[address]
}
```

Llámalo dentro de un `try/catch` en tu manejador de unión. Si lanza un error, dile al cliente que pueda reintentar, e ignora las acciones de ese jugador hasta que se cargue su guardado.

### Progreso mientras no hay nadie en la escena

No mantengas un temporizador en ejecución para cosas que crecen con el tiempo, como cultivos, energía o el reabastecimiento de una tienda. El Server Multiplayer se detiene un par de minutos después de que el último jugador se va (ver [Espera a que el servidor arranque](#wait-for-the-server-to-start-up)), así que el temporizador también se detiene. En su lugar, almacena el momento en que se actualizó por última vez el progreso. Cuando un jugador regrese, calcula el progreso a partir del tiempo transcurrido y adelanta la hora almacenada.

Usa la `Date.now()` para esto; nunca una hora enviada por un cliente: un jugador puede adelantar su reloj para saltarse la espera.

### Evita perder datos con escrituras concurrentes

Storage no tiene forma de actualizar una clave solo si nadie la cambió desde que la leíste (no hay "compare-and-set"). Así que si lees una clave, cambias el valor y lo escribes de nuevo, puedes perder datos:

* Dos instancias del servidor pueden escribir al mismo tiempo. Esto ocurre mientras una versión antigua y una nueva de tu scene se solapan durante un redeploy.
* Una lectura puede devolver un valor más antiguo que el último escrito. Storage ha servido un valor antiguo de una clave durante una hora o más, incluso con `Storage.get(key, { fresh: true })`, y a través de reinicios del servidor. La `fresh` opción solo omite la caché propia del SDK.
* Un listado con `Storage.getValues()` también puede estar desactualizado: puede omitir una clave escrita recientemente o seguir mostrando una que eliminaste.

Para datos que solo crecen, como un registro de resultados o comentarios de jugadores, usa este patrón en lugar de reescribir una sola clave:

1. Escribe cada instantánea en una **nueva clave** que se ordena por tiempo y no puede colisionar, por ejemplo `results-1-20261027-130512-k3fz`: un prefijo, un número de parte, la hora UTC y unos pocos caracteres aleatorios.
2. Para leer, enumera cada clave de la parte con `Storage.getValues({ prefix, offset })`, página por página hasta que tengas `pagination.total` entradas, y fusiónalas por id de fila.
3. Escribe las filas fusionadas como una nueva clave. Vuelve a leer esa clave y, solo si coincide, elimina las claves que fusionaste. Nunca elimines una clave que otro escritor todavía pueda cambiar basándose en tus propias lecturas.

Una condición de carrera entonces deja una clave extra, que la siguiente escritura fusiona, en lugar de una fila perdida. Cualquiera que lea los datos, incluido tú cuando los descargues desde la [UI de storage](#access-stored-data), debe leer cada clave de la parte y descartar los ids duplicados.

```typescript
import { Storage } from "@dcl/sdk/server"

type Row = { id: string; text: string }
const PREFIX = "results-1-"

// Cada clave que empieza con el prefijo, la más antigua primero
async function listAll(prefix: string) {
  const entries: { key: string; value: unknown }[] = []
  for (;;) {
    const page = await Storage.getValues({ prefix, offset: entries.length })
    entries.push(...page.data)
    if (page.data.length === 0 || entries.length >= page.pagination.total) break
  }
  return entries.sort((a, b) => (a.key < b.key ? -1 : 1))
}

async function appendRows(newRows: Row[]): Promise<boolean> {
  const versions = await listAll(PREFIX)
  // Fusionar por id de fila, gana la primera vista
  const rows = new Map<string, Row>()
  for (const { value } of versions) for (const row of value as Row[]) if (!rows.has(row.id)) rows.set(row.id, row)
  for (const row of newRows) if (!rows.has(row.id)) rows.set(row.id, row)
  const merged = [...rows.values()]

  // Una nueva clave cada vez: hora UTC más caracteres aleatorios
  // "2026-10-27T13:05:12.123Z" se convierte en "20261027-130512"
  const stamp = new Date().toISOString().slice(0, 19).replace(/[-:]/g, "").replace("T", "-")
  const key = `${PREFIX}${stamp}-${Math.random().toString(36).slice(2, 6)}`
  if (!(await Storage.set(key, merged))) return false // No se guardó, inténtalo de nuevo más tarde

  // Eliminar las claves fusionadas solo una vez que la nueva se lea de vuelta
  const check = await Storage.get<Row[]>(key, { fresh: true })
  if (JSON.stringify(check) !== JSON.stringify(merged)) return true
  for (const version of versions) {
    try {
      await Storage.delete(version.key)
    } catch {
      // Se deja en su lugar: la siguiente escritura lo vuelve a fusionar
    }
  }
  return true
}
```

Un valor puede contener como máximo 512 KB, así que empieza una nueva parte (`results-2-`) antes de que una parte se acerque a eso. El [ejemplo de Scene Feedback](https://github.com/decentraland/sdk7-test-scenes/tree/main/scenes/0%2C-1-scene-feedback) usa este patrón para sus archivos CSV y pasa a una nueva parte a los 400 KB.

### Acceder a los datos almacenados

Puedes ver y editar los datos almacenados en vivo en tu servidor a través de la UI de storage, entrando en este enlace:

[decentraland.org/storage](https://decentraland.org/storage)

También puedes acceder a esta página a través del Creator Hub. Abre la **Manage** pestaña, haz clic en los tres puntos junto a un lugar donde hayas publicado contenido y selecciona **Ver Storage**.

Allí puedes ver una lista de todos los worlds y LAND donde puedes publicar scenes.

Abre tu scene y luego la **Scene** o **Player** pestaña.

En la **Scene** pestaña verás una lista de todas las variables almacenadas. Desde aquí puedes editar o eliminar cualquiera de estas variables haciendo clic en el icono de lápiz o de papelera.

![Activate stream](/docs/_remote/76bca9abaef9d73bc207163d.webp)

En la **Player** pestaña verás una lista de todos los jugadores que tienen datos almacenados en tu servidor. Puedes buscarlos por dirección o nombre, y luego ver todos sus datos asociados. También puedes editar o eliminar estos datos haciendo clic en el icono de lápiz o de papelera.

### Cambiar la estructura de datos

Los datos almacenados en producción **no se borran cuando publicas una nueva versión de tu scene**. Esto es genial para tablas de clasificación, progreso de jugadores y cambios persistentes en el entorno que los jugadores esperan que sobrevivan más allá de pequeñas actualizaciones de tu scene.

La contrapartida es que los datos que están en storage fueron escritos por una versión anterior de tu código. Si tu nuevo código espera una forma diferente, analizar o leer esos datos antiguos puede fallar de maneras sutiles. Un campo que renombraste faltará. Un campo que antes era una cadena y ahora es un objeto lanzará un error cuando intentes acceder a una propiedad en él. Un jugador que no ha iniciado sesión en meses puede cargar datos anteriores a una estructura que tu código ya no sabe manejar.


> [!WARNING]
> **📔 Nota**: Los cambios de Schema no solo afectan a la primera lectura después de un deploy. Los datos almacenados viven hasta que se sobrescriben o se eliminan, así que un valor en formato antiguo puede aparecer en cualquier momento, a menudo de un jugador que vuelve y del que te habías olvidado.


#### Buenas prácticas

* *Lee siempre de forma defensiva*. Trata cualquier cosa que salga de storage como entrada no confiable, aunque la hayas escrito tú. El tipo que pasas a `get<T>()` no se comprueba, así que verifica que los campos existan y tengan el tipo correcto antes de leerlos, y ten listo un valor predeterminado razonable cuando no lo tengan:

  ```typescript
  import { Storage } from "@dcl/sdk/server"

  const saved = await Storage.player.get<any>(playerAddress, "progress")
  const progress = {
    level: typeof saved?.level === "number" ? saved.level : 1,
    coins: typeof saved?.coins === "number" ? saved.coins : 0,
  }
  ```
* *Añade campos, no los renombres ni los elimines*. El cambio de Schema más seguro es uno aditivo: introduce un nuevo campo con un valor predeterminado y deja los campos existentes intactos. Los datos antiguos simplemente no tendrán el nuevo campo, y tu análisis defensivo ya se encarga de eso. Renombrar un campo obliga a que todos los registros antiguos fallen.
* *Versiona tus objetos almacenados*. Incluye un `version` campo desde el primer día. Cuando leas datos, bifurca según la versión y migra las formas antiguas a la actual antes de usarlas. Esto mantiene el resto de tu código funcionando con una sola forma actual:

  ```typescript
  type ProgressV2 = { version: 2; level: number; coins: number; xp: number }

  function migrate(raw: any): ProgressV2 {
    const version = raw?.version ?? 1
    if (version === 1) {
      // v1 no tenía campo xp — asígnale un valor predeterminado
      return {
        version: 2,
        level: raw.level ?? 1,
        coins: raw.coins ?? 0,
        xp: 0,
      }
    }
    return raw as ProgressV2
  }
  ```
* *Vuelve a escribir el valor migrado*. Una vez que hayas actualizado un registro en memoria, guárdalo de nuevo para que la siguiente lectura ya esté en el nuevo formato. Con el tiempo esto vacía el conjunto de registros con forma antigua sin necesidad de un script de migración de una sola vez.
* *Para cambios incompatibles, usa una nueva clave*. Si la nueva estructura es realmente incompatible y no merece la pena migrar, escribe en una nueva clave de storage (por ejemplo `progress_v2`) e ignora la antigua. La clave antigua permanece inofensivamente en storage y evitas cualquier ruta de lectura que tenga que interpretarla. Puedes limpiar las claves antiguas más tarde mediante el [UI de storage](https://decentraland.org/storage) o los `npx sdk-commands storage` comandos.
* *Prueba con datos reales de producción*. Antes de desplegar un cambio estructural, extrae algunos registros reales de la UI de Storage y ejecuta tu nuevo código de análisis sobre ellos. Los casos límite que causan problemas suelen ser registros de los que no sabías que existían.
* *Deja una vía de escape*. Ten en cuenta que puedes editar o eliminar registros individuales desde la UI de Storage o mediante `npx sdk-commands storage`. Si un solo jugador queda atascado en un estado incorrecto después de un cambio de esquema, puedes corregir su registro directamente sin volver a desplegar.

## Variables de entorno

Configura tu escena sin codificar valores directamente en el código. Las variables de entorno son útiles para datos sensibles, y también para banderas de funciones o parámetros que se pueden cambiar fácilmente sin volver a publicar tu escena.

Las variables de entorno son **solo del servidor**. Protégelas con `isServer()`. El servidor puede leer las variables de entorno, pero no cambiar sus valores.

`EnvVar.get()` devuelve un `Promise<string>` y se resuelve en una cadena vacía cuando la variable no está definida, así que proporciona siempre un valor alternativo para los valores faltantes:

```typescript
import { EnvVar } from "@dcl/sdk/server"
import { isServer } from "@dcl/sdk/network"

export async function main() {
  if (!isServer()) return

  const maxPlayers = parseInt((await EnvVar.get("MAX_PLAYERS")) || "4")
  const gameDuration = parseInt((await EnvVar.get("GAME_DURATION")) || "300")
  const debugMode = ((await EnvVar.get("DEBUG")) || "false") === "true"
}
```

### Datos sensibles

Las variables de entorno son especialmente útiles para almacenar claves privadas, códigos de reclamación de recompensas y otros datos sensibles que sería arriesgado exponer en el código compilado de la escena pública.

Puedes almacenar claves privadas en el almacenamiento del servidor, y hacer que solo el servidor las lea con `isServer()`. De ese modo, los datos sensibles nunca pasan por la máquina del jugador.

### Desarrollo local

Para usar variables de entorno mientras ejecutas tu proyecto localmente, crea un `.env` archivo en la raíz de tu proyecto:

```
MAX_PLAYERS=8
GAME_DURATION=300
DEBUG=true
```

Importante: agrega `.env` a tu `.gitignore`, para que estos valores potencialmente sensibles nunca se suban a los servidores públicos de contenido.

### Cambiar variables de entorno

La forma más fácil de cambiar los valores de tus variables de entorno es a través de la UI de Storage.

Puedes acceder a los datos almacenados por el almacenamiento de la escena entrando en este enlace:

[decentraland.org/storage](https://decentraland.org/storage)

También puedes acceder a esta página a través del Creator Hub. Abre la **Manage** pestaña, haz clic en los tres puntos junto a un lugar donde hayas publicado contenido y selecciona **Ver Storage**.

Allí puedes ver una lista de todos los worlds y LAND donde puedes publicar scenes.

Abre tu scene y luego la **Entorno** pestaña. Deberías ver todas las variables de entorno del proyecto.

![Activate stream](/docs/_remote/13189fe9bb9d6d0cc3454293.webp)

Ten en cuenta que no puedes leer los valores de ninguna de estas variables de entorno (es para proteger los datos sensibles), pero sí puedes eliminar o sobrescribir cualquiera de ellas. Simplemente haz clic en el icono del lápiz o de la papelera.

También puedes gestionar variables de entorno desde la línea de comandos, usando `npx sdk-commands storage env`:

```bash
# Definir una variable
npx sdk-commands storage env set MAX_PLAYERS --value 8

# Eliminar una variable
npx sdk-commands storage env delete OLD_VAR

# Eliminar todas las variables de entorno
npx sdk-commands storage env clear --confirm
```

También puedes apuntar a un entorno específico con la `--target` bandera:

```bash
# Desplegar en staging
npx sdk-commands storage env set MY_KEY --value my_value --target https://storage.decentraland.zone

# Desplegar en un servidor local de desarrollo
npx sdk-commands storage env set MY_KEY --value my_value --target http://localhost:8000
```

Las variables de entorno desplegadas tienen prioridad sobre los `.env` valores.

## Estructura recomendada del proyecto

Separar el código del servidor, del cliente y compartido mantiene la base de código legible a medida que crece:

```
src/
├── index.ts              # Punto de entrada — rama isServer()
├── client/
│   ├── setup.ts          # Manejadores de Input, emisores de mensajes
│   └── ui.tsx            # UI de React ECS (lee el estado sincronizado)
├── server/
│   ├── server.ts         # Bucle de juego, manejadores de mensajes, mutaciones de estado
│   └── gameState.ts      # Funciones auxiliares para el estado del servidor
└── shared/
    ├── schemas.ts        # Definiciones de Component + validateBeforeChange
    └── messages.ts       # registerMessages() — importado por ambos lados
```


> [!NOTE]
> **💡 Consejo**: Mantén todas `registerMessages()` las llamadas y las definiciones de componentes personalizados en `shared/`. Tanto el servidor como el cliente importan desde allí, lo que garantiza que siempre estén de acuerdo en los esquemas de mensajes.


## Mejores prácticas de rendimiento

Cada cambio de Component envía el *entero* datos completos del componente por la red. Esto es diferente de lo que hace Colyseus, que solo envía diffs. Al diseñar componentes personalizados, ten esto en cuenta. La solución óptima puede ser almacenar datos en componentes separados, según la frecuencia de cambio.

### ❌ Evita componentes monolíticos

```typescript
import { engine, Schemas } from "@dcl/sdk/ecs"

// MAL — cambiar la puntuación también envía el array de posiciones
const GameState = engine.defineComponent("GameState", {
  playerAScore: Schemas.Int,
  playerBScore: Schemas.Int,
  timer: Schemas.Int,
  playerPositions: Schemas.Array(Schemas.Vector3), // carga útil grande
})
```

### ✅ Prefiere componentes atómicos

```typescript
import { engine, Schemas } from "@dcl/sdk/ecs"

// BIEN — cada actualización es pequeña e independiente
const PlayerScore = engine.defineComponent("PlayerScore", {
  playerA: Schemas.Int,
  playerB: Schemas.Int,
})

const GameTimer = engine.defineComponent("GameTimer", {
  secondsLeft: Schemas.Int,
})
```

*Regla general*: agrupa los campos que cambian juntos y con una frecuencia similar. Separa los datos que cambian rápido (temporizadores, posiciones) de los datos que cambian lentamente (puntuaciones, configuración).

### Limita la frecuencia de los mensajes frecuentes

Evita enviar mensajes en cada frame. Agrúpalos o limítalos cuando sea posible:

```typescript
import { engine } from "@dcl/sdk/ecs"
import { room } from "./shared/messages"

let lastSend = 0
engine.addSystem((dt) => {
  lastSend += dt
  if (lastSend > 0.1) {
    // cada 100 ms
    room.send("position", transform.position)
    lastSend = 0
  }
})
```

Por ejemplo, si el servidor controla un temporizador de cuenta regresiva, no es necesario enviar actualizaciones a todos los jugadores cada segundo. Lo mejor es que cada cliente calcule por su cuenta el paso del tiempo, y que el servidor transmita su estado actual cada 30 segundos aproximadamente, para garantizar la coherencia.

## Límites de recursos del servidor

El Multiplayer Server ejecuta cada escena en un isolate aislado con límites de recursos estrictos. Alcanzar estos límites puede descartar datos silenciosamente o terminar el servidor para todos en la escena, así que diseña tu escena para mantenerte muy por debajo de ellos.

### Memoria

El isolate tiene un **256 MB** límite máximo de memoria. Si se supera, el isolate se descarta y el servidor se apaga para todos los jugadores conectados. Mantén ligero el estado de trabajo y depura los datos por jugador cuando los jugadores se vayan.

### CPU

Cada turno de ejecución tiene un presupuesto de tiempo real:

* **Ejecución sincrónica**: 10 segundos por turno. Un bucle sin límite que supere esto mata el isolate.
* **Resolución del turno asíncrono**: 60 segundos. Si una `Promise` cadena o `await` tarda más que esto en resolverse, el isolate se termina.

Distribuye el trabajo pesado a lo largo de varios ticks usando un `dt` acumulador en `engine.addSystem()`. Nunca ejecutes bucles sin límite sincrónicos en el servidor.

### Tasa de mensajes entrantes

Cada peer conectado puede enviar aproximadamente hasta **300 mensajes por 1,000 ms**. Los frames de datos excesivos se descartan (no se encolan). Nunca envíes mensajes en cada frame desde el cliente. Consulta [Mejores prácticas de rendimiento](#performance-best-practices) para patrones de limitación.

### Tamaño de los mensajes

* Los paquetes entrantes tienen un límite de **128 KB** por paquete. Los paquetes demasiado grandes se descartan por completo.
* Los mensajes de Scene a comms tienen un límite aproximado de **30 KB**. Para la capa de transporte práctica, mantén los mensajes sincronizados muy por debajo de **13 KB** (consulta la [Mensajes](#messages) sección).

### Fetch externo

Concurrentes `signedFetch` las llamadas tienen un límite de **32** en vuelo. Los fetch adicionales se encolan hasta que se abre un cupo. Cada intento de fetch tiene un **de 15 segundos** de timeout. Las respuestas de fetch tienen un límite de **10 MB**. Las conexiones WebSocket están limitadas a **32** sockets concurrentes, con un tamaño máximo de mensaje de **1 MB** por mensaje.

### Llamadas al host en vuelo

El límite de 40 llamadas descrito en [Almacenamiento de datos](#data-storage) se aplica a todas las llamadas al host en todo el isolate, incluido Storage, `signedFetch`, y otras API de runtime. Las llamadas en exceso se rechazan de inmediato y no se encolan.

## Errores comunes

### Olvidar la validación en el estado exclusivo del servidor

Sin `validateBeforeChange`, los clientes pueden escribir en cualquier Component:

```typescript
import { engine, Schemas } from "@dcl/sdk/ecs"
import { isServer } from "@dcl/sdk/network"
import { AUTH_SERVER_PEER_ID } from "@dcl/sdk/network/message-bus-sync"

// ❌ MAL — los clientes pueden hacer trampas
const Score = engine.defineComponent("Score", { value: Schemas.Int })

// ✅ BIEN — solo servidor
if (isServer()) {
  Score.validateBeforeChange((v) => v.senderAddress === AUTH_SERVER_PEER_ID)
}
```

### Confiar en valores proporcionados por el cliente

Nunca permitas que un cliente dicte sus propios valores para datos importantes como salud, puntuación o posición:

```typescript
import { room } from "./shared/messages"

// ❌ MAL
room.onMessage("setHealth", (data) => {
  player.health = data.health // ¡el cliente controla el valor!
})

// ✅ BIEN — el servidor calcula el resultado
room.onMessage("takeDamage", (data) => {
  const damage = calculateDamage(data.source)
  player.health = Math.max(0, player.health - damage)
})
```

### Confiar en importes y direcciones en los mensajes

Un mensaje de un cliente es una solicitud, no un hecho. Comprueba cada campo antes de usarlo:

* **Importes**: `Schemas.Int` decodifica el valor como un entero de 32 bits, pero aun así puede ser cero o negativo. Rechaza importes de cero o menos, o un jugador podría "enviar" `-500` monedas y recibirlas en su lugar. Comprueba también que el remitente tenga suficiente.
* **Direcciones**: una dirección de wallet que envía el cliente debe ser `0x` seguida de 40 caracteres hexadecimales. Rechaza cualquier otra cosa antes de que llegue a tus maps o a Storage.

```typescript
import { room } from "./shared/messages"

const ADDRESS = /^0x[0-9a-f]{40}$/
const balances: Record<string, number> = {}

room.onMessage("giveCoins", (data, context) => {
  if (!context) return
  const from = context.from.toLowerCase()
  const to = String(data.to).toLowerCase()
  if (!ADDRESS.test(to) || to === from) return
  if (!Number.isInteger(data.amount) || data.amount <= 0) return // Rechaza los negativos y el cero
  if ((balances[from] ?? 0) < data.amount) return // No hay suficientes monedas
  balances[from] -= data.amount
  balances[to] = (balances[to] ?? 0) + data.amount
})
```

### Enviar mensajes antes de la sincronización del estado

Los clientes deben esperar hasta que el estado esté sincronizado antes de interactuar:

```typescript
import { engine } from "@dcl/sdk/ecs"
import { isStateSyncronized } from "@dcl/sdk/network"

engine.addSystem(() => {
  if (!isStateSyncronized()) return
  // seguro para enviar mensajes
})
```

### Espera a que el servidor arranque

El servidor solo está activo mientras haya al menos un jugador en la escena. Después de que se vaya el último jugador, el servidor permanece en marcha durante unos dos minutos y luego se apaga. La próxima visita inicia en frío una instancia nueva, lo que tarda aproximadamente **15 segundos en producción**. La Preview local inicia el servidor al instante, así que los problemas de arranque en frío casi siempre aparecen solo después de publicar.

El código de tu escena debe estar preparado para esperar a que el servidor esté en línea. Los mensajes enviados antes de que el servidor termine de arrancar se pierden silenciosamente. Muestra al jugador un mensaje de "el servidor se está despertando" mientras espera, y añade lógica de reintento a las solicitudes iniciales al servidor.


> [!NOTE]
> **Consejo:** Una forma fiable de detectar si el servidor está listo es un latido: haz que el servidor escriba `Date.now()` en un campo de Component sincronizado cada 2 segundos, y haz que el cliente haga un seguimiento de cuándo vio por última vez cambiar el valor. Si no llega ningún cambio en 6 segundos, considera que el servidor está desconectado. Esto es más robusto que `isStateSyncronized()`, que solo confirma que el transporte está conectado, no que el servidor esté ejecutándose.


## Ejemplo completo

Un contador multijugador minimalista: haz clic en un botón y el servidor incrementa una puntuación sincronizada. El servidor persiste el contador en `Storage` para que el valor sobreviva a los reinicios del servidor. Recuerda que el servidor se apaga cuando no hay jugadores en la escena, así que sin almacenamiento el contador se restablecería a cero cada vez que la escena quedara vacía de jugadores.

```typescript
import { engine, Schemas } from "@dcl/sdk/ecs"
import { registerMessages, isServer, syncEntity } from "@dcl/sdk/network"
import { AUTH_SERVER_PEER_ID } from "@dcl/sdk/network/message-bus-sync"
import { pointerEventsSystem } from "@dcl/sdk/ecs"
import { Storage } from "@dcl/sdk/server"

// 1. Define mensajes (compartido)
const Messages = {
  increment: Schemas.Map({}),
  stateUpdate: Schemas.Map({
    count: Schemas.Int,
    lastPlayer: Schemas.String,
  }),
}

// 2. Define un Component exclusivo del servidor (compartido)
const Counter = engine.defineComponent("Counter", {
  value: Schemas.Int,
  lastPlayer: Schemas.String,
})

// 3. Crea la room
const room = registerMessages(Messages)

export async function main() {
  if (isServer()) {
    // === SERVER ===

    // Solo el server puede modificar este component
    Counter.validateBeforeChange(
      (v) => v.senderAddress === AUTH_SERVER_PEER_ID
    )

    // Restaura el contador desde Storage en caso de que el servidor se haya reiniciado
    let initialCount = 0
    let initialPlayer = "none"
    let loaded = false
    try {
      initialCount = (await Storage.get<number>("counter")) ?? 0
      initialPlayer = (await Storage.get<string>("lastPlayer")) ?? "none"
      loaded = true
    } catch (error) {
      console.log("[Server] No se pudo cargar el contador:", error)
    }

    const counterEntity = engine.addEntity()
    syncEntity(counterEntity, [Counter.componentId], 1)
    Counter.create(counterEntity, {
      value: initialCount,
      lastPlayer: initialPlayer,
    })

    room.onMessage("increment", async (_data, context) => {
      if (!context) return
      if (!loaded) return // Nunca guardes encima de un contador que no se cargó
      const from = context.from.toLowerCase()
      const counter = Counter.getMutable(counterEntity)
      counter.value += 1
      counter.lastPlayer = from

      // Persiste en Storage para que el valor sobreviva a los reinicios del servidor
      const saved =
        (await Storage.set("counter", counter.value)) &&
        (await Storage.set("lastPlayer", from))
      if (!saved) console.log("[Server] El contador no se guardó, el siguiente clic lo guarda de nuevo")

      room.send("stateUpdate", {
        count: counter.value,
        lastPlayer: from,
      })
    })
  } else {
    // === CLIENT ===
    const button = engine.addEntity()
    // ... añade Transform, MeshRenderer, etc.

    pointerEventsSystem.onPointerDown(button, () => {
      room.send("increment", {})
    })

    room.onMessage("stateUpdate", (data) => {
      console.log(`Cuenta: ${data.count} (último clic por ${data.lastPlayer})`)
    })
  }
}
```

## Pruebas locales

La Preview estándar se encarga de todo. Al usar la rama auth-server del SDK, el servidor local se inicia automáticamente en segundo plano junto con la Preview del cliente.

Para probar interacciones multijugador localmente, abre la Preview en dos ventanas separadas; cada ventana se trata como un jugador distinto. Conecta cada ventana con una dirección diferente. Ambos clientes se conectarán a la misma instancia local del servidor.

Usando Creator Hub, activa **Vista previa de varias instancias** en el **Desktop Client** submenú de **Play Options**, luego haz clic en el **Play** botón por segunda vez para abrir una segunda ventana de Explorer de Decentraland. Debes conectarte en ambas ventanas con direcciones diferentes. Las mismas sesiones permanecerán abiertas mientras la escena se recarga.

Como alternativa, puedes abrir una segunda ventana de Explorer de Decentraland escribiendo lo siguiente en una URL del navegador:

> `decentraland://realm=http://127.0.0.1:8000&local-scene=true&debug=true&multi-instance=true`

### Consejos de debugging

* *Antepone un prefijo a tus logs* con `[SERVER]` o `[CLIENT]` para que puedas distinguirlos en la terminal:

  ```typescript
  import { isServer } from "@dcl/sdk/network"

  if (isServer()) {
    console.log("[SERVER] Starting...")
  } else {
    console.log("[CLIENT] Starting...")
  }
  ```
* *Verifica la sincronización de Component* en el cliente registrando los recuentos de entities:

  ```typescript
  import { engine } from "@dcl/sdk/ecs"

  engine.addSystem(() => {
    const entities = Array.from(engine.getEntitiesWith(MyComponent))
    console.log("[CLIENT] Synced entities:", entities.length)
  })
  ```

## Depura en producción

Usa [Scene Insights](/docs/creator/content-creator-es/scenes-sdk7/depuracion/scene-insights/) para buscar logs publicados del servidor, seguirlos en tiempo real y recuperar métricas de CPU, memoria y otras. Los logs y las métricas se conservan durante **7 días**, a través de reinicios y redeploys.

Actualiza a la última `auth-server` SDK, luego ejecuta estos comandos desde el proyecto de tu escena:

```bash
npx sdk-commands scene-insights login
npx sdk-commands scene-insights tail
```

Inicia sesión con una wallet autorizada en el navegador. La sesión de solo lectura se reutiliza para los comandos posteriores durante un máximo de 12 horas. Pulsa **Ctrl+C** para dejar de seguir los logs.

El [Guía de Scene Insights](/docs/creator/content-creator-es/scenes-sdk7/depuracion/scene-insights/) abarca búsquedas históricas, selección de escenas, permisos de acceso, métricas, debugging asistido por IA y límites de colección.

### Ver datos de Storage

Puedes acceder a los datos almacenados por el almacenamiento de la escena entrando en este enlace:

[decentraland.org/storage](https://decentraland.org/storage)

También puedes acceder a esta página a través del Creator Hub. Abre la **Manage** pestaña, haz clic en los tres puntos junto a un lugar donde hayas publicado contenido y selecciona **Ver Storage**.

Allí puedes ver una lista de todos los worlds y LAND donde puedes publicar scenes.

Abre el mundo o los datos del jugador para ver la información almacenada para cada uno.

Por ejemplo, si un jugador concreto tiene un problema al jugar tu escena, podrías buscar a ese jugador por dirección y ver qué datos tiene almacenados para entender su situación. Quizá se toparon con un caso límite en el que acabaron con datos contradictorios. Incluso puedes borrar o editar los datos de ese jugador desde esta página para devolverlo a un estado estable.

## Control de versiones

Cada versión publicada de tu escena obtiene su propio ID hash único, y cada hash se empareja con su propia instancia de servidor. Esto significa que el código del cliente y el código del servidor siempre avanzan juntos; no existe una ventana en la que un cliente que ejecuta lógica antigua hable con un servidor que ejecuta lógica nueva (o viceversa).

Cuando publicas una actualización:

* *Los jugadores que ya están en la escena* siguen viendo la versión antigua de la escena hasta que se van y vuelven. Sus clients permanecen conectados a la instancia del server que coincide con el hash antiguo.
* *Nuevos jugadores que llegan* después de la actualización cargan la nueva versión de la escena y se conectan a la nueva instancia del server.

Esto garantiza que el estado del client y del server nunca se desincronice debido a un cambio de schema o a un component renombrado. Una actualización nunca puede romper la sesión de un jugador que ya está en tu escena.

La desventaja es que, durante una breve ventana justo después de un deploy, los jugadores pueden terminar divididos entre dos instancias distintas del server. Un jugador que ya estaba allí y un jugador que acaba de llegar puede que no se vean entre sí ni puedan interactuar a través de la escena, aunque estén en la misma escena, hasta que los jugadores anteriores se vayan y vuelvan a entrar.


> [!NOTE]
> **💡 Consejo**: Los datos almacenados mediante el [Storage](#data-storage) service (como tablas de clasificación, progreso del jugador o cambios persistentes del entorno) se *no* borra entre versiones. El almacenamiento se conserva a nivel de ubicación y se comparte entre todas las instancias del server que apuntan a la misma escena, por lo que las nuevas versiones continúan exactamente donde la anterior lo dejó.


## Migrar desde Colyseus

Si tienes una escena existente construida sobre Colyseus, la tabla de abajo mapea los patrones comunes de Colyseus a sus equivalentes en SDK7:

| Colyseus                            | SDK7 Multiplayer Server                                      |
| ----------------------------------- | ------------------------------------------------------------ |
| `room.send(type, data)`             | `room.send(type, data)` — misma API                          |
| `room.onMessage(type, cb)`          | `room.onMessage(type, cb)` — misma API                       |
| `room.state.players` (schema)       | `syncEntity` + componentes personalizados                    |
| serialización JSON                  | serialización binaria (automática mediante `Schemas`)        |
| Aplicación de server separada       | Misma base de código — `isServer()` ramificación             |
| Alojamiento personalizado de server | Integrado: la vista previa ejecuta el server automáticamente |

Diferencias clave a tener en cuenta:

* *Serialización*: Colyseus envía diffs JSON; el SDK envía el component completo en cada cambio. Mantén los components pequeños (ver [Mejores prácticas de rendimiento](#performance-best-practices)).
* *Modelo de estado*: Colyseus usa un árbol de estado mutable con diffing automático. El SDK usa componentes ECS sincronizados mediante `syncEntity` y protegidos con `validateBeforeChange`.
* *Alojamiento*: No hay un despliegue de server separado. El Multiplayer Server se despliega automáticamente junto con la escena.


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-es/scenes-sdk7/networking/authoritative-servers.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
