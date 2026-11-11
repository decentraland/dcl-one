> Coisas importantes a ter em mente se você quiser integrar o Estate do Decentraland ao seu marketplace

# Integrações de Marketplace para Estates

## Integrando a Estate da Decentraland no seu Marketplace

A Estate da Decentraland é um NFT compatível com ERC721 e funciona na Ethereum Mainnet. Portanto, muitos Marketplaces de terceiros podem negociá-la. Para isso, este Marketplace deve seguir certas regras para manter as transações seguras para os utilizadores.

Como talvez saiba, cada [Estate](https://docs.decentraland.org/decentraland/faq/#what-is-an-estate) é um grupo de [LANDs](https://docs.decentraland.org/decentraland/faq/#what-is-land) e tem um *bytes* hash associado ao grupo, chamado *`fingerprint`*. Sempre que a Estate muda ao adicionar ou remover uma LAND, o seu *`fingerprint`* também muda.

Para Marketplaces, especialmente os que não têm um sistema de escrow, é 100% recomendado ter um registo do *`fingerprint`* da Estate quando alguém a coloca à venda ou faz uma oferta. Dessa forma, quando a ordem/oferta é executada com sucesso, o proprietário atual não pode alterar a Estate tentando executar a ordem antecipadamente.

Não é recomendado listar Estates vazias. O smart contract da Estate tem um método `getEstateSize` que devolve o número de LANDs na Estate. Se o resultado for `0`, recomendamos não listar essa Estate. Se quiser listá-la na mesma, verá uma imagem como esta:

![](/docs/_remote/3ea9308d2dc5bf30fe069b53.webp)

## Exemplo

### Não usar o fingerprint da Estate

* O Bob tem a Estate1 com as LAND (1,1) e (1,2). fingerprint da Estate1: hash1
* O Bob adiciona a LAND (1,3) à Estate1. A Estate1 tem as LANDs: (1,1), (1,2) e (1,3). fingerprint da Estate1: hash2 (fingerprint alterado)
* O Bob remove a LAND (1,1) da Estate1. A Estate1 tem as LANDs: (1,2) e (1,3). fingerprint da Estate1: hash3 (fingerprint alterado)
* O Bob coloca a Estate1 à venda. A listagem é criada onchain na Ethereum Mainnet com o endereço do smart contract da Estate, o id da Estate, o preço e a data de expiração.
* A Alice envia uma transação para comprar a Estate, especificando o id da Estate e o preço esperado a pagar.
* O Bob deteta que alguém está a tentar comprar a sua Estate1 e envia uma transação com taxas de gas mais altas do que as da Alice para remover as LANDs (1,2) e (1,3) da Estate1.
* A transação do Bob é minerada primeiro. A Estate1 fica com 0 LANDs. fingerprint da Estate1: hash4 (fingerprint alterado)
* A transação da Alice é minerada depois. A Alice comprou a Estate1 com 0 LANDs nela. Isto significa que a Alice sofreu front-running (e foi roubada/enganada) pelo Bob.

### Usando o fingerprint da Estate

* O Bob tem a Estate1 com as LAND (1,1) e (1,2). fingerprint da Estate1: hash1
* O Bob adiciona a LAND (1,3) à Estate1. A Estate1 tem as LANDs: (1,1), (1,2) e (1,3). fingerprint da Estate1: hash2 (fingerprint alterado)
* O Bob remove a LAND (1,1) da Estate1. A Estate1 tem as LANDs: (1,2) e (1,3). fingerprint da Estate1: hash3 (fingerprint alterado)
* O Bob coloca a Estate1 à venda. A listagem é criada onchain na Ethereum Mainnet com o endereço do smart contract da Estate, o id da Estate, o preço e a data de expiração.
* A Alice envia uma transação para comprar a Estate, especificando o id da Estate, o preço esperado a pagar e **o fingerprint que ela viu (hash3)**.
* O Bob deteta que alguém está a tentar comprar a sua Estate1 e envia uma transação com taxas de gas mais altas do que as da Alice para remover as LANDs (1,2) e (1,3) da Estate1.
* A transação do Bob é minerada primeiro. A Estate1 fica com 0 LANDs. fingerprint da Estate1: hash4 (fingerprint alterado)
* A transação da Alice é revertida porque o smart contract verificou que o fingerprint no parâmetro que a Alice enviou não correspondia ao fingerprint atual da Estate1 (hash3 != hash4). Esta verificação impediu a Alice de comprar uma Estate indesejada.

## Interface de Fingerprint do Smart Contract da Estate

O smart contract da Estate é compatível com uma [interface de fingerprint](https://github.com/decentraland/land/blob/master/contracts/estate/EstateStorage.sol#L19). Quando uma Estate é listada para venda ou recebe uma oferta, registe o seu fingerprint chamando *`getFingerprintV2(uint256 estateId)`* e guarde o valor que devolve. Para verificar se essa ordem/oferta ainda é válida no momento da execução, chame *`verifyFingerprint(uint256 estateId, bytes fingerprint)`* com o valor guardado.

Guarde sempre o valor devolvido por *`getFingerprintV2`*. A versão mais antiga *`getFingerprint`* do método é mantida apenas para compatibilidade com versões anteriores e está a ser descontinuada: a partir de 26 de novembro de 2026, *`verifyFingerprint`* deixará de aceitar valores produzidos por ele. As novas integrações devem usar *`getFingerprintV2`*.

Pode consultar um exemplo de produção funcional [aqui](https://github.com/decentraland/marketplace-contracts/blob/master/contracts/marketplace/MarketplaceV2.sol#L382).


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-pt/marketplace/estates-marketplace-integrations.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
