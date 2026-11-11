# Colliders

Aprende sobre los diferentes components que dan a las entities su forma 3D y su colisión.

Las entidades que tienen colliders ocupan espacio y bloquean el paso de un jugador; las entidades sin colliders pueden atravesarse con el avatar de un jugador.

Los colliders también son necesarios para hacer que una entidad sea clicable. Los eventos de botón se basan en la forma del collider de una entidad, no en su forma visible.

Hay capas de colisión separadas para interactuar ya sea con la física del jugador o con pointer events; los colliders se pueden configurar para interactuar solo con una u otra. También se pueden configurar para interactuar con capas personalizadas, que se pueden usar con [raycasts](/docs/creator/content-creator-es/scenes-sdk7/fundamentos-del-contenido-3d/colliders/) para manejar lo que tenga sentido para la escena.


> [!WARNING]
> **📔 Nota**: Los colliders no afectan cómo interactúan las demás entidades entre sí; las entidades siempre pueden superponerse. Los ajustes de colisión solo afectan cómo la entidad interactúa con el avatar del jugador y con los eventos de botón. Decentraland no tiene un motor de físicas nativo, así que si quieres que las entidades caigan, choquen o reboten, debes programar ese comportamiento en la escena, o importar una library para manejarlo.


## Usa el Scene Editor

La forma más sencilla de gestionar los colliders de una entidad es usar el [Scene Editor](/docs/creator/content-creator-es/scene-editor/primeros-pasos/about-editor/).

Puedes agregar un **Mesh Collider** a tu entidad para asignarle una forma primitiva (cubo, plano, esfera, cilindro o cono). Luego puedes elegir [Capas de colisión](#collision-layers) desde un desplegable.

También puedes configurar las capas de colisión en un **GLTF** component para cambiar el valor predeterminado de [Capas de colisión](#collision-layers) que se usa tanto en la geometría del collider como en la geometría visible del modelo. Consulta [Agregar componentes](/docs/creator/content-creator-es/scene-editor/construir/components/#add-components).

![](/docs/_remote/d21ae0005c271ae9c97a8144.webp)

## Colliders en formas primitivas

El `MeshCollider` el componente le da a una entidad un collider simple basado en una forma primitiva (cajas, esferas, planos, cilindros o conos).

Las entidades que tienen un `MeshRenderer` component para darles un [forma primitiva](/docs/creator/content-creator-es/scenes-sdk7/fundamentos-del-contenido-3d/shape-components/#primitive-shapes) no tienen colliders de forma predeterminada. También debes darle a la entidad un `MeshCollider` component.

Las siguientes formas de collider están disponibles en `MeshCollider`. Varias formas incluyen campos adicionales opcionales específicos para esa forma.

* **caja**:

  Usa `MeshCollider.setBox()`, pasando la entidad.
* **plano**:

  Usa `MeshCollider.setPlane()`, pasando la entidad.
* **esfera**:

  Usa `MeshCollider.setSphere()`, pasando la entidad.
* **cilindro**:

  Usa `MeshCollider.setCylinder()`, pasando la entidad. Pasa `radiusTop` y `radiusBottom` como campos adicionales opcionales para modificar el cilindro.


> [!NOTE]
> **💡 Consejo**: Establece cualquiera de `radiusTop` o `radiusBottom` en 0 para hacer un cono.


Este ejemplo define una entidad tipo caja que no se puede atravesar caminando.

```ts
// crear entidad
const myCollider = engine.addEntity()

// forma visible
MeshRenderer.setBox(myCollider)

// collider
MeshCollider.setBox(myCollider)
```

La forma usada por el `MeshCollider` no necesariamente tiene que coincidir con la usada por el `MeshRenderer`. También puedes agregar un `MeshCollider` a una entidad que tiene un modelo 3D de un `GltfContainer` component, o a una entidad que no tiene ninguna forma visible.


> [!WARNING]
> **📔 Nota**: La `MeshCollider` component y `ColliderLayer` deben importarse mediante
> 
> `import { MeshCollider, ColliderLayer } from "@dcl/sdk/ecs"`
> 
> Consulta [Imports](/docs/creator/content-creator-es/scenes-sdk7/primeros-pasos/coding-scenes/#imports) para ver cómo manejarlos fácilmente.


## Colliders en modelos 3D

A los modelos 3D se les pueden asignar colliders en dos niveles de geometría diferentes:

* `visibleMeshesCollisionMask`: Se refiere a la geometría visible del modelo. De forma predeterminada, esta geometría no tiene colliders.
* `invisibleMeshesCollisionMask`: se refiere a las meshes de collider, cuyos nombres terminan en `_collider`. De forma predeterminada, esta geometría se trata como un collider tanto para la física como para pointer events.

Cualquier mesh incrustada como parte de un modelo 3D cuyo nombre termina en `_collider` se trata como parte de la `invisibleMeshesCollisionMask` layer, y se interpreta como un collider de forma predeterminada.

Definir la geometría del collider como una capa invisible separada permite mucho más control y exige mucho menos al sistema que usar la geometría visible, ya que el objeto de colisión suele ser mucho más simple (con menos vértices) que el modelo original.

Si un modelo no tiene ninguna geometría de collider y quieres hacer que afecte a los sistemas de physics o pointer events, puedes hacer una de estas cosas:

* Asignar capas de colisión directamente a la geometría visible, mediante la `visibleMeshesCollisionMask`.


> [!WARNING]
> **📔 Nota**: Si la geometría visible del objeto tiene muchos vértices, ten en cuenta que esto puede tener un mayor costo de rendimiento.


* Darle a la entidad un `MeshCollider` component, para darle un collider de forma primitiva.
* Superponer una entidad invisible que tenga un `MeshCollider` component.
* Editar el modelo en una herramienta externa como Blender para incluir un *mesh de collider*. El collider debe llamarse *x\_collider*, donde *x* es el nombre del modelo. Así que para un modelo llamado *house*, el collider debe llamarse *house\_collider*.

También podrías querer asignar la capa de colisión de pointer events al `visibleMeshesCollisionMask` en caso de que quieras que las pistas al pasar el cursor y los pointer events respondan con más precisión al contorno de la entidad. Ten en cuenta que esto exige más rendimiento.


> [!WARNING]
> **📔 Nota**: Asegúrate de no tener asignada la misma capa (physics, pointer events o custom layers) a ambos `visibleMeshesCollisionMask` y `invisibleMeshesCollisionMask`, ya que eso sería un uso de recursos muy ineficiente. Puedes tener capas diferentes en cada uno, como physics en la capa invisible y pointer events en la capa visible.


```ts
// crear entidad
const myEntity = engine.addEntity()

// asignar forma GLTF
GltfContainer.create(myEntity, {
	src: '/models/myModel.gltf',
	invisibleMeshesCollisionMask: ColliderLayer.CL_PHYSICS,
	visibleMeshesCollisionMask: ColliderLayer.CL_POINTER,
})
```

Consulta [Modelos 3D](https://github.com/decentraland/docs/blob/main/creator/3d-modeling/3d-models.md) para más detalles sobre cómo agregar geometría invisible de collider a un modelo 3D.


> [!WARNING]
> **📔 Nota**: La `GltfContainer` component y `ColliderLayer` deben importarse mediante
> 
> > `import { GltfContainer, ColliderLayer } from "@dcl/sdk/ecs"`
> 
> Consulta [Imports](/docs/creator/content-creator-es/scenes-sdk7/primeros-pasos/coding-scenes/#imports) para ver cómo manejarlos fácilmente.


### Modelos animados

Al configurar colliders para usar la geometría visible en un modelo que incluye [animaciones basadas en armature](/docs/creator/content-creator-es/modelado-y-animaciones-3d/animations/), las animaciones no son seguidas por los colliders. Las meshes de collider conservan su forma original. Si una animación implica deformar la geometría de una mesh, las meshes de collider conservan la forma sin animar mientras la animación se reproduce.

Al reproducir animaciones que implican mover meshes completas sin cambiar su forma, estos cambios se reflejan con precisión en los colliders. Por ejemplo, si una plataforma se mueve como parte de una animación, el collider de la plataforma sí se mueve con la animación.

## Capas de colisión

La escena puede manejar capas de colisión separadas que tienen comportamientos distintos.

Puedes configurar un `MeshCollider` component o el `GltfContainer` component para que solo responda a un tipo de interacción, a varias o a ninguna. Para hacerlo, en el `MeshCollider` configura el `collisionMask` propiedad, y en `GltfContainer` configura el `visibleMeshesCollisionMask` o `invisibleMeshesCollisionMask` propiedades con uno o varios de los siguientes valores:

* `ColliderLayer.CL_PHYSICS`: Bloquea el movimiento del jugador (paredes, suelos, plataformas de la escena). No afecta a pointer events.
* `ColliderLayer.CL_POINTER`: Solo responde a pointer events. No bloquea el movimiento del jugador.
* `ColliderLayer.CL_PLAYER`: Marca el collider como un marcador de avatar. Los raycasts y las áreas de trigger que apuntan a `CL_PLAYER` lo detectarán, pero la cápsula del jugador lo atravesará directamente (sin bloqueo físico). En meshes de escena, esto es útil para marcar una mesh como "un objetivo similar a un avatar" solo para detección.
* `ColliderLayer.CL_MAIN_PLAYER`: Igual que `CL_PLAYER`, pero dirigido solo al jugador local. Los raycasts y las áreas de trigger con `CL_MAIN_PLAYER` en su mask lo detectan; los raycasts/triggers de avatares remotos no.
* `ColliderLayer.CL_CUSTOM1` hasta `CL_CUSTOM8`: Se puede usar junto con raycasts y áreas de trigger para detectar colisiones solo con capas personalizadas específicas.
* `ColliderLayer.CL_NONE`: No responde a colisiones de ningún tipo.


> [!NOTE]
> **💡 Consejo**: `CL_PLAYER` y `CL_MAIN_PLAYER` en un `MeshCollider` / `GltfContainer` son **solo de detección** capas — la cápsula del jugador las atraviesa. Si quieres que la mesh sea TANTO detectable como un avatar COMO que bloquee físicamente al jugador, combina la capa de avatar con `CL_PHYSICS` (p. ej. `CL_PHYSICS | CL_MAIN_PLAYER`).



> [!WARNING]
> **📔 Nota**: Para desactivar las colisiones de un `MeshCollider` component, elimina el componente. No establezcas la capa de colisión en `ColliderLayer.CL_NONE`. Hay un problema conocido con el `MeshCollider` component. En lugar de desactivar todas las colisiones, hace que este valor sea equivalente al valor predeterminado (`ColliderLayer.CL_PHYSICS | ColliderLayer.CL_POINTER`).


```ts
// crear entidad
const myEntity = engine.addEntity()
// forma visible
MeshRenderer.setBox(myEntity)

// crear un componente MeshCollider que solo responda a la física del jugador
MeshCollider.setBox(myEntity, ColliderLayer.CL_PHYSICS)
```

Una sola mask de colisión puede responder a varias capas de colisión. Usa el `|` carácter como un *o*, para incluir tantas capas como necesites. El valor predeterminado en un MeshCollider es `ColliderLayer.CL_PHYSICS | ColliderLayer.CL_POINTER`.

```ts
MeshCollider.setBox(
	myEntity,
	ColliderLayer.CL_CUSTOM1 |
		ColliderLayer.CL_CUSTOM3 |
		ColliderLayer.CL_PHYSICS |
		ColliderLayer.CL_POINTER
)
```

Puedes usar las 8 capas personalizadas diferentes para lo que mejor se adapte a tu escena; por ejemplo, una podría usarse para cálculos de línea de visión de NPC, mientras que otra para estimar trayectorias de objetos que caen. Usar distintas capas para distintos sistemas te permite usar menos recursos, ya que en cada caso solo estarás comprobando colisiones con las entidades relevantes.

Consulta [Raycasting](/docs/creator/content-creator-es/scenes-sdk7/interactividad/raycasting/) para obtener más información sobre cómo usar capas de colisión personalizadas.

### Cámaras y colliders

Cuando la cámara de un jugador se mueve en modo en tercera persona, la cámara puede quedar bloqueada por colliders o no, dependiendo de las capas de colisión asignadas a las entidades. Ten esto en cuenta al diseñar tu escena; quizá quieras evitar que la cámara atraviese paredes u otras entidades.

Para evitar que la cámara atraviese paredes, debes asignar tanto la `ColliderLayer.CL_PHYSICS` y la `ColliderLayer.CL_POINTER` capas a las entidades que quieres que bloqueen la cámara. Es importante que ambas capas se asignen a la misma geometría de la entidad. Así que, si asignas la `ColliderLayer.CL_PHYSICS` capa a la capa visible de la entidad, también debes asignar la `ColliderLayer.CL_POINTER` capa a la misma geometría.

Por ejemplo, en Creator Hub, la siguiente combinación de ajustes evitará que la cámara atraviese las paredes:

![](/docs/_remote/1b4a254a3e56b4b32b988fb4.webp)

Tanto la `ColliderLayer.CL_PHYSICS` y la `ColliderLayer.CL_POINTER` capas se asignan a la misma capa invisible de la geometría de la entidad. Si ambas se asignaran a la capa visible, el resultado sería el mismo. Este es el comportamiento predeterminado, tanto al agregar una entidad mediante Creator Hub como mediante código.

![](/docs/_remote/6d4572b10a1972901a7ea232.webp)

En este segundo ejemplo, la cámara puede atravesar la pared, porque la `ColliderLayer.CL_PHYSICS` capa se asigna a la capa invisible de la entidad, y la `ColliderLayer.CL_POINTER` capa se asigna a la capa visible de la entidad, incluso si ambas geometrías tienen la misma forma general.

```ts
// NO HAY CÁMARA ATRAVESANDO LA PARED
// predeterminado (tanto pointer como physics usan la geometría invisible)
GltfContainer.create(myEntity, {
	src: '/models/myModel.gltf',
})

// NO HAY CÁMARA ATRAVESANDO LA PARED
// Ambos usan la misma geometría invisible
GltfContainer.create(myEntity2, {
	src: '/models/myModel.gltf',
	invisibleMeshesCollisionMask:
		ColliderLayer.CL_PHYSICS | ColliderLayer.CL_POINTER,
})

// NO HAY CÁMARA ATRAVESANDO LA PARED
// Ambos usan la misma geometría visible
GltfContainer.create(myEntity2, {
	src: '/models/myModel.gltf',
	visibleMeshesCollisionMask:
		ColliderLayer.CL_PHYSICS | ColliderLayer.CL_POINTER,
})

// SÍ, LA CÁMARA ATRAVIESA LA PARED
// physics y pointer están en capas diferentes
GltfContainer.create(myEntity2, {
	src: '/models/myModel.gltf',
	invisibleMeshesCollisionMask: ColliderLayer.CL_PHYSICS,
	visibleMeshesCollisionMask: ColliderLayer.CL_POINTER,
})

// SÍ, LA CÁMARA ATRAVIESA LA PARED
// physics y pointer están en capas diferentes
GltfContainer.create(myEntity2, {
	src: '/models/myModel.gltf',
	invisibleMeshesCollisionMask: ColliderLayer.CL_POINTER,
	visibleMeshesCollisionMask: ColliderLayer.CL_PHYSICS,
})
```

### Bloqueo del puntero

Solo las formas que tienen colliders se pueden activar con [pointer events](/docs/creator/content-creator-es/scenes-sdk7/interactividad/eventos-de-button/click-events/). Una entidad también necesita tener un collider para bloquear el paso de pointer events a través de ella y evitar impactar entidades detrás de ella. Por ejemplo, un jugador no puede recoger algo que está encerrado dentro de un cofre, si el cofre tiene colliders alrededor. Los pointer events del jugador solo se ven afectados por meshes que están activas en la `ColliderLayer.CL_POINTER` layer.

De forma predeterminada, un MeshCollider afecta tanto a las capas Physics como Pointer, pero puedes cambiar este valor para que solo afecte a una, a ninguna, o para que afecte a capas personalizadas en su lugar.


> [!WARNING]
> **📔 Nota**: Además de colliders, una entidad también necesita tener un `PointerEvents` component para responder a pointer events. Los `pointerEventsSystem` helpers también se encargan de este requisito.


```ts
// solo responde a la física del jugador
// por ejemplo, para una pared invisible que no puedes atravesar caminando pero sí puedes hacer clic a través de ella
MeshCollider.setBox(myEntity, ColliderLayer.CL_PHYSICS)

// solo responde al puntero del jugador
// por ejemplo, para un objeto que puedes hacer clic para recoger, pero que puedes atravesar caminando
MeshCollider.setBox(myEntity2, ColliderLayer.CL_POINTER)
```

De forma predeterminada, la geometría visible de un `GltfContainer` no se asigna a ninguna capa de colisión, pero la geometría invisible afecta tanto a las capas Physics como Pointer. Puedes cambiar este valor para que solo afecte a una, a ninguna, o para que afecte a capas personalizadas en su lugar. También puedes configurar la capa de geometría visible de la misma manera.

```ts
// predeterminado (tanto pointer como physics usan la geometría invisible)
GltfContainer.create(myEntity, {
	src: '/models/myModel.gltf',
})

// la física del jugador usa la geometría invisible más simple
// pointer events usa el contorno completo y detallado de la geometría visible
GltfContainer.create(myEntity2, {
	src: '/models/myModel.gltf',
	invisibleMeshesCollisionMask: ColliderLayer.CL_PHYSICS,
	visibleMeshesCollisionMask: ColliderLayer.CL_POINTER,
})

// tanto la física del jugador como pointer events usan el contorno completo y detallado de la geometría visible
// la geometría invisible más simple se asigna a ColliderLayer.CL_NONE para evitar calcular ambas
GltfContainer.create(myEntity, {
	src: '/models/myModel.gltf',
	invisibleMeshesCollisionMask: ColliderLayer.CL_NONE,
	visibleMeshesCollisionMask:
		ColliderLayer.CL_POINTER | ColliderLayer.CL_PHYSICS,
})

// no responde a colisiones de ningún tipo, ni con la geometría visible ni con la invisible:
GltfContainer.create(myEntity, {
	src: '/models/myModel.gltf',
	invisibleMeshesCollisionMask: ColliderLayer.CL_NONE,
})
```

## Sintaxis avanzada de MeshCollider

La sintaxis completa para crear un `MeshCollider` component, sin ningún helper que la simplifique, se ve así:

```ts
MeshCollider.create(myBox, {
	mesh: {
		$case: 'box',
		box: {},
	},
})

MeshCollider.create(myPlane, {
	mesh: {
		$case: 'plane',
		plane: {},
	},
})

MeshCollider.create(myShpere, {
	mesh: {
		$case: 'sphere',
		sphere: {},
	},
})

MeshCollider.create(myCylinder, {
	mesh: {
		$case: 'cylinder',
		cylinder: {},
	},
})
```

Así es como el protocolo base interpreta los componentes MeshCollider. Las funciones helper abstraen esto y exponen una sintaxis más amigable, pero internamente generan esta sintaxis.

El `$case` campo te permite especificar uno de los tipos permitidos. Cada tipo admite un conjunto diferente de parámetros.

Los valores compatibles para `$case` son los siguientes:

* `caja`
* `plano`
* `esfera`
* `cilindro`

Según el valor de `$case`, es válido definir el objeto para la forma correspondiente, pasando cualquier propiedad relevante.


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-es/scenes-sdk7/fundamentos-del-contenido-3d/colliders.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
