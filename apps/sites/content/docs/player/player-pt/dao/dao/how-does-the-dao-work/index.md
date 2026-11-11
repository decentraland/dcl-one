> Visão geral das diferentes plataformas e entidades que compõem a DAO da Decentraland.

# Como a DAO funciona

Para contornar as taxas de gas muito altas associadas à governança totalmente on-chain, a DAO da Decentraland usa uma combinação de votação gratuita fora da blockchain para a comunidade e uma carteira multi-sig controlada por um "DAO Committee" para executar essas decisões fora da blockchain na blockchain Ethereum. Isso permite que todos que possuem MANA, NAMES ou LAND participem da DAO sem ter de pagar quaisquer taxas sempre que quiserem votar ou abrir uma proposta.

O uso de uma carteira multi-sig controlada por um comitê de pessoas de confiança garante a segurança da DAO, além de garantir que as propostas aprovadas resultem em uma ação on-chain. Uma segunda multi-sig pertencente ao SAB fornece uma segunda camada de proteção por cima do DAO Committee.

A DAO é um sistema complexo criado a partir de várias camadas de entities e plataformas, cada uma das quais é listada e descrita abaixo:

## Governance dApp

A interface principal para a DAO da Decentraland está localizada em [governance.decentraland.org](https://governance.decentraland.org). É aqui que os usuários fazem login, criam propostas e votam.

*Isto é atualmente mantido por uma equipe de desenvolvimento financiada por uma* [*subvenção da DAO*](https://governance.decentraland.org/proposal/?id=ed53e850-5e70-11ec-8188-4352ce3d30e7)*.*

## Snapshot

[Snapshot](https://snapshot.org/#/) é uma plataforma de votação fora da blockchain que oferece uma maneira gratuita para detentores de tokens votarem em propostas. Ao armazenar tanto as propostas quanto os votos no IPFS como mensagens assinadas criptograficamente, o Snapshot permite resultados seguros e facilmente contestáveis.

A DAO da Decentraland usa o Snapshot para hospedar as propostas e votos gerados pela comunidade.

Sempre que uma proposta é aberta em governance.decentraland.org, ela também é criada automaticamente em [o espaço Snapshot da Decentraland](https://snapshot.org/#/snapshot.dcl.eth). Isso permite que a DAO registre e armazene propostas, votos e resultados de maneira segura e descentralizada - com os resultados exibidos novamente em governance.decentraland.org. A partir do espaço Snapshot, os usuários também podem [delegar seu VP](https://snapshot.org/#/delegate/snapshot.dcl.eth) a outros membros da comunidade.

## Aragon

[Aragon](https://aragon.org/) é uma plataforma segura para criar e gerenciar a coleção de smart contracts necessários para executar uma DAO. O backend da DAO da Decentraland é construído usando Aragon, então, sempre que uma proposta que resulta em uma ação vinculativa é aprovada pela comunidade no Snapshot, ela é registrada na Ethereum mainnet no Aragon pelo DAO Committee.

## DAO Committee

O DAO Committee é um grupo de três indivíduos de confiança que foram selecionados pela comunidade para deter chaves em uma carteira multi-sig. Essa multi-sig é responsável por executar quaisquer votos aprovados com uma ação vinculativa, como financiar uma Grant, banir um NAME, adicionar ou remover um POI, implementar uma proposta de Governança ou adicionar um nó Catalyst.

O DAO Committee é supervisionado pelo SAB, que tem a capacidade de pausar e cancelar qualquer ação iniciada pelo Committee.

Toda transação on-chain iniciada pelo DAO Committee tem um atraso automático de 24 horas antes de ser concluída, permitindo que o SAB ou o Committee revoguem a transação.

![DAO Committee](/docs/_remote/063ee68d732926eeb99173e9.webp)

## Security Advisory Board (SAB)

O Security Advisory Board atua como garantidor da segurança dos smart contracts da Decentraland e tem a tarefa de supervisionar o trabalho do DAO Committee e responder a relatórios de vulnerabilidades e bugs em quaisquer contratos da Decentraland.

O SAB inclui 5 especialistas em Solidity que foram inicialmente selecionados pela equipe de desenvolvimento da Decentraland.

Sempre que for feita uma modificação nos contratos LAND ou Estate, a atualização deve receber apoio unânime da multi-sig do SAB. São necessárias pelo menos três assinaturas, sem votos dissidentes, para fazer quaisquer alterações nos contratos LAND ou Estate.

O SAB tem a capacidade de pausar, retomar ou cancelar qualquer ação tomada pelo DAO Committee.

Iniciar a adição ou remoção de um membro do SAB pode ser feito dando início a um processo de proposta de Governança em [governance.decentraland.org](https://governance.decentraland.org).

![Security Advisory Board](/docs/_remote/199ca9caded989d293a0f36d.webp)

## Wearables Curation Committee

O Curation Committee é responsável por revisar e aprovar wearables e emotes enviados pela comunidade da Decentraland.

![Wearables Curation Committee](/docs/_remote/dc4a02e026b26163106c05ac.webp)

## Revocations Committee

O Revocations Committee é responsável por revisar os casos levantados pela comunidade em relação ao Grants Program e tem a responsabilidade de fornecer uma resolução, que pode ser revogar um contrato de vesting como ação final.

[Revocations Committee](https://github.com/lordlikedao/documentation/blob/main/static/images/DAO%20Organizational%20Chart/Revocation.png?raw=true)

## Governance Squad

O Governance Squad é responsável por desenvolver, aprimorar e manter a infraestrutura tecnológica e as ferramentas de tomada de decisão da DAO.

![Governance Squad](/docs/_remote/f6663bc09324c4879744664c.webp)

## Facilitation Squad

O Facilitation Squad é responsável por gerenciar a comunicação e alinhar as diferentes partes interessadas, promovendo a tomada de decisão coletiva e operações de governança eficientes.

![Facilitation Squad](/docs/_remote/058f90f95aed65ab975d706d.webp)

## Grants Support Squad

O Grants Support Squad tem a responsabilidade de manter um diálogo constante com a comunidade de beneficiários, para entender suas necessidades e fornecer suporte aos seus bloqueios e solicitações.

![Grants Support Squad](/docs/_remote/28cc4e3f4a37dfd9819f791d.webp)

## O Fórum

O [Fórum da Decentraland](https://forum.decentraland.org) é o centro de comunicação da DAO. Sempre que uma nova proposta é criada, um tópico correspondente é aberto automaticamente no Fórum sob a [seção de Governança](https://forum.decentraland.org/c/governance/). Esses tópicos de proposta podem ser encontrados navegando pelo Fórum ou clicando no botão "Discutir no Fórum'' na página de detalhes de qualquer proposta na interface de Governança. Esses tópicos são um espaço onde os votantes podem discutir os vários impactos e questões que podem resultar de uma proposta.

## Servidores Discord

A DAO opera em dois servidores Discord:

* [canal #dao](https://discord.com/channels/417796904760639509/538106198419832844) no servidor Discord da Decentraland para anúncios gerais, discussões e perguntas.
* Um [servidor Discord dedicado](https://dcl.gg/daodiscord) apenas para as operações da DAO, com discussões mais aprofundadas, canais específicos para grupos de trabalho que lidam com questões complexas e espaços para os proprietários de Grants da comunidade publicarem atualizações sobre seus projetos. Nesse canal, a DAO também realiza seu Town Hall mensal.


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/player/player-pt/dao/dao/how-does-the-dao-work.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
