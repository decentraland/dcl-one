> Learn when and how to run asynchronous functions in your scene's code.

# Async Functions

## Overview

Most of the code in your scene runs synchronously using a single thread. That means that commands are executed sequentially line by line. Each command must first wait for the previous command to finish executing before it can start.

Even the functions in your scene's systems are executed one by one, following a [priority order](/docs/creator/sdk7/architecture/systems/#system-execution-order).

Running code synchronously ensures consistency, as you can always be sure you'll know the order in which the commands in your code run.

On the other hand, your scene needs to be updated many times per second, building the next frame. If a part of your code takes too long to respond, then the whole main thread is stuck and this results in lagging frame rates.

That's why, in some cases you want some commands to run asynchronously. This means that you can start off a task, and meanwhile the scene can keep running over the the next lines of code. The scene still runs everything on a single thread, but while the asynchronous task waits for a response, the rest of the code isn't blocked.

This is especially useful for tasks that rely on external services that could take time to respond, as you don't want that idle time waiting for that response to block other tasks.

For example:

* When retrieving data from a REST API
* When performing a transaction on the blockchain


> [!WARNING]
> **📔 Note**: Keep in mind that several frames of your scene might be rendered before the task finishes executing. Make sure your scene's code is flexible enough to handle the in-between scenarios while the asynchronous task is being completed.


## Run an async function

Mark any function as `async` so that it runs asynchronously, without blocking the rest of the scene's code while it waits.

```ts
// declare async function
async function myAsyncTask() {
	// run function's steps
}

// call async function
myAsyncTask()

// rest of the code keeps being executed
```

## The executeTask function

The `executeTask()` function executes a lambda function asynchronously. `executeTask()` allows us to declare and execute the function all in one same statement.

```ts
executeTask(async () => {
	let data = await myAsyncTask()
	console.log(data)
})

// rest of the code keeps being executed
```

## The then function

The `then` function takes in a lambda function as an argument, that only gets executed once the prior statement is finished. This lambda function can optionally have inputs that are mapped from whatever the prior statement returns.

```ts
myAsyncTask().then((data) => {
	console.log(data)
})
```


> [!WARNING]
> **📔 Note**: It's generally better to use the `executeTask` approach rather than the `then` function. If you rely too much on the `then` function at multiple nested levels, you can end up with what's known as "callback hell", where the code can become very hard to read and maintain.


## PointerEvents and RayCast functions

When your scene uses a `PointerEvent` or a `RayCast` component, the calculations of collisions are carried out async in the engine. The engine then returns a results event to the scene, which can arrive one or several ticks of the game loop later than when the event was invoked.

You then need to create a system to process these results in the frame when they arrive.


> [!WARNING]
> **📔 Note**: If you handle clicks via the [**Register a callback**](/docs/creator/sdk7/interactivity/button-events/register-callback/) approach, you don't need to explicitly create a system to handle this, but the same occurs in the background.


See [click events](/docs/creator/sdk7/interactivity/button-events/click-events/) and [raycasting](/docs/creator/sdk7/interactivity/raycasting/).


> [!NOTE]
> **💡 Tip**: If the processing of the results of a raycast takes a lot of calculations (like running a path-finding algorithm) you might want to run that computation in an asynchronous function.


## The await statement

An `await` statement forces the execution to wait for a response before moving over to the next line of code. `await` statements can only be used inside an async block of code.

```ts
// declare function
async function myAsyncTask() {
	try {
		let response = await fetch(callUrl)
		let json = await response.json()
		console.log(json)
	} catch {
		console.log('failed to reach the URL')
	}
}

// call function
myAsyncTask()

// Rest of the code keeps being executed
```

The example above executes a function that includes a `fetch()` operation to retrieve data from an external API. The `fetch()` operation is asynchronous, as we can't predict how long the server will take to respond. However, the next line needs the output of this operation to be ready before we can parse it as a json. The `await` statement here ensures that the next line will only run once that operation has returned a value. Similarly, the `response.json()` function is also asynchronous, but the next line needs the json to be parsed before it can log it. The second `await` statement forces the next line to only be called once the parsing of the json is finished, however long it takes.


## Set a Timeout for a Function call

Use `setTimeout` to wait some time before certain lines of code are run. This takes two arguments:

- The function to execute
- The amount of miliseconds to wait before executing that function

 
The example below waits 1000 milliseconds (equal to 1 second) before executing a simple function that logs a message to console.

```ts
import { timers } from '@dcl/sdk/ecs'

console.log('This gets printed right away')

timers.setTimeout(() => {
	// function to run after delay
    console.log('This gets printed after 1 second')
}, 1000)
```

The `clearTimeout` can be used to cancel the execution of a `setTimeout` function that is still waiting to be executed. `setTimeout` returns a timer id (a number), that you can then pass to `clearTimeout`. In this case, the variable `timeoutId` is obtained when doing the `setTimeout`, and then passed to `clearTimeout` to cancel it.

```ts
import { timers } from '@dcl/sdk/ecs'

const timeoutId = timers.setTimeout(() => {
    console.log('Wait 1 second until executing this function')
}, 1000)

timers.clearTimeout(timeoutId)
```
