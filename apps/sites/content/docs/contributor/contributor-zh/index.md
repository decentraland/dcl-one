# 欢迎贡献者

Decentraland 是一个由其用户拥有的虚拟世界，开放开发，并已准备好随着全球各地人们的贡献而成长。

本文档详细介绍了 Decentraland 开放协议，内容涵盖从架构概览到各个组件的具体能力。在这里，你会找到所需资源，用于：

1. 为生态系统设计产品和应用程序
2. 开展研究并调查世界的内容
3. 创建你自己的世界探索器
4. 探索可能最终会出现在此列表中的新想法

**让我们一起构建元宇宙！**

## 你将在这里找到什么

本文档旨在帮助你理解并参与 Decentraland 项目。无论你是新贡献者还是经验丰富的开发者，你都能找到：

* **架构概览** - 了解完整的 Decentraland 协议
* **代码标准** - 了解我们如何编写简洁、易维护的代码
* **测试实践** - 了解我们全面的测试策略
* **文档指南** - 了解我们如何记录工作成果
* **API 参考** - 所有服务的完整文档

## 从哪里开始？

如果你不知道从哪里开始，最好先看看 [架构](/docs/contributor/contributor-zh/jia-gou/architecture/) 部分，先了解不同组件及其总体用途，再深入阅读更详细的章节。

对于贡献者，请先浏览 [贡献者指南](/docs/contributor/contributor-zh/gong-xian-zhe-zhi-nan/contributor-guides/) 部分，其中涵盖了从测试标准到代码质量指南的方方面面。

### 快速链接

<table data-view="cards"><thead><tr><th></th><th></th><th></th><th data-hidden data-card-target data-type="content-ref"></th></tr></thead><tbody><tr><td><strong>架构</strong></td><td>系统架构概览</td><td></td><td><a href="/docs/contributor/contributor-zh/jia-gou/architecture/">概述</a></td></tr><tr><td><strong>贡献者指南</strong></td><td>指南和最佳实践</td><td></td><td><a href="/docs/contributor/contributor-zh/gong-xian-zhe-zhi-nan/contributor-guides/">概述</a></td></tr><tr><td><strong>API 参考</strong></td><td>服务的 API 文档</td><td></td><td><a href="https://github.com/decentraland/docs/tree/main/apis/README.md">https://github.com/decentraland/docs/tree/main/apis/README.md</a></td></tr></tbody></table>

## 其他资源

要获取更多信息并继续学习，你可能会想查看：

* 该 [AMA（问我任何事）会话](https://www.notion.so/decentraland/Decentraland-Foundation-Technical-AMA-230fcf1ca1ec4d49922744fe91e6995d?d=8508f4ca20eb4623b578feb235f86cef#bff4e53f1c664d66802e4770f46b959c)，在这些会话中，社区成员可以向 Decentraland 基金会的技术团队提出问题、评论或疑虑。这些会话每月举办，欢迎自由参加！
* 该 [状态页面](https://status.decentraland.org/)，查看当前平台状态、事件历史以及一些指标。
* 该 [架构决策记录](https://adr.decentraland.org/)，一系列设计文档，描述 Decentraland 不同方面的现状和演进。
* 该 [GitHub 上的 Decentraland 组织](https://github.com/decentraland)，所有开源仓库都托管于此。

## 贡献

通过遵循这些标准，你有助于在所有 Decentraland 项目中保持代码质量和一致性。这些指南会持续更新，以反映我们不断发展的最佳实践以及从开发经验中总结出的教训。

如果你对这些标准有任何问题或建议，请联系 Decentraland 开发社区。


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/contributor/contributor-zh/readme.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
