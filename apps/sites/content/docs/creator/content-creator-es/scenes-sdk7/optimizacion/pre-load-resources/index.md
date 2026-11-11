> La precarga de recursos permite que los assets se descarguen al iniciar la escena para que estén listos cuando el jugador interactúe con ellos por primera vez.

# Precargar recursos

En algunos casos, un asset se añade a la scene, pero no se usa de inmediato. Por ejemplo, un archivo de sonido puede reproducirse solo cuando el player presiona un botón. En este escenario, la primera vez que el player presiona el botón, el audio puede reproducirse un par de segundos demasiado tarde, porque el archivo solo se descarga cuando se necesita.

Para evitar este problema, usa el `AssetLoad` component para asegurarte de que estos assets se descarguen y estén listos para usar antes de que se necesiten.

```ts
import { AssetLoad } from "@dcl/sdk/ecs"

AssetLoad.create(engine.RootEntity, {
  assets: [
    "assets/scene/bundle1/explosionSound.mp3",
    "assets/scene/bundle1/explosion.glb",
  ],
})
```

Los assets listados en el `AssetLoad` component se descargan y se añaden a la memoria, asegurando que estén disponibles al instante cuando la scene necesite cargarlos.

Algunas consideraciones importantes:

* Puedes colocar el `AssetLoad` component en cualquier entity (no solo en RootEntity), y puedes usarlo en tantas entities como sea necesario. Esto puede ser útil para manejar estados de carga separados para distintos niveles o regiones de tu scene.
* **Mantén cada `AssetLoad` entity en unas 40 paths.** Divide una lista más larga entre varias entities. Consulta [Divide listas largas entre entities](#split-long-lists-across-entities).
* El `AssetLoad` component se usa para añadir assets a la memoria, no para eliminarlos. Quitar un asset de la lista en `AssetLoad.create` no liberará memoria.
* Si un asset se usa inmediatamente cuando la scene se carga (por ejemplo, un modelo GLB colocado en la scene, o un sonido de fondo que se reproduce continuamente), no hace falta usar el `AssetLoad` component, ya que ya se está descargando.
* Ten cuidado al añadir assets a `AssetLoad.create`, y solo precarga assets que no se requieran al inicio de la scene para evitar costos de rendimiento innecesarios.
* Solo puedes precargar assets que se hayan subido como parte de los archivos de la scene. Esta función no funcionará para precargar imágenes desde una URL externa

## Reaccionar al estado de carga

La precarga de assets es más útil si tu scene puede reaccionar cuando termina de descargarse cada asset. Por ejemplo, quizá quieras mantener una pantalla de carga hasta que todos los assets estén listos, mostrar el progreso al player o manejar con elegancia un asset que no se pudo descargar.

Para hacer un seguimiento de esto, usa el `assetLoadLoadingStateSystem`. Llama a su `registerAssetLoadLoadingStateEntity` método, pasando la entity que contiene el `AssetLoad` component y una función callback. El callback se ejecuta cada vez que cambia el estado de carga de uno de los assets de esa entity.

```ts
import {
  AssetLoad,
  LoadingState,
  assetLoadLoadingStateSystem,
} from "@dcl/sdk/ecs"

const preloader = engine.addEntity()
AssetLoad.create(preloader, {
  assets: [
    "assets/scene/bundle1/explosionSound.mp3",
    "assets/scene/bundle1/explosion.glb",
  ],
})

assetLoadLoadingStateSystem.registerAssetLoadLoadingStateEntity(
  preloader,
  (assetLoadState) => {
    console.log(
      `Asset ${assetLoadState.asset} is now: ${assetLoadState.currentState}`
    )
  }
)
```

El callback recibe un objeto con las siguientes propiedades:

* `asset`: La ruta del asset cuyo estado cambió. Esto coincide con una de las cadenas que enumeraste en el `AssetLoad` component.
* `currentState`: Un valor del `LoadingState` enum, que describe el nuevo estado de ese asset.

El `LoadingState` enum puede contener los siguientes valores:

* `LoadingState.LOADING`: El asset se está descargando actualmente.
* `LoadingState.FINISHED`: El asset se descargó correctamente y ya está listo para usar.
* `LoadingState.FINISHED_WITH_ERROR`: El asset se encontró, pero ocurrió un error al descargarlo.
* `LoadingState.NOT_FOUND`: No se encontró ningún asset en la ruta proporcionada.
* `LoadingState.UNKNOWN`: El estado del asset no se conoce.

El siguiente ejemplo usa el estado de carga para cambiar el color de una entity según cómo progresa su precarga:

```ts
import {
  AssetLoad,
  LoadingState,
  Material,
  assetLoadLoadingStateSystem,
} from "@dcl/sdk/ecs"
import { Color4 } from "@dcl/sdk/math"

function getLoadingColor(state: LoadingState): Color4 {
  switch (state) {
    case LoadingState.FINISHED:
      return Color4.Green()
    case LoadingState.LOADING:
      return Color4.Yellow()
    case LoadingState.FINISHED_WITH_ERROR:
    case LoadingState.NOT_FOUND:
      return Color4.Red()
    default:
      return Color4.Gray()
  }
}

assetLoadLoadingStateSystem.registerAssetLoadLoadingStateEntity(
  preloader,
  (assetLoadState) => {
    Material.setPbrMaterial(myEntity, {
      albedoColor: getLoadingColor(assetLoadState.currentState),
    })
  }
)
```


> [!NOTE]
> **💡 Consejo**: Puedes añadir más assets a un `AssetLoad` component después de crearlo, por ejemplo en respuesta a una acción del player. Usa `AssetLoad.getOrCreateMutable()` para obtener el component y añadir nuevas rutas a su `assets` array. El callback registrado también se activará para estos assets recién añadidos a medida que se descarguen.


### Divide listas largas entre entities

Dale a cada `AssetLoad` entity **como máximo 40 paths**. A partir de aproximadamente 50 assets en una entity, su callback deja de activarse y los assets restantes nunca se informan, sin ningún error.

La razón es un límite estricto en cómo se almacenan los estados de carga. El client añade una entrada por cada cambio de estado por asset, normalmente `LOADING` y luego `FINISHED`, en un set que contiene 100 entradas por entity. Una vez que ese set se llena, se queda en 100, y el SDK solo llama a tu callback cuando cambia el número de entradas, así que nada más pasa.

Para precargar más de 40 assets, divide la lista en lotes y dale a cada lote su propia entity y su propio callback:

```ts
import { AssetLoad, assetLoadLoadingStateSystem, engine } from '@dcl/sdk/ecs'

const allAssets = [
	'assets/models/tree.glb',
	'assets/models/rock.glb',
	// ...una lista larga
]

// Una entity AssetLoad por lote de 40 paths
for (let i = 0; i < allAssets.length; i += 40) {
	const batchEntity = engine.addEntity()
	AssetLoad.create(batchEntity, { assets: allAssets.slice(i, i + 40) })

	assetLoadLoadingStateSystem.registerAssetLoadLoadingStateEntity(batchEntity, (assetLoadState) => {
		console.log(assetLoadState.asset, assetLoadState.currentState)
	})
}
```


> [!NOTE]
> **💡 Consejo**: Si tu scene espera a que cada asset termine antes de empezar, ponle un timeout a esa espera y registra los paths sobre los que nunca recibiste respuesta. Una lista con nombre es mucho más fácil de depurar que una pantalla de carga que nunca desaparece.



> [!NOTE]
> **💡 Consejo**: Para ver un ejemplo funcional de este component, consulta la [`88,-12-asset-load`](https://github.com/decentraland/sdk7-test-scenes/tree/main/scenes/88,-12-asset-load) test scene, que precarga un mp3, una texture, un video y un glb a través de un `AssetLoad` component y reporta el estado de cada asset a través de `assetLoadLoadingStateSystem` — incluido un path faltante a propósito que se resuelve como `NOT_FOUND`.



---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-es/scenes-sdk7/optimizacion/pre-load-resources.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
