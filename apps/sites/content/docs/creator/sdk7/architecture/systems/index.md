> Learn how systems are used to update the scene state

# Systems

Decentraland scenes rely on _systems_ to update any data over time, including information stored in each entity's [components](/docs/creator/sdk7/architecture/entities-components/).

![](/docs/creator/images/media/ecs-big-picture.webp)

_systems_ are what make scenes dynamic, they're functions that are executed periodically on every tick of the scene's game loop, changing what will be rendered.

The following example shows a basic system declaration:

```ts
// Define the system
function mySystem() {
  console.log("Performed on every tick. My system is running")
}
// Add system to engine
engine.addSystem(mySystem)
```

The function in a system can perform anything you want. Typically, it will act upon all the entities that meet certain [query](/docs/creator/sdk7/architecture/querying-components/), following certain logic to change the values stored in the entity's components.

```ts
function moveSystem(dt: number) {
  // iterate over all entiities with a Transform
  for (const [entity] of engine.getEntitiesWith(Transform)) {

  // fetch a mutable Transform component
  const transform = Transform.getMutable(entity)

  // update the position value
    transform.position.z += 0.01
  }
}

engine.addSystem(moveSystem)
```

In the example above, the system `MoveSystem` is a function that runs on each tick of the game loop, changing position of every entity in the scene that has a Transform.

![](/docs/creator/images/media/ecs-system-new.webp)

You can have multiple systems in your scene to decouple different behaviors, making your code cleaner and easier to scale and reuse. For example, one system might handle physics, another might make an obstacle entity move back and forth continuously, another could handle the AI of characters.

Multiple systems can act on a single entity. For example a non-player character might move on its own based on an AI, but also be affected by gravity when accidentally walking from off a cliff. In that scenario, the physics and the AI systems don't even need to know about each other. They independently reassess their current state on each tick of the game loop and implement their own separate logic.

## The system function

A system's function is executed periodically, once per every tick of the game loop. This happens automatically, you don't need to explicitly call this function from anywhere in your code.

In a Decentraland scene, you can think of the game loop as the aggregation of all the system functions in your scene.


> [!WARNING]
> **📔 Note**: You can't add the same system function to the engine more than once. If you try to add a system that was already added, the engine throws an error: `System "<name>" already added to the engine`.


## Handle entities by reference

Some components and systems are meant for using only on one entity in the scene. For example, on an entity that stores a game's score or perhaps a main gate that is unique in the scene. To access one of those entities within a system, you can simply refer to the entity or its components by name in the system's functions.

```ts
// declare the entity reference at module scope so systems can access it
let game: Entity

export function main(){
	// create a new entity
	game = engine.addEntity()

	// add component to that entity
	ScoreComponent.create(game)
}

// Define the system
export function UpdateScore() {

  // call reference to individual entity
  const points = ScoreComponent.get(game).points
  console.log(points)
}

// Add system to engine
engine.addSystem(UpdateScore)
```

For larger projects, we recommend that you keep system definitions on separate files from the instancing of entities and components.

## Loop over a component query

A lot of times, your scene will have multiple entities of the same type that will have similar behaviors. For example many doors that can be opened, or many enemies that can attack the player. It makes sense to handle all of these similar entities in a single system, iterating over the list and performing the same checks on each.

You don't want a system's function to iterate over _the entire_ set of entities in the scene, as this could be very costly in terms of processing power. To avoid this, you can [query components](/docs/creator/sdk7/architecture/querying-components/), to only iterate over the relevant entities.

For example, your scene can have a `PhysicsSystem` that calculates the effect of gravity over the entities of your scene. Some entities in your scene, such as trees, are not meant to ever move; so it would be smart to avoid calculating the effects of gravity on these. You can define a `HasPhysics` component to mark entities that could be affected by gravity, and then have `PhysicsSystem` only deal with the entities returned by this query.

```ts
// Define the system
export function PhysicsSystem() {
  // iterate over all entiities with a HasPhysics
  for (const [entity] of engine.getEntitiesWith(HasPhysics)) {

  // fetch a mutable Transform component
  const transform = Transform.getMutable(entity)

  // Calculate effect of physics
  }
}

// Add system to engine
engine.addSystem(PhysicsSystem)
```

## Delta time between frames

The function in a system can optionally include an argument called `dt`, of type `number` (representing _delta time_).

```ts
function MySystem(dt: number) {

  // Udate scene
  console.log("time since last tick: ", dt)
}

engine.addSystem(MySystem)
```

_delta time_ represents time that passed since the last tick of the game loop, in seconds.

The `dt` argument is the real time that elapsed since the last tick of the game loop, measured in seconds. The SDK doesn't enforce a fixed tick rate: the host runtime passes this value, and it varies with the scene's framerate.

The smoother a scene runs, the more frequently ticks occur and the smaller each `dt` value is. When a tick takes longer to process, more time passes before the next one and `dt` grows accordingly.

This scene-side frame rate is caped at 30 ticks per second, in which case the value of `dt` is _1/30_ (0.0333...).

![](/docs/creator/images/media/ecs-framerate.webp)

When the processing of a frame takes longer, the drawing of that frame is delayed, and the `dt` passed on the next tick reflects that longer interval. The engine doesn't compensate for the delay retroactively; `dt` always reports the actual elapsed time so your systems can adjust to it.

![](/docs/creator/images/media/ecs-framerate-heavy.webp)

Ideally, you should avoid your scene dropping frames, as it impacts the quality of the player's experience. Since this is dependant on the processing power of the player's machine, it's always a possibility that your scene should be ready to handle gracefully.

The `dt` variable is useful when frame processing exceeds the default time. Assuming that the current frame will take as much time as the previous one, this information may be used to calculate how much to adjust a gradual change, so that the rate of change appears steady and in proportion to the lag between frames.

See [entity positioning](/docs/creator/sdk7/3d-essentials/entity-positioning/) for examples of how to use `dt` to make movement smoother.

## Loop at a timed interval

If you want a system to execute something at a regular time interval, you can do this by combining the `dt` argument with a timer.

```ts
let timer: number = 10

function LoopSystem(dt: number) {
  timer -= dt
  if (timer <= 0) {
      timer = 10
      // DO SOMETHING
    }
}

engine.addSystem(LoopSystem)
```

There is also a shortcut function `setInterval` and `clearInterval` that uses Systems on the background. This allows an easier and shorter version when the intention is to run a certain function every X amount of time.

```ts
import { timers } from '@dcl/sdk/ecs'

const intervalId = timers.setInterval(() => {
    console.log('Printing this every 10 seconds')
}, 10000)
```

Where the first argument, `callback`, is the function to be executed (in this case, the `console.log()`), and the second argument, `ms`(10000 in this case), is the miliseconds to wait between each function execution.

To stop the `setInterval` function, `clearInterval` is used.

```
timers.clearInterval(intervalId)
```

Where `intervalId` is the reference to the `setInterval` return defined before.

For more complex use cases, where there may be multiple delays and loops being created dynamically, it may be worth defining a custom component to store an individual timer value for each entity. See [Custom components](/docs/creator/sdk7/architecture/custom-components/).

## System execution order

In some cases, when you have multiple systems running, you might care about what system is executed first by your scene.

For example, you might have a _physics_ system that updates the position of entities in the scene, and another _boundaries_ system that ensures that none of the entities are positioned outside the scene boundaries. In this case, you want to make sure that the _boundaries_ system is executed last. Otherwise, the _physics_ system could move entities outside the bounds of the scene but the _boundaries_ system won't find out till it's executed again in the next frame.

When adding a system to the engine, set an optional `priority` field to determine when the system is executed in relation to other systems.

```ts
engine.addSystem(PhysicsSystem, 5)
engine.addSystem(BoundariesSystem, 1)
```

Systems with a higher priority number are executed first, so a system with a priority of _5_ is executed before one of priority _1_.

Systems that aren't given an explicit priority have a default priority of _100000_. Since higher numbers run first, these systems are executed before ones with a lower explicit priority.

If two systems have the same priority number, there's no way to know for certain which of them will be executed first.

## Remove a system

An instance of a system can be added or removed from the engine to turn it on or off.

If a system is defined but isn't added to the engine, its function isn't called by the engine.

To remove a system, you must first give it a name when adding it to the engine, so that you can refer to the system later.

```ts
// declare system
function mySystem(dt: number){
  console.log("delay since last tick: ", dt)
}

// add system (giving it a priority and name)
engine.addSystem(mySystem, 1, "DelaySystem")

// remove system
engine.removeSystem("DelaySystem")
```

The string you pass to `engine.removeSystem()` must match the name you assigned to the system when you added it.

Another way to remove a system is to pass the system's function itself to the `engine.removeSystem()` method. This way you don't need to give the system a name when adding it.

```ts
// declare system
function mySystem(dt: number){
  console.log("delay since last tick: ", dt)
}

// add system
engine.addSystem(mySystem)

// remove system by passing its function
engine.removeSystem(mySystem)
```

`engine.removeSystem()` accepts either the name string you assigned when adding the system, or a reference to the system's function itself.

You can use the method below to make a system self-terminate when its purpose is complete.

```ts
   const mySystem = function(dt: number){
        time += dt
        if(time > 3){
		engine.removeSystem(mySystem)
        }
    }
    engine.addSystem(mySystem)
```
