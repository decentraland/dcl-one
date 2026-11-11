> Saiba o que o SDK oferece para realizar operações com a blockchain Ethereum

# Operações Blockchain da Scene

Uma scene do Decentraland pode interagir com a blockchain Ethereum. Isso pode servir para obter dados sobre a wallet do usuário e os tokens nela, ou para acionar transações que podem envolver qualquer token Ethereum, fungível ou não fungível. Isso pode ser usado de várias maneiras, por exemplo, para vender tokens, para recompensar tokens como parte de uma mecânica de jogo, para mudar como um jogador interage com uma scene se ele possuir certos tokens, etc.

Note que todas as transações na mainnet Ethereum que forem acionadas por uma scene exigirão que um jogador aprove e pague uma taxa de gas.

Todas as operações de blockchain também precisam ser realizadas como [funções assíncronas](/docs/creator/content-creator-pt/scenes-sdk7/padroes-de-programacao/async-functions/), já que o timing depende de eventos externos.

## Obter a conta Ethereum de um jogador

Para obter a conta Ethereum de um jogador, use a `getPlayer()` .

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

Note que, se um jogador tiver entrado no Decentraland como guest, ele não terá uma wallet Ethereum conectada. Se estiverem conectados como guests, o `isGuest` campo na resposta de `getPlayer()` será true. Caso contrário, você pode obter o endereço da wallet do jogador a partir do campo `userId` . Saiba mais sobre os dados que você pode obter de um jogador em [obter dados do jogador](/docs/creator/content-creator-pt/scenes-sdk7/interatividade/user-data/#get-player-data)


> [!WARNING]
> **📔 Nota**: Mesmo que o endereço eth possa conter caracteres maiúsculos, alguns browsers convertem a string retornada automaticamente para minúsculas. Se quiser comparar valores de endereço e fazer com que funcione em todos os browsers, use o método `.toLowerCase()` para converter o valor para minúsculas.


## Verificar preço do gas

Depois de importar a biblioteca `eth-connect` , você deve instanciar um web3 provider e um request manager, o que permitirá conectar-se via web3 ao Metamask no browser do jogador.

A função abaixo obtém o preço atual do gas na mainnet Ethereum e o imprime.

```ts
import { RequestManager } from "eth-connect";
import { createEthereumProvider } from "@dcl/sdk/ethereum-provider";

executeTask(async function () {
  // criar uma instância do web3 provider para interagir com o Metamask
  const provider = createEthereumProvider();
  // Criar o objeto que vai tratar do envio e receção de mensagens RPC
  const requestManager = new RequestManager(provider);
  // Verificar o preço atual do gas na rede Ethereum
  const gasPrice = await requestManager.eth_gasPrice();
  // registar a resposta
  console.log({ gasPrice });
});
```


> [!NOTE]
> **💡 Dica**: Note que as funções tratadas pelo `requestManager` devem ser chamadas usando `await`, já que dependem de obter dados externos e podem levar algum tempo para serem concluídas.


## Importar um ABI de contrato

Um ABI (Application Binary Interface) descreve como interagir com um contrato Ethereum, determinando que funções estão disponíveis, quais inputs recebem e o que devolvem. Cada contrato Ethereum tem o seu próprio ABI; deve importar os ABIs de todos os contratos que quiser usar no seu projeto.

Por exemplo, aqui está um exemplo de uma função no ABI do MANA:

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

As definições de ABI podem ser bastante extensas, pois muitas vezes incluem muitas funções, por isso recomendamos colar o conteúdo JSON de um ficheiro ABI num `.ts` ficheiro separado e importá-lo daí para outros ficheiros da scene. Também recomendamos manter todos os ficheiros ABI numa pasta separada da sua scene, chamada `/contracts`.

```ts
import { abi } from "../contracts/mana";
```

Aqui estão links para diferentes contratos do Decentraland. Obtenha o ABI de cada contrato clicando em *Export ABI* e escolhendo *JSON Format*.

* [MANA Token ABI](https://etherscan.io/address/0x0f5d2fb29fb7d3cfee444a200298f468908cc942#code)
* [Decentraland Marketplace](https://etherscan.io/address/0x19a8ed4860007a66805782ed7e0bed4e44fc6717#code)
* [LAND ABI](https://etherscan.io/address/0xf87e31492faf9a91b02ee0deaad50d51d56d5d4d#code)
* [Estate ABI](https://etherscan.io/address/0x959e104e1a4db6317fa58f8295f586e1a978c297#code)
* [AvatarNameRegistry ABI](https://etherscan.io/address/0x894b883905bfEe2CC448880F1b59f4A762E67566)
* [Catalyst ABI](https://etherscan.io/address/0xcc054fab08127c19f621ab83ade5962cd10584ec)

Estes são os contratos para as várias coleções de wearables: (cada coleção foi emitida como um contrato separado)

* [ExclusiveMasksCollection ABI](https://etherscan.io/address/0xc04528c14c8ffd84c7c1fb6719b4a89853035cdd)
* [Halloween2019Collection ABI](https://etherscan.io/address/0xc1f4b0eea2bd6690930e6c66efd3e197d620b9c2)
* [Halloween2019CollectionFactory ABI](https://etherscan.io/address/0x07ccfd0fbada4ac3c22ecd38037ca5e5c0ad8cfa)
* [Xmas2019Collection ABI](https://etherscan.io/address/0xc3af02c0fd486c8e9da5788b915d6fff3f049866)
* [MCHCollection ABI](https://etherscan.io/address/0xf64dc33a192e056bb5f0e5049356a0498b502d50)
* [CommunityContestCollection ABI](https://etherscan.io/address/0x32b7495895264ac9d0b12d32afd435453458b1c6)
* [DCLLaunchCollection ABI](https://etherscan.io/address/0xd35147be6401dcb20811f2104c33de8e97ed6818)
* [DCGCollection ABI](https://etherscan.io/address/0x3163d2cfee3183f9874e2869942cc62649eeb004)


> [!NOTE]
> **💡 Dica**: Para ver claramente as funções expostas por um contrato, abra-o em [abitopic.io](https://abitopic.io). Basta colar lá o endereço do contrato e abrir o separador *functions* para ver a lista completa de funções suportadas e os seus argumentos. Pode até testar a chamada das funções com parâmetros diferentes através da página web.


Configurar TypeScript para conseguir importar de um ficheiro JSON tem as suas dificuldades. A solução alternativa mais simples recomendada é alterar a extensão do ficheiro `ABI.JSON` para `.ts` e modificá-lo ligeiramente para que o seu conteúdo comece com `export default`.

Por exemplo, se o conteúdo do ficheiro ABI começar com `[{"constant":true,"inputs":[{"internalType":"bytes4" ...etc`, modifique-o para que comece com `export default [{"constant":true,"inputs":[{"internalType":"bytes4" ...etc`.

### Instanciar um contrato

Depois de importar a biblioteca `eth-connect` a biblioteca e o abi de um contrato *abi*, você deve instanciar vários objetos que lhe permitirão usar as funções no contrato e ligar-se ao Metamask no browser do jogador.

Também deve importar o web3 provider. Isto porque o Metamask no browser do jogador usa web3, por isso precisamos de uma forma de interagir com isso.

```ts
import { RequestManager, ContractFactory } from "eth-connect";
import { createEthereumProvider } from "@dcl/sdk/ethereum-provider";
import { abi } from "../contracts/mana";

executeTask(async () => {
  // criar uma instância do web3 provider para interagir com o Metamask
  const provider = createEthereumProvider();
  // Criar o objeto que vai tratar do envio e receção de mensagens RPC
  const requestManager = new RequestManager(provider);
  // Criar um objeto factory baseado no abi
  const factory = new ContractFactory(requestManager, abi);
  // Use o objeto factory para instanciar um objeto `contract`, referenciando um contrato específico
  const contract = (await factory.at(
    "0x2a8fd99c19271f4f04b1b7b9c4f7cf264b626edb"
  )) as any;
});
```


> [!NOTE]
> **💡 Dica**: Para contratos que seguem um mesmo padrão, como ERC20 ou ERC721, pode importar um único ABI genérico para todos. Depois, gera um único `ContractFactory` objecto com esse ABI e use essa mesma factory para instanciar interfaces para cada contrato.


### Chamar os métodos de um contrato

Depois de criar um objeto `contract` , pode chamar facilmente as funções definidas no seu ABI, passando os parâmetros de input especificados.

```ts
import { getPlayer } from "@dcl/sdk/src/players";
import { createEthereumProvider } from "@dcl/sdk/ethereum-provider";
import { RequestManager, ContractFactory } from "eth-connect";
import { abi } from "../contracts/mana";

executeTask(async () => {
  try {
    // Passos de configuração explicados na secção acima
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

    // Executar uma função do contrato
    const res = await contract.setBalance(
      "0xaFA48Fad27C7cAB28dC6E970E4BFda7F7c8D60Fb",
      100,
      {
        from: userData.userId,
      }
    );
    // Registar a resposta
    console.log(res);
  } catch (error: any) {
    console.log(error.toString());
  }
});
```

O exemplo acima usa o abi de um *falso MANA* contrato de teste, e chama o seu método `setBalance` para conceder 100 *falso MANA* a uma conta. Ao adaptar este exemplo, substitua o endereço e o abi pelos do contrato que quer chamar, e chame os métodos que o seu abi define.

### Outras funções

A biblioteca eth-connect inclui vários outros auxiliares que pode usar. Por exemplo, para:

* Obter um preço de gas estimado
* Obter o saldo de um endereço fornecido
* Obter um recibo de transação
* Obter o número de transações enviadas a partir de um endereço
* Converter entre vários formatos, incluindo hexadecimal, binário, utf8, etc.

## Usando a rede de teste Ethereum

Ao testar a sua scene, para evitar transferir MANA real ou outras moedas, pode usar a *rede de teste Ethereum Sepolia* e transferir MANA falso da testnet em vez disso.

Para usar a rede de teste, você deve configurar sua extensão Metamask no Chrome para usar a *rede de teste Sepolia* em vez da *rede principal*.

Tem de adquirir Sepolia Ether, que pode obter gratuitamente em vários faucets externos como [este](https://www.alchemy.com/faucets/ethereum-sepolia/).


> [!NOTE]
> **💡 Dica**: Para executar a transação de transferir MANA Sepolia para sua wallet, você precisará pagar uma taxa de gas em Ether Sepolia.


Para visualizar sua cena usando a rede de teste, cole a seguinte URL em uma aba do navegador. Isso abrirá a cena no cliente de desktop do Decentraland:

`decentraland://realm=http://127.0.0.1:8000&local-scene=true&debug=true&dclenv=zone&position=0,0`


> [!NOTE]
> **💡 Dica**: Altere o parâmetro position para as coordenadas da sua scene, para carregar diretamente na sua scene..


Quaisquer transações que você aceitar enquanto visualiza a cena neste modo ocorrerão apenas na rede de teste e não afetarão o saldo de MANA na sua wallet real.

Se precisar de testar transações na Polygon Testnet e precisar de ter MANA nessa testnet, terá de fazer swap de MANA para essa rede depois de o adquirir em Sepolia. Para fazer bridge do MANA de Sepolia para a Polygon Testnet, visite a sua [página da conta do Decentraland em Sepolia](https://account.decentraland.zone/) e clique em ‘swap’ no lado Ethereum MANA.

Ao executar um Preview no cliente web legado, de uma scene que usa uma das bibliotecas ethereum, deve abrir o Preview numa janela separada do browser, ter o Metamask aberto no seu browser e incluir manualmente a string `&ENABLE_WEB3`.

## Enviar mensagens RPC personalizadas

Use a função `sendAsync()` para enviar mensagens através do [protocolo RPC](https://en.wikipedia.org/wiki/Remote_procedure_call).

```ts
import { sendAsync } from "~system/EthereumController";

// enviar uma mensagem
await sendAsync({
  id: 1,
  method: "myMethod",
  jsonParams: "{ myParam: myValue }",
});
```

## smart contracts do Decentraland

No link seguinte, pode encontrar uma lista de smart contracts Ethereum relevantes para o ecossistema do Decentraland. A lista inclui os contratos na mainnet, bem como noutras redes de teste Ethereum.

[contracts.decentraland.org](https://contracts.decentraland.org/links)


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-pt/scenes-sdk7/blockchain/scene-blockchain-operations.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
