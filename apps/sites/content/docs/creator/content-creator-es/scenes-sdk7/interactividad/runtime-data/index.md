> Obtén datos del contexto en el que se ejecuta tu escena y de la propia escena.

# Datos en tiempo de ejecución

## Obtener hora de Decentraland

Decentraland sigue un ciclo de día/noche que tarda 2 horas en completarse, así que hay 12 ciclos completos cada día. Los jugadores también pueden cambiar la configuración para experimentar una hora fija específica del día; por ejemplo, para ver siempre Decentraland con un cielo nocturno de las 10 p. m. Por esta razón, la hora de Decentraland puede variar de un jugador a otro.

Usa `getWorldTime()` para obtener la hora del día que el jugador está experimentando dentro de Decentraland.

```ts
import { getWorldTime } from '~system/Runtime'

executeTask(async () => {
  let time = await getWorldTime({})
  console.log(time.seconds)
})
```


> [!NOTE]
> **💡 Consejo**: El `getWorldTime()` función es asíncrona. Ver [Funciones asíncronas](/docs/creator/content-creator-es/scenes-sdk7/patrones-de-programacion/async-functions/) si no estás familiarizado con ellas.


`getWorldTime()` devuelve un objeto con una `seconds` propiedad. Esta propiedad indica cuántos segundos han transcurrido (en tiempo de Decentraland) desde el inicio del día, suponiendo que el ciclo completo dura 24 horas. Divide el valor de seconds entre 60 para obtener los minutos, y entre 60 de nuevo para obtener las horas desde el inicio del día. Por ejemplo, si el `seconds` valor es *36000*, corresponde a *10 a. m.*.

En el tiempo de Decentraland, el sol siempre sale a las 6:15 y se pone a las 19:50.

Podrías usar esta información para cambiar la escena en consecuencia; por ejemplo, para reproducir sonidos de pájaros cuando hay luz del día y grillos cuando está oscuro, o para activar los materiales emisivos de las farolas cuando está oscuro.

```ts
import { getWorldTime } from '~system/Runtime'

executeTask(async () => {
  let time = await getWorldTime({})
  console.log(time.seconds)
  if (time.seconds < 6.25 * 60 * 60 || time.seconds > 19.85 * 60 * 60) {
    // hora nocturna
    console.log('playing cricket sounds')
  } else {
    // hora diurna
    console.log('playing bird sounds')
  }
})
```

## Obtener datos del Realm

Los jugadores en Decentraland existen en varios *realms*. Los jugadores en diferentes realms no pueden verse, interactuar ni chatear entre sí, incluso si están de pie en los mismos parcels. Dividir a los jugadores de esta manera permite a Decentraland gestionar una cantidad ilimitada de jugadores sin encontrar ninguna limitación. También agrupa a los jugadores que están en regiones cercanas, para garantizar que los tiempos de ping entre los jugadores que interactúan sean aceptables.

Si tu escena envía datos a un [servidor de terceros](/docs/creator/content-creator-es/scenes-sdk7/networking/third-party-servers/) para sincronizar cambios entre jugadores en tiempo real, entonces a menudo es importante que los cambios solo se sincronicen entre jugadores que estén en el mismo realm. Debes tratar todos los cambios que pertenezcan a un realm como separados de los de un realm diferente. De lo contrario, los jugadores verán que las cosas cambian de una forma extraña, sin que nadie haga el cambio.

```ts
import { getRealm } from '~system/Runtime'

executeTask(async () => {
  const { realmInfo } = await getRealm({})
  if (!realmInfo) return
  console.log(`Estás en el realm: `, realmInfo.realmName)
})
```


> [!NOTE]
> **💡 Consejo**: El `getRealm()` función es asíncrona. Ver [Funciones asíncronas](/docs/creator/content-creator-es/scenes-sdk7/patrones-de-programacion/async-functions/) si no estás familiarizado con ellas.


Decentraland gestiona sus comunicaciones entre jugadores (incluidas las posiciones de los jugadores, el chat, los mensajes de messageBus y los cambios de estado de smart items) a través de una red descentralizada de servidores de comunicación; cada uno de estos servidores se llama **Realm**. Cada uno de estos servidores puede admitir múltiples **salas** (también llamados **islands**salas), cada uno agrupando un conjunto diferente de jugadores que están cerca unos de otros en el mapa de Decentraland.

El `getRealm()` La función devuelve la siguiente información:

* `baseUrl`: *(string)* El dominio del servidor del Realm
* `realmName`: *(string)* El nombre del servidor del Realm
* `networkId`: *(number)* La red de Ethereum
* `commsAdapter`: *(string)* Adaptador de comunicaciones, eliminando todos los parámetros de consulta (credenciales)
* `isPreview`: *(boolean)* Verdadero si la escena se está ejecutando como una vista previa local, en lugar de estar publicada en Decentraland.
* `isConnectedSceneRoom`: *(boolean)* Verdadero si el usuario está conectado a la sala de la escena.


> [!WARNING]
> **📔 Nota**: El `layer` la propiedad está obsoleta y debería evitarse.


A medida que los jugadores se mueven por el mapa, pueden cambiar de salas para agruparse con aquellos jugadores que ahora están más cerca de ellos. Las salas también desplazan sus límites dinámicamente para adaptarse a un grupo manejable de personas, así que incluso si un jugador se queda quieto, a medida que los jugadores entran y salen del World, el jugador podría encontrarse en otra sala. Los jugadores en un mismo `room` se comunican y compartirán mensajes a través de MessageBus incluso si están demasiado lejos para verse entre sí. Los jugadores en el mismo server pero en diferentes salas no se están comunicando actualmente, pero podrían empezar a comunicarse a medida que se mueven por el mapa y cambian de sala.

Para reaccionar a cambios relacionados con el realm o la sala del jugador, usa el `onChange` función en el `RealmInfo` componente, que el engine añade a la `engine.RootEntity`. Este componente contiene los mismos campos que devuelve `getRealm()`.

```ts
import { engine, RealmInfo } from '@dcl/sdk/ecs'

export function main() {
	RealmInfo.onChange(engine.RootEntity, (realmInfo) => {
		if (!realmInfo) return
		console.log('Realm cambiado: ', realmInfo.realmName)
	})
}
```


> [!WARNING]
> **📔 Nota**: Cuando la escena se carga por primera vez, puede que todavía no se haya asignado una sala al jugador. El Explorer terminará asignando una sala al jugador, pero a veces esto puede ocurrir un par de segundos después de que la escena se haya cargado.


## Obtener plataforma del jugador

Los jugadores pueden acceder a Decentraland a través de varias plataformas, incluida la aplicación oficial de escritorio, y las versiones web y de escritorio obsoletas, así como [clientes experimentales alternativos](https://github.com/decentraland/protocol-squad) construidos para otros motores.

Usa `getExplorerInformation()` para saber en qué plataforma está ejecutando Decentraland el jugador actual.

```ts
import { getExplorerInformation } from '~system/Runtime';

executeTask(async () => {
  let data = await getExplorerInformation({})
  console.log("PLATFORM: ", data.platform, " AGENT: ", data.agent)
})
```

Al usar la aplicación oficial de escritorio de Decentraland, esta función debería devolver los siguientes datos:

```
{
    agent: unity-explorer,
    platform: desktop
}
```

Los jugadores que usan la aplicación oficial de escritorio de Decentraland probablemente tendrán una experiencia mucho más fluida que los del navegador, ya que el navegador impone limitaciones de rendimiento sobre cuánto del poder de procesamiento de la máquina puede usar la pestaña del navegador. También les faltarán muchas funciones como el control de la cámara, las luces dinámicas, la congelación del movimiento del jugador, las mejoras de UI, etc.

## Obtener el idioma del jugador

Usa `getPlayerLanguage()` para saber a qué idioma está configurado el client del jugador. Devuelve una [BCP 47](https://en.wikipedia.org/wiki/IETF_language_tag) tag, como `'en'`, `'es'` o `'pt-BR'`. Úsalo para elegir qué traducción de los textos de tu escena mostrar.

```ts
import { getPlayerLanguage, onPlayerLanguageChanged } from '@dcl/sdk/platform'

console.log('LANGUAGE: ', getPlayerLanguage())

// Reacciona cuando el jugador cambia de idioma a mitad de sesión
onPlayerLanguageChanged.add(({ language }) => {
  console.log('LANGUAGE CHANGED TO: ', language)
})
```

`onPlayerLanguageChanged` se activa cada vez que el jugador cambia el idioma del client mientras está en tu escena. Los clients antiguos nunca lo emiten.


> [!WARNING]
> **📔 Nota**: `getPlayerLanguage()` devuelve `'en'` hasta que el Explorer haya informado su idioma a la escena, lo que ocurre poco después de que la escena comienza. También devuelve `'en'` en clients que no informan de un idioma. Ambas funciones requieren `@dcl/sdk` la versión 7.30.0 o posterior.


## El componente EngineInfo

El `EngineInfo`el componente hace un seguimiento de los datos sobre el ciclo de vida de la escena, lo cual a veces puede ser útil para saber cuándo ocurre un evento, en relación con la inicialización de la escena.

Este componente se añade a la `engine.RootEntity`.

```ts
engine.addSystem((deltaTime) => {
  const engineInfo = EngineInfo.getOrNull(engine.RootEntity)
  if (!engineInfo) return

  console.log(
    '--------------' +
      '\nframeNumber: ' +
      engineInfo.frameNumber +
      '\ntickNumber: ' +
      engineInfo.tickNumber +
      '\ntotalRuntime: ' +
      engineInfo.totalRuntime +
      '\nsceneHidden: ' +
      engineInfo.sceneHidden +
      '\n--------------'
  )
})
```

Puedes usar `sceneHidden` para pausar el trabajo intensivo en recursos mientras la escena no sea visible:

```ts
engine.addSystem((deltaTime) => {
  const engineInfo = EngineInfo.getOrNull(engine.RootEntity)
  if (engineInfo?.sceneHidden) return

  // La lógica normal del juego se ejecuta solo cuando la escena es visible
})
```

El `EngineInfo` el componente contiene los siguientes datos:

* `frameNumber`: contador de fotogramas del engine.
* `totalRuntime`: tiempo total de ejecución de esta escena, en segundos.
* `tickNumber`: contador de ticks de la escena según [ADR-148](https://adr.decentraland.org/adr/ADR-148).
* `sceneHidden`: `true` cuando la escena está oculta detrás de la UI de pantalla completa del Explorer. Eso cubre la pantalla de carga, y también el mapa, la mochila y el menú de ajustes. Úsalo para silenciar el audio y pausar el trabajo pesado mientras el jugador no puede ver la escena.


> [!WARNING]
> **📔 Nota**: El `EngineInfo` component debe importarse mediante
> 
> > `import { EngineInfo } from "@dcl/sdk/ecs"`
> 
> Consulta [Importaciones](/docs/creator/content-creator-es/scenes-sdk7/primeros-pasos/coding-scenes/#imports) para saber cómo manejarlos fácilmente.


### Reaccionar al desvanecimiento de la pantalla de carga

El `scene_hidden` campo te indica si el jugador realmente puede ver tu escena, o si está cubierta por la UI de pantalla completa del Explorer. Mientras la pantalla de carga está visible, `sceneHidden` es `true`. En el momento en que la pantalla de carga se desvanece y el jugador ve por primera vez el mundo, se vuelve `false`.

Se vuelve `true` de nuevo más tarde cada vez que una UI de Explorer a pantalla completa cubre la escena, como el mapa, la mochila o el menú de ajustes. Eso lo convierte en una buena señal para silenciar [audio](/docs/creator/content-creator-es/scenes-sdk7/fundamentos-del-contenido-3d/sounds/) y pausar sistemas costosos, y luego reanudarlos cuando vuelva a `false`.

Esta es la única manera en que una escena puede saber cuándo ocurre esa primera revelación. Úsalo para retener cualquier cosa que, de otro modo, se desarrollaría detrás de la pantalla de carga y el jugador se perdería: cinemáticas de introducción, sonidos de bienvenida, un tween que solo se aprecia bien si se observa, una UI de apertura o un evento de analítica que solo debería contarse cuando el jugador esté realmente allí.

```ts
import { engine, EngineInfo } from '@dcl/sdk/ecs'

function onSceneRevealed() {
  // El jugador ahora está mirando la escena, inicia la intro aquí
  console.log('La pantalla de carga acaba de desvanecerse')
}

engine.addSystem(function waitForSceneRevealed() {
  const engineInfo = EngineInfo.getOrNull(engine.RootEntity)
  if (!engineInfo || engineInfo.sceneHidden) return

  // Ejecutar solo una vez
  engine.removeSystem(waitForSceneRevealed)
  onSceneRevealed()
})
```


> [!WARNING]
> **📔 Nota**: Tu escena sigue ejecutándose normalmente mientras `sceneHidden` es `true`, simplemente no se está mostrando. No uses este campo para pausar la lógica de tu escena; úsalo para medir el momento en que se supone que el jugador debe presenciar algo.
> 
> `scene_hidden` requiere un `@dcl/sdk` y una versión reciente del Decentraland Explorer. En clients más antiguos, el campo se queda con su valor predeterminado de `false`, así que una escena que espera por él sigue ejecutándose; simplemente no estará sincronizada con el desvanecimiento de la pantalla de carga.



---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-es/scenes-sdk7/interactividad/runtime-data.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
