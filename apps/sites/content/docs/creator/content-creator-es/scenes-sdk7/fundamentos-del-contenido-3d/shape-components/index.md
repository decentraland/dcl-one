> Aprende sobre los diferentes components que dan a las entities su forma 3D y su colisión.

# Componentes de forma

Las escenas tridimensionales en Decentraland se basan en el [modelo Entity-Component](https://en.wikipedia.org/wiki/Entity%E2%80%93component%E2%80%93system) , donde todo en una scene es un *entity*, y cada entity puede incluir *components* que moldean sus características y funcionalidad.

La forma renderizada de un entity está determinada por el component que usa.

![](/docs/_remote/d0b3835ca11dd6cc73d8d261.webp)

## Usa el Scene Editor en Creator Hub

La forma más fácil de darle a un entity una forma es usar el Scene Editor. Puedes agregar un **Mesh Renderer** component para proporcionar una forma primitiva, o un **GLTF** component para referenciar un modelo 3D desde un archivo. Consulta [Agregar components](/docs/creator/content-creator-es/scene-editor/construir/components/#add-components).

## Formas primitivas

Se pueden agregar varias formas básicas, a menudo llamadas *primitives*, a un entity dándole un `MeshRenderer` componente.

Las siguientes formas están disponibles. Varias formas incluyen campos adicionales opcionales, específicos de esa forma.

* **caja**:

  Usa `MeshRenderer.setBox()`, pasando el entity. Pasa `uvs` como un campo opcional adicional para mapear la alineación de la textura. Consulta [materials](/docs/creator/content-creator-es/scenes-sdk7/fundamentos-del-contenido-3d/materials/) para más detalles.
* **plano**:

  Usa `MeshRenderer.setPlane()`, pasando el entity. Pasa `uvs` como un campo opcional adicional para mapear la alineación de la textura. Consulta [materials](/docs/creator/content-creator-es/scenes-sdk7/fundamentos-del-contenido-3d/materials/) para más detalles.
* **esfera**:

  Usa `MeshRenderer.setSphere()`, pasando el entity.
* **cilindro**:

  Usa `MeshRenderer.setCylinder()`, pasando el entity. Pasa `radiusTop`  y `radiusBottom` como campos opcionales adicionales para modificar el cilindro.

  CONSEJO: Establece cualquiera de `radiusTop` o `radiusBottom` en 0 para crear un cono.

El siguiente ejemplo crea un cubo:

```ts
const myCube = engine.addEntity()

Transform.create(myCube, {
	position: Vector3.create(8, 1, 8),
})

MeshRenderer.setBox(myCube)
```

El siguiente ejemplo crea un cilindro con un `radiusTop` de 0, lo que produce un cono:

```ts
const myCone = engine.addEntity()

Transform.create(myCone, {
	position: Vector3.create(8, 1, 8),
})

MeshRenderer.setCylinder(myCone, 1, 0)
```

Las primitive shapes no incluyen materials. Para darle un color o una textura, debes asignar un [material component](/docs/creator/content-creator-es/scenes-sdk7/fundamentos-del-contenido-3d/materials/) al mismo entity.

Para hacer que una primitive sea clicable, o para impedir que los jugadores atraviesen, debes darle al entity un *collider* mediante un [MeshCollider](/docs/creator/content-creator-es/scenes-sdk7/fundamentos-del-contenido-3d/colliders/) componente.

Para cambiar la forma de un entity que ya tiene un `MeshRenderer` component, ejecuta `MeshRenderer.setBox()` o cualquiera de las otras funciones auxiliares y sobrescribirá la forma original. No hace falta eliminar el `MeshRenderer` original ni usar la sintaxis avanzada.

```ts
const myCube = engine.addEntity()

Transform.create(myCube, {
	position: Vector3.create(8, 1, 8),
})

MeshRenderer.setBox(myCube)

// sobrescribir la forma
MeshRenderer.setSphere(myCube)
```


> [!WARNING]
> **📔 Nota**: El `MeshRenderer` component debe importarse mediante
> 
> > `import { MeshRenderer } from "@dcl/sdk/ecs"`
> 
> Consulta [Importaciones](/docs/creator/content-creator-es/scenes-sdk7/primeros-pasos/coding-scenes/#imports) para ver cómo manejarlas fácilmente.


## Modelos 3D

Para formas más complejas, puedes construir un modelo 3D en una herramienta externa como Blender y luego importarlos en *.glTF* o *.glb* (binario *.glTF*). [glTF](https://www.khronos.org/gltf) (GL Transmission Format) es un proyecto abierto de Khronos que ofrece un formato común y extensible para assets 3D que es tanto eficiente como altamente interoperable con las tecnologías web modernas.

Para agregar un modelo externo a una scene, agrega un `GltfContainer` component a un entity y establece su `src` en la ruta del archivo glTF que contiene el modelo.

```ts
const houseEntity = engine.addEntity()

GltfContainer.create(houseEntity, {
	src: 'models/House.gltf',
})
```

La sección `src` el campo es obligatorio, debes darle un valor al construir el component. En el ejemplo anterior, el model se encuentra en una `models` carpeta en la raíz de la carpeta del proyecto de la scene.


> [!NOTE]
> **💡 Consejo**: Recomendamos mantener tus models separados en la `assets/scene/models` carpeta dentro de tu scene.


Los modelos glTF pueden incluir sus propias textures, materials, colliders y animations integradas. Consulta [Modelos 3D](https://github.com/decentraland/docs/blob/main/creator/3d-modeling/3d-models.md) para más información sobre esto. Para sobrescribir los materials de un model, usa [GltfNodeModifiers](/docs/creator/content-creator-es/scenes-sdk7/fundamentos-del-contenido-3d/materials/#modify-gltf-materials) componente. Vea [Modificar materiales glTF](/docs/creator/content-creator-es/scenes-sdk7/fundamentos-del-contenido-3d/materials/#modify-gltf-materials) para más detalles.

Para evitar que los jugadores atraviesen un modelo 3D, o para hacer que un model sea clicable, debes tener un [collider](/docs/creator/content-creator-es/scenes-sdk7/fundamentos-del-contenido-3d/colliders/), que puede estar incrustado en el model o proporcionado mediante un `MeshCollider` componente.

Ten en cuenta que todos los models, sus shaders y sus textures deben estar dentro de los parámetros del [limitaciones de la escena](/docs/creator/content-creator-es/scenes-sdk7/optimizacion/scene-limitations/).


> [!WARNING]
> **📔 Nota**: El `GltfContainer` component debe importarse mediante
> 
> `import { GltfContainer } from "@dcl/sdk/ecs"`
> 
> Consulta [Importaciones](/docs/creator/content-creator-es/scenes-sdk7/primeros-pasos/coding-scenes/#imports) para ver cómo manejarlas fácilmente.


### Precarga de un modelo 3D

En algunos casos, se agrega un modelo 3D personalizado a la scene pero no se usa inmediatamente. Por ejemplo, un modelo de silla personalizado puede cargarse solo cuando el jugador interactúa con otro objeto. En este escenario, la primera interacción podría tardar un tiempo hasta que termine de descargarse y cargarse el modelo.

Para evitar esto, usa el `AssetLoad.create` método para asegurar que el asset se descargue antes de que se necesite.

```ts
import { AssetLoad } from "@dcl/sdk/ecs"

AssetLoad.create(engine.RootEntity, {
  assets: [
    "assets/scene/bundle1/explosion.glb",
  ],
})
```

Para más información, consulta la [Pre Load Resources](/docs/creator/content-creator-es/scenes-sdk7/optimizacion/pre-load-resources/) documentación.

### Bibliotecas gratuitas para modelos 3D

En lugar de construir tus propios modelos 3D, también puedes descargarlos de varias bibliotecas gratuitas o de pago.

Para ayudarte a empezar, a continuación hay una lista de bibliotecas que tienen contenido gratuito o relativamente económico:

* [IWB Catalog](https://dcl-iwb.co/)
* [Asset Ovi](https://assetovi.com/)
* [Assets del Builder](https://github.com/decentraland/builder-assets/tree/master/assets)
* [SketchFab](https://sketchfab.com/)
* [Clara.io](https://clara.io/)
* [Archive3D](https://archive3d.net/)
* [SketchUp 3D Warehouse](https://3dwarehouse.sketchup.com/)
* [Thingiverse](https://www.thingiverse.com/) (modelos 3D hechos principalmente para impresión 3D, pero adaptables a Virtual Worlds)
* [ShareCG](https://www.sharecg.com/)
* [CGTrader](https://www.cgtrader.com/)


> [!WARNING]
> **📔 Nota**: Presta atención a las restricciones de licencia que tenga el contenido que descargas.


Ten en cuenta que en varios de estos sitios puedes elegir en qué formato descargar el modelo. Elige siempre el *.glTF* formato GLB si está disponible. Si no está disponible, debes convertirlos a *glTF* antes de poder usarlos en una scene. Para ello, recomendamos importarlos en Blender y exportarlos como *.glTF* desde allí.

### Optimizar modelos 3D

Para asegurar que los modelos 3D en tu scene carguen más rápido y ocupen menos memoria, sigue estas buenas prácticas:

* Guarda tus models en *.glb* formato, que es una versión más ligera de *.gltf*.
* Si tienes varios modelos que comparten las mismas textures, exporta tus modelos con las textures en un archivo separado. Así varios modelos pueden hacer referencia a un solo archivo de texture que solo necesita cargarse una vez.
* Si tu scene tiene entities que aparecen y desaparecen, puede ser una buena idea agrupar estas entities y mantenerlas bajo tierra, o con una escala de 0. Esto ayudará a que aparezcan más rápido; la desventaja es que ocuparán memoria cuando no se estén usando.

## Estirar una forma

Las primitive shapes y los modelos 3D tienen dimensiones predeterminadas que puedes alterar cambiando la escala en el entity `Transform` componente.

```ts
const primitiveEntity = engine.addEntity()

MeshRenderer.setBox(primitiveEntity)

Transform.create(primitiveEntity, {
	position: { x: 8, y: 1, z: 8 },
	scale: { x: 4, y: 0.5, z: 4 },
})
```

## Hacer invisible

Puedes hacer que un entity sea invisible dándole a un entity un `VisibilityComponent`, con su propiedad `visible` establecida en *false*.

```ts
const myEntity = engine.addEntity()
Transform.create(myEntity, {
	position: Vector3.create(4, 0, 4),
})
MeshRenderer.setBox(myEntity)

VisibilityComponent.create(myEntity, { visible: false })
```

La sección `VisibilityComponent` funciona igual para entities con primitive shapes y con `GLTFContainer` components.

Si un entity es invisible, su collider puede bloquear el camino de un jugador y/o impedir hacer click en entities que estén detrás de él, dependiendo de las collision layers asignadas al collider.

### Propagar la visibilidad

Puedes usar el `propagateToChildren` campo en el `VisibilityComponent` para aplicar una configuración a cada child en el árbol de children del entity. Si `propagateToChildren` se establece en *true*, estos ajustes afectan a todos los children en todos los niveles descendentes. Esto puede ahorrarte mucho trabajo tedioso al marcar cada child entity como invisible o visible también.

```ts
// Entity padre (explícitamente invisible)
const parentEntity = engine.addEntity()
Transform.create(parentEntity, {
	position: Vector3.create(4, 0, 4),
})
MeshRenderer.setBox(parentEntity)
VisibilityComponent.create(parentEntity, { visible: false, propagateToChildren: true })

// Entity hijo (implícitamente invisible por el padre)
const child = engine.addEntity()
Transform.create(child, {
	position: Vector3.create(0, 1, 0),
	parent: parentEntity
})
MeshRenderer.setBox(child)
```


> [!WARNING]
> **📔 Nota**: Si un entity tiene su propio `VisibilityComponent`, esto anula cualquier configuración de los parents.
> 
> Si un entity no tiene su propio `VisibilityComponent`, entonces su visibilidad la determina su parent más cercano con un `VisibilityComponent`  y `propagateToChildren` establecido en *true*.


## Estado de carga

Si un modelo 3D es bastante grande, podría tardar un tiempo notable en renderizarse; este tiempo puede variar dependiendo del hardware del jugador y de muchos otros factores. A veces necesitas asegurarte de que un model haya terminado de cargarse antes de realizar otra acción. Por ejemplo, si quieres teletransportar al jugador a una plataforma en el cielo, primero necesitas asegurarte de que la plataforma esté completamente renderizada antes de mover al jugador allí, o de lo contrario el jugador podría caer directamente a través de la plataforma.

Para comprobar si un modelo 3D ha terminado de renderizarse, revisa el `GltfContainerLoadingState` component. Este component está pensado para ser de solo lectura y existe en cualquier entity que también tenga un `GltfContainer`componente.

Este component tiene una sola propiedad llamada `currentState`, que contiene un valor del `LoadingState` enum.

El siguiente ejemplo usa un system para comprobar periódicamente el estado de carga del modelo 3D de un entity. Si el estado es `LoadingState.FINISHED`, quizá quieras ejecutar allí una lógica personalizada y finalizar la ejecución del system.

```ts
export function main() {
	const meshEntity = engine.addEntity()
	GltfContainer.create(meshEntity, { src: 'models/Monster.glb' })
	engine.addSystem((deltaTime) => {
		const loadingState = GltfContainerLoadingState.getOrNull(meshEntity)
		if (!loadingState) return
		switch (loadingState.currentState) {
			case LoadingState.LOADING:
				console.log('mesh está LOADING')
				break
			case LoadingState.FINISHED:
				console.log('mesh está FINISHED')
				// Ejecutar lógica personalizada
				break
			case LoadingState.FINISHED_WITH_ERROR:
				console.log('mesh ha FINISHED PERO PUEDE TENER PROBLEMAS')
				break
			case LoadingState.UNKNOWN:
				console.log('mesh está en un ESTADO UNKNOWN')
				break
		}
	})
}
```

## Sintaxis avanzada

La sintaxis completa para crear un `MeshRenderer` component, sin ningún helper para simplificarla, se ve así:

```ts
MeshRenderer.create(myBox, {
	mesh: {
		$case: 'box',
		box: { uvs: [] },
	},
})

MeshRenderer.create(myPlane, {
	mesh: {
		$case: 'plane',
		plane: { uvs: [] },
	},
})

MeshRenderer.create(myShpere, {
	mesh: {
		$case: 'sphere',
		sphere: {},
	},
})

MeshRenderer.create(myCylinder, {
	mesh: {
		$case: 'cylinder',
		cylinder: {},
	},
})
```

Así es como el protocolo base interpreta los components de MeshRenderer. Las funciones auxiliares abstraen esto y exponen una sintaxis más amigable, pero detrás de escena generan esta sintaxis.

La sección `$case` campo te permite especificar uno de los tipos permitidos. Cada tipo admite un conjunto diferente de parámetros. En el ejemplo anterior, el `caja` tipo admite un `uvs` campo.

Los valores compatibles para `$case` son los siguientes:

* `caja`
* `plano`
* `esfera`
* `cilindro`

Dependiendo del valor de `$case`, es válido definir el objeto para la forma correspondiente, pasando cualquier propiedad relevante.

Para agregar un `MeshRenderer` component a un entity que potencialmente ya tiene una instancia de este component, usa `MeshRenderer.createOrReplace()`. Las funciones auxiliares como `MeshRenderer.setBox()` se encargan de sobrescribir las instancias existentes del component, pero ejecutar `MeshRenderer.create()` en un entity que ya tiene este component devuelve un error.


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-es/scenes-sdk7/fundamentos-del-contenido-3d/shape-components.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
