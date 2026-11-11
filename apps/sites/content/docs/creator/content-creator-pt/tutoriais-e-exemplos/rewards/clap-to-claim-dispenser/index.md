> Um dispenser de wearable que os jogadores reivindicam fazendo o emote de aplauso em uma área de trigger.

# Dispenser de claim por aplauso

Permita que os jogadores reivindiquem um Wearable fazendo o emote clap em frente a um dispenser, um mecanismo usado na Metaverse Fashion Week. O README explica como apontar o dispenser para sua própria campanha de Rewards.

## Informações do projeto

**Mantido por:** Decentraland

[Ver código-fonte](https://github.com/decentraland/sdk7-goerli-plaza/tree/main/clap-claim-dispenser)

**Licença:** Não identificado neste catálogo. Consulte o repositório upstream e o mantenedor para os termos aplicáveis antes de reutilizar.

## Explorar o código

Estes links apontam para um snapshot específico da source para que as explicações continuem úteis à medida que o desenvolvimento upstream avança. São referências de aprendizagem, não uma avaliação de segurança.

### Ative um dispenser enquanto o jogador estiver em frente a ele

Cada dispenser armazena sua campanha em um `DispenserComponent`. Uma esfera `TriggerArea` à sua frente define `isActive` e mostra um toast de "clap para reivindicar" ao entrar, e limpa ambos ao sair.

**Contexto do projeto:** O ID da campanha e a chave do dispenser vêm de `ClaimConfig` em `src/modules/claiming/claimConfig.ts`.

**Referências de origem:**

* [`createDispenserWithWearable`](https://github.com/decentraland/sdk7-goerli-plaza/blob/54a07775ee340f0ce22eb23280ef0e9c35f5e386/clap-claim-dispenser/src/modules/dispensers.ts#L28-L89)

### Reivindique quando o jogador reproduzir o emote clap

`initClapToClaim` observa `AvatarEmoteCommand` no entity do jogador. Quando o emote é `clap`, ele espera a animação e faz a reivindicação em todos os dispensers que estiverem ativos no momento.

**Contexto do projeto:** O tratamento da requisição e da resposta de claim segue o mesmo fluxo de Rewards do exemplo do dispenser de reivindicação de recompensa.

**Referências de origem:**

* [`initClapToClaim`](https://github.com/decentraland/sdk7-goerli-plaza/blob/54a07775ee340f0ce22eb23280ef0e9c35f5e386/clap-claim-dispenser/src/modules/clapToClaim.ts#L6-L33)


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-pt/tutoriais-e-exemplos/rewards/clap-to-claim-dispenser.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
