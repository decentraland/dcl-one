> 检测玩家何时进入或离开某个实体的接近范围。

# 接近事件

近距离交互让实体在玩家进入、靠近并大致正对它时响应按钮事件，而无需将光标瞄准该实体。

传统的指针事件使用射线投射：玩家必须先将光标对准实体的碰撞体，然后才能按下按钮触发事件。相比之下，近距离事件会检查位于角色周围给定区域内的实体，而不管玩家的光标正朝向哪里。

用于近距离交互的交互区域是一个从玩家位置向前投射的、位于球体中的宽三角形切片。与指针事件不同，角色面向的方向很重要，而与摄像机朝向无关。

若要设置更具体的自定义逻辑，你可能需要处理原始数据并使用 [高级](/docs/creator/content-creator-zh/chang-jing-sdk7/jiao-hu-xing/an-niu-shi-jian/advanced-button-events/) 方法。

一个实体要能交互，必须具有一个 [collider](/docs/creator/content-creator-zh/chang-jing-sdk7/3d-nei-rong-ji-chu/colliders/)。参见 [障碍物](/docs/creator/content-creator-zh/chang-jing-sdk7/jiao-hu-xing/an-niu-shi-jian/click-events/#obstacles) 以了解更多详情。

## 注册近距离回调

该 `pointerEventsSystem` 包含用于近距离事件的辅助函数，遵循与 [指针事件回调](/docs/creator/content-creator-zh/chang-jing-sdk7/jiao-hu-xing/an-niu-shi-jian/register-callback/).

### 近距离按钮按下

使用 `pointerEventsSystem.onProximityDown` 用于在玩家处于范围内时检测按钮按下。与 `onPointerDown`不同，这不需要玩家将光标瞄准该实体。

```ts
pointerEventsSystem.onProximityDown(
    {
        entity: myEntity,
        opts: {
            button: InputAction.IA_PRIMARY,
            hoverText: 'Press E',
            maxDistance: 5,
        },
    },
    function () {
        console.log('Player pressed button near entity')
    }
)
```

使用 `pointerEventsSystem.onProximityUp` 用于检测玩家在范围内释放按钮的时机。

```ts
pointerEventsSystem.onProximityUp(
    {
        entity: myEntity,
        opts: {
            button: InputAction.IA_PRIMARY,
            hoverText: 'Release E',
            maxDistance: 5,
        },
    },
    function () {
        console.log('Player released button near entity')
    }
)
```


> [!WARNING]
> **📔 注意**：每个 `onProximityDown` 和一个 `onProximityUp` 都只能为每个实体注册一个。添加后，它们会持续监听，直到被移除。不要在系统循环中调用它们，因为那样会不断重写行为。


## 近距离事件类型

已向 `PointerEventType` 枚举中使用：

* `PET_PROXIMITY_ENTER`：当玩家走入实体的近距离范围时触发。
* `PET_PROXIMITY_LEAVE`：当玩家移出实体的近距离范围时触发。

这些可以与 `PointerEvents` 组件以及 `inputSystem` 一起用于 [基于系统的方法](/docs/creator/content-creator-zh/chang-jing-sdk7/jiao-hu-xing/an-niu-shi-jian/system-based-events/#proximity-events)。该方法还支持使用 `InteractionType.PROXIMITY` 字段中，指向你自己的流媒体基础设施。

## 优先级

当多个实体都在范围内并且都可能响应同一输入时，只会激活一个，默认是最近的那个。使用 `优先级` 字段来控制哪个优先。具有 **更高数值** 作为优先级值的实体会先响应。

如果玩家既处于具有近距离交互的实体范围内，同时玩家的光标又指向了具有指针交互的实体，那么始终会优先选择具有指针交互的实体。再次说明，只会激活一个实体。

```ts
pointerEventsSystem.onProximityDown(
    {
        entity: doorEntity,
        opts: {
            button: InputAction.IA_PRIMARY,
            hoverText: 'Open door',
            maxDistance: 5,
            priority: 2,
        },
    },
    function () {
        console.log('Door activated')
    }
)

pointerEventsSystem.onProximityDown(
    {
        entity: floorEntity,
        opts: {
            button: InputAction.IA_PRIMARY,
            hoverText: 'Step here',
            maxDistance: 5,
            priority: 1,
        },
    },
    function () {
        console.log('Floor activated')
    }
)
```

在上面的示例中，如果两个实体都在范围内，门实体会响应，因为它的优先级更高。

该 `优先级` 字段也可以在 `PointerEvents` 组件，则抛出错误。

## 选项

近距离辅助函数接受与其指针对应项相同的选项：

* `按钮将你的钱包连接起来`：要监听的按钮。参见 [指针按钮](/docs/creator/content-creator-zh/chang-jing-sdk7/jiao-hu-xing/an-niu-shi-jian/click-events/#pointer-buttons) ，可选支持项请参见。
* `maxDistance`：从玩家的 **化身** 到实体的最大距离，单位为米。默认值为 10。
* `hoverText`：当玩家靠近实体时在界面中显示的文本。
* `showHighlight`：如果为 true，当玩家处于范围内时，会在实体上显示边缘高亮。 *true* 默认情况下。
* `showFeedback`：如果为 true，会显示悬停反馈，在实体中心周围悬停。 *true* 默认情况下。
* `优先级`：在多个实体处于近距离范围内时解决冲突。数值越高优先级越高。如果多个实体具有相同的优先级值，则会选择最近的一个。

`maxPlayerDistance` 是已弃用的别名，指向 `maxDistance`。两者都以化身为起点测量距离，因此在新的场景中请使用 `maxDistance` 。

### 什么算是在范围内

近距离事件的测量起点是 **玩家胶囊体的中心**，大致在胸口高度，到实体碰撞体上最近的点。这个位置大约比光标点击测量的起点高一米，因此同一个点在两种路径下读出的距离可能会略有不同。

仅有距离还不够。要触发近距离事件，玩家还必须：

* 处于 **面向** 该实体的位置，且位于其前方大约 120 度的锥形范围内。
* 拥有 **清晰的视线** ，中间不能有任何阻挡。


> [!WARNING]
> **📔 注意**：近距离事件会忽略 `maxCameraDistance`。摄像机所在位置不会产生影响。只会考虑角色的距离、朝向和视线。有关光标事件中摄像机限制的工作方式，请参见 [距离限制](/docs/creator/content-creator-zh/chang-jing-sdk7/jiao-hu-xing/an-niu-shi-jian/register-callback/#distance-limits) 。


## 移除回调

要移除近距离回调，请使用对应的移除函数：

```ts
pointerEventsSystem.removeOnProximityDown(myEntity)
pointerEventsSystem.removeOnProximityUp(myEntity)
pointerEventsSystem.removeOnProximityEnter(myEntity)
pointerEventsSystem.removeOnProximityLeave(myEntity)
```

移除后，该实体将不再对该近距离事件有响应。

## 近距离进入和离开

使用 `pointerEventsSystem.onProximityEnter` 用于在玩家走入实体的近距离范围时运行回调，以及 `pointerEventsSystem.onProximityLeave` 用于在玩家走出时运行回调。这可以用来向玩家显示额外的反馈提示，例如当物品可以交互时播放声音或动画。

```ts
pointerEventsSystem.onProximityEnter(
    {
        entity: myEntity,
        opts: {
            button: InputAction.IA_POINTER,
            hoverText: 'Nearby',
            maxDistance: 5,
        },
    },
    function () {
        console.log('Player entered proximity')
    }
)

pointerEventsSystem.onProximityLeave(
    {
        entity: myEntity,
        opts: {
            button: InputAction.IA_POINTER,
            hoverText: 'Nearby',
            maxDistance: 5,
        },
    },
    function () {
        console.log('Player left proximity')
    }
)
```

## 示例：近距离门

下面的示例会在玩家站在附近并按下按钮时打开或关闭一扇门，而无需把光标对准门。

```ts
const doorPivot = engine.addEntity()
Transform.create(doorPivot, { position: Vector3.create(3, 0, 4) })

const door = engine.addEntity()
GltfContainer.create(door, { src: 'assets/door.glb' })
Transform.create(door, { position: Vector3.create(-1, 0, 0), parent: doorPivot })

let isDoorOpen = false
const closedRot = Quaternion.fromEulerDegrees(0, 0, 0)
const openRot = Quaternion.fromEulerDegrees(0, 90, 0)

pointerEventsSystem.onProximityDown(
    {
        entity: door,
        opts: {
            button: InputAction.IA_PRIMARY,
            hoverText: 'Open / Close',
            maxDistance: 5,
            priority: 1,
        },
    },
    function () {
        if (isDoorOpen) {
            Tween.setRotate(doorPivot, openRot, closedRot, 700)
            isDoorOpen = false
        } else {
            Tween.setRotate(doorPivot, closedRot, openRot, 700)
            isDoorOpen = true
        }
    }
)
```


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-zh/chang-jing-sdk7/jiao-hu-xing/an-niu-shi-jian/proximity-events.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
