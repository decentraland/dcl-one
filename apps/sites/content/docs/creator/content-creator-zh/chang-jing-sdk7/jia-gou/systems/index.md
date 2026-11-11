> 了解系统如何用于更新场景状态

# 系统

Decentraland 场景依赖于 *系统* 随时间更新任何数据，包括存储在每个实体中的信息 [组件](/docs/creator/content-creator-zh/chang-jing-sdk7/jia-gou/entities-components/).

![](/docs/_remote/0b852c135fff08d1f4c9243d.webp)

*系统* 它们让场景具有动态性；它们是在场景游戏循环的每个 tick 上定期执行的函数，会改变将要渲染的内容。

以下示例展示了一个基本的系统声明：

```ts
// 定义系统
function mySystem() {
  console.log("Performed on every tick. My system is running")
}
// 将系统添加到引擎
engine.addSystem(mySystem)
```

系统中的函数可以执行你想要的任何操作。通常，它会作用于所有满足某些 [查询](/docs/creator/content-creator-zh/chang-jing-sdk7/jia-gou/querying-components/)，并按照特定逻辑更改存储在实体组件中的值。

```ts
function moveSystem(dt: number) {
  // 遍历所有具有 Transform 的实体
  for (const [entity] of engine.getEntitiesWith(Transform)) {

  // 获取一个可变的 Transform 组件
  const transform = Transform.getMutable(entity)

  // 更新位置值
    transform.position.z += 0.01
  }
}

engine.addSystem(moveSystem)
```

在上面的示例中，系统 `MoveSystem` 是一个在游戏循环的每个 tick 上运行的函数，会改变场景中每个具有 Transform 的实体的位置。

![](/docs/_remote/7e85b65f2626d96c8f892499.webp)

你可以在场景中使用多个系统来解耦不同的行为，使代码更简洁，并且更易于扩展和复用。例如，一个系统可以处理物理，另一个系统可以让障碍实体持续来回移动，另一个则可以处理角色的 AI。

多个系统可以作用于同一个实体。例如，一个非玩家角色可能会基于 AI 自主移动，但如果不小心走下悬崖，也会受到重力影响。在这种情况下，物理和 AI 系统甚至不需要知道彼此的存在。它们会在游戏循环的每个 tick 独立重新评估当前状态，并分别实现各自的逻辑。

## 系统函数

系统的函数会周期性执行，每个游戏循环 tick 执行一次。这会自动发生，你不需要在代码中的任何地方显式调用这个函数。

在 Decentraland 场景中，你可以把游戏循环理解为场景中所有系统函数的汇总。


> [!WARNING]
> **📔 注意**：你不能将同一个系统函数添加到引擎中超过一次。如果你尝试添加一个已添加过的系统，引擎会抛出错误： `系统“<name>”已添加到引擎中`.


## 通过引用处理实体

某些组件和系统只适合在场景中的一个实体上使用。例如，存储游戏分数的实体，或者场景中独一无二的主门。要在系统中访问这些实体之一，你只需在系统函数中通过名称引用该实体或其组件。

```ts
// 在模块作用域声明实体引用，以便系统可以访问它
let game: Entity

export function main(){
	// 创建一个新实体
	game = engine.addEntity()

	// 向该实体添加组件
	ScoreComponent.create(game)
}

// 定义系统
export function UpdateScore() {

  // 调用对单个实体的引用
  const points = ScoreComponent.get(game).points
  console.log(points)
}

// 将系统添加到引擎
engine.addSystem(UpdateScore)
```

对于较大的项目，我们建议将系统定义与实体和组件的实例化放在不同的文件中。

## 遍历组件查询

很多时候，你的场景里会有多个同类型实体，并且它们具有相似的行为。例如，许多可以打开的门，或者许多可以攻击玩家的敌人。将这些相似实体统一交给一个系统处理是很合理的，对列表进行迭代，并对每个实体执行相同的检查。

你不希望系统函数遍历 *整个* 场景中的实体集合，因为这在处理能力方面可能代价很高。为避免这种情况，你可以 [查询组件](/docs/creator/content-creator-zh/chang-jing-sdk7/jia-gou/querying-components/)，只遍历相关实体。

例如，你的场景可以有一个 `PhysicsSystem` 来计算重力对场景中实体的影响。你场景中的某些实体，比如树木，并不应该移动；因此，最好避免计算重力对这些实体的影响。你可以定义一个 `HasPhysics` 组件来标记可能受重力影响的实体，然后让 `PhysicsSystem` 只处理此查询返回的实体。

```ts
// 定义系统
export function PhysicsSystem() {
  // 遍历所有具有 HasPhysics 的实体
  for (const [entity] of engine.getEntitiesWith(HasPhysics)) {

  // 获取一个可变的 Transform 组件
  const transform = Transform.getMutable(entity)

  // 计算物理效果
  }
}

// 将系统添加到引擎
engine.addSystem(PhysicsSystem)
```

## 帧之间的时间差

系统中的函数可以选择性地包含一个名为 `dt`、类型为 `number` （表示 *delta 时间*).

```ts
function MySystem(dt: number) {

  // 更新场景
  console.log("time since last tick: ", dt)
}

engine.addSystem(MySystem)
```

*delta 时间* 表示自游戏循环上一次 tick 以来经过的时间，单位为秒。

该 `dt` 该参数是自游戏循环上一次 tick 以来经过的真实时间，以秒为单位。SDK 不强制固定的 tick 速率：宿主运行时会传递这个值，而它会随着场景的帧率而变化。

场景运行得越流畅，tick 出现得就越频繁，而每个 `dt` 值就越小。当某个 tick 的处理耗时更长时，在下一个 tick 到来之前会经过更多时间，并且 `dt` 会相应变大。

该场景侧帧率上限为每秒 30 个 tick，在这种情况下， `dt` 是 *1/30* (0.0333...).

![](/docs/_remote/d1bc4cb88306d00b1e56431f.webp)

当一帧的处理时间更长时，该帧的绘制会被延迟，而 `dt` 在下一个 tick 中传递的值会反映出这个更长的间隔。引擎不会事后补偿这一延迟； `dt` 始终报告实际经过的时间，以便你的系统据此调整。

![](/docs/_remote/5ee6f60ba5826b88ac1603b7.webp)

理想情况下，你应避免场景掉帧，因为这会影响玩家体验的质量。由于这取决于玩家机器的处理能力，你的场景始终可能需要能够优雅地处理这种情况。

该 `dt` 当帧处理超过默认时间时，变量很有用。假设当前帧会花费与上一帧相同的时间，这些信息可用于计算需要对渐变变化调整多少，以便变化速率看起来平稳，并与帧之间的延迟成比例。

参见 [实体定位](/docs/creator/content-creator-zh/chang-jing-sdk7/3d-nei-rong-ji-chu/entity-positioning/) 有关如何使用 `dt` 使移动更平滑。

## 按时间间隔循环

如果你想让系统以固定时间间隔执行某些操作，可以通过结合 `dt` 参数和计时器来实现。

```ts
let timer: number = 10

function LoopSystem(dt: number) {
  timer -= dt
  if (timer <= 0) {
      timer = 10
      // 执行某些操作
    }
}

engine.addSystem(LoopSystem)
```

还有一个快捷函数 `setInterval` 和 `clearInterval` 它在后台使用系统。这在你希望每隔 X 时间运行某个函数时，提供了更简单、更简短的写法。

```ts
import { timers } from '@dcl/sdk/ecs'

const intervalId = timers.setInterval(() => {
    console.log('每 10 秒打印一次')
}, 10000)
```

其中第一个参数， `回调函数`，是要执行的函数（在本例中是 `console.log()`），而第二个参数， `毫秒`（本例中为 10000），是每次函数执行之间要等待的毫秒数。

要停止 `setInterval` 函数， `clearInterval` 则使用。

```
timers.clearInterval(intervalId)
```

其中 `intervalId` 是对 `setInterval` 之前定义的返回值的引用。

对于更复杂的用例，当可能动态创建多个延迟和循环时，或许值得定义一个自定义组件，为每个实体存储一个单独的计时器值。参见 [自定义组件](/docs/creator/content-creator-zh/chang-jing-sdk7/jia-gou/custom-components/).

## 系统执行顺序

在某些情况下，当你运行多个系统时，你可能会关心场景中哪个系统先执行。

例如，你可能有一个 *物理* 用于更新场景中实体位置的系统，以及另一个 *边界* 用于确保没有任何实体被放置在场景边界之外的系统。在这种情况下，你需要确保 *边界* 系统最后执行。否则， *物理* 系统可能会把实体移到场景范围之外，但 *边界* 系统要到下一帧再次执行时才会发现。

在将系统添加到引擎时，设置一个可选的 `优先级` 字段，以确定该系统相对于其他系统的执行时机。

```ts
engine.addSystem(PhysicsSystem, 5)
engine.addSystem(BoundariesSystem, 1)
```

优先级数值更高的系统会先执行，因此优先级为 *5* 的系统会先于优先级为 *1*.

未显式指定优先级的系统默认优先级为 *100000*。由于数值更高的会先运行，因此这些系统会在显式优先级更低的系统之前执行。

如果两个系统具有相同的优先级数值，就无法确定它们中哪一个会先执行。

## 移除系统

系统实例可以被添加到引擎中或从引擎中移除，以启用或停用它。

如果系统已定义但未添加到引擎，它的函数就不会被引擎调用。

要移除系统，你必须先在将其添加到引擎时给它命名，以便之后可以引用该系统。

```ts
// 声明系统
function mySystem(dt: number){
  console.log("delay since last tick: ", dt)
}

// 添加系统（为其指定优先级和名称）
engine.addSystem(mySystem, 1, "DelaySystem")

// 移除系统
engine.removeSystem("DelaySystem")
```

你传递给 `engine.removeSystem()` 的字符串必须与添加系统时分配给它的名称一致。

移除系统的另一种方式，是将系统函数本身传递给 `engine.removeSystem()` 方法。这样在添加系统时就不需要给它命名。

```ts
// 声明系统
function mySystem(dt: number){
  console.log("delay since last tick: ", dt)
}

// 添加系统
engine.addSystem(mySystem)

// 通过传入其函数来移除系统
engine.removeSystem(mySystem)
```

`engine.removeSystem()` 可以接受你在添加系统时指定的名称字符串，也可以接受系统函数本身的引用。

你可以使用下面的方法，让系统在其目的完成后自行终止。

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
GET https://docs.decentraland.org/creator/content-creator-zh/chang-jing-sdk7/jia-gou/systems.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
