> 由 Decentraland 团队构建并针对移动端游玩优化的开源场景。

# 示例场景

以下场景均为完全开源，并专为在移动设备上良好运行而设计。请将它们作为结构、输入处理和性能模式的参考。

## 死潮突袭

多人僵尸射击游戏。

* **世界：** `/deadsurge.dcl.eth`
* **来源：** [github.com/dcl-regenesislabs/dead-surge](https://github.com/dcl-regenesislabs/dead-surge)

## 舒适农场

农场模拟游戏。

* **世界：** `/cozyfarm.dcl.eth`
* **来源：** [github.com/dcl-regenesislabs/cozy-farm](https://github.com/dcl-regenesislabs/cozy-farm)

## 开球

世界杯预测游戏。

* **世界：** `/kickoff.dcl.eth`
* **来源：** [github.com/dcl-regenesislabs/kickoff-2026](https://github.com/dcl-regenesislabs/kickoff-2026)

## 木筏游戏

单人求生游戏。

* **世界：** `/raft.dcl.eth`
* **来源：** [github.com/dcl-regenesislabs/raft-game](https://github.com/dcl-regenesislabs/raft-game)

## 威尼斯狩猎

多人道具捉迷藏。

* **世界：** `/venetianhunt.dcl.eth`
* **来源：** [github.com/dcl-regenesislabs/venetian-hunt](https://github.com/dcl-regenesislabs/venetian-hunt)

## 疯狂之塔

多人跑酷/跳跃挑战。

* **世界：** `/towerofmadness.dcl.eth`
* **来源：** [github.com/dcl-regenesislabs/towerofmadness](https://github.com/dcl-regenesislabs/towerofmadness)


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-zh/mian-xiang-yi-dong-duan-gou-jian/yi-dong-ke-hu-duan/sample-scenes.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
