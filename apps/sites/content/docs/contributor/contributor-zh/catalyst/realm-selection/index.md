# Realm 选择

一个 [Catalyst](https://github.com/decentraland/catalyst) 是一个服务器，整合了 Decentraland 世界所需的不同服务。该服务器的主要职责之一是管理用于客户端所需大部分内容的去中心化存储，以及协调节点之间的通信。

当进入 Decentraland 客户端时，你需要连接到一个特定的 Catalyst，以支持后端服务 Content、Lambdas 和 Comms。必须使用 DAO 的某个 Catalyst，虽然可以随机选择，但这可能会导致用户连接到另一个后端服务时访问到延迟很高的服务，或是一个用户数量很少的 realm。

为缓解这一问题，Kernel 中实现了一个用于选择 realm 的算法，如下所述： [ADR-86](https://rfc.decentraland.org/adr/ADR-86).

这是一个可配置的算法，会按顺序使用不同的判定标准；如果其中某个条件足够强，可以做出决定，它就会选择一个 realm；否则就把决定交给下一个标准。如果没有任何条件足够强，则会做出默认决定（随机）。

默认配置如下：

1. 优先选择用户数量最多的 Catalyst 节点
2. 优先选择在所访问位置拥有更多用户的 Catalyst 节点

还可以使用的其他标准有：

* 高延迟：优先选择延迟更低的 Catalyst
* 负载均衡：一种轮询机制，用于在所有 Catalyst 之间分配所有节点。


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/contributor/contributor-zh/catalyst/realm-selection.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
