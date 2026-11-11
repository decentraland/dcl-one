> 面向数据编程是一种强大的编程方法，能最大限度发挥性能。

# 面向数据编程

数据导向编程是一种强大的编程方法，能够显著提升性能。它侧重于将 *数据* 作为核心元素，其他一切都围绕它组织，以便访问或修改这些数据。这种方法也非常适合多人游戏，因为它让玩家之间需要同步的数据更容易、更快速地访问。

数据导向编程鼓励你把场景中的一切都看作需要在各种系统之间被复制和修改的数据。这种方法的主要好处在于优化数据从内存中读取的速度，而这通常是运行现代应用和游戏时的主要瓶颈。

由于这种在性能上的显著提升，过去几年里，游戏开发行业的很大一部分都在转向采用这种方法。

Decentraland 的 SDK 在 JavaScript 中执行场景，而这种语言的一个缺点是它无法控制内存分配。不过，运行 Decentraland 的引擎使用的是 C#，而遵循数据导向原则对它有很大帮助。SDK 和引擎之间会不断互相发送消息。为了让这种通信尽可能高效，最好让两边的数据结构尽量相似，以避免不断重新组织这些数据。

## 它看起来是这样的

数据导向编程不同于面向对象编程，后者是许多开发者目前都很熟悉的一种方法。在面向对象编程中，代码会按照试图模拟现实世界构造的抽象来组织：对象。每个对象都可以同时包含数据和功能。采用这种方法的应用通常在概念上很容易规划，但运行起来也往往更低效。

在数据导向编程中，数据不是围绕对象来组织的，而是围绕如何优化访问便捷性来组织的。数据所代表的现实世界构造不会参与那些修改该数据的不同流程。

Decentraland SDK 所基于的实体组件系统（ECS）模型与数据导向编程方法非常契合。每个组件都是一个结构化数据集合的一部分。组件通过引用属于某个实体，但数据并不是围绕实体来组织的，而是作为一组相似组件来组织。例如，所有 `Transform` 场景中的组件都是平等的。其中一个变换组件可能属于你主建筑的模型，另一个可能属于桌子上的一个玻璃物体。然后场景中的系统会逐个处理这个 `Transform` 组件列表，不做任何区分。所有 `Transform` 组件都具有相同的字段，并经历相同的检查。

![](/docs/_remote/29703f638d6018eac3f30fbf.webp)

想象一个场景里有十几扇门，它们要么是打开的，要么是关闭的。你可以用一个简单的“isOpen”组件来表示所有这些门的状态，它保存一个布尔值。如果“isOpen”为 true，门就应该是打开的；如果为 false，门就应该是关闭的。如果玩家点击某扇门，它应该切换状态，其他玩家也应该能看到它发生切换。当你的场景在处理门状态的变化并与其他玩家同步时，场景其实并不关心“isOpen”具体代表什么。整套组件只是一个需要与其他玩家同步的布尔值集合。然后你场景中的另一个单独系统可以负责定期将每个“isOpen”的状态与其对应门的旋转同步起来。

数据导向编程并不一定更难，但它是一种需要学习并采用的不同方法。那些不习惯这种方法的开发者可能需要一些时间来熟悉它，我们鼓励你去探索并动手体验一些示例，以获得更好的感觉。

## 为什么它有效

要理解为什么数据导向编程能带来如此大的差异，我们需要看看硬件。

当处理器需要从内存中读取数据时，它会把一整块数据载入缓存，其中也包括刚好写在我们想要的那个值旁边的其他数据。你的代码越能利用已经在缓存中的数据，运行速度就越快。

想象机器的内存就像一个真实的仓库，数据被存放在许多堆叠的箱子里。每当我们需要某些数据时，就得叫来一辆叉车去取出存放这些数据的箱子，然后把它送到前台检查。前台一次只能放下几个箱子，所以你无法保留太多数据。

叉车要跑到仓库后面把箱子搬给我们，需要花很长时间。如果你想要的这些箱子里的数据是分散的，那就意味着要让叉车跑很多趟。大多数时候，你都会站在前台无所事事地等着，等待你请求的下一个箱子到来。

如果你能聪明地堆放这些箱子，并把那些你很可能会同时用到的数据打包在一起，就可以避免很多浪费的时间。如果你巧妙地分组数据，通常会发现下一个你需要的东西已经在前台的某个箱子里了。你可以直接开始使用，而不用打扰叉车司机。

例如，如果你的场景需要获取某扇门的状态来检查它是打开还是关闭，硬件并不只是读取描述该门状态的那个布尔值，它还会读取很多其他可能相关或不相关的数据。

假设你的场景里有一个系统，需要每一帧都更新十几扇门的开/关状态。如果你的代码是按照面向对象编程的方式组织的，就无法知道相关信息的不同部分会如何被分组。也许我们仓库里的一个“箱子”里装着门 A 的“isOpen”状态，同时也装着门 A 的材质，以及它在打开时播放的音频。你可能需要为这十几扇门中的每一扇都跑一趟去取一个新的数据“箱子”。当然，这整个过程还需要在每一帧都重新执行一次。这样一来，即使门都没有改变状态，每秒也要进行 360 次（30 x 12）比喻性的往返仓库后方的行程。

另一方面，如果你的代码遵循数据导向的方法，那么这 12 个布尔值很可能都会在同一个箱子里。这是因为这些布尔值都属于同一个一次性写入内存的数组。你从来不会显式地组织这些数据如何放入内存，所以在最坏情况下，这个数组可能会被拆分到两个箱子里。但即使在最坏的情况下，2 次往返也远比 12 次要好。

每一帧都检查每扇门的状态听起来像很多工作，但如果所有数据都已经在内存缓存中，那其实会非常快。如果你的数据组织得井井有条，你的场景就能对大量实体执行这类流程，同时依然运行得非常快。


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-zh/chang-jing-sdk7/jia-gou/data-oriented-programming.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
