> Cómo mover, rotar y escalar una entidad gradualmente a lo largo del tiempo, con cambios incrementales.

# Mover entidades

Para mover, rotar o redimensionar una entity en tu scene durante un período de tiempo, usa el `Tween` component. El engine realiza la transformación deseada suavemente, mostrando actualizaciones en cada frame hasta que termina la duración especificada. Además, los valores del component de la entity afectada se actualizan en tiempo real en caso de que sea necesario hacer comprobaciones de proximidad en el código de la scene. `Transform` component de la entity afectada se actualizan en tiempo real en caso de que sea necesario hacer comprobaciones de proximidad en el código de la scene.


> [!NOTE]
> **💡 Consejo**: En el [Scene Editor en Creator Hub](/docs/creator/content-creator-es/scene-editor/primeros-pasos/about-editor/), puedes mover entities de forma sin código mediante **Actions**, consulta [Make any item smart](/docs/creator/content-creator-es/scene-editor/interactividad/make-any-item-smart/).


El component Tween tiene las siguientes funciones:

* `setMove`: Mover entre dos puntos
* `setRotate`: Rotar entre dos direcciones
* `setScale`: Escalar entre dos tamaños
* `setMoveRotateScale`: Transición simultánea en los tres parámetros
* `setMoveContinuous`: Mover constantemente en la misma dirección
* `setRotateContinuous`: Rotar constantemente en la misma dirección
* `setTextureMove`: Desplazar la texture de un material entre dos posiciones
* `setTextureMoveContinuous`: Desplazar la texture de un material constantemente en la misma dirección

## Mover entre dos puntos

Para mover una entity entre dos puntos, crea un `Tween` component con el `setMove` función.

```ts
const myEntity = engine.addEntity()
Transform.create(myEntity, {
	position: Vector3.create(4, 1, 4),
})
MeshRenderer.setBox(myEntity)

Tween.setMove(myEntity, 
	Vector3.create(1, 1, 1), 
	Vector3.create(8, 1, 8), 
	2000
)
```

El tween de movimiento toma la siguiente información:

* `entity`: La entity a mover
* `start`: Un Vector3 para la posición inicial
* `end`: Un Vector3 para la posición final
* `duration`: Cuántos milisegundos tarda en moverse entre las dos posiciones

También está disponible este otro parámetro opcional:

* `easingFunction`: Qué easing function usar. Mira [tweens no lineales](#non-linear-tweens)

## Rotar entre dos direcciones

Para rotar una entity entre dos puntos, crea un `Tween` component con el `setRotate` función.

```ts
const myEntity = engine.addEntity()
Transform.create(myEntity, {
	position: Vector3.create(4, 1, 4),
})
MeshRenderer.setBox(myEntity)

Tween.setRotate(myEntity, 
	Quaternion.fromEulerDegrees(0, 0, 0), 
	Quaternion.fromEulerDegrees(0, 170, 0), 
	700
)
```

El tween de rotación toma la siguiente información:

* `start`: Un Quaternion para la rotación inicial
* `end`: Un Quaternion para la rotación final
* `duration`: Cuántos milisegundos tarda en moverse entre las dos posiciones

También está disponible este otro parámetro opcional:

* `easingFunction`: Qué easing function usar. Mira [tweens no lineales](#non-linear-tweens)

### Rotar con un punto de pivote

Al rotar una entity, la rotación siempre hace referencia a la coordenada central de la entity. Para rotar una entity usando otro conjunto de coordenadas como punto de pivote, crea una segunda entity (invisible) con el punto de pivote como su posición y haz que sea parent de la entity que quieres rotar.

Al rotar la entity padre, sus children se rotarán usando la posición del parent como punto de pivote. Ten en cuenta que la `posición` de la child entity hace referencia a la del parent entity.

```ts
const pivotEntity = engine.addEntity()
Transform.create(pivotEntity, {
	position: Vector3.create(4, 1, 4),
})

const childEntity = engine.addEntity()
Transform.create(childEntity, {
	position: Vector3.create(1, 0, 0),
	parent: pivotEntity,
})
MeshRenderer.setBox(childEntity)

Tween.setRotate(pivotEntity, 
	Quaternion.fromEulerDegrees(0, 0, 0), 
	Quaternion.fromEulerDegrees(0, 170, 0), 
	700
)
```

Ten en cuenta que en este ejemplo, el system está rotando la `pivotEntity` entity, que es parent de la `childEntity` entity.

## Escalar entre dos tamaños

Para cambiar la escala de una entity entre dos tamaños, crea un `Tween` component con el `setScale` función.

```ts
const myEntity = engine.addEntity()
Transform.create(myEntity, {
	position: Vector3.create(4, 1, 4),
})
MeshRenderer.setBox(myEntity)

Tween.setScale(myEntity, 
	Vector3.create(1, 1, 1), 
	Vector3.create(4, 4, 4), 
	2000
)	

```

El tween de escala toma la siguiente información:

* `start`: Un Vector3 para el tamaño inicial
* `end`: Un Vector3 para el tamaño final
* `duration`: Cuántos milisegundos tarda en moverse entre las dos posiciones

También está disponible este otro parámetro opcional:

* `easingFunction`: Qué easing function usar. Mira [tweens no lineales](#non-linear-tweens)

## Mover desde la posición actual

Mientras un tween está activo, el engine actualiza los `Transform` component values de la entity en tiempo real. Esto significa que puedes leer la posición actual de la entity desde su `Transform` component en cualquier momento, y usar ese valor como el `start` de un nuevo tween. La entity entonces se mueve desde donde esté en ese momento hasta el objetivo, sin importar cómo haya llegado allí.

```ts
pointerEventsSystem.onPointerDown(
	{
		entity: myEntity,
		opts: { button: InputAction.IA_POINTER, hoverText: 'Mover' },
	},
	() => {
		Tween.setMove(myEntity, 
			Transform.get(myEntity).position, 
			Vector3.create(8, 1, 8), 
			2000,
			EasingFunction.EF_EASEOUTQUAD
		)
	}
)
```


> [!WARNING]
> **📔 Nota**: Pasa siempre un `start` valor explícito. Si el `start` parámetro se deja sin asignar, se trata como *(0, 0, 0)*, así que la entity se teletransporta al origen de la scene antes de moverse. Del mismo modo, si pasas una `start` hardcoded `Transform` que no coincide con la posición actual de la entity, la entity se teletransporta visiblemente a esa posición antes de moverse. Lee la posición actual desde el


### Retarget while traveling

Llamar `setMove` (o cualquiera de las otras funciones tween) en una entity que ya tiene un tween en ejecución reemplaza el tween en ejecución por el nuevo. Como el `start` se lee desde la posición en vivo de la entity, esto redirige la entity suavemente a mitad del trayecto: cambia de rumbo desde donde esté en ese momento, sin saltos.

El ejemplo de abajo mueve una entity hacia el pad que el jugador haga clic. Si el jugador hace clic en otro pad mientras la entity sigue viajando, la entity cambia de dirección suavemente hacia el nuevo objetivo.

```ts
function moveTo(target: Vector3) {
	Tween.setMove(shipEntity, 
		Transform.get(shipEntity).position, 
		target, 
		2000,
		EasingFunction.EF_EASEOUTQUAD
	)
}

pointerEventsSystem.onPointerDown(
	{
		entity: padA,
		opts: { button: InputAction.IA_POINTER, hoverText: 'Mover aquí' },
	},
	() => moveTo(Vector3.create(2, 1, 2))
)

pointerEventsSystem.onPointerDown(
	{
		entity: padB,
		opts: { button: InputAction.IA_POINTER, hoverText: 'Mover aquí' },
	},
	() => moveTo(Vector3.create(14, 1, 14))
)
```

Para ejecutar lógica cuando la entity llega a su destino, consulta [Al finalizar el tween](#on-tween-finished).


> [!WARNING]
> **📔 Nota**: Retargetea de esta manera solo en respuesta a eventos ocasionales, como un clic o llegar a un waypoint. Si el objetivo cambia constantemente, por ejemplo una entity que sigue al player, no crees un nuevo tween en cada frame: la entity se entrecortará visiblemente. Consulta [Seguir a un objetivo en movimiento](#follow-a-moving-target).


## tweens no lineales

Los tweens pueden seguir diferentes **Easing Functions** que afectan la tasa de cambio con el tiempo. Una **lineal** function significa que la velocidad del cambio es constante desde el inicio hasta el final. Hay muchas opciones para elegir, que dibujan curvas de distintas formas dependiendo de si el inicio y/o el final empiezan lentos, y cuánto. Una curva **easeinexpo** empieza lenta y termina rápida, aumentando la velocidad exponencialmente; por el contrario, una **easeoutexpo** curva empieza rápida y termina lenta.


> [!NOTE]
> **💡 Consejo**: Experimenta con diferentes curvas de movimiento. Las diferencias suelen ser sutiles, pero interpretamos subconscientemente información a partir de cómo se mueven las cosas, como el peso, la fricción o incluso la personalidad.


```ts
Tween.setScale(myEntity, 
	Vector3.create(1, 1, 1), 
	Vector3.create(4, 4, 4), 
	2000,
	EasingFunction.EF_EASEOUTBOUNCE
)

```

El `easingFunction` parámetro opcional toma su valor del `EasingFunction` enum, que ofrece las siguientes opciones:

* `EF_EASEBACK`
* `EF_EASEBOUNCE`
* `EF_EASECIRC`
* `EF_EASECUBIC`
* `EF_EASEELASTIC`
* `EF_EASEEXPO`
* `EF_EASEINBACK`
* `EF_EASEINBOUNCE`
* `EF_EASEINCIRC`
* `EF_EASEINCUBIC`
* `EF_EASEINELASTIC`
* `EF_EASEINEXPO`
* `EF_EASEINQUAD`
* `EF_EASEINQUART`
* `EF_EASEINQUINT`
* `EF_EASEINSINE`
* `EF_EASEOUTBACK`
* `EF_EASEOUTBOUNCE`
* `EF_EASEOUTCIRC`
* `EF_EASEOUTCUBIC`
* `EF_EASEOUTELASTIC`
* `EF_EASEOUTEXPO`
* `EF_EASEOUTQUAD`
* `EF_EASEOUTQUART`
* `EF_EASEOUTQUINT`
* `EF_EASEOUTSINE`
* `EF_EASEQUAD`
* `EF_EASEQUART`
* `EF_EASEQUINT`
* `EF_EASESINE`
* `EF_LINEAR`

## Rotación constante

Para hacer que una entity rote constantemente, usa el `Tween` component con el `setRotateContinuous` función.

```ts
Tween.setRotateContinuous(myEntity, 
	Quaternion.fromEulerDegrees(0, -1, 0), 
	45
)
```

El tween de rotación continua toma la siguiente información:

* `entity`: La entity a rotar
* `direction`: Un Quaternion que determina el axis alrededor del que rotar. Solo importa el axis de esta rotación; se ignora el tamaño de su ángulo. Por ejemplo, `Quaternion.fromEulerDegrees(0, -1, 0)` y `Quaternion.fromEulerDegrees(0, -90, 0)` se comportan igual, ambos rotando alrededor del *y* axis en dirección negativa.
* `speed`: Cuántos grados por segundo rotará la entity. Un valor negativo rota en la dirección opuesta.

También está disponible este otro parámetro opcional:

* `duration`: Cuántos milisegundos mantener la rotación. Después de este tiempo, la rotación se detendrá.

## Movimiento constante

Para hacer que una entity se mueva constantemente en la misma dirección, usa el `Tween` component con el `setMoveContinuous` función.

```ts
Tween.setMoveContinuous(myEntity, 
	Vector3.create(0, 0, 1), 
	0.7
)
```

El tween de movimiento continuo toma la siguiente información:

* `entity`: La entity a mover
* `direction`: Un Vector3 para el movimiento
* `speed`: Cuántos metros por segundo se moverá la entity

También está disponible este otro parámetro opcional:

* `duration`: Cuántos milisegundos mantener el movimiento. Después de este tiempo, el movimiento se detendrá.

El tween de movimiento continuo toma la siguiente información:

### Seguir a un objetivo en movimiento

`setMoveContinuous` también es la herramienta adecuada cuando el destino sigue cambiando, por ejemplo una entity que sigue al player. Resulta tentador crear en su lugar un nuevo `setMove` tween en cada frame, apuntando a la última posición del player, pero eso produce un tirón visible.

La razón es que la posición que lees desde el `Transform` component de una entity con tween es la posición que el engine devolvió por última vez a la scene, así que va con unos cuantos frames de retraso respecto a donde realmente está la entity. Cuando pasas ese valor como el `start` de un nuevo tween, el engine lo aplica inmediatamente, y la entity salta de vuelta a donde estaba unos cuantos frames atrás. Una de esas correcciones es imperceptible, pero repetirla muchas veces por segundo no lo es.

`setMoveContinuous` evita el problema por completo, porque le das una dirección y una velocidad en lugar de un punto de inicio. El engine sigue usando la posición real de la entity, así que reemplazar el tween para cambiar de dirección nunca hace que la entity salte.

```ts
const CHASE_SPEED = 3
const STOP_DISTANCE = 1

engine.addSystem(() => {
	const playerPosition = Transform.get(engine.PlayerEntity).position
	const myPosition = Transform.get(myEntity).position

	if (Vector3.distance(myPosition, playerPosition) <= STOP_DISTANCE) {
		// Lo bastante cerca: un tween continuo no tiene destino, así que lo detienes tú mismo
		if (Tween.has(myEntity)) {
			Tween.deleteFrom(myEntity)
		}
		return
	}

	const direction = Vector3.subtract(playerPosition, myPosition)
	direction.y = 0 // mantenerse en el suelo, aunque el player salte

	Tween.setMoveContinuous(myEntity, Vector3.normalize(direction), CHASE_SPEED)
})
```


> [!WARNING]
> **📔 Nota**: Un tween continuo nunca termina por sí solo, así que la entity sigue moviéndose hasta que eliminas el `Tween` component con `Tween.deleteFrom()`, o hasta que el `duration` opcional se agote. Ten en cuenta también que reorientar en cada frame envía una actualización al engine en cada frame. Para evitar eso, llama a `setMoveContinuous` de nuevo solo cuando la dirección haya cambiado lo suficiente como para importar.


## Secuencias de tweens

Para hacer que una entity reproduzca una serie de tweens en secuencia, usa el `TweenSequence` component. Este component requiere dos campos:

* `sequence`: Un array con múltiples definiciones de tween, que se ejecutarán secuencialmente. El array puede estar vacío, en cuyo caso solo reproduce el tween actual.
* `loop` *(opcional)*: Si no se proporciona, la secuencia solo se reproduce una vez. Si el campo está presente, el valor debe ser un valor del `TweenLoop` enum. Los valores aceptados son:
  * `TL_RESTART`: Cuando la secuencia termina, se reinicia. Si el último estado no coincide con el primero, la entity salta instantáneamente de uno al otro.
  * `TL_YOYO`: Cuando la secuencia termina, retrocede, haciendo todos los tweens en orden inverso hasta volver a llegar al inicio. Entonces comienza una vez más.

### Mover de un lado a otro

Para hacer que una plataforma se mueva constantemente de un lado a otro entre dos posiciones, deja el `sequence` array vacío, y establece `loop` en `TweenLoop.TL_YOYO`

```ts
const myEntity = engine.addEntity()
Transform.create(myEntity, {
	position: Vector3.create(4, 1, 4),
})
MeshRenderer.setBox(myEntity)

Tween.setMove(myEntity, 
	Vector3.create(1, 1, 1), 
	Vector3.create(8, 1, 8), 
	2000
)

TweenSequence.create(myEntity, { sequence: [], loop: TweenLoop.TL_YOYO })
```

La entity se moverá de un lado a otro entre el punto inicial y el punto final, con la misma duración y el mismo easing function en ambas direcciones.

### Seguir una ruta

Para hacer que una entity siga una ruta más compleja con múltiples puntos, proporciona una lista de definiciones de tween en el `sequence` de un `TweenSequence` componente.

```ts
const myEntity = engine.addEntity()
Transform.create(myEntity, {
	position: Vector3.create(4, 1, 4),
})
MeshRenderer.setBox(myEntity)

Tween.setMove(myEntity, 
	Vector3.create(6.5, 7, 4), 
	Vector3.create(6.5, 7, 12), 
	4000
)

TweenSequence.create(myEntity, {
	sequence: [
		{
			duration: 2000,
			easingFunction: EasingFunction.EF_LINEAR,
			mode: Tween.Mode.Move({
				start: Vector3.create(6.5, 7, 12),
				end: Vector3.create(6.5, 10.5, 12),
			}),
		},
		{
			duration: 3000,
			easingFunction: EasingFunction.EF_LINEAR,
			mode: Tween.Mode.Move({
				start: Vector3.create(6.5, 10.5, 12),
				end: Vector3.create(6.5, 10.5, 4),
			}),
		},
		{
			duration: 3000,
			easingFunction: EasingFunction.EF_LINEAR,
			mode: Tween.Mode.Move({
				start: Vector3.create(6.5, 10.5, 4),
				end: Vector3.create(6.5, 7, 4),
			}),
		},
	],
	loop: TweenLoop.TL_RESTART,
})
```

Ten en cuenta que al definir un tween dentro de un TweenSequence, necesitas usar el formato más detallado de `Tween.Mode.Move`, o `Tween.Mode.Rotate`, o `Tween.Mode.Scale` para definir el tween. En este formato más detallado, necesitas especificar:

* `duration`: Cuántos milisegundos tarda en moverse entre las dos posiciones
* `easingFunction`: Qué easing function usar. Mira [tweens no lineales](#non-linear-tweens). En este formato, el valor es obligatorio.
* `mode`: El modo del tween, que puede ser `Tween.Mode.Move`, `Tween.Mode.Rotate`, o `Tween.Mode.Scale`.

Y dentro del `mode` campo, necesitas especificar:

* `start`: El valor inicial del tween
* `end`: El valor final del tween

## Al finalizar el tween

Usa `tweenSystem.tweenCompleted` para detectar cuándo ha terminado un tween. Esto puede ser útil para realizar acciones cuando un tween termina, por ejemplo para abrir la puerta de un ascensor.

```ts
engine.addSystem(() => {
	const tweenCompleted = tweenSystem.tweenCompleted(myEntity)
	if (tweenCompleted) {
		//reproducir sonido
	}
})
```

## Tweens simultáneos

Para mover, rotar y escalar una entity entre un estado inicial y uno final, crea una `Tween` component con el `setMoveRotateScale` function. Esta function también se puede usar en cualquier combinación de estos tres parámetros.

```ts
const myEntity = engine.addEntity()
Transform.create(myEntity, {
	position: Vector3.create(14, 1, 2),
})
MeshRenderer.setBox(myEntity)

Tween.setMoveRotateScale(myEntity, {
	position: { start: Vector3.create(14, 1, 2), end: Vector3.create(14, 3, 2) },
	rotation: { start: Quaternion.fromEulerDegrees(0, 0, 0), end: Quaternion.fromEulerDegrees(0, 180, 90) },
	scale: { start: Vector3.One(), end: Vector3.create(2, 0.5, 2) },
	duration: 2000
})
```

El tween de movimiento toma la siguiente información:

* `entity`: La entity a mover
* `params`: Un object con los siguientes parámetros
  * `posición`: *(opcional)* Un object con un `start` y `end` value, tanto como `Vector3`.
  * `rotation`: *(opcional)* Un object con un `start` y `end` value, tanto como `Quaternion`.
  * `scale`: *(opcional)* Un object con un `start` y `end` value, tanto como `Vector3`. Al menos uno de `posición`, `rotation` ni a `scale` debe proporcionarse.
  * `duration`: *(obligatorio)* Cuántos milisegundos tarda en realizar la transición entre los dos conjuntos de valores
  * `easingFunction`: *(opcional)* Qué easing function usar. Mira [tweens no lineales](#non-linear-tweens)

Una entity solo puede tener un `Tween` component, y cada component tween solo puede realizar una transformación a la vez. Mediante el `setMoveRotateScale` tipo tween, puedes hacer que una entity se mueva lateralmente y también rote al mismo tiempo, pero ambos movimientos seguirán la misma línea de tiempo. Si necesitas que las transiciones sean independientes entre sí, como solución alternativa puedes usar entities parented. Por ejemplo, puedes tener una entity padre invisible que se mueva lateralmente, con un child visible que rote.

En el siguiente fragmento, una entity padre rota mientras un child se escala.

```ts
const parentEntity = engine.addEntity()
Transform.create(parentEntity, {
	position: Vector3.create(4, 1, 4),
})
MeshRenderer.setBox(parentEntity)
Tween.setRotate(parentEntity, 
	Quaternion.fromEulerDegrees(0, 0, 0), 
	Quaternion.fromEulerDegrees(0, 170, 0), 
	5000
)

const childEntity = engine.addEntity()
Transform.create(childEntity, {
	position: Vector3.create(0, 0, 0),
	parent: parentEntity,
})
MeshRenderer.setBox(childEntity)
Tween.setScale(childEntity, 
	Vector3.create(1, 1, 1), 
	Vector3.create(4, 4, 4), 
	2000
)
```

## Pausar un tween

Para pausar un tween, cambia la `playing` propiedad a false. Para reanudarlo, vuelve a cambiarla a true.

```ts
pointerEventsSystem.onPointerDown(
	{
		entity: button,
		opts: { button: InputAction.IA_POINTER, hoverText: 'pausa' },
	},
	() => {
		let tweenData = Tween.getMutable(myEntity)
		tweenData.playing = !tweenData.playing
	}
)
```

Para finalizar un tween que no necesita continuar, elimina el `Tween` component de la entity. Si la entity también estaba usando un `TweenSequence` component, elimínalo también.

```ts
pointerEventsSystem.onPointerDown(
	{
		entity: button,
		opts: { button: InputAction.IA_POINTER, hoverText: 'pausa' },
	},
	() => {
		if( Tween.has(myEntity)){
			Tween.deleteFrom(myEntity)
		}
		if( TweenSequence.has(myEntity)){
			TweenSequence.deleteFrom(myEntity)
		}
	}
)
```

## Tweens basados en un system

En lugar de usar el componente Tween y dejar que el engine maneje la transformación, puedes preferir hacer esta transición de forma incremental, frame a frame, mediante un [system](/docs/creator/content-creator-es/scenes-sdk7/arquitectura/systems/) en tu scene. Al mover la entity una pequeña cantidad cada vez que se ejecuta la función.

Por un lado, esto te da más control para recalcular los movimientos en cada frame. Por otro lado, el código es más complicado, y los players con máquinas menos potentes podrían experimentar el tween como lento, notando cada incremento.

### Mover mediante system

La forma más sencilla de mover una entity es modificar gradualmente el *posición* valor almacenado en el `Transform` componente.

```ts
function SimpleMove() {
	let transform = Transform.getMutable(myEntity)
	transform.position = Vector3.add(
		transform.position,
		Vector3.scale(Vector3.Forward(), 0.05)
	)
}

engine.addSystem(SimpleMove)

const myEntity = engine.addEntity()
Transform.create(myEntity, {
	position: Vector3.create(4, 1, 4),
})
MeshRenderer.setBox(myEntity)
```

En este ejemplo estamos moviendo una entity 0.05 metros por tick del game loop.

`Vector3.Forward()` devuelve un vector que apunta hacia adelante y mide 1 metro de longitud. En este ejemplo luego reducimos este vector a 1/20 de su longitud con `Vector3.scale()`. Si nuestra scene tiene 30 frames por second, la entity se está moviendo a una velocidad de 1,5 metros por segundo.

![](/docs/_remote/90ee9b2fdf4c6fb2a573ce79.webp)

### Rotar mediante system

La forma más sencilla de rotar una entity es cambiar gradualmente los valores en el componente Transform de forma incremental, y ejecutar esto como parte de la función de un system.

```ts
function SimpleRotate() {
	let transform = Transform.getMutable(myEntity)
	transform.rotation = Quaternion.multiply(
		transform.rotation,
		Quaternion.fromAngleAxis(1, Vector3.Up())
	)
}

engine.addSystem(SimpleRotate)

const myEntity = engine.addEntity()
Transform.create(myEntity, {
	position: Vector3.create(4, 1, 4),
})
MeshRenderer.setBox(myEntity)
```

Ten en cuenta que, para combinar la rotación actual con cada incremento, estamos usando `Quaternion.multiply`. En matemática de quaternion, combinas dos rotaciones multiplicándolas, NO sumándolas. La rotación resultante de multiplicar un quaternion por otro será la rotación final equivalente tras realizar primero una rotación y luego la otra.

En este ejemplo, estamos rotando la entity 1 grado en dirección ascendente en cada tick del game loop.


> [!NOTE]
> **💡 Consejo**: Para hacer que una entity rote siempre para mirar al player, puedes añadir un [`Billboard` component](/docs/creator/content-creator-es/scenes-sdk7/fundamentos-del-contenido-3d/entity-positioning/#face-the-player).


![](/docs/_remote/13d2f2aa20f3bb7131ff9274.webp)

### Rotar mediante system sobre un punto de pivote

Al rotar una entity, la rotación siempre hace referencia a la coordenada central de la entity. Para rotar una entity usando otro conjunto de coordenadas como punto de pivote, crea una segunda entity (invisible) con el punto de pivote como su posición y haz que sea parent de la entity que quieres rotar.

Al rotar la entity padre, sus children se rotarán usando la posición del parent como punto de pivote. Ten en cuenta que la `posición` de la child entity hace referencia a la del parent entity.

```ts
function SimpleRotate() {
	let transform = Transform.getMutable(pivotEntity)
	transform.rotation = Quaternion.multiply(
		transform.rotation,
		Quaternion.fromAngleAxis(1, Vector3.Up())
	)
}

engine.addSystem(SimpleRotate)

const pivotEntity = engine.addEntity()
Transform.create(pivotEntity, {
	position: Vector3.create(4, 1, 4),
})

const childEntity = engine.addEntity()
Transform.create(childEntity, {
	position: Vector3.create(1, 0, 0),
	parent: pivotEntity,
})
MeshRenderer.setBox(childEntity)
```

Ten en cuenta que en este ejemplo, el system está rotando la `pivotEntity` entity, que es parent de la `childEntity` entity.

![](/docs/_remote/3c145695aa57e42efe942e39.webp)

### Ajustar el movimiento al tiempo de delay

Supongamos que el player que visita tu scene tiene dificultades para seguir el ritmo del frame rate. Eso podría hacer que el movimiento parezca entrecortado, ya que no todos los frames están espaciados uniformemente, pero cada uno mueve la entity en la misma cantidad.

Puedes compensar este tiempo desigual usando el `dt` parámetro para ajustar la escala del movimiento.

```ts
function SimpleMove(dt: number) {
	let transform = Transform.getMutable(myEntity)
	transform.position = Vector3.add(
		transform.position,
		Vector3.scale(Vector3.Forward(), dt)
	)
}

engine.addSystem(SimpleMove)

const myEntity = engine.addEntity()
Transform.create(myEntity, {
	position: Vector3.create(4, 1, 4),
})
MeshRenderer.setBox(myEntity)
```

El ejemplo anterior mantiene el movimiento aproximadamente a la misma velocidad que el ejemplo de movimiento anterior, incluso si el frame rate baja. Al ejecutarse a 30 frames por second, el valor de `dt` es 1/30.

También puedes suavizar las rotaciones de la misma manera multiplicando la cantidad de rotación por `dt`.

### Mover entre dos puntos mediante system

Si quieres que una entity se mueva suavemente entre dos puntos, usa el *lerp* (algoritmo de interpolación lineal). Este algoritmo es muy conocido en el desarrollo de juegos, ya que resulta realmente útil.

El `lerp()` la función toma tres parámetros:

* El vector de la posición de origen
* El vector de la posición de destino
* La cantidad, un valor de 0 a 1 que representa qué fracción de la traslación realizar.

```ts
const originVector = Vector3.Zero()
const targetVector = Vector3.Forward()

let newPos = Vector3.lerp(originVector, targetVector, 0.6)
```

El algoritmo de interpolación lineal encuentra un punto intermedio en el recorrido entre ambos vectores que coincide con la cantidad proporcionada.

Por ejemplo, si el vector de origen es *(0, 0, 0)* y el vector de destino es *(10, 0, 10)*:

* Usar una cantidad de 0 devolvería *(0, 0, 0)*
* Usar una cantidad de 0,3 devolvería *(3, 0, 3)*
* Usar una cantidad de 1 devolvería *(10, 0, 10)*

Para implementar esto `lerp()` en tu scene, recomendamos crear un [custom component](/docs/creator/content-creator-es/scenes-sdk7/arquitectura/custom-components/) para almacenar la información necesaria. También necesitas definir un system que implemente el movimiento gradual en cada frame.

```ts
// definir custom component
const MoveTransportData = {
	start: Schemas.Vector3,
	end: Schemas.Vector3,
	fraction: Schemas.Float,
	speed: Schemas.Float,
}

export const LerpTransformComponent = engine.defineComponent(
	'LerpTransformComponent',
	MoveTransportData
)

// definir system
function LerpMove(dt: number) {
	let transform = Transform.getMutable(myEntity)
	let lerp = LerpTransformComponent.getMutable(myEntity)
	if (lerp.fraction < 1) {
		lerp.fraction += dt * lerp.speed
		transform.position = Vector3.lerp(lerp.start, lerp.end, lerp.fraction)
	}
}

engine.addSystem(LerpMove)

// crear entity
const myEntity = engine.addEntity()

Transform.create(myEntity, {
	position: Vector3.create(4, 1, 4),
})

MeshRenderer.setBox(myEntity)

LerpTransformComponent.create(myEntity, {
	start: Vector3.create(4, 1, 4),
	end: Vector3.create(8, 1, 8),
	fraction: 0,
	speed: 1,
})
```

![](/docs/_remote/751a298f237c2c997d52fdec.webp)

### Seguir suavemente un target en movimiento mediante system

Para acercar una entity a un target que sigue moviéndose, como una mascota que sigue al player, muévela una parte de la distancia restante en cada frame. No uses una parte fija por frame, como `Vector3.lerp(current, target, 0.1)`: el resultado entonces depende del frame rate, por lo que la entity alcanza al target más rápido en un computer veloz que en uno lento. Calcula la parte a partir de `dt` en su lugar:

```ts
const follower = engine.addEntity()
Transform.create(follower, { position: Vector3.create(8, 1, 8) })
MeshRenderer.setBox(follower)

const RATE = 5 // Valores más altos alcanzan al target más rápido

function FollowSystem(dt: number) {
	const transform = Transform.getMutable(follower)
	const target = Transform.get(engine.PlayerEntity).position
	// El mismo suavizado a cualquier frame rate
	const t = 1 - Math.exp(-RATE * dt)
	transform.position = Vector3.lerp(transform.position, target, t)
}

engine.addSystem(FollowSystem)
```

La misma fórmula funciona para las rotaciones con `Quaternion.slerp()`.

### Rotar entre dos ángulos mediante system

Para rotar suavemente entre dos ángulos, usa el *slerp* (*esférica* interpolación lineal). Este algoritmo es muy similar a un *lerp*, pero gestiona rotaciones de quaternion.

El `slerp()` la función toma tres parámetros:

* El [quaternion](https://en.wikipedia.org/wiki/Quaternion) ángulo para la rotación de origen
* El [quaternion](https://en.wikipedia.org/wiki/Quaternion) ángulo para la rotación de destino
* La cantidad, un valor de 0 a 1 que representa qué fracción de la traslación realizar.


> [!NOTE]
> **💡 Consejo**: Puedes pasar valores de rotación en [euler](https://en.wikipedia.org/wiki/Euler_angles) grados (de 0 a 360) usando `Quaternion.fromEulerDegrees()`.


```ts
const originRotation = Quaternion.fromEulerDegrees(0, 90, 0)
const targetRotation = Quaternion.fromEulerDegrees(0, 0, 0)

let newRotation = Quaternion.slerp(originRotation, targetRotation, 0.6)
```

Para implementar esto en tu scene, recomendamos almacenar los datos que entran en la `Slerp()` función en un [custom component](/docs/creator/content-creator-es/scenes-sdk7/arquitectura/custom-components/). También necesitas definir un system que implemente la rotación gradual en cada frame.

```ts
// definir custom component
const RotateSlerpData = {
	start: Schemas.Quaternion,
	end: Schemas.Quaternion,
	fraction: Schemas.Float,
	speed: Schemas.Float,
}

export const SlerpData = engine.defineComponent('SlerpData', RotateSlerpData)

// definir system
function SlerpRotate(dt: number) {
	let transform = Transform.getMutable(myEntity)
	let slerpData = SlerpData.getMutable(myEntity)
	if (slerpData.fraction < 1) {
		slerpData.fraction += dt * slerpData.speed
		transform.rotation = Quaternion.slerp(
			slerpData.start,
			slerpData.end,
			slerpData.fraction
		)
	}
}

engine.addSystem(SlerpRotate)

// crear entity
const myEntity = engine.addEntity()

Transform.create(myEntity, {
	position: Vector3.create(4, 1, 4),
})

MeshRenderer.setBox(myEntity)

SlerpData.create(myEntity, {
	start: Quaternion.fromEulerDegrees(0, 0, 0),
	end: Quaternion.fromEulerDegrees(0, 180, 0),
	fraction: 0,
	speed: 0.3,
})
```


> [!WARNING]
> **📔 Nota**: También podrías representar la rotación con ángulos euler como `Vector3` values y usar una `Lerp()` function, pero eso implicaría una conversión desde `Vector3` en `Quaternion` en cada frame. Los valores de rotación se almacenan internamente como quaternions en el `Transform` component, así que es más eficiente para la scene trabajar con quaternions.


![](/docs/_remote/e4559d5d1a49351c4ded356f.webp)

Un enfoque más simple pero menos eficiente para esto aprovecha la `Quaternion.rotateTowards` function, y evita usar custom components.

```ts
function SimpleRotate(dt: number) {
	let transform = Transform.getMutable(myEntity)
	transform.rotation = Quaternion.rotateTowards(
		transform.rotation,
		Quaternion.fromEulerDegrees(90, 0, 0),
		dt * 10
	)
	if (
		Quaternion.angle(transform.rotation, Quaternion.fromEulerDegrees(90, 0, 0)) < 0.01
	) {
		console.log('done')
		engine.removeSystem(SimpleRotate)
	}
}

engine.addSystem(SimpleRotate)

const myEntity = engine.addEntity()
Transform.create(myEntity, {
	position: Vector3.create(4, 1, 4),
	rotation: Quaternion.fromEulerDegrees(0, 0, 90),
})

MeshRenderer.setBox(myEntity)
```

En el ejemplo anterior `Quaternion.rotateTowards` toma tres argumentos: la rotación inicial, la rotación final deseada y el incremento máximo por frame. En este caso, como el incremento máximo es de `dt * 10` grados, la rotación se realizará a lo largo de un período de un par de 9 segundos.

Ten en cuenta que el system también comprueba si la rotación está completa y, si es así, elimina el system del engine. De lo contrario, el system seguiría haciendo cálculos en cada frame, incluso una vez completada la rotación.

### Cambiar la escala entre dos tamaños mediante system

Si quieres que una entity cambie de tamaño suavemente y sin cambiar sus proporciones, usa el *lerp* (algoritmo de interpolación lineal) del `Scalar` objeto.

De lo contrario, si quieres cambiar el axis en diferentes proporciones, usa `Vector3` para representar la escala de origen y la escala de destino, y luego usa la *lerp* función del `Vector3`.

El `lerp()` función del `Scalar` objeto toma tres parámetros:

* Un número para la escala de origen
* Un número para la escala de destino
* La cantidad, un valor de 0 a 1 que representa qué fracción del escalado realizar.

```ts
const originScale = 1
const targetScale = 10

let newScale = Scalar.lerp(originScale, targetScale, 0.6)
```

Para implementar este lerp en tu scene, te recomendamos crear un custom component para almacenar la información necesaria. También necesitas definir un system que implemente el escalado gradual en cada frame.

```ts
// definir custom component
const ScaleTransportData = {
	start: Schemas.Number,
	end: Schemas.Number,
	fraction: Schemas.Float,
	speed: Schemas.Float,
}

export const ScaleTransformComponent = engine.defineComponent(
	'ScaleTransformComponent',
	ScaleTransportData
)

// definir system
function LerpMove(dt: number) {
	let transform = Transform.getMutable(myEntity)
	let lerp = ScaleTransformComponent.getMutable(myEntity)
	if (lerp.fraction < 1) {
		lerp.fraction += dt * lerp.speed
		const newScale = Scalar.lerp(lerp.start, lerp.end, lerp.fraction)
		transform.scale = Vector3.create(newScale, newScale, newScale)
	}
}

engine.addSystem(LerpMove)

// crear entity
const myEntity = engine.addEntity()

Transform.create(myEntity, {
	position: { x: 4, y: 1, z: 4 },
})

MeshRenderer.setBox(myEntity)

ScaleTransformComponent.create(myEntity, {
	start: 1,
	end: 2,
	fraction: 0,
	speed: 1,
})

Vector3.create(1, 1, 1)
```

![](/docs/_remote/3b128506c9be0c97b7264b06.webp)

### Mover a velocidades irregulares entre dos puntos mediante system

Mientras usas el método lerp, puedes hacer que la velocidad de movimiento no sea lineal. En el ejemplo anterior incrementamos la cantidad de lerp en una cantidad dada en cada frame, pero también podríamos usar una función matemática para incrementar el número exponencialmente o con otras medidas que te den un ritmo de movimiento diferente.

También podrías usar una función que dé resultados recurrentes, como una función seno, para describir un movimiento que va y viene.

A menudo estas transiciones no lineales pueden dar mucha vida a una scene. Un movimiento que acelera sobre una curva o desacelera gradualmente puede decir mucho sobre la naturaleza de un objeto o character. Incluso podrías aprovechar funciones matemáticas que añadan efectos rebote.

```ts
// definir custom component
const MoveTransportData = {
	start: Schemas.Vector3,
	end: Schemas.Vector3,
	fraction: Schemas.Float,
	speed: Schemas.Float,
}

export const LerpTransformComponent = engine.defineComponent(
	'LerpTransformComponent',
	MoveTransportData
)

// definir system
function LerpMove(dt: number) {
	let transform = Transform.getMutable(myEntity)
	let lerp = LerpTransformComponent.getMutable(myEntity)
	if (lerp.fraction < 1) {
		lerp.fraction += dt * lerp.speed
		const interpolatedValue = interpolate(lerp.fraction)
		transform.position = Vector3.lerp(lerp.start, lerp.end, interpolatedValue)
	}
}

// mapear la fraction de lerp a una curva exponencial
function interpolate(t: number) {
	return t * t
}

engine.addSystem(LerpMove)

// crear entity
const myEntity = engine.addEntity()

Transform.create(myEntity, {
	position: { x: 4, y: 1, z: 4 },
})

MeshRenderer.setBox(myEntity)

LerpTransformComponent.create(myEntity, {
	start: Vector3.create(4, 1, 4),
	end: Vector3.create(8, 1, 8),
	fraction: 0,
	speed: 1,
})
```

El ejemplo anterior es igual que el ejemplo de lerp lineal que hemos mostrado antes, pero el `fraction` field asignado a un valor no lineal en cada tick. Este valor no lineal se usa para calcular la `lerp` función, dando como resultado un movimiento que sigue una curva exponencial.

También puedes mapear una transición en rotation o en scale de la misma manera que se muestra arriba, mapeando una transición lineal a una curva.

![](/docs/_remote/95fb7814c57340842f047b62.webp)

### Seguir una path mediante system

Puedes hacer que una entity recorra en bucle un array de vectors, realizando un movimiento lerp entre cada uno para seguir una path más compleja.

```ts
// definir custom component
const PathTransportData = {
	path: Schemas.Array(Schemas.Vector3),
	start: Schemas.Vector3,
	end: Schemas.Vector3,
	fraction: Schemas.Float,
	speed: Schemas.Float,
	pathTargetIndex: Schemas.Int,
}

export const LerpTransformComponent = engine.defineComponent(
	'LerpTransformComponent',
	PathTransportData
)

// definir system
function PathMove(dt: number) {
	let transform = Transform.getMutable(myEntity)
	let lerp = LerpTransformComponent.getMutable(myEntity)
	if (lerp.fraction < 1) {
		lerp.fraction += dt * lerp.speed
		transform.position = Vector3.lerp(lerp.start, lerp.end, lerp.fraction)
	} else {
		lerp.pathTargetIndex += 1
		if (lerp.pathTargetIndex >= lerp.path.length) {
			lerp.pathTargetIndex = 0
		}
		lerp.start = lerp.end
		lerp.end = lerp.path[lerp.pathTargetIndex]
		lerp.fraction = 0
	}
}

engine.addSystem(PathMove)

// crear entity
const myEntity = engine.addEntity()

Transform.create(myEntity, {
	position: Vector3.create(1, 1, 1),
})

MeshRenderer.setBox(myEntity)

const point1 = Vector3.create(1, 1, 1)
const point2 = Vector3.create(8, 1, 3)
const point3 = Vector3.create(8, 4, 7)
const point4 = Vector3.create(1, 1, 7)

const myPath = [point1, point2, point3, point4]

LerpTransformComponent.create(myEntity, {
	path: myPath,
	start: Vector3.create(4, 1, 4),
	end: Vector3.create(8, 1, 8),
	fraction: 0,
	speed: 1,
	pathTargetIndex: 1,
})
```

El ejemplo anterior define una path 3D que está compuesta por cuatro vectors 3D. El `PathTransportData` custom component contiene los mismos datos usados por el custom component en el *lerp* ejemplo anterior, pero añade un `path` array, con todos los puntos de nuestra path, y un `pathTargetIndex` field para llevar un seguimiento de qué segmento de la path está actualmente en uso.

El system es muy similar al system del *lerp* ejemplo, pero cuando se completa una acción lerp, establece los `target` y `origin` fields con nuevos valores. Si llegamos al final de la path, volvemos al primer valor de la path.

![](/docs/_remote/98be0ad1836095ed41c8b3e5.webp)

## Tweens de texture

Para hacer que una texture se deslice suavemente, usa el `Tween` component con el `setTextureMove` función.

```ts
Tween.setTextureMove(myEntity, 
	Vector2.create(0, 0), 
	Vector2.create(1, 0), 
	2000
)
```

El texture tween toma la siguiente información:

* `entity`: La entity a la que mover la texture
* `start`: Un Vector2 para la posición inicial
* `end`: Un Vector2 para la posición final
* `duration`: Cuántos milisegundos tarda en moverse entre las dos posiciones

También está disponible este otro parámetro opcional:

* `movementType`: (opcional), define si el movimiento será en el campo offset o en el campo tiling. Por defecto usa offset.
* `easingFunction`: Qué easing function usar. Mira [tweens no lineales](#non-linear-tweens).

## Movimiento constante de texture

Para hacer que una texture se deslice constantemente, usa el `Tween` component con el `setTextureMoveContinuous` función.

```ts
Tween.setTextureMoveContinuous(myEntity, 
	Vector2.create(0, 1), 
	0.7
)
```

El texture continuous tween toma la siguiente información:

* `entity`: La entity a la que mover la texture
* `direction`: Un Vector2 para el movimiento
* `speed`: Cuántas unidades por segundo se moverá la entity

También está disponible este otro parámetro opcional:

* `movementType`: define si el movimiento será en el campo offset o en el campo tiling. Por defecto usa offset.
* `duration`: Cuántos milisegundos mantener el movimiento. Después de este tiempo, el movimiento se detendrá.

Lee más sobre texture tweens en el [Texture Tweens](/docs/creator/content-creator-es/scenes-sdk7/fundamentos-del-contenido-3d/materials/#texture-tweens) sección.


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-es/scenes-sdk7/fundamentos-del-contenido-3d/move-entities.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
