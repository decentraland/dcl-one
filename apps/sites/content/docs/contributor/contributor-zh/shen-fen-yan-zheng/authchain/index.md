# 认证链

Decentraland 协议中的许多操作都需要，或会从由用户的 Ethereum 账户签名进行授权中受益。例如：

1. 上传以下任意对象的新版本： [实体](/docs/contributor/contributor-zh/nei-rong/entities/) 他们拥有的（例如其个人资料）。
2. 向第三方服务进行身份验证。
3. 授权委托人代表自己行事。

要执行这些操作，用户和委托人必须签署描述其意图的载荷，并生成一个 *认证链*.

## 简介

认证链封装了一系列验证步骤，其中每一步都依赖于前一步的成功验证。要让任何受限操作发生，验证方必须认定所有步骤都有效。

每条链都从识别用户开始，并以一个表示所请求操作的已签名载荷结束。因此，最小的链包含两个元素，分别表示：

```md
1. 授权方是 Ethereum 账户 <user-address>
2. 载荷是 <payload>，由 <user-signature> 授权
```

可通过验证签名的公钥是否与该地址对应来评估这个基本链。在这种情况下，它等同于普通签名。

当用户授权委托人代表自己行事时，链中会出现中间步骤。例如，只有一次委托的链会表示：

```md
1. 授权方是 Ethereum 账户 <user-address>
2. 委托人是 <delegate-address>，由 <user-signature> 授权，直到 <date>
3. 载荷是 <payload>，由 <delegate-signature> 授权
```

当委托人再授权自己的委托人时，链会更长。换句话说，授权是可传递的。


> [!NOTE]
> 可以将认证链类比为 HTTPS 中使用的 TLS 证书链，其中用户相当于根证书颁发机构。


这种单一委托链是 Decentraland 中最常见的授权形式，因为用户会为其 World Explorer 授权一个密钥，以避免必须用其 Ethereum 账户为每个单独操作签名。

### 构建链 <a href="#constructing" id="constructing"></a>

认证链中的每一步都包含三项信息：一个 `类型`，一个 `载荷` 以及对应的 `签名`.

| 字段   | 值                                     |
| ---- | ------------------------------------- |
| `类型` | 某个类型的名称（[见下文](#types)).               |
| `载荷` | 一个依赖于类型的字符串。                          |
| `签名` | 以下内容的十六进制编码 Ethereum 签名： `载荷`，以 `0x`. |


> [!NOTE]
> 由于认证链最常见的序列化形式是 JSON 数组，下面的示例也采用这种形式。另见 [传输链](#transmitting) 以了解更多详情。


### 第一步：身份识别

第一步用于标识最初的授权方（即用户），必须满足以下条件：

1. 该 `类型` 是 [`SIGNER`](#signer)
2. 该 `载荷` 是编码后的 Ethereum 账户。
3. 该 `签名` 为空。

例如：

```json
// 任何认证链中的第一步：
{
  "type": "SIGNER",
  "payload": "0xdB055877e6c13b6A6B25aBcAA29B393777dD0a73",
  "signature": ""
}
```

第二步必须携带一个 `签名` 来自此账户的，用于其 `载荷`.

### 中间步骤：委托

当委托人代表用户行事时，会为每个委托人在链的中间添加一项。这些步骤的条件如下：

1. 类型是 [`ECDSA_EPHEMERAL`](#ecdsa_ephemeral)
2. 该 `载荷` 是专门构造的文本（见下文）。

该 `载荷` 其设计目标是易于阅读且易于解析，因为人类（在使用其钱包界面时）和程序（在构造和验证时）都必须处理它。它恰好包含 3 行区分大小写的纯文本：

```md
<purpose>
临时地址：<delegate-address>
到期时间：<date>
```

例如，这是 World Explorers 在登录期间使用的典型载荷，当他们为用户生成临时委托密钥供其批准时：

```
Decentraland 登录
临时地址：0xBB9aDF0183b1742196A4Aa55622D5838f4f483a7
到期时间：2023-02-25T13:00:19.730Z
```

请注意：

1. 该 `临时地址` 不需要是区块链中拥有资金的真实 Ethereum 账户。该字段可能仅表示一个临时公钥。
2. 该 `到期时间` 是序列化后的日期时间，采用 [ISO-8601](https://en.wikipedia.org/wiki/ISO_8601) 格式。它不必是 UTC。
3. 由于这个到期时间，委托密钥必须定期由用户提供新的签名进行续期。


> [!NOTE]
> 每次续期都生成新密钥是标准做法（并且强烈推荐）（因此得名 `ECDSA_EPHEMERAL`).


一个委托步骤示例（为清晰起见，值已缩写）：

```json
{
  "type": "ECDSA_EPHEMERAL",
  "payload": "Decentraland 登录\n临时地址: 0xBBa7...\n到期时间: 2021-01-25T...",
  "signature": "0x1370a4120a7cb0d2f6e4a5..."
}
```

下一步，无论是另一次委托还是最终授权，都会带有来自该委托密钥的签名。

### 最后一步：授权

在……之后 `SIGNER` 已被指定，并且所有 `ECDSA_EPHEMERAL` 在通过验证中间签名链确认所有委托密钥都有效之后，最后一步就是需要实际授权的操作。

该 `载荷` 取决于 `类型` 该步骤的。例如，如果用户正在将新的个人资料（或其拥有的任何实体）上传到内容服务器，则最后一个元素将具有以下形式：

```json
{
  // 类型表示 `payload` 是实体 ID（详细信息见内容部分）：
  "type": "ECDSA_SIGNED_ENTITY",

  // 载荷是原始 ID 字符串：
  "payload": "bafkreicfbg7ybpuoslkcf6x2vfnvzl5vwgqtb2pnheqiut2i4sgpblicqi",

  // 签名由前一步中的账户（用户或委托人）生成：
  "signature": "0x7e71dbbab..."
}
```

如果最后这个签名也有效，则可继续执行该操作。

### 传输链 <a href="#transmitting" id="transmitting"></a>

如上所述，认证链最常见的序列化形式是 JSON 数组。这是推荐的方法。

不过，协议并未强制要求这样做。开发者可以在特定用例中采用更方便的替代序列化策略，例如 YAML、CSV 文件、优化后的二进制格式，或带分隔字段的简单字符串。

协议内部的替代序列化示例可见于 [`SignedFetch`](/docs/contributor/contributor-zh/chang-jing-yun-xing-shi/yun-xing-shi-mo-kuai/signed-fetch/) 模块，它使用一系列 HTTP 头而不是 JSON 数组。

### 选择到期日期

在为委托密钥选择有效期限时，需要权衡：较短的到期时间会提高安全性，但较长的到期时间会改善用户体验（因为委托需要更少频率的人为交互来续期）。

没有一种通用策略可以决定有效时间窗口应为多久。作为参考，Foundation 的 World Explorer 会为其委托密钥请求一个月的授权。


> [!NOTE]
> 临时密钥绝不应持有资金或具备转移数字资产的能力。如果临时密钥泄露不会导致财务损失，对终端用户来说会安全得多。


## 形式化

以下是所涉流程更正式且更精确的定义。请遵循这些说明以成功处理认证链。

### 创建

为用户构造认证链的客户端遵循以下步骤：

1. 添加身份识别步骤：
   1. 将 `类型` 设置为 `SIGNER`.
   2. 将 `载荷` 用户的 Ethereum 地址。
   3. 将 `签名` 为空字符串。
2. 添加委托步骤：
   1. 生成或使用现有的委托私钥（可能需要交互）。
   2. 计算由相应公钥派生的委托 Ethereum 地址。
   3. 将 `类型` 设置为 `ECDSA_EPHEMERAL`.
   4. 将 `到期时间` 设置为未来的某个日期。
   5. 选择一个 `用途` 用于此密钥。
   6. 将 `载荷` 设为以下精确格式：

      ```md
      <purpose>
      临时地址：<delegate-address>
      到期时间：<date>
      ```
   7. 将 `签名` 字段设置为 `载荷` 来自前一个密钥（用户或委托人）的签名。
   8. 对所有后续委托人重复此操作。
3. 添加操作授权步骤：
   1. 将 `类型` 设置为有效值（[见下文](#types))
   2. 将 `载荷` 设置为该类型特定的值（例如实体 ID）。
   3. 将 `签名` 字段设置为 `载荷` 来自前一个密钥（用户或委托人）的签名。
4. 将认证链发送给验证方。

### 验证

实现验证的内容服务器和第三方服务遵循以下步骤：

1. 验证身份识别：
   1. 验证 `类型` 是 `SIGNER`.
   2. 验证 `载荷` 是用户的 Ethereum 地址。
   3. 验证 `签名` 是空字符串。
2. 验证委托：
   1. 验证 `类型` 是 `ECDSA_EPHEMERAL`.
   2. 验证 `载荷` 具有以下形式，并提取字段：

      ```md
      <purpose>
      临时地址：<delegate-address>
      到期时间：<date>
      ```
   3. 验证 `日期` 仍在未来。
   4. 验证 `用途` 受你的服务支持。
   5. 验证 `签名` 对给定的……有效 `载荷` 以及前一个公钥有效。
   6. 对所有后续委托人重复此操作。
3. 验证操作授权：
   1. 验证 `类型` 是有效值（[见下文](#types)).
   2. 验证 `载荷` 对该……有效 `类型`.
   3. 验证 `签名` 对给定的……有效 `载荷` 以及前一个公钥有效。
4. 接受该认证链。

### 标准操作类型和用途 <a href="#types" id="types"></a>

该 `类型` 和 `载荷` 用于身份识别和委托的值是标准化的，必须按上文所述进行验证，但客户端和服务可以就任何操作达成一致 `类型`，以及其 `载荷` 结构，并设置一个 `用途` 他们认为有效的委托密钥的

该协议定义了三种标准类型和一种标准用途：

#### 类型 `SIGNER` <a href="#signer" id="signer"></a>

**必须** 是初始的 `类型` 在链中，其中 `载荷` 是用户的 Ethereum 地址，并且 `签名` 是空字符串。

#### 类型 `ECDSA_EPHEMERAL` <a href="#ecdsa_ephemeral" id="ecdsa_ephemeral"></a>

**必须** 是 `类型` 作为链中间步骤的……，其中 `载荷` 采用上文所述的格式。

#### 类型 `ECDSA_SIGNED_ENTITY` <a href="#ecdsa_signed_entity" id="ecdsa_signed_entity"></a>

最终的 `类型` 在链中用于授权实体部署的……，其中 `载荷` 是用户拥有的实体 ID。

#### 用途 `Decentraland 登录` <a href="#decentraland-login" id="decentraland-login"></a>

通常的 `用途` 用于 World Explorers 的……，在登录世界以及续期其委托密钥时都会签名。


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/contributor/contributor-zh/shen-fen-yan-zheng/authchain.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
