> A DAO possui e controla os Smart Contracts mais críticos da Decentraland.

# Os Smart Contracts da DAO

A DAO da Decentraland possui vários dos smart contracts mais críticos da plataforma Decentraland. Estão listados abaixo:

## ⛰️ O contrato LAND

Este é o contrato que gere os tokens LAND. A DAO é a proprietária do smart contract LAND. Isto significa que quaisquer alterações ou modificações a esse contrato têm de ser realizadas pela DAO e pela SAB.

## 🏘️ O contrato Estate

Tal como o contrato LAND, a DAO é a proprietária do contrato Estate, que só pode ser modificado pela SAB, depois de a aprovação ter sido dada pela DAO através de uma votação da comunidade.

## 📍 POIs

A lista de Points of Interest (locais notáveis na Decentraland que são divulgados aos utilizadores como bons locais para começar a explorar o mundo virtual) também pertence à DAO. Esta lista está armazenada num contrato e só pode ser modificada após a aprovação de uma votação pela comunidade, que é então executada on-chain pela DAO Committee.

## 🏷️ Names

Os contracts usados para minting dos NFTs para nomes de avatar únicos na Decentraland pertencem e são controlados pela DAO. Quaisquer alterações ao contract dos names têm de ser aprovadas pela DAO.

## 🚫 Banned names

A lista de names que foram banidos do cliente Decentraland está armazenada num contract pertencente à DAO. Esta lista só pode ser modificada após a aprovação de uma votação pela comunidade, que é então executada on-chain pela DAO Committee.

## 🌐 Nós Catalyst

A lista de nós Catalyst que fornecem conteúdo e estabelecem as ligações peer-to-peer necessárias para manter o mundo virtual da Decentraland em funcionamento também pertence e é controlada pela DAO. Esta lista está armazenada num contrato e só pode ser modificada após a aprovação de uma votação pela comunidade, que é então executada on-chain pela DAO Committee.

## 👕 Coleções de wearables

Na Decentraland, os wearables podem ser agrupados em coleções antes do minting. Os contracts usados para gerir coleções de wearables pertencem e são controlados pela DAO.

## 🛒 Contracts do Marketplace

A dApp do Marketplace da Decentraland faz uso de vários smart contracts para gerir o processo de venda e licitação de LAND, Estates e outros NFTs. Estes contracts também são onde as taxas do marketplace são definidas, e só podem ser alteradas com a aprovação da DAO.

## 💰 Grants

Os contratos de vesting usados para efetuar pagamentos recorrentes no âmbito da estrutura de Grants da DAO também pertencem à DAO. Estes contratos são criados pelo DAO Committee em nome da DAO e são supervisionados pela SAB para evitar qualquer risco de perda monetária devido a vulnerabilidades ou erros cometidos pelo Committee. O estado de qualquer contrato de vesting pode ser verificado utilizando [esta ferramenta](https://vesting.decentraland.org/)


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/player/player-pt/dao/dao/what-smart-contracts-does-the-dao-control.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
