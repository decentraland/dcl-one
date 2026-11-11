> Cómo establecer la posición, rotación y escala de una entidad en una escena

# Posicionamiento de entidades

Puedes establecer la *posición*, *rotación* y *escala* de cualquier entidad usando el `Transform` componente. Esto se puede usar en cualquier entidad en el espacio 3D, afectando dónde se renderiza la entidad. Esto incluye formas primitivas (cubo, esfera, plano, etc.), formas de texto 3D, formas NFT y modelos 3D (`GltfContainer`).

## Usa el Scene Editor en Creator Hub

Al agregar un elemento a tu escena mediante el Scene Editor, incluye implícitamente un **Transform** componente. Luego cambias los valores del componente Transform de la entidad de forma implícita al cambiar la posición, la rotación o la escala de una entidad. También puedes usar la UI del Scene Editor para proporcionar valores numéricos con mayor precisión.

## Conceptos básicos del código

![](/docs/_remote/d0b3835ca11dd6cc73d8d261.webp)

```ts
// Crear una nueva entidad
const ball = engine.addEntity()

// Dale a esta entidad una forma para hacerla visible
MeshRenderer.setSphere(ball)

// Dale a esta entidad un componente Transform
Transform.create(ball, {
	position: Vector3.create(5, 1, 5),
	scale: Vector3.create(1, 1, 1),
	rotation: Quaternion.Identity(),
})
```

Para mover, rotar o redimensionar una entidad en tu escena a lo largo del tiempo, cambia los valores de este componente de forma incremental, fotograma a fotograma. Consulta [Mover entidades](/docs/creator/content-creator-es/scenes-sdk7/fundamentos-del-contenido-3d/move-entities/) para más detalles y buenas prácticas.

Cualquier campo que dejes fuera toma su valor predeterminado: position `0, 0, 0`, sin rotación, y una escala de `1, 1, 1`. Deja un campo fuera en lugar de establecerlo en `undefined`undefined `undefined`. Un campo que esté explícitamente `Transform.create()`.

```ts
// ❌ Si savedRotation es undefined, la escena lanza un error más tarde
Transform.create(ball, { position: Vector3.create(5, 1, 5), rotation: savedRotation })

// ✅ Usa un valor real como respaldo
Transform.create(ball, { position: Vector3.create(5, 1, 5), rotation: savedRotation ?? Quaternion.Identity() })
```


> [!WARNING]
> **📔 Nota**: `Vector3` y `Quaternion` debe importarse mediante
> 
> > `import { Vector3, Quaternion } from "@dcl/sdk/math"`
> 
> Ver [Importaciones](/docs/creator/content-creator-es/scenes-sdk7/primeros-pasos/coding-scenes/#imports) para ver cómo manejarlos fácilmente.


## Posición

`posición` es un *vector 3D*, establece la posición del centro de la entidad en los tres ejes, *x*, *y*y *z*. Consulta [Tipos de geometría](/docs/creator/content-creator-es/scenes-sdk7/fundamentos-del-contenido-3d/special-types/) para más detalles.

```ts
// Crear una nueva entidad
const ball = engine.addEntity()

// Crear un transform con una posición predefinida
Transform.create(ball, {
	  position: Vector3.create(5, 1, 5)
})

// Obtén una versión mutable del transform
const mutableTransform = Transform.getMutable(ball)

// Establece la posición con un objeto
mutableTransform.position = { x: 5, y: 1, z: 5 }

// Establece la posición con un objeto (sintaxis alternativa)
mutableTransform.position = Vector3.create(2, 1, 4)

// Establece cada eje de forma individual
mutableTransform.position.x = 3
mutableTransform.position.y = 1
mutableTransform.position.z = 3
```

Cuando establezcas una posición, ten en cuenta las siguientes consideraciones:

* Los números en un vector de posición representan *metros* (a menos que la entidad sea hija de una entidad escalada).
* Una escena compuesta por una sola parcela mide 16 m x 16 m. El centro de la escena (a nivel del suelo) está en `x:8, y:0, z:8`. Si la escena está compuesta por varias parcelas, entonces el centro variará según su distribución.
* `x:0, y:0, z:0` se refiere a la *suroeste* esquina de la parcela base de la escena, a nivel del suelo.

  > Consejo: Al ver una vista previa de la escena, aparece una brújula en el punto (0,0,0) de la escena con etiquetas para cada eje como referencia.

  > Nota: Puedes cambiar la parcela base de una escena editando el `base` atributo de *scene.json*.
* Para orientarte mejor, usa tu *izquierda* mano:
  * tu dedo índice (apuntando hacia delante) es el *z* eje
  * tu dedo medio (apuntando hacia un lado) es el *x* eje
  * tu pulgar (apuntando hacia arriba) es el *y* eje.
* Si una entidad es hija de otra, entonces `x:0, y:0, z:0` se refiere al centro de su entidad padre, esté donde esté en la escena.
* Cada entidad de tu escena debe estar posicionada dentro de los límites de las parcelas que ocupa en todo momento. Si una entidad sale de estos límites, se producirá un error.

  > Consejo: Al ver una escena en modo preview, las entidades que están fuera de los límites se resaltan en *rojo*.
* Tu escena también está limitada en altura. La altura máxima es de 330 metros para cada escena, independientemente de cuántas parcelas ocupe. Las entidades colocadas por encima de 200 metros pueden sufrir problemas de sincronización en multiplayer. Consulta [limitaciones de la escena](/docs/creator/content-creator-es/scenes-sdk7/optimizacion/scene-limitations/) para más detalles.

## Rotación

`rotación` se almacena como una [*cuaternión*](https://en.wikipedia.org/wiki/Quaternion), un sistema de cuatro números, *x*, *y*, *z* y *w*. Cada uno de estos números va de -1 a 1. Consulta [Tipos de geometría](/docs/creator/content-creator-es/scenes-sdk7/fundamentos-del-contenido-3d/special-types/) para más detalles.

```ts
// Crear una nueva entidad
const cube = engine.addEntity()

// Crear un transform con una rotación predefinida de 0
Transform.create(cube, {
	  rotation: Quaternion.Identity()
})

// Obtén una versión mutable del transform
const mutableTransform = Transform.getMutable(cube)

// Establece la rotación con un objeto, a partir de ángulos de Euler
mutableTransform.rotation = Quaternion.fromEulerDegrees(0, 90, 0)

// Establece la rotación con un objeto
mutableTransform.rotation = { x: 0.1, y: 0.5, z: 0.5, w: 0 }

// Establece cada eje de forma individual
mutableTransform.rotation.x = 0
mutableTransform.rotation.y = 1
mutableTransform.rotation.z = 0.3
mutableTransform.rotation.w = 0
```

También puedes establecer el campo rotation con [*Euler* ángulos](https://en.wikipedia.org/wiki/Euler_angles), la notación más común con números que van de 0 a 360 con la que la mayoría de las personas está familiarizada. Para usar ángulos de Euler, usa una de las siguientes notaciones: *x*, *y* y *z* // Crear un transform con una rotación predefinida en ángulos de Euler

```ts
rotation: Quaternion.fromEulerDegrees(0, 90, 0)
Transform.create(cube, {
	  Cuando uses un
})

// Obtén una versión mutable del transform
const mutableTransform = Transform.getMutable(cube)

// Establece la rotación con un objeto, a partir de ángulos de Euler
mutableTransform.rotation = Quaternion.fromEulerDegrees(0, 90, 0)
```

para representar ángulos de Euler, *vector 3D* representan la rotación en ese eje, medida en grados. Una vuelta completa requiere 360 grados. *x*, *y* y *z* Cuando recuperas la rotación de una entidad, devuelve un cuaternión de forma predeterminada. Para obtener la rotación expresada en ángulos de Euler, usa

Cuando recuperas la rotación de una entidad, devuelve un cuaternión de forma predeterminada. Para obtener la rotación expresada en ángulos de Euler, usa `Quaternion.toEulerAngles()`:

```ts
// Obtén una versión de solo lectura del transform
const transform = Transform.get(cube)

// Obtén la rotación expresada en ángulos de Euler
const eulerAngle = Quaternion.toEulerAngles(transform.rotation)
```

## Obtener la posición global y la rotación global de una entidad

El `getWorldPosition` y `getWorldRotation` Las funciones devuelven la posición global y la rotación global de una entidad. Eso significa que devuelve la posición o rotación percibida en la que el jugador verá el elemento, ignorando cualquier jerarquía padre.

* `getWorldPosition(engine, entity: Entity): Vector3Type`: Esta función devuelve la posición global de una entidad, teniendo en consideración todas las posiciones de las entidades padre, si la propia entidad tiene alguna, devolviendo `{x: 0, y: 0, z: 0}` si la entidad no tiene Transform.

```ts
const worldPos = getWorldPosition(engine, childEntity)
console.log(`World position: ${worldPos.x}, ${worldPos.y}, ${worldPos.z}`)
```

* `getWorldRotation(engine, entity: Entity): QuaternionType`: Esta función devuelve la rotación global de una entidad, teniendo en consideración todas las rotaciones de las entidades padre, si la propia entidad tiene alguna. Devuelve un `Quaternion` tipo, devolviendo el cuaternión identidad `{x: 0, y: 0, z: 0, w: 1}` si la entidad no tiene Transform.

```ts
const worldRot = getWorldRotation(engine, childEntity)
console.log(`World rotation: ${worldRot.x}, ${worldRot.y}, ${worldRot.z}, ${worldRot.w}`)
```


> [!NOTE]
> **Nota:** La posición global y la rotación global son relativas a las coordenadas dentro de la escena y no a Genesis City.


## Mirar al jugador

Añade un *Billboard* componente a una entidad para que siempre rote para mirar a la cámara del jugador.

Los billboards eran una técnica común usada en los juegos 3D de los 90, donde la mayoría de las entidades eran planos 2D que siempre miraban al jugador. La misma idea también se puede usar para rotar un modelo 3D.


> [!WARNING]
> **📔 Nota**: Un billboard mira a la **cámara**, no al avatar. En tercera persona la cámara está detrás y encima del avatar, así que ambos están en lugares diferentes, y un billboard orientado hacia la cámara claramente queda girado lejos del avatar. Para que mire al avatar en su lugar, establece `targetEntity` para `engine.PlayerEntity`.


```ts
// Crear una nueva entidad
const cube = engine.addEntity()

// Dale a la entidad una forma visible
MeshRenderer.setBox(cube)

// Crear un transform con una posición predefinida
Transform.create(cube, {
	  position: Vector3.create(5, 1, 5)
})

// Dale a la entidad un componente Billboard
Billboard.create(cube, {})
```

Puedes configurar cómo se comporta el billboard con los siguientes parámetros:

* `billboardMode`: Usa un valor de `BillboardMode` para definir qué ejes de rotación giran automáticamente para mirar al jugador:
  * `BillboardMode.BM_ALL`: La entidad gira para mirar al jugador en todos sus ejes de rotación. Si el jugador está muy por encima de la entidad, la entidad mirará hacia arriba.
  * `BillboardMode.BM_NONE`: La entidad no girará en absoluto.
  * `BillboardMode.BM_X`: La entidad solo gira en su *x* eje de rotación.
  * `BillboardMode.BM_Y`: La entidad solo gira en su *y* : La entidad solo gira en su eje de rotación. Solo gira a izquierda y derecha, no hacia arriba y hacia abajo. Permanece perpendicular al suelo si el jugador está por encima o por debajo de la entidad.
  * `BillboardMode.BM_Z`: La entidad solo gira en su *z* eje de rotación.
* `targetEntity`: (opcional) Una entidad hacia la que gira el billboard, en lugar de la cámara del jugador. Si la entidad referenciada no existe (por ejemplo, aún no se ha creado o se eliminó), el billboard deja de girar y mantiene su última orientación hasta que la entidad vuelva a existir. Establecer esto en `engine.CameraEntity` es equivalente a dejarlo sin definir.

```ts
// billboard plano
const perpendicularPlane = engine.addEntity()

Transform.create(perpendicularPlane, {
	position: Vector3.create(8, 1, 8),
})

MeshRenderer.setPlane(perpendicularPlane)

Billboard.create(perpendicularPlane, {
	billboardMode: BillboardMode.BM_Y,
})

// etiqueta de texto
const textLabel = engine.addEntity()

Transform.create(textLabel, {
	position: Vector3.create(6, 1, 6),
})

TextShape.create(textLabel, {
	text: 'Este texto siempre se puede leer',
})

Billboard.create(textLabel)

// letrero que mira a otra entidad en lugar del jugador
const sphere = engine.addEntity()

Transform.create(sphere, {
	position: Vector3.create(4, 2, 4),
})

MeshRenderer.setSphere(sphere)

const sign = engine.addEntity()

Transform.create(sign, {
	position: Vector3.create(8, 1, 8),
})

TextShape.create(sign, {
	text: 'Observando la esfera',
})

Billboard.create(sign, {
	targetEntity: sphere,
})
```


> [!NOTE]
> **💡 Consejo**: Los billboards son muy prácticos para añadir a *texto* entidades, ya que las hace siempre legibles.


Establece `targetEntity` para `engine.PlayerEntity` para que siga al avatar en lugar de a la cámara. Esto es lo que quieres para una etiqueta con nombre, un globo de diálogo o cualquier otra cosa que pertenezca al avatar en lugar de a la vista del jugador:

```ts
// Una etiqueta que gira para mirar al avatar, no a la cámara
Billboard.create(label, {
	targetEntity: engine.PlayerEntity,
	billboardMode: BillboardMode.BM_Y,
})
```

La posición del avatar está en sus pies, así que una etiqueta sobre su cabeza se inclinaría hacia ellos. `BM_Y` mantiene la etiqueta erguida y solo la gira a izquierda y derecha.

### Hacia qué dirección mira un billboard

Un billboard apunta su eje local **-Z** eje hacia aquello que esté mirando. Eso importa cuando haces que otras entidades sean hijas de un billboard:

* Una entidad hija con un desplazamiento local **+Z** se sitúa **detrás** del billboard, oculta por él.
* Una entidad hija con un desplazamiento local **-Z** se sitúa **delante**, más cerca del espectador.

Así que, para colocar un marco o un fondo detrás de una etiqueta con billboard, desplázalo en **+Z**. Para colocar un resaltado delante de ella, desplázalo en **-Z**.

El `rotación` el valor del `Transform` componente de la entidad no cambia mientras el billboard sigue a los jugadores.

Si una entidad tiene tanto un `Billboard` componente como `Transform` componente con `rotación` valores, los jugadores verán la entidad girar como un billboard. Si el billboard no afecta a todos los ejes, el eje restante se rotará según el `Transform` componente.


> [!WARNING]
> **📔 Nota**: Si hay varios jugadores presentes al mismo tiempo, cada uno verá las entidades con billboard mode mirándolos a ellos. Las rotaciones de los billboards se calculan localmente para cada jugador y no afectan lo que ven los demás. Esto no se aplica a billboards con un `targetEntity`: ya que la posición del objetivo forma parte de la escena, todos los jugadores ven esos billboards orientados de la misma manera.


## Orientar hacia un conjunto de coordenadas

Para que la entidad A mire a la entidad B:

```
1) Resta la posición de la entidad A de la entidad B para obtener un vector que describa la distancia entre ellas.
2) Normaliza ese vector para que tenga una longitud de 1, manteniendo su dirección.
3) Usa `Quaternion.lookRotation` para obtener una rotación Quaternion que describa rotarse en esa dirección.
4) Establece ese Quaternion como la rotación de la entidad A
```

```ts
export function turn(entity: Entity, target: Vector3.ReadonlyVector3) {
	const transform = Transform.getMutable(entity)
	const difference = Vector3.subtract(target, transform.position)
	const normalizedDifference = Vector3.normalize(difference)
	transform.rotation = Quaternion.lookRotation(normalizedDifference)
}
```

## Escala

`escala` también es una *vector 3D*, almacenada como un `Vector3` objeto, incluido el factor de escala en el *x*, *y* y *z* eje. La forma de la entidad se escala en consecuencia, ya sea una primitiva o un modelo 3D.

La escala predeterminada es 1, así que asigna un valor mayor que 1 para estirar una entidad o menor que 1 para reducirla.

```ts
// Crear una nueva entidad
const ball = engine.addEntity()

// Crear un transform con una posición predefinida
Transform.create(ball, {
	  scale: Vector3.create(5, 5, 5)
})

// Obtén una versión mutable del transform
const mutableTransform = Transform.getMutable(ball)

// Establece la escala con un Vector3

mutableTransform.scale = Vector3.create(2, 2, 2)

// Establece la posición con un objeto
mutableTransform.scale = { x: 5, y: 1, z: 5 }

// Establece cada eje de forma individual
mutableTransform.scale.x = 3
mutableTransform.scale.y = 3
mutableTransform.scale.z = 2
```

## Heredar transformaciones del padre

Cuando una entidad está anidada dentro de otra, las entidades hijas heredan componentes de los padres. Esto significa que si una entidad padre se posiciona, escala o rota, sus hijas también se ven afectadas. Los valores de posición, rotación y escala de las entidades hijas no anulan los de los padres; en cambio, se combinan.

Asignas una entidad como padre de otra estableciendo el `parent` campo en el `Transform` componente.

Si una entidad padre se escala, todos los valores de posición de sus hijas también se escalan.

```ts
// Crear entidades
const parentEntity = engine.addEntity()
const childEntity = engine.addEntity()

// Crear un Transform para el padre
Transform.create(parentEntity, {
	position: Vector3.create(3, 1, 1),
	scale: Vector3.create(0.5, 0.5, 0.5),
})

// Crear un Transform para la hija, y asignarla como hija
Transform.create(childEntity, {
	position: Vector3.create(0, 1, 0),
	parent: parentEntity,
})
```

En este ejemplo, la entidad hija se escalará hasta 0.5, ya que su padre tiene esa escala. La posición de la entidad hija también será relativa a su padre. Tenemos que sumar la posición del padre más la de la hija. En este caso, como el padre está escalado a la mitad de su tamaño, la transformación de la hija también se escala proporcionalmente. En términos absolutos, la hija se posiciona en `{ x: 3, y: 1.5, z: 1 }`. Si el padre tuviera un `rotación`, esto también afectaría la posición final de la hija, ya que cambia el eje en el que se desplaza la hija.

Si una entidad hija no tiene `posición` en su Transform, el valor predeterminado es `0,0,0`, lo que la dejará posicionada en la misma posición que su padre.

Puedes usar una entidad invisible sin componente de forma como padre para agrupar un conjunto de otras entidades. Esta entidad no será visible en la escena renderizada, pero puede usarse para agrupar a sus hijas y aplicarles un transform a todas.


> [!WARNING]
> **📔 Nota**Evita crear un bucle en la jerarquía, en el que una entidad termina siendo su propio ancestro. El SDK detecta estos bucles y registra una advertencia nombrando las entidades involucradas. Las entidades afectadas no se posicionan correctamente hasta que corrijas la relación padre-hija.


## Adjuntar una entidad a un avatar

Hay tres métodos para adjuntar una entidad al jugador:

* Hazla hija de la **Entidad del avatar**
* Hazla hija de la **Entidad de la cámara**
* Usa la **componente AvatarAttach**

La forma más sencilla de adjuntar una entidad al avatar es establecer como padre la [entidad reservada](/docs/creator/content-creator-es/scenes-sdk7/arquitectura/entities-components/#reserved-entities) `engine.PlayerEntity`. La entidad se moverá entonces junto con la posición del jugador.

```ts
let childEntity = engine.addEntity()

MeshRenderer.setCylinder(childEntity)

Transform.create(childEntity, {
	scale: Vector3.create(0.2, 0.2, 0.2),
	position: Vector3.create(0, 0.4, 0),
	parent: engine.PlayerEntity,
})
```

También puedes establecer una entidad en la [entidad reservada](/docs/creator/content-creator-es/scenes-sdk7/arquitectura/entities-components/#reserved-entities) `engine.CameraEntity`. Al usar la entidad de la cámara en primera persona, la entidad adjunta seguirá los movimientos de la cámara. Esto es ideal para mantener algo siempre a la vista; por ejemplo, para mantener el modelo 3D del arma siempre visible, incluso cuando la cámara apunta hacia arriba.

```ts
let childEntity = engine.addEntity()

MeshRenderer.setCylinder(childEntity)

Transform.create(childEntity, {
	scale: Vector3.create(0.2, 0.2, 0.2),
	position: Vector3.create(0, 0.4, 0),
	parent: engine.CameraEntity,
})
```

Para adjuntar un objeto a uno de los huesos del avatar y hacer que se mueva junto con las animaciones del avatar, añade un `AvatarAttach` componente de la entidad.

Puedes elegir diferentes puntos de anclaje en el avatar; la mayoría de estos puntos están vinculados al armature del jugador y siguen las animaciones del jugador. Por ejemplo, cuando usas el punto de anclaje de la mano derecha, la entidad adjunta se moverá cuando el avatar salude o balancee los brazos mientras corre, igual que si el jugador estuviera sosteniendo la entidad en su mano.

```ts
// Adjuntar al jugador principal; si no se establece avatarId, se usa engine.PlayerEntity de forma predeterminada
AvatarAttach.create(myEntity, {
	anchorPointId: AvatarAnchorPointType.AAPT_NAME_TAG,
})

// Adjuntar a un jugador por ID
AvatarAttach.create(myEntity, {
	avatarId: '0xAAAAAAAAAAAAAAAAA',
	anchorPointId: AvatarAnchorPointType.AAPT_NAME_TAG,
})
```

Al crear un `AvatarAttach` componente, pasa un objeto con los siguientes datos:

* `avatarId`: *Opcional* El ID del jugador al que adjuntar. Es el mismo que la dirección de Ethereum del jugador, para aquellos jugadores conectados con una Wallet de Ethereum. Si no se especifica, adjunta la entidad al avatar del jugador local.
* `anchorPointId`: Qué punto de anclaje del esqueleto del avatar usar para adjuntar la entidad, usando un valor del enum `AvatarAnchorPointType`.


> [!WARNING]
> **📔 Nota**: Si quieres que todos los jugadores de la escena vean un objeto adjunto al mismo jugador, por ejemplo, para que todos vean que el jugador A recogió un objeto y lo lleva en su mano izquierda, entonces debes proporcionar un valor a `avatarId`. Si no se especifica, entonces todos los jugadores verán el objeto adjunto a sus propios avatares.


El siguiente ejemplo coloca una entidad adjunta a un avatar concreto, para que todos los demás jugadores la vean adjunta a ese mismo avatar.

```ts
import { getPlayer } from '@dcl/sdk/src/players'
import { AvatarAnchorPointType, AvatarAttach, engine, Entity } from '@dcl/sdk/ecs'
import { syncEntity } from '@dcl/sdk/network'

async function attachToPlayer(){

 let userData = await getPlayer()
 console.log(userData)

 if (!userData || !userData.wearables) return

  let entity = engine.addEntity()

  AvatarAttach.create(entity, {
    avatarId: userData.userId,
    anchorPointId: AvatarAnchorPointType.AAPT_RIGHT_HAND,
  })

  // Otros componentes

  syncEntity(entity, [AvatarAttach.componentId])

}
```

Los siguientes puntos de anclaje están disponibles en el `AvatarAnchorPointType` enum:

* `AAPT_RIGHT_HAND`: Fijo en la mano derecha del jugador
* `AAPT_LEFT_HAND`: Fijo en la mano izquierda del jugador
* `AAPT_HEAD`: Fijo en el centro de la cabeza del jugador.
* `AAPT_NECK`: Fijo en la base del cuello del jugador.
* `AAPT_SPINE`: Fijo en la sección superior de la columna vertebral.
* `AAPT_SPINE1`: Fijo en la sección media de la columna vertebral.
* `AAPT_SPINE2`: Fijo en la sección inferior de la columna vertebral.
* `AAPT_HIP`: Fijo al hueso de la cadera.
* `AAPT_LEFT_SHOULDER`: Fijo al hombro izquierdo.
* `AAPT_LEFT_ARM`: Fijo al primer hueso del brazo izquierdo, a la altura del hombro.
* `AAPT_LEFT_FOREARM`: Fijo al hueso del antebrazo izquierdo.
* `AAPT_LEFT_HAND_INDEX`: Fijo en la punta del dedo índice izquierdo.
* `AAPT_RIGHT_SHOULDER`: Fijo al hombro derecho.
* `AAPT_RIGHT_ARM`: Fijo al primer hueso del brazo derecho, a la altura del hombro.
* `AAPT_RIGHT_FOREARM`: Fijo al hueso del antebrazo derecho.
* `AAPT_RIGHT_HAND_INDEX`: Fijo en la punta del dedo índice derecho.
* `AAPT_LEFT_UP_LEG`: Fijo al hueso superior de la pierna izquierda.
* `AAPT_LEFT_LEG`: Fijo al hueso inferior de la pierna izquierda.
* `AAPT_LEFT_FOOT`: Fijo al tobillo de la pierna izquierda.
* `AAPT_LEFT_TOE_BASE`: Fijo en la punta del dedo del pie izquierdo.
* `AAPT_RIGHT_UP_LEG`: Fijo al hueso superior de la pierna derecha.
* `AAPT_RIGHT_LEG`: Fijo al hueso inferior de la pierna derecha.
* `AAPT_RIGHT_FOOT`: Fijo al tobillo de la pierna derecha.
* `AAPT_RIGHT_TOE_BASE`: Fijo en la punta del dedo del pie derecho.
* `.AAPT_NAME_TAG`: Flota justo encima de la etiqueta del nombre del jugador y no se ve afectado por las animaciones del jugador.

  > Nota: La altura de la etiqueta del nombre se ajusta dinámicamente según la altura de los wearables que lleve puestos el jugador. Así, un jugador que lleve un sombrero alto tendrá su etiqueta del nombre un poco más alta que la de otros.
* `AAPT_POSITION` *OBSOLETO*: La posición general del jugador. Esto aparece a una altura de 0.8 por encima de los pies del jugador.

  >


> [!WARNING]
> \> \*\*📔 Nota\*\*: \`AAPT\_POSITION\` está obsoleto. Para seguir la posición general del jugador, lo mejor es hacer que la entidad sea hija de la Avatar Entity. Consulta el inicio de esta sección para ver un ejemplo. >



> [!NOTE]
> **💡 Consejo**: Para usar estos valores, escribe `AvatarAnchorPointType.` y VS Code mostrará la lista completa de opciones en un desplegable.


![](/docs/_remote/a3894bbddd0ce5df1254538a.webp)

La renderización de las entidades se determina localmente en cada instancia de la escena. Adjuntar una entidad a un jugador no la hace visible para otros jugadores que estén viendo a ese jugador. Si una entidad está adjunta al jugador local predeterminado, cada jugador verá la entidad como adjunta a su propio avatar.


> [!WARNING]
> **📔 Nota**: Las entidades adjuntas a un avatar deben permanecer dentro de los límites de la escena para que se rendericen. Si un jugador sale de tu escena, cualquier entidad adjunta deja de renderizarse hasta que el jugador vuelva a entrar. Los Smart wearables no tienen esta limitación.


El `AvatarAttach` componente sobrescribe los valores en el `Transform` componente. Cualquier valor que pongas en el `Transform` componente se reemplaza por la posición relativa de la entidad con respecto a la del Transform del jugador; estos valores se actualizan fotograma a fotograma a medida que el jugador se mueve y anima.

Si necesitas posicionar una entidad con un desplazamiento desde el punto de anclaje en el avatar, o con una rotación o escala diferente, hazlo mediante una entidad padre.

1. Crea una entidad invisible con solo un `Transform` y un `AvatarAttach` componente. Sus `Transform` valores se sobrescribirán a medida que el jugador se mueve
2. Establece la entidad que quieres adjuntar como hija de este padre. Sus `Transform` valores pueden describir el desplazamiento desde el punto de anclaje.

```ts
// Crear entidad padre
const parentEntity = engine.addEntity()

// Adjuntar la entidad padre al jugador
AvatarAttach.create(parentEntity, {
	anchorPointId: AvatarAnchorPointType.AAPT_NAME_TAG,
})

// Crear entidad hija
let childEntity = engine.addEntity()

MeshRenderer.setCylinder(childEntity)

Transform.create(childEntity, {
	scale: Vector3.create(0.2, 0.2, 0.2),
	position: Vector3.create(0, 0.4, 0),
	parent: parentEntity,
})
```


> [!WARNING]
> **📔 Nota**: Si la entidad adjunta tiene colliders, estos colliders podrían bloquear el movimiento del jugador o causar efectos de vibración. Quizá quieras desactivar la capa física de los colliders de la entidad adjunta. Consulta [Capas de colisión](/docs/creator/content-creator-es/scenes-sdk7/fundamentos-del-contenido-3d/colliders/#collision-layers)



> [!WARNING]
> **📔 Nota**: Si tienes una entidad hija y quieres conocer su Position y/o Rotation global, puedes usar las `getWorldPosition` y `getWorldRotation` funciones. Puedes consultarlas en la [Obtener la posición y la rotación global](#getting-global-position-and-rotation-of-an-entity) sección.


### Adjuntar a otros jugadores

Puedes usar el `AvatarAttach` componente para adjuntar una entidad a otro jugador. Para hacerlo, debes conocer el ID del jugador.

Para adjuntar una entidad al avatar de otro jugador, debes proporcionar el ID del usuario en el campo `avatarId`. Hay [varias formas](/docs/creator/content-creator-es/scenes-sdk7/interactividad/user-data/#get-player-data) de obtener estos datos.


> [!WARNING]
> **📔 Nota**: Para aquellos jugadores conectados con una Wallet de Ethereum, su `userId` es el mismo que su dirección de Ethereum.


Obtén el `userId` de todos los demás jugadores cercanos mediante `getPlayer()`

```ts
executeTask(async () => {
	for (const [entity, data] of engine.getEntitiesWith(PlayerIdentityData)) {
		console.log('ID del jugador: ', data.address)
	}
})
```

Usándolo junto con `AvatarAttach`, podrías usar el siguiente código para añadir un cubo flotando sobre la cabeza de cada otro jugador en la escena:

```ts
executeTask(async () => {
        for (const [entity, data] of engine.getEntitiesWith(PlayerIdentityData)) {
            const myEntity = engine.addEntity()
            MeshRenderer.setBox(myEntity)
            AvatarAttach.create(myEntity, {
                anchorPointId: AvatarAnchorPointType.AAPT_LEFT_HAND,
                avatarId: data.address,
            })
        }
    })
```

Consulta otras formas de obtener los IDs de otros usuarios en [Obtener datos del jugador](/docs/creator/content-creator-es/scenes-sdk7/interactividad/user-data/#get-player-data).


> [!WARNING]
> **📔 Nota**: No establezcas la entidad de otro jugador como la `parent` de tu entidad. El engine escribe el `Transform` para que tu escena lo lea, pero no es un padre del que tu entidad pueda colgarse, así que la hija vuelve a la raíz de la escena y se queda en el origen de la escena. Usa `AvatarAttach` con la dirección del jugador en su lugar, como se muestra arriba.


Los IDs de las entidades de los jugadores se reutilizan a medida que la gente entra y sale, así que la misma entidad puede pertenecer a otra persona más adelante. Si mantienes un marcador por jugador, guarda la dirección para la que lo creaste y compárala en cada fotograma. Cuando la dirección de una entidad cambie, elimina el marcador antiguo y crea uno nuevo:

```ts
import { AvatarAttach, AvatarAnchorPointType, engine, Entity, PlayerIdentityData } from '@dcl/sdk/ecs'

// dirección para la que se creó el marcador, para poder detectar un ID de entidad reutilizado
const markers = new Map<Entity, { address: string; marker: Entity }>()

function markerSystem() {
	// eliminar marcadores cuyo jugador se fue, o cuyo ID de entidad ahora pertenece a otra persona
	for (const [player, entry] of markers) {
		if (PlayerIdentityData.getOrNull(player)?.address === entry.address) continue
		engine.removeEntity(entry.marker)
		markers.delete(player)
	}

	for (const [player, data] of engine.getEntitiesWith(PlayerIdentityData)) {
		if (markers.has(player)) continue
		const marker = engine.addEntity()
		AvatarAttach.create(marker, {
			avatarId: data.address,
			anchorPointId: AvatarAnchorPointType.AAPT_NAME_TAG,
		})
		markers.set(player, { address: data.address, marker })
	}
}

engine.addSystem(markerSystem)
```

## Límites de la escena

Todas las entidades de tu escena deben caber dentro de los límites de la escena, ya que lo que queda fuera de esos límites son parcelas de terreno que pertenecen a otros jugadores.

Si alguna parte de tus modelos se extiende más allá de estos límites al ejecutar una vista previa, esas partes que sobresalgan se recortarán y no se renderizarán, tanto al ejecutar una vista previa como en la escena publicada.

La posición de las entidades en tu escena se comprueba constantemente a medida que se mueven; si una entidad sale de la escena y luego vuelve, se eliminará y después se renderizará normalmente de nuevo.

Una cuadrícula en el suelo de la escena muestra los límites de la escena, que por defecto van de 0 a 16 en el *x* y *z* eje, y hasta 20 en el *y* eje. Puedes colocar entidades bajo tierra, por debajo de 0 en el *y* eje.


> [!NOTE]
> **💡 Consejo**: Si tu escena necesita más parcelas, puedes agregarlas en el `scene.json` archivo. Consulta [Metadatos de la escena](/docs/creator/content-creator-es/scenes-sdk7/tipos-de-proyectos/scene-metadata/) para ver las instrucciones. Una vez agregadas, deberías ver cómo la cuadrícula se extiende para cubrir las parcelas adicionales.



---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-es/scenes-sdk7/fundamentos-del-contenido-3d/entity-positioning.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
