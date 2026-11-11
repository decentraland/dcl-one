# 签名获取

当协议客户端想要发起经过身份验证的 HTTP 请求时，它们可以利用标准的 *签名获取* 机制。

签名获取是一种包含一个 [身份验证链](/docs/contributor/contributor-zh/shen-fen-yan-zheng/authchain/)，通过标头表示。兼容 Decentraland 的 API 服务器可以在例如， [允许来自场景的请求](/docs/contributor/contributor-zh/chang-jing-yun-xing-shi/yun-xing-shi-mo-kuai/signed-fetch/) 或创建 [适配器](/docs/contributor/contributor-zh/tong-xin/overview/).

## 标头

服务器需要用于验证认证链的信息通过 3 个以上的标头传递：

* `X-Identity-Timestamp`：即 `时间戳` 字段，该字段包含在签名载荷中（见下文）。
* `X-Identity-Metadata`：即 `元数据` 字段，该字段包含在签名载荷中（见下文）。
* `X-Identity-AuthChain-<index>`：即 [JSON 序列化的认证步骤](/docs/contributor/contributor-zh/shen-fen-yan-zheng/authchain/#constructing) `<索引>`，从 `0`.

传输的链由服务器验证 [如所指定](/docs/contributor/contributor-zh/shen-fen-yan-zheng/authchain/).

## 主体

请求主体未指定。服务可以完全灵活地使用他们喜欢的任何协议或格式。

## 载荷

认证链 [载荷](/docs/contributor/contributor-zh/shen-fen-yan-zheng/authchain/#constructing) 对于签名获取来说，是一个 **小写、以冒号分隔的** 包含请求某些元素的字符串：

```
<方法>:<路径>:<时间戳>:<元数据>
```

该 `方法` 和 `路径` 字段必须与请求中的一致，并且 `时间戳` 与……中的相同 `X-Identity-Timestamp` 标头。

最后一个字段， `元数据`，可以包含任意内容。

例如：

```
get:/some/path:1682790056:{"some":"custom json"}
```


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/contributor/contributor-zh/shen-fen-yan-zheng/signed-fetch.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
