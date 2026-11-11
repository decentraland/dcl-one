> 版本支持协议

# 版本支持协议

本文档描述了应用于 Creator Hub 中的 Scene Editor 以及脚本框架（ `@dcl/sdk` 软件包）。

此版本控制策略的目标，是在 Decentraland Foundation 的 SDK 团队与内容创作者之间建立一份契约，以明确预期并让内容创作者能够据此规划工作。

## 定义

* *版本号* ——一个公开可用的软件版本的唯一标识符。该标识符由一个 *主版本号* 和一个 *次版本号* 组成，并以点号分隔（例如 7.2）。
* *版本家族* ——所有主版本号相同的版本构成一个家族。若 A 和 B 属于同一家族，且 A 的次版本号高于 B，则称版本 A 为版本 B 的 *后继版本* （例如，7.11 是 7.10 的后继版本）。
* *破坏性变更* ——一种会迫使用户修改其代码或资产以保持其正常运行状态的变更。例如，一个属性更名了，用户就必须在代码中每次使用该属性名时都进行修改。
* *非破坏性变更* ——一种不需要用户采取任何行动的变更。用户代码和资产的行为及属性保持不变。
* *稳定版本* ——一种在其所有后继版本中都禁止破坏性变更的版本（其后继版本也都被视为稳定版本）。任何破坏性变更都必须仅通过递增主版本号来创建新的版本家族引入，不过请参见 [破坏性变更](#breaking-changes) 章节中的注意事项。非破坏性变更则通过递增次版本号来体现。
* *不稳定版本* ——一种允许其后继版本中出现破坏性变更的版本。破坏性变更可以通过递增次版本号引入。

## 支持政策

在每个稳定版本家族中，Decentraland Foundation 只支持最新的次版本号。在任何给定时间，最多只能有两个受支持的家族。

例如，如果 6.x 家族的最新次版本号是 6.11，而 7.x 家族的最新次版本号是 7.3，则内容创作者应在 6.11 或 7.3 上开发他们的场景。使用 6.10 或更早版本开发并发布的场景大概率仍然可以正常运行，玩家也仍能体验它们。不过，如果这些较旧的场景出现任何问题，必须先更新到受支持的版本，只有在问题在受支持版本中仍然存在时，才会进一步调查。

Scene Editor 会自动更新同一版本家族内所有场景的脚本包，因此所有开发者在开发场景时都使用最新受支持版本。

## 功能开发

新功能只会发布到正在开发中的最新版本中。一旦开发团队开始着手新的版本家族，仍处于支持期内的旧版本家族将只接受重大漏洞修复，不再实现任何额外功能。

## 破坏性变更

破坏性变更应只出现在主版本发布中。同一版本家族的稳定次版本发布中不应出现破坏性变更，除非在紧急情况下且别无他法。次版本发布中的破坏性变更是极端措施，开发者会尽一切可能避免。新的次版本发布会扩展现有语法的能力，但除修复漏洞外，绝不应改变既有语法所产生的结果。

### 局部变更

在极少数情况下，如果一个小的、局部的破坏性变更只会给一小部分用户带来不便，那么采用这种方式可能更合适。（创建一个新的主版本会给所有用户带来不便。）在这种情况下，SDK 可能会弃用某个功能，但必须在合理时间内继续支持它。

### 紧急变更

在某些特殊情况下，例如安全问题或监管要求，任何功能都可能在不考虑其稳定级别的情况下以破坏性方式更改，在这些情况下也不会承诺弃用期。

## 稳定版与不稳定版发布

### Alpha

每当引入一个新的主版本发布时，最初的几个次版本发布可能会被标记为不稳定 **alpha** 版本。在 alpha 发布中必须允许并预期出现破坏性变更，用户不应对其稳定性抱有任何期望。

开发者可以自由使用这些 alpha 版本进行试验，但不建议使用不稳定的 alpha 版本发布内容，因为无法保证在后续更改后内容仍能正常工作。此时也不建议开始大型复杂迁移，因为在下一个稳定版发布前可能还需要更多更改。

### Beta

一旦某个版本家族达到一定成熟度，就会提供一个 **beta** 发布版本。Beta 发布被视为已完成并准备好声明为稳定版，但前提是经过公开测试。

Beta 版本家族应尽可能稳定；不过，它们允许随时间发生变化。这些变化应尽量很小，但也可能包括破坏性变更。破坏性变更只能在合理的弃用期之后引入，以便内容创作者有机会迁移他们的场景。该弃用期必须在引入破坏性变更时定义。

Beta 版本家族应只在有限时间内处于 beta 状态，该时间在标记为 beta 时会被指定。如果在该期间未发现问题，应将其提升为稳定版。该时间长度可因情况而异，但一个经验法则是 90 天。

使用 beta 版本语法编写的内容，应当容易迁移到同一家族的后续版本。尽管如此，仍不建议使用 beta 版本为大型活动开发内容，因为测试仍在进行中，而且很可能存在错误。

### 稳定版

一旦 beta 期结束且未出现重大问题，该版本家族就被视为 **稳定** ，并且除新增功能外，不应再对语法进行任何进一步更改。从此时起，该版本被视为所有开发者推荐并鼓励使用的选项。

稳定版本家族在其整个生命周期内都必须获得完整支持。除下文所述的注意事项外，不得有任何破坏性变更。

## 稳定发布中的不稳定功能

某次发布中的特定功能，其稳定级别可能与整个发布版本不同。这可能是因为该功能最近才引入，需要更多测试；也可能是因为它注定很快会被替换。

例如，SDK 框架中一个已经稳定的发布版本，可以将一种新的组件类型以 alpha 形式引入，因为该特定组件可能仍需要其自身的测试周期。随后它可以经历上文所述的版本演进流程，从 alpha 到 beta，再到稳定版。

任何来自稳定发布版本但不被视为稳定的功能，都应在文档中明确标注。使用不稳定功能的创作者必须意识到，该功能可能会经历破坏性变更。任何破坏性变更都会被清楚地通知，包括迁移指南，并且会留出过渡期让创作者调整其场景代码。

## 我们会支持稳定版本家族多久？

一旦新的版本家族变为稳定版（7.x），团队会承诺在前一个版本（6.x）上继续支持（重大漏洞修复）数月，以便为创作者留出充足的迁移时间。具体时长会根据所需的迁移工作量按个案确定。

## 预发布版本

通过将 `@next` 版本的 `@dcl/sdk` 包安装到场景中，始终可以获取脚本框架的最新新增内容。

该分支中的功能可能是不稳定的或未记录的，因为它们并未作为官方受支持的 SDK 版本的一部分发布。


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-zh/chang-jing-sdk7/api-can-kao/version-support-agreement.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
