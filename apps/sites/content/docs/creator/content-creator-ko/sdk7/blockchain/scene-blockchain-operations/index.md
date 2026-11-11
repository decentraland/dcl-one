> Ethereum 블록체인에서 작업을 수행하기 위해 SDK가 제공하는 기능을 알아봅니다

# 씬 블록체인 작업

Decentraland 씬은 이더리움 블록체인과 연동할 수 있습니다. 이를 통해 사용자의 지갑과 그 안의 토큰에 대한 데이터를 가져오거나, 대체 가능 토큰과 대체 불가능 토큰을 포함한 어떤 이더리움 토큰과도 관련될 수 있는 트랜잭션을 트리거할 수 있습니다. 이는 토큰을 판매하거나, 게임 메커니즘의 일부로 토큰을 보상으로 지급하거나, 특정 토큰을 소유한 경우 플레이어가 씬과 상호작용하는 방식을 바꾸는 등 다양한 방식으로 사용할 수 있습니다.

씬에서 트리거되는 이더리움 메인넷의 모든 트랜잭션은 플레이어의 승인과 가스비 지불이 필요합니다.

모든 블록체인 작업도 다음과 같이 수행되어야 합니다. [비동기 함수로](/docs/creator/content-creator-ko/sdk7/programming-patterns/async-functions/)실행해야 합니다. 시점이 외부 이벤트에 따라 달라지기 때문입니다.

## 플레이어의 이더리움 계정 가져오기

플레이어의 이더리움 계정을 가져오려면 다음을 사용하세요. `getPlayer()` 함수입니다.

```ts
import { getPlayer } from "@dcl/sdk/src/players";

export function main() {
  let userData = getPlayer();
  if (!userData) return;
  if (!userData.isGuest) {
    console.log(userData.userId);
  } else {
    console.log("플레이어가 Web3에 연결되어 있지 않습니다");
  }
}
```

플레이어가 게스트로 Decentraland에 प्रवेश했다면 연결된 이더리움 지갑이 없습니다. 게스트로 연결된 경우, 응답의 `isGuest` 필드는 `getPlayer()` 에서 true가 됩니다. 그렇지 않으면 다음에서 플레이어의 지갑 주소를 가져올 수 있습니다. `userId` 필드입니다. 플레이어에서 얻을 수 있는 데이터에 대해 자세히 알아보려면 [플레이어 데이터 가져오기](/docs/creator/content-creator-ko/sdk7/interactivity/user-data/#get-player-data)


> [!WARNING]
> **📔 참고**: eth 주소에는 대문자가 포함될 수 있지만, 일부 브라우저는 반환된 문자열을 자동으로 소문자로 변환합니다. 주소 값을 비교하고 모든 브라우저에서 동작하게 하려면 `.toLowerCase()` 메서드를 사용해 값을 소문자로 변환하세요.


## 가스 가격 확인

다음을 가져온 후 `eth-connect` 라이브러리를 가져온 뒤에는 web3 프로바이더와 request manager를 인스턴스화해야 하며, 이를 통해 플레이어의 브라우저에서 Metamask에 web3로 연결할 수 있습니다.

아래 함수는 이더리움 메인 네트워크의 현재 가스 가격을 가져와 출력합니다.

```ts
import { RequestManager } from "eth-connect";
import { createEthereumProvider } from "@dcl/sdk/ethereum-provider";

executeTask(async function () {
  // Metamask와 연동할 web3 프로바이더 인스턴스 생성
  const provider = createEthereumProvider();
  // RPC 메시지의 송수신을 처리할 객체 생성
  const requestManager = new RequestManager(provider);
  // 이더리움 네트워크의 현재 가스 가격 확인
  const gasPrice = await requestManager.eth_gasPrice();
  // 응답 로그 출력
  console.log({ gasPrice });
});
```


> [!NOTE]
> **💡 팁**: 다음에 의해 처리되는 함수들은 `requestManager` 를 사용해 호출해야 합니다. `await`외부 데이터 가져오기에 의존하므로 완료되는 데 시간이 걸릴 수 있기 때문입니다.


## 컨트랙트 ABI 가져오기

ABI(Application Binary Interface)는 이더리움 컨트랙트와 상호작용하는 방법을 설명하며, 어떤 함수가 사용 가능한지, 어떤 입력을 받는지, 무엇을 출력하는지를 결정합니다. 각 이더리움 컨트랙트에는 고유한 ABI가 있으며, 프로젝트에서 사용하려는 모든 컨트랙트의 ABI를 가져와야 합니다.

예를 들어, MANA ABI의 한 함수 예시는 다음과 같습니다.

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

ABI 정의는 종종 많은 함수를 포함하므로 꽤 길 수 있습니다. 따라서 ABI 파일의 JSON 내용을 별도의 `.ts` 파일에 붙여 넣고 거기에서 다른 씬 파일로 가져오는 방식을 권장합니다. 또한 모든 ABI 파일은 씬의 별도 폴더에 보관할 것을 권장하며, 해당 폴더 이름은 `/contracts`.

```ts
import { abi } from "../contracts/mana";
```

다음은 여러 Decentraland 컨트랙트로 연결되는 링크입니다. 각 컨트랙트의 ABI는 *Export ABI* 를 클릭하고 *JSON Format*.

* [MANA Token ABI](https://etherscan.io/address/0x0f5d2fb29fb7d3cfee444a200298f468908cc942#code)
* [Decentraland Marketplace](https://etherscan.io/address/0x19a8ed4860007a66805782ed7e0bed4e44fc6717#code)
* [LAND ABI](https://etherscan.io/address/0xf87e31492faf9a91b02ee0deaad50d51d56d5d4d#code)
* [Estate ABI](https://etherscan.io/address/0x959e104e1a4db6317fa58f8295f586e1a978c297#code)
* [AvatarNameRegistry ABI](https://etherscan.io/address/0x894b883905bfEe2CC448880F1b59f4A762E67566)
* [Catalyst ABI](https://etherscan.io/address/0xcc054fab08127c19f621ab83ade5962cd10584ec)

다음은 다양한 웨어러블 컬렉션에 대한 컨트랙트입니다: (각 컬렉션은 별도의 컨트랙트로 발행되었습니다)

* [ExclusiveMasksCollection ABI](https://etherscan.io/address/0xc04528c14c8ffd84c7c1fb6719b4a89853035cdd)
* [Halloween2019Collection ABI](https://etherscan.io/address/0xc1f4b0eea2bd6690930e6c66efd3e197d620b9c2)
* [Halloween2019CollectionFactory ABI](https://etherscan.io/address/0x07ccfd0fbada4ac3c22ecd38037ca5e5c0ad8cfa)
* [Xmas2019Collection ABI](https://etherscan.io/address/0xc3af02c0fd486c8e9da5788b915d6fff3f049866)
* [MCHCollection ABI](https://etherscan.io/address/0xf64dc33a192e056bb5f0e5049356a0498b502d50)
* [CommunityContestCollection ABI](https://etherscan.io/address/0x32b7495895264ac9d0b12d32afd435453458b1c6)
* [DCLLaunchCollection ABI](https://etherscan.io/address/0xd35147be6401dcb20811f2104c33de8e97ed6818)
* [DCGCollection ABI](https://etherscan.io/address/0x3163d2cfee3183f9874e2869942cc62649eeb004)


> [!NOTE]
> **💡 팁**: 컨트랙트가 노출하는 함수를 명확히 보려면 다음에서 열어보세요. [abitopic.io](https://abitopic.io). 컨트랙트 주소를 여기에 붙여 넣고 *functions* 탭을 열어 지원되는 함수와 인자의 전체 목록을 확인하세요. 웹페이지를 통해 서로 다른 파라미터로 함수를 호출해 테스트할 수도 있습니다.


JSON 파일에서 import할 수 있도록 TypeScript를 설정하는 것은 까다로울 수 있습니다. 더 쉬운 권장 우회 방법은 `ABI.JSON` 파일 확장자를 `.ts` 로 바꾸고, 내용이 다음으로 시작하도록 약간 수정하는 것입니다. `export default`.

예를 들어, ABI 파일의 내용이 다음으로 시작한다면 `[{"constant":true,"inputs":[{"internalType":"bytes4" ...etc`, 다음과 같이 시작하도록 수정하세요. `export default [{"constant":true,"inputs":[{"internalType":"bytes4" ...etc`.

### 컨트랙트 인스턴스화

다음을 가져온 후 `eth-connect` 라이브러리와 컨트랙트의 *abi*를 사용하려면, 컨트랙트의 함수를 사용하고 플레이어의 브라우저에서 Metamask에 연결할 수 있게 해주는 여러 객체를 인스턴스화해야 합니다.

또한 web3 프로바이더도 가져와야 합니다. 플레이어의 브라우저에 있는 Metamask는 web3를 사용하므로, 이를 상호작용할 방법이 필요하기 때문입니다.

```ts
import { RequestManager, ContractFactory } from "eth-connect";
import { createEthereumProvider } from "@dcl/sdk/ethereum-provider";
import { abi } from "../contracts/mana";

executeTask(async () => {
  // Metamask와 연동할 web3 프로바이더 인스턴스 생성
  const provider = createEthereumProvider();
  // RPC 메시지의 송수신을 처리할 객체 생성
  const requestManager = new RequestManager(provider);
  // abi를 기반으로 factory 객체 생성
  const factory = new ContractFactory(requestManager, abi);
  // factory 객체를 사용해 특정 컨트랙트를 참조하는 `contract` 객체 인스턴스 생성
  const contract = (await factory.at(
    "0x2a8fd99c19271f4f04b1b7b9c4f7cf264b626edb"
  )) as any;
});
```


> [!NOTE]
> **💡 팁**: ERC20 또는 ERC721과 같이 동일한 표준을 따르는 컨트랙트의 경우, 모든 컨트랙트에 대해 하나의 일반 ABI를 가져올 수 있습니다. 그런 다음 하나의 `ContractFactory` 객체를 해당 ABI로 생성하고, 같은 factory를 사용해 각 컨트랙트의 인터페이스를 인스턴스화할 수 있습니다.


### 컨트랙트의 메서드 호출

한 번 `contract` 객체를 만들면, 해당 ABI에 정의된 함수들을 지정된 입력 매개변수와 함께 쉽게 호출할 수 있습니다.

```ts
import { getPlayer } from "@dcl/sdk/src/players";
import { createEthereumProvider } from "@dcl/sdk/ethereum-provider";
import { RequestManager, ContractFactory } from "eth-connect";
import { abi } from "../contracts/mana";

executeTask(async () => {
  try {
    // 위 섹션에서 설명한 설정 단계
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

    // 컨트랙트의 함수 실행
    const res = await contract.setBalance(
      "0xaFA48Fad27C7cAB28dC6E970E4BFda7F7c8D60Fb",
      100,
      {
        from: userData.userId,
      }
    );
    // 응답 로그 출력
    console.log(res);
  } catch (error: any) {
    console.log(error.toString());
  }
});
```

위 예시는 *가짜 MANA* 테스트 컨트랙트의 abi를 사용하며, 그 `setBalance` 메서드를 호출해 계정에 100을 부여합니다. *가짜 MANA* 이 예시를 수정할 때는 주소와 abi를 호출하려는 컨트랙트의 것으로 바꾸고, 해당 abi가 정의한 메서드를 호출하세요.

### 기타 함수

eth-connect 라이브러리에는 사용할 수 있는 여러 다른 헬퍼가 포함되어 있습니다. 예를 들어 다음을 할 수 있습니다.

* 예상 가스 가격 가져오기
* 주어진 주소의 잔액 가져오기
* 트랜잭션 영수증 가져오기
* 주소에서 전송된 트랜잭션 수 가져오기
* 16진수, 이진수, utf8 등을 포함한 다양한 형식 간 변환

## 이더리움 테스트 네트워크 사용

씬을 테스트할 때 실제 MANA나 다른 통화를 전송하지 않으려면 *이더리움 Sepolia 테스트 네트워크* 를 사용하고 대신 가짜 테스트넷 MANA를 전송할 수 있습니다.

테스트 네트워크를 사용하려면 Metamask Chrome 확장 프로그램이 다음을 사용하도록 설정해야 합니다. *Sepolia 테스트 네트워크* 대신 *메인 네트워크*.

Sepolia Ether를 구해야 하며, 다음과 같은 여러 외부 faucet에서 무료로 받을 수 있습니다. [이것](https://www.alchemy.com/faucets/ethereum-sepolia/).


> [!NOTE]
> **💡 팁**: 지갑으로 Sepolia MANA를 전송하는 거래를 실행하려면 Sepolia Ether로 가스 수수료를 지불해야 합니다.


테스트 네트워크를 사용하여 장면을 미리 보려면 다음 URL을 브라우저 탭에 붙여넣으세요. 그러면 Decentraland 데스크톱 클라이언트에서 장면이 열립니다:

`decentraland://realm=http://127.0.0.1:8000&local-scene=true&debug=true&dclenv=zone&position=0,0`


> [!NOTE]
> **💡 팁**: 위치 매개변수를 씬의 좌표로 변경하여 씬으로 직접 로드되게 하세요..


이 모드에서 장면을 보며 승인하는 모든 거래는 테스트 네트워크에서만 발생하며, 실제 지갑의 MANA 잔액에는 영향을 주지 않습니다.

Polygon Testnet에서 트랜잭션을 테스트해야 하고 그 테스트넷에서 MANA가 필요하다면, Sepolia에서 MANA를 획득한 뒤 해당 네트워크로 스왑해야 합니다. Sepolia MANA를 Polygon Testnet으로 브리지하려면 다음에서 Decentraland 계정 페이지를 방문하세요. [Sepolia의 Decentraland 계정 페이지](https://account.decentraland.zone/) 그리고 이더리움 MANA 측에서 ‘swap’을 클릭하세요.

레거시 웹 클라이언트에서, 이더리움 라이브러리 중 하나를 사용하는 씬의 프리뷰를 실행할 때는 별도의 브라우저 창에서 프리뷰를 열고, 브라우저에서 Metamask를 열어 둔 상태로, 문자열 `&ENABLE_WEB3`.

## 사용자 지정 RPC 메시지 보내기

다음 함수를 사용하세요. `sendAsync()` 를 사용해 다음을 통해 메시지를 전송합니다. [RPC 프로토콜](https://en.wikipedia.org/wiki/Remote_procedure_call).

```ts
import { sendAsync } from "~system/EthereumController";

// 메시지 전송
await sendAsync({
  id: 1,
  method: "myMethod",
  jsonParams: "{ myParam: myValue }",
});
```

## Decentraland 스마트 컨트랙트

다음 링크에서 Decentraland 생태계와 관련된 이더리움 스마트 컨트랙트 목록을 찾을 수 있습니다. 이 목록에는 메인넷의 컨트랙트뿐 아니라 다른 이더리움 테스트 네트워크의 컨트랙트도 포함되어 있습니다.

[contracts.decentraland.org](https://contracts.decentraland.org/links)


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-ko/sdk7/blockchain/scene-blockchain-operations.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
