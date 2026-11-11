> A DAO é a plataforma de tomada de decisão da Decentraland.

# O que é a DAO


> [!NOTE]
> **Missão da DAO:** Garantir a sustentabilidade do ecossistema Decentraland por meio de governança representativa, gestão eficiente de recursos coletivos e inovação técnica. Leia a [missão, visão e papéis](/docs/player/player-pt/dao/dao/mission-vision-and-roles/) da DAO, do DAO Council, do DCL Regenesis Labs e da Decentraland Foundation.


A Decentraland DAO é a ferramenta de tomada de decisão para os detentores de MANA, NAMES e LAND no mundo virtual da Decentraland. Por meio de votos na DAO, a comunidade pode emitir grants e fazer alterações nas listas de nomes banidos, POIs e catalyst nodes. A DAO também controla os smart contracts de LAND e Estate.

A emissão de grants e a realização de alterações nos registros e contratos pertencentes à DAO só podem ser feitas usando propostas predefinidas acessíveis em [governance.decentraland.org](https://governance.decentraland.org).

Essas propostas, os votos enviados e os resultados finais são todos armazenados no IPFS via Snapshot, um cliente de votação sem gas. As propostas aprovadas com ações vinculativas são executadas na blockchain Ethereum por um comitê por meio de uma carteira multi-sig. Esse comitê é supervisionado pelo Security Advisory Board (SAB), outra multisig com titulares de chaves de confiança. Esse comitê foi eleito pela comunidade na versão anterior da DAO. [A proposta original pode ser encontrada aqui](https://forum.decentraland.org/t/proposal-for-a-more-accessible-and-affordable-dao/450).

O restante deste documento explica em mais detalhes o que é a DAO, como ela funciona e para que ela pode ser usada.

Para um tutorial detalhado sobre como usar a Decentraland DAO, visite a [Guia do Usuário da DAO](/docs/player/player-pt/dao/dao-userguide/).

## A DAO é alimentada por smart contracts

Todas as DAOs, ou organizações autônomas descentralizadas, fazem parte de uma nova abordagem de gestão organizacional e tomada de decisão possibilitada pelo Ethereum.

O Ethereum expandiu o que era possível com blockchains ao adicionar a capacidade de descentralizar o tratamento de dados mais complexos do que apenas registros de propriedade de tokens. O Ethereum fez isso permitindo que as pessoas colocassem smart contracts em uma blockchain.

### O que é um smart contract?

Um smart contract é um programa de computador executado na blockchain Ethereum. Ele pode armazenar tanto funções (trechos de código que fazem coisas) quanto dados (informações). Smart contracts são frequentemente comparados a máquinas de venda automática. Se você inserir entradas específicas, obtém saídas específicas. Se eu me aproximar de uma máquina de venda automática, inserir US$ 1 e apertar o botão "refrigerante de laranja", então vou receber um refrigerante de laranja se ainda houver algum na máquina. Se não houver mais refrigerantes de laranja, receberei meu dinheiro de volta.

Smart contracts funcionam da mesma forma: as pessoas podem interagir com eles enviando informações com a expectativa de receber resultados ou informações específicas. Assim como a máquina de venda automática não tem uma pessoa pequena dentro distribuindo refrigerantes, os smart contracts são automáticos (ousamos dizer, autônomos).

Se você quiser saber mais sobre smart contracts do Ethereum, a [documentação do Ethereum](https://ethereum.org/en/developers/docs/smart-contracts/) é o melhor lugar para começar.

### A DAO controla os smart contracts críticos da Decentraland

A segunda qualidade importante dos smart contracts é sua **capacidade de possuir outros smart contracts**.

Isso mesmo, todo smart contract tem o seu próprio endereço (assim como o endereço da sua carteira Ethereum) que lhe permite possuir outros smart contracts e criptomoedas.

Então, em termos um pouco mais técnicos, uma DAO é um ou mais smart contracts que podem executar tarefas específicas e predefinidas e manter a propriedade de criptomoedas. As DAOs são construídas de modo que só executem suas tarefas sob condições específicas, como a aprovação de uma proposta votada por um grupo de pessoas que possuem determinado token (como MANA, NAMES ou LAND). Tudo isso é feito em uma blockchain. Daí o nome, "organização autônoma descentralizada".

A DAO da Decentraland também possui uma quantia de MANA e outros tokens, juntamente com os smart contracts de LAND e Estate. [Este fundo](https://governance.decentraland.org/transparency/) foi reservado para ajudar a patrocinar grants da comunidade e para ajudar a expandir a plataforma Decentraland de acordo com as decisões e diretrizes votadas pela comunidade.


> [!WARNING]
> **📔 Observação** A DAO não possui, e portanto não pode modificar, o [smart contract de MANA](https://etherscan.io/address/0x0f5d2fb29fb7d3cfee444a200298f468908cc942#readContract).
> 
> O proprietário do contrato de MANA é o [contrato TokenSale](https://etherscan.io/address/0xa66d83716c7cfe425b44d0f7ef92de263468fb3d#readContract). O proprietário do contrato TokenSale é um contrato separado que se autodestruiu na implantação ([como você pode ver no Etherscan aqui](https://etherscan.io/address/0xdf861993edbe95bafbfa7760838f8ebbd5afda9f)). Isso significa que não há outro contrato ou carteira com permissões para modificar ou pausar o fornecimento de MANA.


Também há outras informações que a DAO controla, como a lista de nomes prejudiciais ou ofensivos que não são permitidos em Decentraland, uma lista de locais notáveis (POIs ou Points of Interest) para serem promovidos a novos usuários e a lista de servidores operados pela comunidade que hospedam o mundo virtual da Decentraland.

Transferir qualquer parte do MANA da DAO, modificar os smart contracts de LAND ou Estate, ou modificar qualquer uma das outras informações listadas controladas pela DAO **só pode ser feito** com a aprovação dos detentores de MANA, NAMES e LAND.


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/player/player-pt/dao/dao/what-is-the-dao.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
