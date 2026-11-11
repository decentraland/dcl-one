> Referência para criar sua própria aplicação descentralizada

# Criar um dApp

Você pode criar suas próprias aplicações descentralizadas (dApps) para interagir com os smart contracts da Decentraland e expor suas funcionalidades de formas mais elaboradas e amigáveis.

## O que é uma dApp

Uma aplicação descentralizada, ou dApp, é aquela que roda em uma rede distribuída ponto a ponto, em vez de a partir de um servidor central.

No contexto de blockchain, uma dApp usa smart contracts e possivelmente uma rede P2P, em vez de um serviço de API Web. Uma dApp também pode expor um front end e armazenar temporariamente informações do blockchain em cache, mas sua saída é, em última instância, refletida on-chain.

Veja [este site](https://blockchainhub.net/decentralized-applications-dapps/) para uma visão geral mais completa sobre dApps.

## smart contracts da Decentraland

A Decentraland escreveu e mantém uma série de smart contracts que interagem com os tokens LAND e MANA.

Os próprios tokens LAND e MANA são definidos pelos *contratos LANDregistry* e *contratos MANAtoken* respectivamente. A lista também inclui contratos mais específicos, como criar uma estate a partir de vários parcels ou hipotecar parcels.

* [smart contracts da Decentraland](https://contracts.decentraland.org/addresses.json): Você pode encontrar uma lista completa de cada um dos nossos contratos e seus endereços.

Observe que cada contrato tem uma versão de produção em *mainnet* e uma versão de teste em *sepolia* e que cada uma tem um endereço diferente.

Você pode ler o código completo de cada contrato, pois é informação pública no blockchain. Por exemplo, você pode encontrar o contrato pelo nome em [Etherscan](https://etherscan.io/contractsVerified) para ler seu conteúdo.

## código boilerplate de dApp

* [Código boilerplate](https://github.com/decentraland/dapp-boilerplate): Este código boilerplate pode ser um ponto de partida para construir sua própria dApp. Observe que este repositório está arquivado e não é mais mantido, então trate-o como referência, e não como um template atualizado.

## Bibliotecas auxiliares

Enquanto construíamos nossas próprias dApps internamente, reunimos algumas bibliotecas auxiliares que você também pode achar úteis.

* [Decentraland-dapps](https://github.com/decentraland/decentraland-dapps): Esta é uma biblioteca de nível mais alto com módulos comuns para dApps, usada ativamente pelas próprias aplicações da Decentraland.
* [Decentraland UI](https://ui.decentraland.org/): Esta biblioteca contém uma seleção de elementos de UI reutilizáveis que estão incluídos nos projetos da Decentraland.
* [Decentraland-eth](https://github.com/decentraland/decentraland-eth): Esta é uma biblioteca de baixo nível com funções utilitárias para trabalhar com o blockchain Ethereum. Observe que este repositório está arquivado e não é mais mantido.

## dApps de exemplo

Abaixo estão links para o código completo de várias dApps que construímos em torno da Decentraland; eles podem ajudar você a criar a sua própria:

* [Gate](https://github.com/decentraland/gate): Esta dApp básica cria um NFT de convite que foi usado quando a Decentraland estava em estágio de beta fechado. Nos primeiros tempos, os jogadores na whitelist que possuíam este NFT eram os únicos que conseguiam entrar no metaverso.
* [Marketplace](https://github.com/decentraland/marketplace): Esta é a aplicação completa que roda a Decentraland [Marketplace](https://market.decentraland.org/). Para fazê-lo rodar rapidamente, ele requer um banco de dados e um servidor backend conectado à rede Ethereum para criar índices com informações sobre LAND.

## framework de testes para dApp

Antes de lançar sua dApp em produção, recomendamos testá-la primeiro.

* [dAppeteer](https://github.com/decentraland/dappeteer): Criamos este framework para ajudar você a executar testes na sua dApp. Observe que este repositório está arquivado e não é mais mantido.

## smart contracts da Decentraland

No link a seguir, você pode encontrar uma lista de smart contracts da Ethereum relevantes para o ecossistema da Decentraland. A lista inclui os contratos em mainnet, bem como em outras redes de teste da Ethereum.

[contracts.decentraland.org](https://contracts.decentraland.org/links)


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-pt/scenes-sdk7/blockchain/create-a-dapp.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
