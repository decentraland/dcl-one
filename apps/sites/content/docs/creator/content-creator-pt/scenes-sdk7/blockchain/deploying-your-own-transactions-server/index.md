> Ofereça aos usuários transações sem custo

# Implantando seu Servidor de Transações

Os [transactions-server](https://github.com/decentraland/transactions-server/tree/1.8.0) é um servidor proxy que encaminha transações para [Gelato](https://www.gelato.network/relay). Ele recebe uma transação assinada do cliente, que é então enviada para a rede apropriada nos bastidores. Isso permite que o proprietário do servidor facilite para seus usuários transações sem custo

O servidor de transações é usado para ajudar com a UX de usar várias redes e para evitar que eles troquem de provedor de rede em tempo real. Os usuários podem permanecer conectados a [Ethereum](https://ethereum.org/en/) e interagir com [Polygon](https://polygon.technology/) [assinando apenas transações](https://docs.decentraland.org/blockchain-integration/transactions-in-polygon/)

A DAO da Decentraland montou um servidor usado pelos nossos dapps, cobrindo o custo até um certo limite com algumas [restrições](#restrictions). Este documento explica como você pode [implantar este servidor](#running-the-server) para permitir que seus usuários encaminhem transações com as restrições de que você precisar, se houver.

## Restrições

Todas as restrições são por transação que o usuário tenta enviar. O que, na prática, se traduz em uma solicitação POST para o servidor.

As restrições configuráveis que o servidor possui são:

* Verifica uma cota de máximo de transações por dia. Veja [a seção de transações](#transactions) para mais informações.
* Verifica contratos na lista de permissões, então se uma transação estiver tentando interagir com um contrato que não é reconhecido, ela falhará. Para isso, ele usa
  * Os contratos e coleções implantados. Veja [a seção de contratos e coleções](#contracts-and-collections) para mais informações
* O preço das vendas, restringindo-o se estiver abaixo de um limite. Veja [a seção de valor mínimo de venda](#min-sale-value) para mais informações

## Configurando o Gelato

[Gelato](https://www.gelato.network/relay) é um Protocolo de Relayer Multichain. Usamos a infraestrutura dele para permitir transações sem custo. Isso significa, na prática, que, quando você for enviar uma transação, você estará assinando uma mensagem e enviando isso para o Gelato. O serviço cuidará de enviar a transação para você e de devolver uma resposta (hash da transação).

Ele precisa de um contrato para encaminhar as transações, mas felizmente podemos reutilizar o que está sendo usado pela Decentraland (veja abaixo)

[Gelato](https://www.gelato.network/relay) funciona como uma API para o servidor. [Para configurar](#configuring-the-server) isso, primeiro você precisará de uma API KEY, que nós [usaremos depois](#gelato). Para obter isso:

* [Registre-se](https://app.gelato.network/relay) no serviço
* Crie um novo dapp para a rede que você pretende direcionar. Para imitar a configuração da Decentraland:
  * Selecione a `Mainnets` opções
  * Defina um `App name` apropriado para o seu dApp
  * escolha `Polygon` como a rede para o `Smart Contract`
  * Ative a `Any Contract` opção de alternância
* Copie a API KEY do `API Key` seção

Por fim, você precisará financiar sua dapp recém-criada. Você pode fazer isso conectando sua Wallet na [1Balance](https://app.gelato.network/1balance) seção na barra lateral esquerda. Uma vez conectado, isso permitirá que você deposite seu [USDC](https://polygonscan.com/token/0x3c499c542cef5e3811e1192ce70d8cc03d5c3359) para financiar as transações que seus usuários enviarão. Se você precisar obter MATIC, confira este [post](https://docs.decentraland.org/blockchain-integration/transactions-in-polygon/#where-can-i-get-matic-to-pay-for-transaction-fees).

### Testnet

Se você quiser testar seu app antes de entrar no ar e estiver usando Polygon, você pode fazer isso em `Polygon Amoy`, a testnet do Polygon.

Para fazer isso, basta repetir [o processo](#configuring-gelato) mas escolhendo `Matic Testnet (Amoy)` no campo de rede.

Você precisará financiar sua dapp, mas pode fazer isso facilmente obtendo tokens Sepolia ETH do [faucet](https://sepoliafaucet.com/).

## Baixando o servidor de transações

Primeiramente, você precisará de uma cópia do código do transactions-server da Decentraland. Você pode encontrá-lo [no GitHub](https://github.com/decentraland/transactions-server/tree/v1). A partir daí, você tem duas opções:

1. **Baixando o código**: Para baixar o código, primeiro você precisa clicar no botão verde `Code` e depois em

* Clique em `Download ZIP`
* Copie a URL abaixo do título `Clone` e então execute `$ git clone THE_URL_HERE`

2. **Fazendo fork do código**: Você pode clicar no botão `fork` no canto superior direito da página. Quando o processo estiver concluído, você poderá baixar seu código da mesma forma que faria na primeira opção. Você precisará de uma conta no GitHub para isso; para mais informações sobre fazer fork de repositórios, veja [aqui](https://docs.github.com/en/enterprise-server@3.5/get-started/quickstart/fork-a-repo)

## Configurando o servidor

O servidor de transações foi escrito em [NodeJS](https://nodejs.org/en/) usando [Typescript](https://www.typescriptlang.org/). Antes de executá-lo, você precisará configurar algumas variáveis de ambiente.

Para fazer isso:

* Copie o `.env.defaults` arquivo e cole-o renomeado para `.env`
* Abra o `.env` arquivo. Você verá que algumas variáveis têm um valor padrão, como `HTTP_SERVER_PORT=5000` (em qual porta executar o servidor)
* Você pode deixar a maioria dos valores como estão, mas há alguns valores importantes a considerar:
  * [Gelato](#gelato)
  * [Transações](#transactions)
  * [Contratos e coleções](#contracts-and-collections)
  * [Valor mínimo de venda](#min-sale-value)

### Gelato

Use a API KEY que obtivemos ao [configurar o Gelato](#configuring-gelato).

```
GELATO_API_KEY=<YOUR_GELATO_API_KEY>
```

### Transações

Quando uma nova solicitação de transação chegar, ele verificará o valor **um endereço** enviou naquele dia. Se estiver acima do valor definido, a transação falhará.

```
MAX_TRANSACTIONS_PER_DAY=10
```

Para remover completamente essa verificação, você pode ir ao código e remover a

```ts
await checkQuota(components, transactionData)
```

método de `async function checkData(transactionData: TransactionData): Promise<void> {` em `src/ports/transaction/component.ts`

### Contratos e coleções

O servidor buscará a URL dos endereços dos contratos, armazenará localmente e consultará o subgraph. Quando uma nova solicitação de transação chegar, ele verificará se o contrato com o qual a transação está interagindo pertence aos contratos implantados na URL ou às coleções implantadas no subgraph.

Se você quiser fornecer seus próprios contratos, altere a URL e mantenha a mesma estrutura que a atual <https://contracts.decentraland.org/addresses.json> tem. A rede usada é determinada por COLLECTIONS\_CHAIN\_ID, e o intervalo com que o cache é buscado novamente é COLLECTIONS\_FETCH\_INTERVAL\_MS

Se você tiver suas próprias collections, também pode alterar a URL do subgraph.

Para remover completamente essas verificações, você pode ir ao código e remover a

```ts
await checkContractAddress(components, transactionData)
```

método de `async function checkData(transactionData: TransactionData): Promise<void> {` em `src/ports/transaction/component.ts`

```
CONTRACT_ADDRESSES_URL=https://contracts.decentraland.org/addresses.json
COLLECTIONS_FETCH_INTERVAL_MS=3600000
COLLECTIONS_CHAIN_ID=80002

COLLECTIONS_SUBGRAPH_URL=https://subgraph.decentraland.org/decentraland/collections-matic-amoy
```

### Valor mínimo de venda

Quando uma nova solicitação de transação chegar, ela primeiro analisará os dados que está tentando retransmitir. Se detectar uma venda (compra no Marketplace, lance etc.), verificará o valor em relação a MIN\_SALE\_VALUE\_IN\_WEI. Se for menor, a transação falhará.

```
MIN_SALE_VALUE_IN_WEI=1000000000000000000
```

Para verificar os métodos de venda relevantes, você pode ver `src/ports/transaction/validation/checkSalePrice.ts` e, para remover completamente essa verificação, você pode ir ao código e remover a

```ts
await checkSalePrice(components, transactionData)
```

método de `async function checkData(transactionData: TransactionData): Promise<void> {` em `src/ports/transaction/component.ts`

## Executando o servidor

Agora que toda a configuração está definida, o que resta é realmente executar o servidor. Você pode seguir [seu README](/docs/creator/content-creator-pt/scenes-sdk7/blockchain/deploying-your-own-transactions-server/) mas, em resumo, você terá que:

* Ter [NodeJS](https://nodejs.org/en/) instalado
* Abra seu terminal de sua preferência
* Execute os seguintes comandos:

```bash
npm install
npm run migrate # apenas na primeira execução
npm run start
```

É claro que você provavelmente vai querer fazer deploy disso no serviço de sua preferência, como [AWS](https://aws.amazon.com/) por exemplo. Você pode usar o [Dockerfile](https://github.com/decentraland/transactions-server/blob/master/Dockerfile) para o fazer.

## Usando o servidor

Agora que tudo está configurado e a funcionar, é hora de usar realmente o servidor.

Para enviar efetivamente uma transação, precisa de fazer POST para `/transactions`. O schema necessário para o request é definido por `transactionSchema` em `src/ports/transaction/types.ts`.

Se, em vez disso, quiser usar as nossas libs pré-fabricadas para facilitar a sua vida, pode experimentar [decentraland-transactions](https://github.com/decentraland/decentraland-transactions). É usado via [decentraland-dapps](https://github.com/decentraland/decentraland-dapps) nas nossas dapps, como a [Marketplace](https://market.decentraland.org), com as utils [`sendTransaction`](https://github.com/decentraland/decentraland-dapps/blob/master/src/modules/wallet/utils.ts#L104). Veja [este código](https://github.com/decentraland/marketplace/blob/a2191515c6ae7ede54a685cc2dd9f9fafa35366b/webapp/src/modules/vendor/decentraland/OrderService.ts#L33) como exemplo.


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-pt/scenes-sdk7/blockchain/deploying-your-own-transactions-server.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
