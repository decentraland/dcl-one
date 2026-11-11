> Use a sidechain da Polygon em sua scene para habilitar transações blockchain muito mais rápidas e baratas.

# Segunda Camada

## Sobre soluções de segunda camada

Qualquer transação que afete a blockchain leva tempo para ser concluída e custa gas. Essas duas coisas são obstáculos para tornar os jogos em blockchain populares, porque os jogadores geralmente não têm paciência para esperar tanto tempo para que suas ações tenham efeito e não estão dispostos a gastar dinheiro em muitas transações enquanto jogam.

Uma solução comum é manter a maior parte da gameplay fora da blockchain e realizar transações na blockchain apenas para eventos-chave, como ganhar um item do jogo ou registrar uma pontuação alta.

No entanto, outra forma de superar essas limitações, bem como outros problemas de escalabilidade inerentes às blockchains, é recorrer a uma *segunda camada* de blockchain, também chamada de uma *side-chain*.

Uma segunda camada é outra blockchain que atua como intermediária entre uma aplicação descentralizada e a main chain. Essa camada é mais leve e, portanto, pode oferecer respostas mais rápidas e com um custo de gas muito menor.

Em vez de fazer transações diretamente na main chain, as transações são feitas na side-chain e, depois, cabe à side-chain sincronizar eventualmente essas mudanças com a main chain.

A side-chain consegue oferecer respostas mais rápidas porque é uma rede menor, com menos nodes. As transações realizadas na segunda camada são inicialmente menos seguras, mas acabam sendo confirmadas na main chain em lotes, e todas as verificações de segurança da main chain podem ser aplicadas ali.

A side-chain também consegue reduzir significativamente os custos de transações individuais porque agrupa muitas transações em uma única ao sincronizar com a main chain. Assim, o gas que precisaria ser pago por uma transação na main chain pode ser dividido entre vários milhares de transações.

Além disso, ao sincronizar com a main chain, várias transações redundantes podem ser evitadas por completo, reduzindo ainda mais os custos de transação. Por exemplo, se Alice transfere 1 ETH para Bob, e depois Bob transfere 1 ETH para Carol, então essas duas transações podem ser simplificadas em uma só, registrando que Alice transfere diretamente para Carol.

Decentraland tem uma parceria com [Polygon](https://polygon.technology/), que fornece sua própria side-chain na Ethereum. A side-chain deles pode ser usada por scenes para oferecer transações rápidas e muito baratas.

## Experimente a Polygon Testnet

Para usar a Matic Testnet da Polygon, você deve primeiro ter:

* Ether na rede Sepolia. Você pode obtê-lo gratuitamente em vários faucets externos, como [este](https://www.alchemy.com/faucets/ethereum-sepolia).

Em seguida, você deve configurar sua conta do Metamask para incluir a Matic Testnet.

Para fazer isso:

1. Alterne as redes no Metamask clicando no nome da rede atual e selecionando *Custom RPC* na parte inferior do dropdown.
2. Preencha 'Polygon Amoy Testnet' como *Network name*, [**https://rpc-amoy.polygon.technology**](https://rpc-amoy.polygon.technology) como *New RPC URL* e [**https://amoy.polygonscan.com**](https://amoy.polygonscan.com) como *Block Explorer URL*. Para *Symbol* defina **POL**
3. Clique em 'Save' para adicionar esta nova rede ao Metamask.


> [!WARNING]
> **📔 Nota**: Saiba mais sobre como configurar a Polygon na sua [página de documentação](https://docs.polygon.technology/)



---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-pt/scenes-sdk7/blockchain/second-layer.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
