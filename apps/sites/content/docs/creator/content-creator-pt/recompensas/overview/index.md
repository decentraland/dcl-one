> O serviço Decentraland Rewards cria e entrega wearables e emotes aos jogadores por meio de solicitações HTTP, lidando com transações e taxas.

# Visão geral

O [Rewards dApp](https://decentraland.org/rewards/) é uma ferramenta poderosa que permite aos criadores recompensar usuários com Decentraland Wearables ou Emotes por meio de solicitações HTTP. Pode ser usada para motivar usuários, promover cenas ou aumentar a retenção. As recompensas podem ser concedidas diretamente de uma cena ou de um servidor.

O sistema Rewards gerencia a maioria das complexidades envolvidas em mintar itens na blockchain, incluindo:

* Agrupar vários itens mintados em uma única transação para reduzir o tempo de espera e as taxas
* Lidar com o pagamento das taxas de transação
* Gerenciar novas tentativas em caso de falhas ou reordenações na blockchain
* Limitar o número de itens mintados por endereço de usuário
* Minimizar explorações de automação por meio de um sistema de captcha
* Verificar a presença do usuário na cena por meio de nós Catalyst
* Monitorar o status da transação
* Informar os usuários sobre o item que receberão antes de a transação ser confirmada
* Emite Notifications quando as recompensas são concedidas e recebidas
* ![](/docs/_remote/0aefd219f02433f2b3736381.webp)

## Limitações

Embora o serviço ofereça algumas proteções de automação para impedir que uma pessoa faça mint de Wearables em massa, é impossível estar 100% seguro contra explorações. **Não é RECOMENDADO usar Rewards para mintar Wearables/Emotes com um nível de raridade mais raro do que \[EPIC]\(**[**Veja a documentação**](/docs/creator/content-creator-pt/readme/)**#rarity) (100 cópias ou menos)**.


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-pt/recompensas/overview.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
