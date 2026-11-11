> 构成 Decentraland DAO 的不同平台和实体概览。

# DAO 如何运作

为了规避完全链上治理所带来的极高 gas 费用，Decentraland 的 DAO 结合了面向社区的免费链下投票，以及由“DAO 委员会”控制的多签钱包，用于在以太坊区块链上执行这些链下决定。这样，任何持有 MANA、NAMES 或 LAND 的人都可以参与 DAO，而无需在每次想要投票或发起提案时支付任何费用。

使用由一组受信任人士控制的多签钱包，既能保障 DAO 的安全，也能确保通过的提案会转化为链上操作。由 SAB 持有的第二个多签钱包在 DAO 委员会之上提供了第二层保护。

DAO 是一个由多层实体和平台构成的复杂系统，下面将逐一列出并说明：

## 治理 dApp

Decentraland DAO 的主界面位于 [governance.decentraland.org](https://governance.decentraland.org)。用户在这里登录、创建提案并投票。

*这目前由一个由……资助的开发团队维护* [*DAO 资助*](https://governance.decentraland.org/proposal/?id=ed53e850-5e70-11ec-8188-4352ce3d30e7)*.*

## Snapshot

[Snapshot](https://snapshot.org/#/) 是一个链下投票平台，为代币持有者提供免费方式对提案进行投票。通过将提案和投票都作为加密签名消息存储在 IPFS 上，Snapshot 能够实现安全且易于质疑的结果。

Decentraland 的 DAO 使用 Snapshot 来承载社区产生的提案和投票。

每当在 governance.decentraland.org 上发起一个提案时，它也会自动创建在 [Decentraland 的 Snapshot 空间](https://snapshot.org/#/snapshot.dcl.eth)。这使 DAO 能够以安全且去中心化的方式记录和存储提案、投票和结果——结果会显示回 governance.decentraland.org 上。用户还可以从 Snapshot 空间 [将他们的 VP 委托给](https://snapshot.org/#/delegate/snapshot.dcl.eth) 其他社区成员。

## Aragon

[Aragon](https://aragon.org/) 是一个用于创建和管理运行 DAO 所需的一组智能合约的安全平台。Decentraland DAO 的后端使用 Aragon 构建，因此，每当社区在 Snapshot 上通过一个会产生具约束力行动的提案时，DAO 委员会就会将其提交到 Aragon 中的以太坊主网。

## DAO 委员会

DAO 委员会由三位受信任的个人组成，由社区选出，持有一个多签钱包的密钥。该多签负责执行任何已通过且具有约束力的投票结果，例如资助一个 Grant、封禁一个名称、添加或移除一个 POI、实施一项治理提案，或添加一个 Catalyst 节点。

DAO 委员会由 SAB 监督，SAB 有权暂停并取消委员会发起的任何行动。

DAO 委员会发起的每笔链上交易在完成前都会自动延迟 24 小时，以便 SAB 或委员会撤销该交易。

![DAO 委员会](/docs/_remote/063ee68d732926eeb99173e9.webp)

## 安全顾问委员会（SAB）

安全顾问委员会作为 Decentraland 智能合约安全的保障者，负责监督 DAO 委员会的工作，并响应 Decentraland 任何合约中的漏洞和错误报告。

SAB 包括 5 位 Solidity 专家，最初由 Decentraland 开发团队选出。

每当需要对 LAND 或 Estate 合约进行修改时，该更新必须获得 SAB 多签的一致支持。要对 LAND 或 Estate 合约做出任何更改，至少需要三名签署人，且不得有任何反对票。

SAB 有权暂停、恢复或取消 DAO 委员会采取的任何行动。

要发起新增或移除 SAB 成员，可以在……上启动一个治理提案流程 [governance.decentraland.org](https://governance.decentraland.org).

![安全顾问委员会](/docs/_remote/199ca9caded989d293a0f36d.webp)

## 可穿戴物策展委员会

策展委员会负责审核并批准 Decentraland 社区提交的可穿戴物和表情动作。

![可穿戴物策展委员会](/docs/_remote/dc4a02e026b26163106c05ac.webp)

## 撤销委员会

撤销委员会负责审查社区就 Grants Program 提出的案件，并负责给出解决方案，最终措施可能是撤销一份归属合约。

[撤销委员会](https://github.com/lordlikedao/documentation/blob/main/static/images/DAO%20Organizational%20Chart/Revocation.png?raw=true)

## 治理小组

治理小组负责开发、改进并维护 DAO 的技术基础设施和决策工具。

![治理小组](/docs/_remote/f6663bc09324c4879744664c.webp)

## 协调小组

协调小组负责管理沟通并协调不同利益相关方，促进集体决策和高效的治理运作。

![协调小组](/docs/_remote/058f90f95aed65ab975d706d.webp)

## 资助支持小组

资助支持小组负责与受资助者社区保持持续对话，以了解他们的需求，并为他们的阻碍和请求提供支持。

![资助支持小组](/docs/_remote/28cc4e3f4a37dfd9819f791d.webp)

## 论坛

这个 [Decentraland 论坛](https://forum.decentraland.org) 是 DAO 的沟通中心。每当创建一个新提案时，论坛上都会自动在……下方开启一个相应的帖子，位于 [治理版块](https://forum.decentraland.org/c/governance/)。这些提案主题可以通过浏览论坛找到，或者在治理界面中任一提案的详情页点击“在论坛中讨论”按钮找到。这些帖子是投票者讨论某项提案可能带来的各种影响和问题的空间。

## Discord 服务器

DAO 在两个 Discord 服务器上运作：

* [#dao 频道](https://discord.com/channels/417796904760639509/538106198419832844) 位于 Decentraland Discord 服务器上，用于一般公告、讨论和提问。
* 一个 [专门的 Discord 服务器](https://dcl.gg/daodiscord) 只用于 DAO 运作，提供更深入的讨论、供工作组处理复杂问题的专门频道，以及供社区资助项目负责人发布项目更新的空间。在这个频道中，DAO 还会举办每月的 Town Hall。


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/player/player-zh/dao/dao/how-does-the-dao-work.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
