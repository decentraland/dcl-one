> Dispenseres de wearable e caixas de airdrop que reivindicam recompensas de uma campanha de Rewards.

# Dispenser de claim de recompensa

Dê aos jogadores wearables de uma campanha de Rewards do Decentraland. A cena inclui um dispenser de clique para resgatar, um dispenser de botão e uma alavanca que faz airdrop de crates resgatáveis.

## Informações do projeto

**Mantido por:** Decentraland

[Ver código-fonte](https://github.com/decentraland/sdk7-goerli-plaza/tree/main/reward-claim)

**Licença:** [Apache-2.0](https://github.com/decentraland/sdk7-goerli-plaza/blob/main/reward-claim/LICENSE). Verifique os termos upstream antes de reutilizar; o código e os assets podem ter termos diferentes.

## Explorar o código

Estes links apontam para um snapshot específico da fonte, para que as explicações continuem úteis à medida que o desenvolvimento upstream avança. São referências de aprendizado, não uma avaliação de segurança.

### Solicite uma recompensa com um fetch assinado

`claimToken` limita cliques repetidos, ignora dispensers já resgatados com sucesso nesta sessão e verifica se o jogador tem uma wallet. `requestToken` em seguida, envia a chave do dispenser, o Catalyst atual e o endereço do jogador para o servidor Rewards usando `signedFetch`.

**Contexto do projeto:** Os testes estão ativados por padrão (`CONFIG_CLAIM_TESTING_ENABLED` em `src/config.ts`), que usa uma campanha de teste incluída. Para usar a sua, substitua o ID da campanha de espaço reservado e a chave do dispenser em `ClaimConfig` e desative os testes. Veja o guia de início do Rewards para criar uma campanha e um dispenser.

**Referências da fonte:**

* [`claimToken` e `requestToken`](https://github.com/decentraland/sdk7-goerli-plaza/blob/54a07775ee340f0ce22eb23280ef0e9c35f5e386/reward-claim/src/dispenser/claiming/claim.ts#L26-L99)
* [`ClaimConfig`](https://github.com/decentraland/sdk7-goerli-plaza/blob/54a07775ee340f0ce22eb23280ef0e9c35f5e386/reward-claim/src/dispenser/claiming/claimConfig.ts#L21-L34)

### Mapeie as respostas do Rewards para o feedback do jogador

`processResponse` mostra uma UI de dispenser diferente para campanhas indisponíveis, dispensers sem estoque, recompensas já atribuídas e resgates bem-sucedidos. Campanhas não iniciadas, desativadas e concluídas compartilham um único ramo, que mostra uma tela de não iniciada ou encerrada apenas quando a configuração da campanha define `startDate` ou `endDate`, e, caso contrário, uma tela genérica de concluída.

**Contexto do projeto:** Os estados da UI são definidos em `dispenserUi.tsx`. As datas de início e fim da campanha são campos opcionais na configuração da campanha.

**Referências da fonte:**

* [`processResponse`](https://github.com/decentraland/sdk7-goerli-plaza/blob/54a07775ee340f0ce22eb23280ef0e9c35f5e386/reward-claim/src/dispenser/claiming/claim.ts#L101-L197)

### Tentar novamente um resgate com um captcha

Quando o servidor Rewards solicitar um captcha, `getCaptcha` busca uma imagem de captcha e a exibe na UI do dispenser. Ao enviar a resposta, chama `claimToken` novamente com o ID e o valor do captcha.

**Contexto do projeto:** Se um captcha é exigido é uma configuração do dispenser no painel do Rewards.

**Referências da fonte:**

* [`getCaptcha`](https://github.com/decentraland/sdk7-goerli-plaza/blob/54a07775ee340f0ce22eb23280ef0e9c35f5e386/reward-claim/src/dispenser/claiming/claim.ts#L209-L216)
* [Botão de envio do captcha](https://github.com/decentraland/sdk7-goerli-plaza/blob/54a07775ee340f0ce22eb23280ef0e9c35f5e386/reward-claim/src/dispenser/claiming/ui/dispenserUi.tsx#L640-L643)

### Resgatar de uma crate em airdrop

`randomCrateSpawn` solta uma crate com paraquedas em um ponto aleatório da área do palco. `DropCrate` faz raycast para baixo para encontrar o chão e move a crate até ele com tween, e clicar na crate resgata a recompensa e faz com que ela desapareça.

**Contexto do projeto:** A queda da crate é acionada pela `Activate` ação de uma alavanca do Creator Hub em `main`.

**Referências da fonte:**

* [`randomCrateSpawn` e `SpawnCrate`](https://github.com/decentraland/sdk7-goerli-plaza/blob/54a07775ee340f0ce22eb23280ef0e9c35f5e386/reward-claim/src/crate.ts#L40-L119)
* [`DropCrate`](https://github.com/decentraland/sdk7-goerli-plaza/blob/54a07775ee340f0ce22eb23280ef0e9c35f5e386/reward-claim/src/crate.ts#L142-L189)


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-pt/tutoriais-e-exemplos/rewards/reward-claim-dispenser.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
