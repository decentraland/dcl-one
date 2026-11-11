> Perguntas frequentes sobre transações em Polygon

# Transações em Polygon

## O que é Polygon?

Tal como indicado no website oficial da Polygon, [Polygon](https://polygon.technology/) é um protocolo e um framework para construir e conectar redes blockchain compatíveis com Ethereum. Agregando soluções escaláveis no Ethereum, suportando um ecossistema Ethereum multi-chain."

## O que é Matic?

MATIC é o token nativo da Polygon. É uma criptomoeda usada para cobrir taxas de gas na rede Polygon, entre outros casos de uso. MATIC está para a Polygon assim como ETH está para Ethereum.

Pode comprar MATIC na maioria das exchanges de criptomoedas.

## Polygon no Marketplace e no Builder da Decentraland

Ao usar a rede Polygon, e graças à DAO da Decentraland, os utilizadores podem listar, vender e comprar wearables no Marketplace ou publicar as suas coleções no Builder sem pagar o gas da transação usando os serviços de meta-transactions.

**As transações em Polygon não são gratuitas**. A DAO da Decentraland cobre o custo das transações em Polygon para que os utilizadores possam desfrutar de muitas transações sem custo no Marketplace.

Para desfrutar de transações sem custo, há três condições que precisam de ser cumpridas:

* É necessário estar ligado à Ethereum Mainnet.
* O item que pretende comprar precisa de ter um preço de 1 MANA ou superior.
* Não atingiu o seu limite de transações gratuitas.

A DAO da Decentraland reserva-se o direito de consumir ou pausar os serviços de meta-transactions quando as taxas de gas da rede são elevadas, para evitar o consumo dos tanques de gas, de modo a que durem mais e possam ser utilizados pelo maior número possível de pessoas.

## Porque tenho de cobrir as taxas de transação de itens que custam menos de 1 MANA?

Para evitar exploração e proteger o serviço gratuito de gas, os itens que custam menos de 1 MANA não estão incluídos nas transações sem custo. Pode comprar estes itens ligando-se à rede Polygon com MATIC na sua wallet.

As taxas de gas são variáveis.

## O que posso fazer se as taxas da rede forem superiores a 300 gwei?

Gwei é uma unidade de medida usada na blockchain Ethereum. É abreviação de "Giga-wei", onde "giga" denota mil milhões. No Polygon, gwei é a menor denominação de MATIC, equivalente a 1/1.000.000.000 de 1 MATIC (1 MATIC = 1.000.000.000 gwei). Todas as taxas de transação na rede Polygon são denominadas em gwei.

As taxas da rede (expressas em gwei) são variáveis, por isso o melhor que pode fazer é esperar e tentar novamente noutra altura. [Aqui](https://polygonscan.com/gastracker) pode verificar a taxa de gas da Polygon em tempo real.

Em alternativa, pode comprar estes itens ligando-se à rede Polygon para usar o MATIC na sua wallet para pagar as taxas. A taxa em MATIC será deduzida automaticamente como parte da taxa de transação da rede Polygon. Só precisa de estar ligado à rede Polygon e ter MATIC na sua wallet.

As taxas de gas são variáveis.

## O que acontece se o limite de transações gratuitas for atingido?

O limite de transações gratuitas renova-se todos os dias, por isso pode tentar no dia seguinte. Em alternativa, pode cobrir o custo da sua transação com MATIC enquanto estiver ligado à rede Polygon.

A taxa em MATIC será deduzida automaticamente como parte da taxa de transação da rede Polygon. Só precisa de estar ligado à rede Polygon e ter MATIC na sua wallet.

As taxas de gas são variáveis.

## Onde posso obter MATIC para pagar as taxas de transação?

Uma forma de obter MATIC é comprá-lo através do [a dapp da Conta](https://account.decentraland.org/).

Basta clicar no botão BUY na secção Polygon MANA e trocar a cripto que pretende comprar de MANA para MATIC.

Outra forma é através de exchanges descentralizadas ou centralizadas. Pode consultar <https://polygon.technology/matic-token/> para ver que exchanges operam com MATIC. Se quiser usar uma exchange centralizada, certifique-se de que permite levantamentos através da rede Polygon.

Se tiver MATIC na rede Ethereum, pode sempre usar a [Polygon Bridge](https://wallet.polygon.technology/bridge/) para depositar esse MATIC na rede Polygon.


> [!WARNING]
> **Aviso:** Depois de concluir uma transação no Polygon, lembre-se de voltar a mudar para a Ethereum Mainnet para usufruir de todas as funcionalidades no Marketplace e no Builder que não são suportadas na rede Polygon.



---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/player/player-pt/integracao-com-a-blockchain/transactions-in-polygon.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
