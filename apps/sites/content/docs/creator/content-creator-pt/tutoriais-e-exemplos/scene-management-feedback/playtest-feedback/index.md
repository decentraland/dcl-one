> Recolhe classificações e comentários dos jogadores durante o jogo ou através de um botão de feedback, quiosque ou área, com resultados CSV no Storage da cena.

# Feedback da cena

<figure><img src="/docs/_remote/a0baa00d823b7604d1c33550.webp" alt="Coin Hunt participation invitation with Skip and Give feedback buttons" width="400"><figcaption><p>A demo convida os jogadores a escolher se querem participar.</p></figcaption></figure>

Saiba o que os jogadores consideram confuso, frustrante ou agradável, mesmo quando não os pode ver a jogar. Este módulo reutilizável inclui uma cena de caça às moedas que mostra como recolher feedback durante o jogo e quando os jogadores escolhem fornecê-lo.

* [Jogar build de teste partilhada (Zone)](https://decentraland.zone/jump/?dclenv=zone&realm=sdk7testscenes.dcl.eth&position=0,-1)
* [Guia de instalação e utilização](https://github.com/decentraland/sdk7-test-scenes/blob/main/scenes/0,-1-scene-feedback/README.md)

## Informações do projeto

**Mantido por:** Decentraland

[Ver código-fonte](https://github.com/decentraland/sdk7-test-scenes/tree/main/scenes/0,-1-scene-feedback)

**Licença:** Não identificado neste catálogo. Consulte o repositório upstream e o mantenedor para conhecer os termos aplicáveis antes de reutilizar.

## Adicione-o com um agente de IA

1. Certifique-se [Decentraland SDK Skills](https://github.com/decentraland/sdk-skills#updating-skills) de que estão instalados e atualizados.
2. Diga ao seu agente de IA: "Quero recolher feedback dos jogadores nesta cena."

O agente pode usar a habilidade Scene Feedback para ajudar em duas etapas:

* **Antes de um playtest:** adapte perguntas, escalas de avaliação e temporização ao seu jogo e, depois, integre o módulo após rever o plano.
* **Depois de um playtest:** use novamente a habilidade com os seus ficheiros CSV para identificar problemas e preferências recorrentes, reveja as evidências e decida o que investigar a seguir. Também pode pedir um [relatório HTML legível por humanos](#example-ai-generated-feedback-report) com respostas agrupadas e resumos das avaliações.

Prefere integrá-lo você mesmo? Siga o [guia de instalação](https://github.com/decentraland/sdk7-test-scenes/blob/main/scenes/0,-1-scene-feedback/README.md#add-it-to-your-scene).

## Como o módulo se enquadra na sua cena

Você escolhe quando convidar os jogadores, que perguntas fazer e quando essas perguntas aparecem no seu jogo. A [demonstração de caça às moedas em Zone](https://decentraland.zone/jump/?dclenv=zone&realm=sdk7testscenes.dcl.eth&position=0,-1) mostra uma configuração possível:

1. **Recolha a primeira moeda.** Um convite pergunta se o jogador quer dar feedback: "Olá, somos a equipa da Coin Hunt!" Se aceitar, segue-se uma pergunta: "Quão fácil ou difícil foi ver as moedas?" O jogador escolhe uma avaliação e pode adicionar um comentário.
2. **Conclua a primeira ronda (cinco moedas).** Os jogadores que concordaram em participar recebem duas perguntas, uma a seguir à outra: quão claro é o próximo objetivo e se querem continuar a jogar.
3. **Dê feedback em qualquer altura.** Qualquer visitante pode usar o **Deixar feedback** botão, quiosque clicável ou área marcada para abrir uma mensagem de boas-vindas seguida de um conjunto de perguntas, mesmo depois de recusarem as perguntas de jogabilidade.

Escolha como os jogadores participam:

* **Jogadores que aderem:** convide os jogadores com um aviso no ecrã ou deixe-os abrir um formulário de feedback através de um botão, quiosque ou área marcada.
* **Testadores selecionados:** use a lógica do seu jogo para escolher testadores por lista de carteiras ou outra regra e, depois, inscreva-os com `feedback.enroll`. Veja o [exemplo de testador selecionado](https://github.com/decentraland/sdk7-test-scenes/blob/main/scenes/0,-1-scene-feedback/EXAMPLES.md#4-selected-testers-without-an-intro).
* **Ou pergunte a qualquer pessoa:** mostre perguntas aos visitantes durante o jogo sem um convite prévio ou lista de testadores. Os jogadores podem ainda assim recusar. Veja as [definições de participação](https://github.com/decentraland/sdk7-test-scenes/blob/main/scenes/0,-1-scene-feedback/README.md#asking-participants-during-play-dynamic-approach).

<figure><img src="/docs/_remote/c600bed5d83d04aa544fb1e0.webp" alt="Example feedback form with five rating choices and an optional comment field" width="400"><figcaption><p>Uma pergunta com uma avaliação rotulada e um comentário opcional.</p></figcaption></figure>

## Principais recursos

* **Perguntas flexíveis.** Escolha perguntas e escalas de avaliação prontas a usar, adapte-as ao seu jogo ou crie as suas próprias. Use avaliações rotuladas, comentários opcionais ou respostas só de texto.
* **Recompensas pela participação.** Incentive a participação com uma recompensa que se adeque ao seu jogo. Na demonstração, responder a todas as perguntas num formulário concede uma placa Playtest Contributor apenas para a visita, visível para si, +5 pontos de moedas e o dobro dos pontos para as próximas cinco recolhas, uma vez por visita. Qualquer avaliação conta; os comentários permanecem opcionais.
* **Resultados CSV no Storage da cena.** Um Multiplayer Server guarda cada resposta com a respetiva pergunta e momento de jogo. Transfira os ficheiros para rever numa folha de cálculo.

## Adapte-o à sua cena

Use o [receitas de integração](https://github.com/decentraland/sdk7-test-scenes/blob/main/scenes/0,-1-scene-feedback/EXAMPLES.md) para ligar perguntas aos seus próprios eventos de jogo, incluindo convidar participantes agora e perguntar depois no jogo. Para planeamento de playtests e encontrar jogadores, consulte [Playtests](/docs/creator/sdk7/design-experience/playtests/).

## Exemplo de relatório de feedback gerado por IA

**O relatório é criado por um agente de IA, não pelo módulo Scene Feedback.** Dê ao seu agente os ficheiros CSV exportados e use a habilidade Scene Feedback SDK para os analisar e gerar um relatório HTML.

Este exemplo usa respostas sintéticas de uma cena fictícia. O seu agente pode adaptar a análise e o relatório às suas próprias perguntas e dados de playtest.

<figure><img src="/docs/_remote/ad88dfb78877894462538f58.webp" alt="Synthetic racing-scene report showing a main finding, response counts and a rating distribution" width="600"><figcaption><p>Relatório ilustrativo usando respostas sintéticas de uma cena fictícia.</p></figcaption></figure>

{% file src="/files/7833c89b75c9cacd9cbccab6ef91cea6e39087ef" %}
Transfira o relatório de exemplo, extraia o ZIP e abra o ficheiro HTML no seu navegador.
{% endfile %}


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-pt/tutoriais-e-exemplos/scene-management-feedback/playtest-feedback.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
