> Como a Decentraland usa a blockchain Ethereum.

# Sobre a Blockchain

Todas as blockchains são, na sua essência, bases de dados descentralizadas distribuídas pelas máquinas de uma rede. As transações são agrupadas em "blocos" e processadas sequencialmente para formar uma *cadeia* de eventos.

O Ethereum é uma das blockchains mais populares. O que o distingue de outras, como o Bitcoin, é que usa a blockchain como armazenamento para mais do que apenas um registo de transações de moeda. O Ethereum pode armazenar informação mais complexa para distinguir diferentes tipos de tokens ou até lidar com tokens únicos com características específicas. A blockchain do Ethereum também executa smart contracts, que permitem realizar transações mais complexas que também podem depender de eventos acordados.

Decentraland usa a blockchain do Ethereum para registar a propriedade dos ativos digitais e de outros itens negociáveis que podem ser lidos e aos quais uma scene 3D pode reagir.

A blockchain não é usada para armazenar o estado da scene, a posição do player ou qualquer coisa que precise de mudar em tempo real à medida que um player interage com uma scene; tudo isso é armazenado localmente na máquina de cada player ou num servidor privado pertencente ao dono da scene. Os developers de cada scene têm de escolher que informação vale a pena guardar na blockchain e o que guardar num servidor privado.

## Carteiras

Os tokens do Ethereum são guardados em carteiras. Uma wallet Ethereum pode guardar vários tokens, incluindo Ether, MANA, LAND e outros tokens que possam ser usados por jogos ou experiências em Decentraland.

Existem muitos fornecedores de wallet onde pode guardar tokens de Decentraland. Para usar o Marketplace, ou para entrar em Decentraland, tem de usar uma wallet integrada no seu navegador web, por isso recomendamos que use:

* [Metamask](https://metamask.io/)
* [Trezor](https://trezor.io/)/[Ledger](https://www.ledger.com/) carteiras de hardware

Cada wallet tem uma chave pública e uma chave privada. O hash da sua chave pública é o endereço único da sua wallet, usado para encaminhar transações e identificar um player. A sua chave privada é usada pela sua wallet para assinar cada transação que envia para a rede e certificar que foi realmente enviada por si. A sua chave privada também é usada para restaurar a sua wallet caso se esqueça da sua palavra-passe, por isso guarde-a num local seguro e não a partilhe com ninguém.

Em Decentraland, as identidades dos players são construídas em torno de wallets. Como as chaves públicas das wallets são únicas, a sua scene pode usá-las para identificar um utilizador de Decentraland de forma persistente. As wallets também podem conter diferentes tokens que podem dar a um player um avatar único, um wearable item, permissões para entrar em scenes que optem por restringir o acesso, uma arma especial para usar num jogo, etc.

## Transações

As transações alteram a informação que está armazenada na blockchain. As transações típicas envolvem a mudança de proprietário dos tokens, por exemplo, o utilizador A dar o seu token LAND ao utilizador B em troca de uma quantidade de tokens MANA. Na rede Ethereum, no entanto, uma transação também pode significar alterar a informação armazenada sobre um token sem alterar o seu proprietário. Por exemplo, alterar a descrição de um parcel, ou fundir vários parcels num Estate.

Todas as transações que ocorrem na main chain do Ethereum têm um custo pago em tokens Ether. Esta taxa é designada por taxa de "gas", e é paga ao utilizador da rede que "mina" a transação.

Quando solicita que uma transação ocorra, define o preço de gas que está disposto a pagar para que a transação seja minerada. As transações que oferecem preços mais altos são mineradas mais depressa, uma vez que os miners lhes dão prioridade. Os preços de mercado destas transações oscilam regularmente; tendem a ser mais caros quando há maior utilização da rede. Certifique-se de que o que oferece não está abaixo do preço de mercado, caso contrário a sua transação pode permanecer indefinidamente num pool por processar.

Todas as transações têm de ser assinadas por um endereço Ethereum, usando a chave privada do endereço. É isto que certifica que a transação foi efetuada por esse endereço.

#### Validação de transações

As transações na blockchain não são imediatas, requerem tempo para serem "mineradas" por um dos nós da rede e depois propagadas pelo resto das máquinas. Quanto mais transações estiverem a ser solicitadas pela rede, mais tempo demoram a ser validadas.

Em termos sucintos, é assim que uma transação é validada:

1. Ocorre uma nova transação, que entra num pool de transações não confirmadas.
2. Uma das máquinas da rede resolve com sucesso um algoritmo para minerar um novo "bloco" que contém algumas transações deste pool, incluindo esta. Anexa este novo bloco ao final da cadeia.
3. O bloco é partilhado com outras máquinas da rede. Cada máquina verifica que cada transação num bloco é válida e verifica o hash do bloco para garantir que é legítimo; depois, adiciona-o à sua própria versão da cadeia.
4. O novo bloco é propagado por toda a rede. Existe uma compreensão partilhada universalmente de que esta transação ocorreu.

#### Cadeias laterais

Decentraland está a fazer parceria com [Matic](https://matic.network/) para criar uma *cadeia lateral* (um tipo especial de blockchain) que será capaz de processar transações mais rapidamente e a um custo mais baixo do que a mainchain do Ethereum. Esta cadeia lateral será ideal para transações dentro do jogo, já que as alterações podem ocorrer quase em tempo real e a um custo muito baixo. Para transações que envolvam itens valiosos, continuaremos a recomendar a mainchain do Ethereum, pois será mais segura.

Cada developer a trabalhar numa scene poderá escolher usar a mainchain, a sidechain ou uma combinação de ambas para diferentes transações.

A sidechain será mantida interoperável com a mainchain do Ethereum. Poderá carregar tokens da mainchain para a sidechain e vice-versa. As transações que acontecem na sidechain acabam por ser refletidas na mainchain quando os tokens "saem" de volta para a mainchain.

#### Disparar transações a partir de uma scene

O código da sua scene pode disparar transações, tanto na mainchain do Ethereum como na sidechain de Decentraland. Pode ter uma loja na sua scene que venda tokens (como NFTs) ou um jogo que recompense os players com itens de jogo ao atingirem determinados objetivos.

O utilizador tem sempre de aprovar explicitamente estas transações no seu cliente Ethereum. Por exemplo, ao usar Metamask, o Metamask pede ao utilizador para aceitar cada transação antes de ela ser processada.

## Tipos de tokens

Diferentes tipos de tokens podem ser tratados na rede Ethereum. Surgiram alguns padrões que agrupam tokens que partilham as mesmas características.

Em Decentraland, pode usar tokens para representar itens relacionados com o seu jogo ou experiência, como uma arma ou um troféu. Como os tokens são guardados na wallet de um player, acompanham o player de scene para scene, por isso cada scene pode escolher se e como quer reagir a cada tipo de token existente.

Ler [O que são NFTs](https://decentraland.org/blog/technology/what-are-nfts/) no nosso blog para uma análise mais aprofundada do aparecimento e da evolução dos tokens não fungíveis.

#### Tokens fungíveis

Se um item for fungível, então pode ser substituído ou trocado por qualquer item semelhante. As moedas fiduciárias, como o dólar norte-americano, são fungíveis. Uma nota de um dólar pode ser trocada por qualquer outra nota de um dólar.

Os tokens de criptomoeda como Bitcoin, Ethereum e MANA são todos fungíveis porque uma unidade de token pode ser trocada por qualquer outra unidade de token.

Também pode criar tokens fungíveis personalizados para usar em scenes de Decentraland e utilizá-los para representar itens que são todos iguais e que não têm propriedades distintivas ou personalizáveis entre si. Pode, por exemplo, criar um jogo que gira em torno da recolha de uma grande quantidade de itens idênticos e representá-los através de um token fungível. Também pode usar um token fungível para representar um bilhete dourado que dá aos players que o possuem acesso a uma região ou serviço específico.

*ERC20* é o padrão mais aceite para tokens fungíveis na rede Ethereum. O MANA é construído com base neste padrão.

#### Tokens não fungíveis

Os tokens não fungíveis (ou NFTs) têm características que tornam cada unidade objetivamente diferente das outras. Os Parcels de LAND em Decentraland são NFTs, uma vez que a localização de cada Parcel é única. A adjacência a outros Parcels, estradas ou distritos torna estas localizações relevantes para os proprietários dos tokens.

Em Decentraland, pode usar NFTs para representar itens dentro do jogo, como avatars, wearables, weapons e outros itens do inventário. Pode, por exemplo, usar um único tipo de NFT para representar todas as weapons do seu jogo e diferenciá-las definindo propriedades diferentes nestes NFTs.

Os NFTs podem ser usados para disponibilizar bens digitais comprovadamente escassos. Graças à escassez legítima possibilitada pela blockchain, os compradores podem ter a certeza de que a arte que adquirem é, de facto, rara. Isto dá à arte digital um valor real que nunca tínhamos visto antes.

Os itens de jogo terão um histórico armazenado na blockchain. Esse histórico pode tornar um item mais valioso, por exemplo se foi usado para alcançar grandes feitos ou utilizado por alguém admirado.

Dependendo do contrato que descreve o token, cada NFT pode ser imutável, ou pode permitir que os players personalizem e alterem certas características, caso assim o desejem.

*ERC721* é o padrão mais aceite para tokens não fungíveis na rede Ethereum. Os tokens LAND seguem o padrão ERC721.

## Smart Contracts

Um contrato consiste tanto em código (os seus métodos) como em dados (o seu estado), que residem num endereço específico na blockchain do Ethereum.

Os métodos de um contrato são sempre chamados através de uma transação que tem o *campo "to"* definido com o endereço do contrato. O código executado pelo método do contrato pode incluir chamadas a outros contratos, as quais desencadeiam mais transações que têm o *campo "from"* definido com o endereço do contrato.

Um contrato não pode desencadear quaisquer ações por si próprio nem com base num evento temporal. Todas as ações executadas por um smart contract resultam sempre de uma transação que chama uma das funções do contrato.

Pode usar smart contracts para condicionar transações com base em condições personalizadas. Por exemplo, os players podiam fazer uma aposta no resultado de um jogo, e os pagamentos correspondentes ocorreriam assim que o resultado do jogo fosse comunicado ao contrato.

Todo o código de um smart contract é público para quem o quiser ler. Isto permite aos developers criar regras verificáveis publicamente.

Todos os tokens são definidos por um smart contract que especifica as suas características e o que pode ser feito com ele. Decentraland escreveu e mantém vários smart contracts. Os próprios tokens LAND e MANA são definidos pelos *LANDregistry* e *MANAtoken* contracts, respetivamente.

Pode encontrar o endereço de cada contrato criado por Decentraland em [smart contracts da Decentraland](https://contracts.decentraland.org/addresses.json).

Pode ler o código completo de cada um desses contratos, uma vez que é informação pública na blockchain. Pode encontrar o contrato pelo nome em [Etherscan](https://etherscan.io/contractsVerified) e ler o seu conteúdo aí.

## dApps

*dApps* (aplicações descentralizadas) são aplicações construídas com base em smart contracts e na blockchain.

Uma dApp pode ser tão simples como algo que valida que a sua wallet contém um determinado token e lhe permite usar um serviço. Ou pode ser uma aplicação completa com a sua própria UI, como o Decentraland Marketplace.

## rede de testes Sepolia

Antes de fazer deploy de um smart contract, criar um novo tipo de token ou criar uma scene de Decentraland que dependa de transações na rede Ethereum, precisa de se certificar de que não tem bugs ou falhas que utilizadores maliciosos possam explorar.

A rede de testes Sepolia é uma versão alternativa do Ethereum feita especificamente para executar testes.

Os tokens na rede Sepolia não têm valor real, por isso pode dar-se ao luxo de cometer erros sem correr qualquer risco real. Pode repor gratuitamente quaisquer tokens perdidos usando um faucet:

* faucet de Ether Sepolia (<https://www.alchemy.com/faucets/ethereum-sepolia/>)

Se estiver a desenvolver uma scene que desencadeia transações, testar estas transações na rede Sepolia é gratuito, uma vez que os tokens que envia não têm valor. Na mainnet, teria de pagar pelo menos uma verdadeira taxa de gas em Ether por cada transação de teste que realizar.

Quando tiver a certeza de que o seu código funciona como esperado e não pode ser explorado, pode fazer deploy para a mainnet do Ethereum.

## Reorgs da blockchain

Ocasionalmente, várias máquinas criam novos blocos alternativos aproximadamente ao mesmo tempo. Isto é um problema, porque faz bifurcar a cadeia em duas versões divergentes que potencialmente se podem contradizer. Quando ocorre uma bifurcação, o Ethereum resolve isto dando sempre prioridade à cadeia mais longa e descartando quaisquer cadeias mais curtas. Embora seja possível existirem duas cadeias rivais ao mesmo tempo, em breve uma das duas adicionará outro bloco e ultrapassará a outra. Devido ao tempo necessário para resolver os algoritmos de mineração, torna-se cada vez mais difícil para cadeias rivais continuarem a crescer em perfeita sincronia entre si. Mais cedo ou mais tarde, uma prevalecerá sobre a outra.

Quando uma cadeia ultrapassa a outra e a disputa é resolvida, as máquinas que tinham adotado a cadeia mais curta precisam de fazer ajustes. É isto que se chama um "reorg". Precisam de reverter todas as transações incluídas nos blocos do ramo em que se encontram até chegarem ao ponto em que ocorreu a bifurcação. Depois, precisam de adicionar os novos blocos do ramo mais longo que é considerado legítimo.

As transações revertidas podem regressar ao pool de transações pendentes até serem novamente apanhadas por um miner (ou serem descartadas). Quaisquer taxas de gas pagas por estas transações também são revertidas.

Os blocos que acabaram de ser adicionados ao final de uma cadeia têm uma probabilidade substancial de ser revertidos devido aos mecanismos explicados acima. À medida que blocos subsequentes são adicionados ao final da cadeia, torna-se cada vez menos provável que os blocos mais atrás na blockchain possam ser revertidos, porque isso exigiria um reorg maior. Devido a isto, cada novo bloco que é adicionado ao final da cadeia após uma transação é chamado de confirmação dessa transação.

Ao criar aplicações (ou scenes) que usem informação off-chain, deve estar ciente da ocorrência de reorgs. Pode querer considerar as transações como verificadas apenas quando tiver ocorrido um certo número de confirmações e a transação já não estiver no final da cadeia.

Usar várias confirmações tornará a informação muito estável, mas as transações demorarão muito tempo a ser refletidas.

Usando poucas confirmações, as alterações serão refletidas mais depressa, mas por vezes haverá soluços que parecem desfazer transações quando ocorrem reorgs. Se estas transações tiverem consequências off-chain na sua scene, poderá então ser necessário reverter essas consequências também de alguma forma.


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/player/player-pt/integracao-com-a-blockchain/ethereum-essentials.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
