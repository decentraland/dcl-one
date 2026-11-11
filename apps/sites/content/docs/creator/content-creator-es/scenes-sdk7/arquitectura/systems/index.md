> Aprende cómo se usan los Systems para actualizar el estado de la escena

# Sistemas

Las scenes de Decentraland dependen de *systems* para actualizar cualquier dato a lo largo del tiempo, incluida la información almacenada en el [components](/docs/creator/content-creator-es/scenes-sdk7/arquitectura/entities-components/).

![](/docs/_remote/0b852c135fff08d1f4c9243d.webp)

*systems* son lo que hacen que las scenes sean dinámicas; son functions que se ejecutan periódicamente en cada tick del game loop de la scene, cambiando lo que se renderizará.

El siguiente ejemplo muestra una declaración básica de system:

```ts
// Definir el system
function mySystem() {
  console.log("Se ejecuta en cada tick. Mi system está funcionando")
}
// Añadir system al engine
engine.addSystem(mySystem)
```

La función de un system puede hacer lo que quieras. Normalmente, actuará sobre todas las entities que cumplan ciertos [query](/docs/creator/content-creator-es/scenes-sdk7/arquitectura/querying-components/), siguiendo cierta lógica para cambiar los valores almacenados en los components de la entity.

```ts
function moveSystem(dt: number) {
  // iterar sobre todas las entities con un Transform
  for (const [entity] of engine.getEntitiesWith(Transform)) {

  // obtener un componente Transform mutable
  const transform = Transform.getMutable(entity)

  // actualizar el valor de position
    transform.position.z += 0.01
  }
}

engine.addSystem(moveSystem)
```

En el ejemplo anterior, el system `MoveSystem` es una función que se ejecuta en cada tick del game loop, cambiando la posición de cada entity en la scene que tiene un Transform.

![](/docs/_remote/7e85b65f2626d96c8f892499.webp)

Puedes tener múltiples systems en tu scene para desacoplar distintos comportamientos, haciendo tu código más limpio y más fácil de escalar y reutilizar. Por ejemplo, un system podría encargarse de la física, otro podría hacer que una entity obstáculo se moviera de ida y vuelta continuamente, otro podría encargarse de la IA de los personajes.

Múltiples systems pueden actuar sobre una sola entity. Por ejemplo, un personaje no jugador podría moverse por su cuenta según una IA, pero también verse afectado por la gravedad cuando accidentalmente camina desde un acantilado. En ese escenario, los systems de física y de IA ni siquiera necesitan saber el uno del otro. Reevalúan de forma independiente su estado actual en cada tick del game loop e implementan su propia lógica por separado.

## La función del system

La función de un system se ejecuta periódicamente, una vez por cada tick del game loop. Esto ocurre automáticamente; no necesitas llamar explícitamente a esta función desde ningún lugar de tu código.

En una scene de Decentraland, puedes pensar en el game loop como la agregación de todas las functions de system de tu scene.


> [!WARNING]
> **📔 Nota**: No puedes añadir la misma función de system al engine más de una vez. Si intentas añadir un system que ya fue añadido, el engine lanza un error: `System "<name>" ya fue añadido al engine`.


## Manejar entities por referencia

Algunos components y systems están pensados para usarse solo en una entity de la scene. Por ejemplo, en una entity que almacena la puntuación de un juego o quizá una puerta principal que es única en la scene. Para acceder a una de esas entities dentro de un system, puedes simplemente referirte a la entity o a sus components por nombre en las functions del system.

```ts
// declarar la referencia de la entity en el ámbito del módulo para que los systems puedan acceder a ella
let game: Entity

export function main(){
	// crear una nueva entity
	game = engine.addEntity()

	// añadir component a esa entity
	ScoreComponent.create(game)
}

// Definir el system
export function UpdateScore() {

  // llamar a la referencia de la entity individual
  const points = ScoreComponent.get(game).points
  console.log(points)
}

// Añadir system al engine
engine.addSystem(UpdateScore)
```

Para proyectos más grandes, recomendamos que mantengas las definiciones de system en archivos separados de la instanciación de entities y components.

## Recorrer una component query

Muchas veces, tu scene tendrá múltiples entities del mismo tipo que tendrán comportamientos similares. Por ejemplo, muchas puertas que se pueden abrir, o muchos enemigos que pueden atacar al jugador. Tiene sentido manejar todas estas entities similares en un solo system, iterando sobre la lista y realizando las mismas comprobaciones en cada una.

No quieres que la función de un system itere sobre *todo el* conjunto de entities de la scene, ya que esto podría ser muy costoso en términos de poder de procesamiento. Para evitar esto, puedes [consultar components](/docs/creator/content-creator-es/scenes-sdk7/arquitectura/querying-components/), para iterar solo sobre las entities relevantes.

Por ejemplo, tu scene puede tener un `PhysicsSystem` que calcula el efecto de la gravedad sobre las entities de tu scene. Algunas entities de tu scene, como los árboles, no están pensadas para moverse nunca; así que sería inteligente evitar calcular los efectos de la gravedad sobre ellas. Puedes definir un `HasPhysics` component para marcar entities que podrían verse afectadas por la gravedad, y luego hacer que `PhysicsSystem` solo trate con las entities devueltas por esta query.

```ts
// Definir el system
export function PhysicsSystem() {
  // iterar sobre todas las entities con un HasPhysics
  for (const [entity] of engine.getEntitiesWith(HasPhysics)) {

  // obtener un componente Transform mutable
  const transform = Transform.getMutable(entity)

  // Calcular el efecto de la física
  }
}

// Añadir system al engine
engine.addSystem(PhysicsSystem)
```

## Tiempo delta entre frames

La función de un system puede incluir opcionalmente un argumento llamado `dt`, de tipo `number` (que representa *delta time*).

```ts
function MySystem(dt: number) {

  // Actualizar scene
  console.log("tiempo desde el último tick: ", dt)
}

engine.addSystem(MySystem)
```

*delta time* representa el tiempo transcurrido desde el último tick del game loop, en segundos.

El `dt` el argumento es el tiempo real que ha transcurrido desde el último tick del game loop, medido en segundos. El SDK no impone una tasa de ticks fija: el runtime anfitrión pasa este valor, y varía con el framerate de la scene.

Cuanto más fluida se ejecute una scene, con más frecuencia ocurren los ticks y menor es cada `dt` valor. Cuando un tick tarda más en procesarse, pasa más tiempo antes del siguiente y `dt` aumenta en consecuencia.

La tasa de frames del lado de la scene está limitada a 30 ticks por segundo, en cuyo caso el valor de `dt` es *1/30* (0.0333...).

![](/docs/_remote/d1bc4cb88306d00b1e56431f.webp)

Cuando el procesamiento de un frame tarda más, el dibujo de ese frame se retrasa, y el `dt` que se pasa en el siguiente tick refleja ese intervalo más largo. El engine no compensa el retraso de forma retroactiva; `dt` siempre informa del tiempo transcurrido real para que tus systems puedan ajustarse a él.

![](/docs/_remote/5ee6f60ba5826b88ac1603b7.webp)

Idealmente, deberías evitar que tu scene pierda frames, ya que esto afecta la calidad de la experiencia del jugador. Dado que esto depende de la potencia de procesamiento de la máquina del jugador, siempre existe la posibilidad de que tu scene deba estar preparada para manejarlo con gracia.

El `dt` variable es útil cuando el procesamiento del frame supera el tiempo predeterminado. Suponiendo que el frame actual tardará tanto como el anterior, esta información puede usarse para calcular cuánto ajustar un cambio gradual, de modo que la tasa de cambio parezca constante y proporcional al retraso entre frames.

Consulta [posicionamiento de entities](/docs/creator/content-creator-es/scenes-sdk7/fundamentos-del-contenido-3d/entity-positioning/) para ver ejemplos de cómo usar `dt` para hacer que el movimiento sea más suave.

## Recorrer en un intervalo temporizado

Si quieres que un system ejecute algo a un intervalo de tiempo regular, puedes hacerlo combinando el `dt` argumento con un temporizador.

```ts
let timer: number = 10

function LoopSystem(dt: number) {
  timer -= dt
  if (timer <= 0) {
      timer = 10
      // HACER ALGO
    }
}

engine.addSystem(LoopSystem)
```

También existe una función atajo `setInterval` y `clearInterval` que usa Systems en segundo plano. Esto permite una versión más fácil y corta cuando la intención es ejecutar una determinada función cada X cantidad de tiempo.

```ts
import { timers } from '@dcl/sdk/ecs'

const intervalId = timers.setInterval(() => {
    console.log('Imprimiendo esto cada 10 segundos')
}, 10000)
```

Donde el primer argumento, `callback`, es la función que se va a ejecutar (en este caso, la `console.log()`), y el segundo argumento, `ms`(10000 en este caso), es la cantidad de milisegundos que hay que esperar entre cada ejecución de la función.

Para detener el `setInterval` function, `clearInterval` se usa.

```
timers.clearInterval(intervalId)
```

Donde `intervalId` es la referencia al `setInterval` return definido antes.

Para casos de uso más complejos, donde puede haber múltiples retrasos y loops creados dinámicamente, puede valer la pena definir un component personalizado para almacenar un valor de temporizador individual para cada entity. Ver [Custom components](/docs/creator/content-creator-es/scenes-sdk7/arquitectura/custom-components/).

## Orden de ejecución de Systems

En algunos casos, cuando tienes múltiples systems ejecutándose, puede importarte qué system se ejecuta primero en tu scene.

Por ejemplo, podrías tener un *física* system que actualiza la posición de entities en la scene, y otro *boundaries* system que asegura que ninguna de las entities se posicione fuera de los límites de la scene. En este caso, quieres asegurarte de que el *boundaries* system se ejecute al final. De lo contrario, el *física* system podría mover entities fuera de los límites de la scene, pero el *boundaries* system no se enterará hasta que se ejecute de nuevo en el siguiente frame.

Al añadir un system al engine, establece un `prioridad` campo opcional para determinar cuándo se ejecuta el system en relación con otros systems.

```ts
engine.addSystem(PhysicsSystem, 5)
engine.addSystem(BoundariesSystem, 1)
```

Los systems con un número de prioridad más alto se ejecutan primero, así que un system con una prioridad de *5* se ejecuta antes que uno con prioridad *1*.

A los systems a los que no se les asigna una prioridad explícita se les asigna una prioridad predeterminada de *100000*. Como los números más altos se ejecutan primero, estos systems se ejecutan antes que los que tienen una prioridad explícita más baja.

Si dos systems tienen el mismo número de prioridad, no hay forma de saber con certeza cuál de ellos se ejecutará primero.

## Eliminar un system

Una instancia de un system puede añadirse o eliminarse del engine para encenderla o apagarla.

Si se define un system pero no se añade al engine, su función no es llamada por el engine.

Para eliminar un system, primero debes darle un nombre al añadirlo al engine, para poder referirte al system más tarde.

```ts
// declarar system
function mySystem(dt: number){
  console.log("retraso desde el último tick: ", dt)
}

// añadir system (dándole una prioridad y un nombre)
engine.addSystem(mySystem, 1, "DelaySystem")

// eliminar system
engine.removeSystem("DelaySystem")
```

La cadena que pasas a `engine.removeSystem()` debe coincidir con el nombre que asignaste al system cuando lo añadiste.

Otra forma de eliminar un system es pasar la propia function del system a `engine.removeSystem()` el método. De esta forma no necesitas darle un nombre al system cuando lo añades.

```ts
// declarar system
function mySystem(dt: number){
  console.log("retraso desde el último tick: ", dt)
}

// añadir system
engine.addSystem(mySystem)

// eliminar system pasando su function
engine.removeSystem(mySystem)
```

`engine.removeSystem()` acepta ya sea la cadena de nombre que asignaste al añadir el system, o una referencia a la propia function del system.

Puedes usar el método de abajo para hacer que un system se termine a sí mismo cuando su propósito se complete.

```ts
   const mySystem = function(dt: number){
        time += dt
        if(time > 3){
		engine.removeSystem(mySystem)
        }
    }
    engine.addSystem(mySystem)
```


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-es/scenes-sdk7/arquitectura/systems.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
