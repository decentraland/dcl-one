> 了解 SDK 提供了哪些用于执行以太坊区块链操作的功能

# 场景区块链操作

Decentraland 场景可以与以太坊区块链交互。这可用于获取用户钱包及其中代币的数据，或触发可能涉及任何以太坊代币（同质或非同质）的交易。这可以有很多用途，例如出售代币、作为游戏机制的一部分奖励代币、如果玩家持有某些代币则改变其与场景的交互方式等。

请注意，场景触发的以太坊主网上的所有交易都需要玩家批准并支付 gas 费用。

所有区块链操作也需要以 [异步函数](/docs/creator/content-creator-zh/chang-jing-sdk7/bian-cheng-mo-shi/async-functions/)的形式执行，因为其时序取决于外部事件。

## 获取玩家的以太坊账户

要获取玩家的以太坊账户，请使用 `getPlayer()` 函数。

```ts
import { getPlayer } from "@dcl/sdk/src/players";

export function main() {
  let userData = getPlayer();
  if (!userData) return;
  if (!userData.isGuest) {
    console.log(userData.userId);
  } else {
    console.log("Player is not connected with Web3");
  }
}
```

请注意，如果玩家以访客身份进入 Decentraland，他们将不会有已连接的以太坊钱包。如果他们以访客身份连接，响应中的 `isGuest` 字段 `getPlayer()` 将为 true。否则，你可以从 `userId` 字段中获取玩家的钱包地址。了解更多关于你可以从玩家那里获取的数据，请参阅 [获取玩家数据](/docs/creator/content-creator-zh/chang-jing-sdk7/jiao-hu-xing/user-data/#get-player-data)


> [!WARNING]
> **📔 注意**：尽管 eth 地址可能包含大写字符，但某些浏览器会自动将返回的字符串转换为小写。如果你希望比较地址值并确保在所有浏览器中都能正常工作，请使用 `.toLowerCase()` 方法将该值转换为小写。


## 检查 gas 价格

导入 `eth-connect` 库后，你必须实例化一个 web3 provider 和一个 request manager，这将允许你通过 web3 在玩家的浏览器中连接到 Metamask。

下面的函数会获取以太坊主网当前的 gas 价格并打印出来。

```ts
import { RequestManager } from "eth-connect";
import { createEthereumProvider } from "@dcl/sdk/ethereum-provider";

executeTask(async function () {
  // 创建 web3 provider 的实例以与 Metamask 交互
  const provider = createEthereumProvider();
  // 创建用于处理 RPC 消息收发的对象
  const requestManager = new RequestManager(provider);
  // 检查以太坊网络上的当前 gas 价格
  const gasPrice = await requestManager.eth_gasPrice();
  // 记录响应
  console.log({ gasPrice });
});
```


> [!NOTE]
> **💡 提示**：请注意， `requestManager` 处理的函数必须使用 `await`调用，因为它们依赖于获取外部数据，可能需要一些时间才能完成。


## 导入合约 ABI

ABI（应用程序二进制接口）描述了如何与以太坊合约交互，确定有哪些函数可用、它们接收什么输入以及输出什么。每个以太坊合约都有自己的 ABI，你应该导入项目中想要使用的所有合约的 ABI。

例如，下面是 MANA ABI 中一个函数的示例：

```ts
{
  anonymous: false,
  inputs: [
    {
      indexed: true,
      name: 'burner',
      type: 'address'
    },
    {
      indexed: false,
      name: 'value',
      type: 'uint256'
    }
  ],
  name: 'Burn',
  type: 'event'
}
```

ABI 定义可能相当长，因为它们通常包含很多函数，因此我们建议将 ABI 文件的 JSON 内容粘贴到单独的 `.ts` 文件中，然后从那里将其导入到其他场景文件中。我们还建议将所有 ABI 文件放在场景中的一个单独文件夹中，命名为 `/contracts`.

```ts
import { abi } from "../contracts/mana";
```

以下是不同 Decentraland 合约的链接。通过点击 *Export ABI* 并选择 *JSON 格式*.

* [MANA 代币 ABI](https://etherscan.io/address/0x0f5d2fb29fb7d3cfee444a200298f468908cc942#code)
* [Decentraland 市场](https://etherscan.io/address/0x19a8ed4860007a66805782ed7e0bed4e44fc6717#code)
* [LAND ABI](https://etherscan.io/address/0xf87e31492faf9a91b02ee0deaad50d51d56d5d4d#code)
* [Estate ABI](https://etherscan.io/address/0x959e104e1a4db6317fa58f8295f586e1a978c297#code)
* [AvatarNameRegistry ABI](https://etherscan.io/address/0x894b883905bfEe2CC448880F1b59f4A762E67566)
* [Catalyst ABI](https://etherscan.io/address/0xcc054fab08127c19f621ab83ade5962cd10584ec)

以下是各个可穿戴系列的合约：（每个系列都是作为单独的合约发布的）

* [ExclusiveMasksCollection ABI](https://etherscan.io/address/0xc04528c14c8ffd84c7c1fb6719b4a89853035cdd)
* [Halloween2019Collection ABI](https://etherscan.io/address/0xc1f4b0eea2bd6690930e6c66efd3e197d620b9c2)
* [Halloween2019CollectionFactory ABI](https://etherscan.io/address/0x07ccfd0fbada4ac3c22ecd38037ca5e5c0ad8cfa)
* [Xmas2019Collection ABI](https://etherscan.io/address/0xc3af02c0fd486c8e9da5788b915d6fff3f049866)
* [MCHCollection ABI](https://etherscan.io/address/0xf64dc33a192e056bb5f0e5049356a0498b502d50)
* [CommunityContestCollection ABI](https://etherscan.io/address/0x32b7495895264ac9d0b12d32afd435453458b1c6)
* [DCLLaunchCollection ABI](https://etherscan.io/address/0xd35147be6401dcb20811f2104c33de8e97ed6818)
* [DCGCollection ABI](https://etherscan.io/address/0x3163d2cfee3183f9874e2869942cc62649eeb004)


> [!NOTE]
> **💡 提示**：要清楚查看合约暴露的函数，请在 [abitopic.io](https://abitopic.io)中打开它。只需将合约地址粘贴到那里并打开 *函数* 选项卡，以查看受支持函数及其参数的完整列表。你甚至可以通过网页使用不同参数测试调用这些函数。


配置 TypeScript 以便从 JSON 文件导入会有些困难。更简单、推荐的变通方法是将 `ABI.JSON` 文件扩展名改为 `.ts` 并稍作修改，使其内容以 `export default`.

例如，如果 ABI 文件内容以 `[{"constant":true,"inputs":[{"internalType":"bytes4" ...etc`，则将其修改为以 `export default [{"constant":true,"inputs":[{"internalType":"bytes4" ...etc`.

### 实例化合约

导入 `eth-connect` 库和合约的 *abi*，你必须实例化几个对象，使你能够使用合约中的函数并在玩家的浏览器中连接到 Metamask。

你还必须导入 web3 provider。这是因为玩家浏览器中的 Metamask 使用 web3，所以我们需要一种与之交互的方式。

```ts
import { RequestManager, ContractFactory } from "eth-connect";
import { createEthereumProvider } from "@dcl/sdk/ethereum-provider";
import { abi } from "../contracts/mana";

executeTask(async () => {
  // 创建 web3 provider 的实例以与 Metamask 交互
  const provider = createEthereumProvider();
  // 创建用于处理 RPC 消息收发的对象
  const requestManager = new RequestManager(provider);
  // 基于 abi 创建一个工厂对象
  const factory = new ContractFactory(requestManager, abi);
  // 使用工厂对象实例化一个 `contract` 对象，引用某个特定合约
  const contract = (await factory.at(
    "0x2a8fd99c19271f4f04b1b7b9c4f7cf264b626edb"
  )) as any;
});
```


> [!NOTE]
> **💡 提示**：对于遵循同一标准的合约，例如 ERC20 或 ERC721，你可以为所有合约导入一个通用 ABI。然后你只需生成一个 `ContractFactory` 对象，并使用同一个工厂为每个合约实例化接口。


### 调用合约中的方法

一旦你创建了 `contract` 对象，你就可以轻松调用其 ABI 中定义的函数，并传入指定的输入参数。

```ts
import { getPlayer } from "@dcl/sdk/src/players";
import { createEthereumProvider } from "@dcl/sdk/ethereum-provider";
import { RequestManager, ContractFactory } from "eth-connect";
import { abi } from "../contracts/mana";

executeTask(async () => {
  try {
    // 上一节中说明的设置步骤
    const provider = createEthereumProvider();
    const requestManager = new RequestManager(provider);
    const factory = new ContractFactory(requestManager, abi);
    const contract = (await factory.at(
      "0x2a8fd99c19271f4f04b1b7b9c4f7cf264b626edb"
    )) as any;
    let userData = getPlayer();
    if (!userData || userData.isGuest) {
      return;
    }

    // 执行合约中的一个函数
    const res = await contract.setBalance(
      "0xaFA48Fad27C7cAB28dC6E970E4BFda7F7c8D60Fb",
      100,
      {
        from: userData.userId,
      }
    );
    // 记录响应
    console.log(res);
  } catch (error: any) {
    console.log(error.toString());
  }
});
```

上面的示例使用了一个 *伪造的 MANA* 测试合约的 abi，并调用其 `setBalance` 方法向一个账户授予 100 *伪造的 MANA* 。在改编此示例时，请将地址和 abi 替换为你想要调用的合约的地址和 abi，并调用其 abi 定义的方法。

### 其他函数

eth-connect 库还包含一些你可以使用的其他辅助函数。例如可用于：

* 获取预估 gas 价格
* 获取指定地址的余额
* 获取交易回执
* 获取从某个地址发送的交易数量
* 在多种格式之间转换，包括十六进制、二进制、utf8 等。

## 使用以太坊测试网络

在测试你的场景时，为了避免转移真实的 MANA 或其他货币，你可以使用 *以太坊 Sepolia 测试网络* 并改为转移测试网中的虚拟 MANA。

要使用测试网络，你必须将 Metamask Chrome 扩展设置为使用 *Sepolia 测试网络* 而不是 *主网*.

你必须获取 Sepolia Ether，可通过各种外部水龙头免费获得，例如 [这个](https://www.alchemy.com/faucets/ethereum-sepolia/).


> [!NOTE]
> **💡 提示**：要执行将 Sepolia MANA 转入你的钱包的交易，你需要支付 Sepolia Ether 的 gas 费用。


要使用测试网络预览你的场景，请将以下 URL 粘贴到浏览器标签页中。这将会在 Decentraland 桌面客户端中打开该场景：

`decentraland://realm=http://127.0.0.1:8000&local-scene=true&debug=true&dclenv=zone&position=0,0`


> [!NOTE]
> **💡 提示**：将 position 参数改为你场景的坐标，即可直接加载进入你的场景。


在此模式下查看场景时，你接受的任何交易都只会发生在测试网络中，不会影响你真实钱包中的 MANA 余额。

如果你需要在 Polygon 测试网上测试交易，并且需要在该测试网上拥有 MANA，那么你需要在 Sepolia 中获取后再将 MANA 兑换到该网络。要将 Sepolia MANA 跨链到 Polygon 测试网，请访问你的 [Sepolia 中的 Decentraland 账户页面](https://account.decentraland.zone/) 并在以太坊 MANA 一侧点击“swap”。

当你在旧版 web 客户端中预览使用了某个以太坊库的场景时，你必须在单独的浏览器窗口中打开预览，确保浏览器中已打开 Metamask，并手动包含字符串 `&ENABLE_WEB3`.

## 发送自定义 RPC 消息

使用函数 `sendAsync()` 通过以下方式发送消息： [RPC 协议](https://en.wikipedia.org/wiki/Remote_procedure_call).

```ts
import { sendAsync } from "~system/EthereumController";

// 发送消息
await sendAsync({
  id: 1,
  method: "myMethod",
  jsonParams: "{ myParam: myValue }",
});
```

## Decentraland 智能合约

在以下链接中，你可以找到与 Decentraland 生态系统相关的以太坊智能合约列表。该列表包括主网中的合约以及其他以太坊测试网络中的合约。

[contracts.decentraland.org](https://contracts.decentraland.org/links)


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-zh/chang-jing-sdk7/qu-kuai-lian/scene-blockchain-operations.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
