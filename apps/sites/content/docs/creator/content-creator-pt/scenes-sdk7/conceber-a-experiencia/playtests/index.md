> Como realizar playtests do teu jogo Decentraland e transformar o feedback dos jogadores em melhorias

# Testes de jogo

O playtesting ajuda-o a criar jogos melhores ao descobrir como jogadores reais os vivenciam.

Não precisa de um jogo acabado nem de um grande grupo de testers para começar. Pode testar cedo, fazer melhorias e voltar a testar à medida que o seu jogo evolui.

Este guia explica como organizar playtests, recolher feedback dos jogadores e usar o que aprende para melhorar o seu jogo.

## Por que razão fazer playtest ao seu jogo?

Quando constrói um jogo, já sabe como ele funciona. Os novos jogadores não sabem.

O playtesting ajuda-o a descobrir coisas que, de outra forma, poderia não notar:

* **Clareza:** Os jogadores entendem o que têm de fazer?
* **Facilidade de uso:** Os jogadores conseguem concluir ações sem ficarem bloqueados?
* **Diversão:** Que partes são divertidas e quais são frustrantes?
* **Motivação:** Os jogadores querem continuar a jogar ou voltar?

**Comece a fazer playtesting o mais cedo possível.** Pode aprender muito ao observar apenas algumas pessoas a experimentar a sua primeira versão jogável.

## Escolha o que quer testar

Antes de cada playtest, decida o que quer aprender.

Concentre-se numa ou duas perguntas em vez de tentar testar tudo ao mesmo tempo.

| O seu objetivo              | O que procurar                                                                                                          |
| --------------------------- | ----------------------------------------------------------------------------------------------------------------------- |
| Testar onboarding           | Os novos jogadores conseguem perceber o que fazer sem ajuda?                                                            |
| Testar a mecânica principal | Os jogadores conseguem usar a mecânica principal do jogo? É divertida?                                                  |
| Testar o ciclo de jogo      | Os jogadores entendem como progredir e ganhar recompensas?                                                              |
| Testar a dificuldade        | Onde é que os jogadores ficam bloqueados, aborrecidos ou frustrados?                                                    |
| Testar a reação emocional   | Os jogadores sentem entusiasmo, curiosidade ou vontade de continuar a jogar? Que momentos geram as reações mais fortes? |

Escolha jogadores que não conheçam o seu jogo sempre que possível. É mais provável que identifiquem problemas que os jogadores experientes já deixaram de notar.

### Adapte o playtest à fase do seu jogo

Decida que tipo de resposta procura. No início, precisa sobretudo de saber se os jogadores **compreendem** o jogo e conseguem formar uma **estratégia**: sabem o que fazer e conseguem perceber como fazê-lo bem? Quando isso estiver sólido, precisa de saber como o jogo os faz **sentir**: é entusiasmante, satisfatório ou tenso nos momentos certos, e eles querem voltar?

O tipo de jogador que convida também muda consoante a fase. Alguém que se sente à vontade com Decentraland ultrapassará o client e entrará diretamente no seu jogo, por isso é ideal quando está a testar mecânicas. Um novato total tropeça em tudo, o que é exatamente o que quer quando está a testar a experiência completa da primeira vez.

| Fase                 | O que quer aprender                                                                                           | Quem convidar                                                                           |
| -------------------- | ------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------- |
| V0 (vertical slice)  | A mecânica principal é compreensível e utilizável? Os jogadores conseguem formar uma estratégia à volta dela? | Alguns jogadores que já conhecem Decentraland, para que o client não atrapalhe o teste. |
| V1 (first full loop) | Os jogadores conseguem progredir sozinhos? Onde é que ficam bloqueados ou perdem o interesse?                 | Uma mistura de jogadores de Decentraland e pessoas que nunca jogaram o seu jogo.        |
| V2 e além            | Como é que o jogo faz os jogadores sentirem-se? Eles voltam? Recomendá-lo-iam?                                | Novos jogadores, incluindo pessoas de fora da comunidade Decentraland.                  |

Escreva as suas perguntas para a fase em que se encontra. Perguntar «divertiu-se?» acerca de uma slice V0 diz-lhe pouco, porque ainda não há jogo suficiente para a resposta ter muito significado. Perguntar «percebeu o que tinha de fazer?» acerca de uma V2 que já passou por várias rondas desperdiça uma sessão com algo que já sabe.

## Recolha feedback automaticamente com Scene Feedback

Scene Feedback é um módulo reutilizável que lhe permite fazer perguntas aos jogadores diretamente dentro do seu jogo Decentraland.

Ajuda-o a perceber como os jogadores experienciam o seu jogo, mesmo quando não está presente para os observar a jogar.

Pode:

* Fazer perguntas aos jogadores em momentos específicos durante a jogabilidade.
* Recolher avaliações de 1 a 5 e comentários escritos.
* Transferir as respostas como um ficheiro CSV a partir do Scene Storage do seu World.

### O que precisa primeiro

Scene Feedback armazena as respostas num server, por isso a sua scene precisa de um [Multiplayer Server](/docs/creator/content-creator-pt/scenes-sdk7/rede/authoritative-servers/), ativado através de `@dcl/sdk@auth-server` e definindo `"authoritativeMultiplayer": true` em `scene.json`.

### Instale o Scene Feedback

1. Abra o [repositório do Scene Feedback](https://github.com/decentraland/sdk7-test-scenes/tree/main/scenes/0,-1-scene-feedback).
2. Siga o README para instalar o módulo na sua scene. Também pode dar o prompt de instalação ao seu agente de programação de IA.
3. Escolha um pequeno conjunto de perguntas que corresponda aos objetivos do seu playtest.
4. Decida quando cada pergunta deve aparecer.
5. Transfira as respostas dos jogadores como um ficheiro CSV. No Creator Hub, abra **Gerenciar**, depois o menu do seu World **⋮** menu, depois **Ver Storage**. As respostas podem estar distribuídas por vários ficheiros durante algum tempo (guardados mais recentes que ainda não foram fundidos, além de um ficheiro único mais antigo de versões anteriores do módulo). Transfira todos os ficheiros cujo nome comece pelo mesmo `scene-feedback-<number>`, e elimine as linhas com duplicados `id`s.

O repositório inclui perguntas prontas a usar, exemplos e orientações para escolher as perguntas.

Se usar um assistente de programação de IA, instale ou atualize o [Decentraland SDK Skills](/docs/creator/content-creator-pt/scenes-sdk7/primeiros-passos/vibe-coding/). Incluem uma skill de Scene Feedback que pode ajudá-lo a decidir o que perguntar e quando, instalar o módulo e analisar as respostas depois:

```bash
npx skills add decentraland/sdk-skills --all
```

## Organize o seu playtest

Pode organizar um playtest de duas formas.

**Opção 1: Organize um playtest ao vivo**

Convide toda a gente para jogar ao mesmo tempo. Pode observar os jogadores, discutir as suas experiências e recolher feedback depois.

Também pode criar um evento em [Decentraland Events](https://decentraland.org/events/) para que os jogadores possam encontrar e aderir ao seu playtest.

**Opção 2: Realize um playtest ao longo de vários dias**

Permita que os jogadores testem o seu jogo sempre que estiverem disponíveis durante um período definido.

Instale o Scene Feedback para recolher respostas enquanto jogam.

Isto é útil quando quer feedback de mais jogadores sem organizar uma sessão ao vivo.

## Organize um playtest ao vivo

Um playtest ao vivo permite-lhe observar os jogadores a experienciar o seu jogo e fazer-lhes perguntas depois.

### Passo 1. Prepare o seu jogo

Certifique-se de que os jogadores conseguem aceder ao seu jogo e concluir as ações que quer testar. Prepare duas ou três tarefas para os jogadores experimentarem.

Por exemplo:

* Descobrir como iniciar o jogo.
* Concluir uma ronda.
* Comprar uma melhoria.

### Passo 2. Convide playtesters

Convide um pequeno grupo de jogadores. Três a cinco jogadores é um bom ponto de partida. Tente incluir pessoas que ainda não jogaram o seu jogo.

Pode encontrar jogadores interessados no canal #playtesters no [Discord de Decentraland](https://discord.gg/decentraland). Veja [Encontrar playtesters](#find-playtesters) para outros locais onde procurar.

### Passo 3. Observe os jogadores a jogar

Peça aos jogadores para experimentar o jogo sem explicar como tudo funciona. Observe o que fazem, onde ficam bloqueados e do que gostam.

**Não guie os jogadores a menos que não consigam continuar.** Se tiver de explicar alguma coisa, isso pode ser sinal de que o seu jogo precisa de instruções mais claras.

### Passo 4. Peça feedback

Depois de jogar, faça algumas perguntas abertas:

* Do que mais gostou?
* Houve algo confuso ou frustrante?
* Houve algum momento em que não soube o que fazer?
* Jogaria novamente? Porquê ou por que não?
* Recomendaria este jogo a um amigo? Porquê ou por que não?

Evite perguntas sugestivas como «Gostou do novo sistema de melhorias?» Estas podem influenciar as respostas que recebe.

Se não tiver a certeza do que perguntar, a skill de Scene Feedback mencionada acima pode guiá-lo na escolha de um foco e na formulação das perguntas, quer pretenda fazê-las presencialmente quer no jogo.

Anote os problemas que os jogadores tiveram, e não apenas as suas sugestões.

## Melhore o seu jogo com o método RITE

RITE significa **Rapid Iterative Testing and Evaluation**.

É uma abordagem simples ao playtesting: encontre um problema, corrija-o e teste novamente o mais rapidamente possível. Em vez de realizar um grande playtest e recolher uma longa lista de problemas, faça vários pequenos e corrija os problemas mais evidentes entre cada um.

### Siga o ciclo RITE

1. **Testar:** Observe um pequeno grupo de jogadores a experimentar o seu jogo. Três a cinco jogadores por ronda são suficientes.
2. **Identificar:** Encontre problemas que afetem a experiência deles.
3. **Corrigir:** Entre rondas, faça uma alteração quando o problema e a solução estiverem ambos claros.
4. **Repetir:** Execute a ronda seguinte com novos jogadores. Continue até que os jogadores consigam completar a experiência sem problemas maiores.

Por exemplo, se os jogadores não conseguirem encontrar onde começar o seu jogo, torne o botão Start mais claro antes do próximo playtest. Depois observe se os novos jogadores o conseguem encontrar sem ajuda.

**Mantenha o jogo estável enquanto uma ronda está a decorrer.** RITE significa corrigir entre rondas, não durante elas. Se fizer um redeploy enquanto os jogadores estão a meio de uma sessão, pode estragar o jogo deles e já não consegue saber a que versão se refere cada pedaço de feedback. Recolha tudo de uma ronda, depois faça as suas alterações e, em seguida, teste novamente.

### Faça o RITE funcionar em Decentraland

Alguns hábitos tornam o RITE prático para uma scene do Decentraland:

* **Corrija pouco, corrija depressa.** Entre rondas, altere apenas o que a ronda anterior mostrou claramente ser um problema. Se uma correção precisar de uma nova mecânica ou de uma reformulação, anote-a para mais tarde e mantenha as alterações entre rondas suficientemente pequenas para serem lançadas num dia.
* **Use um World para testes.** Publique as suas builds de teste num [World](/docs/creator/content-creator-pt/scenes-sdk7/publicacao/publishing-options/) em vez de no seu LAND em produção ou no seu World principal. Os jogadores entram sempre numa build conhecida e pode republicar entre rondas sem tocar num jogo publicado.
* **Mantenha as perguntas do Scene Feedback iguais entre rondas.** Se alterar a redação de uma pergunta, não pode comparar as classificações de uma ronda para a seguinte. Adicione perguntas se precisar, mas não reescreva as que está a acompanhar.
* **Decida o que conta como «claro».** Um problema é claro quando viu que aconteceu, consegue dizer o que o jogador esperava em vez disso e sabe o que mudar. Se apenas um jogador o encontrou e não tem a certeza do porquê, deixe-o para a ronda seguinte e veja se volta a acontecer.
* **Registe cada ronda.** Mantenha uma nota curta por ronda: que build, quem jogou, o que viu, o que alterou. À terceira ronda, este registo diz-lhe se uma alteração realmente funcionou.

O RITE funciona especialmente bem durante o desenvolvimento inicial, quando pode fazer alterações rapidamente.

## Encontrar playtesters

Os jogadores regulares de Decentraland são os mais fáceis de contactar, mas procure também [fora da comunidade Decentraland](#outside-the-decentraland-community): eles detetam problemas que os jogadores experientes não veem.

### No Discord

O canal #playtesters no [Discord de Decentraland](https://discord.gg/decentraland) é para pessoas que gostam de testar jogos e ajudar os criadores a melhorá-los. Junte-se ao server e encontre o canal.

### Publique o seu playtest

Quando estiver pronto, publique uma mensagem em #playtesters que inclua:

* **Jogo:** O nome do seu jogo e uma breve descrição.
* **Localização:** Uma ligação para o seu jogo em Decentraland.
* **Quando:** A data e a hora do seu playtest, ou o período em que o feedback está aberto.
* **O que testar:** As funcionalidades específicas ou a jogabilidade sobre as quais quer feedback.
* **Como dar feedback:** Através de discussão ao vivo, Discord ou Scene Feedback.
* **Recompensas:** Qualquer incentivo que esteja a oferecer pela participação.

### Incentive os jogadores a participar

Considere oferecer uma pequena recompensa para agradecer o tempo dos jogadores.

Por exemplo:

* Ofereça aos participantes um Wearable ou Emote especial.
* Ofereça uma recompensa exclusiva no jogo. O exemplo do Scene Feedback inclui uma receita que mostra um **Contribuidor de Playtest** sobre o avatar do jogador com a etiqueta [`AvatarNametag`](/docs/creator/content-creator-pt/scenes-sdk7/interatividade/avatares/avatar-nametags/) assim que responderem a todas as perguntas de um formulário. A etiqueta só é visível para esse jogador e dura durante a visita. Veja [Uma recompensa local da scene pela participação](https://github.com/decentraland/sdk7-test-scenes/blob/main/scenes/0%2C-1-scene-feedback/EXAMPLES.md#6-a-scene-local-reward-for-participation).
* Dê Marketplace Credits aos jogadores elegíveis.

**Oferecer Wearables e Emotes.** Pode oferecer qualquer Wearable ou Emote que possua a outro jogador diretamente a partir do desktop client: clique com o botão direito no jogador e escolha **Oferecer**. O item é transferido para a conta deles e aparece na respetiva Backpack. Também pode transferir um item que possua a partir do [Marketplace](https://decentraland.org/marketplace/) usando o endereço da wallet do destinatário. Se criou a coleção de Wearable ou Emote, pode mintar itens diretamente para qualquer endereço de wallet a partir do Builder, mesmo que não encontre o jogador in-world. Veja [Mint de Wearables](/docs/creator/content-creator-pt/wearables-e-emotes/publicacao-de-colecoes/publishing-collections/#minting-wearables).

**A oferta de Marketplace Credits está atualmente em beta e disponível para estúdios convidados.** Os estúdios elegíveis podem oferecer Credits aos jogadores, que os podem gastar na Decentraland Shop. Veja [Oferecer Credits (Beta)](/docs/creator/content-creator-pt/recompensas/gifting-credits/).

As recompensas podem incentivar a participação, mas deixe claro que quer feedback genuíno, incluindo críticas.

### Fora da comunidade Decentraland

Também pode convidar pessoas que nunca usaram Decentraland. Os jogadores de Decentraland já sabem como o client funciona, por isso vão diretamente para o seu jogo. Pessoas de fora veem tudo com olhos frescos, incluindo as partes que já deixou de notar. Fazer playtesting com elas é também uma boa forma de apresentar Decentraland a novos jogadores.

Os desenvolvedores indie normalmente recrutam em:

* **Amigos e família.** São fáceis de contactar e não se importam de lhe dar uma hora. Peça-lhes que sejam honestos e que lhe digam quando estiverem confusos.
* **Pessoas que normalmente não jogam.** Se conseguirem passar pelo seu onboarding, quase toda a gente consegue.
* **Comunidades das quais já faz parte.** Um server de game jam, um fórum de hobby, um encontro local ou um grupo que combine com o tema do seu jogo.

Se os seus testers forem novos no Decentraland, ajude-os a entrar: partilhe a ligação web para a sua scene (`https://decentraland.org/jump/?realm=<your-world-name>.dcl.eth` para um World, ou `https://decentraland.org/jump/?position=<x>,<y>` para LAND), que o Creator Hub mostra depois de publicar, mais uma breve nota sobre instalar o client e iniciar sessão. A ligação abre num browser mesmo antes de o client estar instalado. Tome notas sobre onde têm dificuldades antes de chegarem à sua scene, mas trate isso separadamente do feedback sobre o seu jogo.

## Use o feedback para melhorar a sua próxima versão

Após cada playtest, reveja o que aprendeu antes de decidir o que construir a seguir.

1. **Agrupe problemas semelhantes.** Procure problemas que vários jogadores tenham experienciado.
2. **Priorize os problemas importantes.** Corrija tudo o que impeça os jogadores de compreender, progredir ou desfrutar do jogo.
3. **Faça melhorias.** Comece com alterações rápidas de implementar e com grande probabilidade de ajudar.
4. **Teste novamente.** Verifique se os novos jogadores têm uma experiência melhor.
5. **Acompanhe as suas alterações.** Mantenha um registo curto do que alterou e do que aconteceu no playtest seguinte.
6. **Partilhe o que mudou.** Informe os seus playtesters do que aprendeu e do que melhorou graças ao feedback deles. Publique uma atualização no seu tópico original no Discord para que possam ver o impacto da sua contribuição.

Lembre-se de que o que os jogadores **fazem** pode ser diferente do que **dizem**. Use observações ao vivo, respostas do Scene Feedback e dados de jogabilidade em conjunto, sempre que possível.

## Partilhe as suas melhorias com os playtesters

Os jogadores têm maior probabilidade de participar novamente quando veem que o seu feedback fez a diferença. Depois de fazer alterações, volte à sua publicação de playtest em #playtesters e partilhe uma breve atualização.

Use este formato simples: **Disse, aprendemos, mudámos.**

### Exemplo: resultados do playtest

**Atualização do playtest: \[Nome do jogo]**

Obrigado a todos os que testaram o nosso jogo!

**Disse:** Não estava claro como começar a primeira ronda.

**Aprendemos:** Os novos jogadores não conseguiam encontrar facilmente o botão Start.

**Mudámos:** Tornámos o botão maior e adicionámos instruções mais claras.

**Jogar novamente:** \[Ligação para o seu jogo atualizado]

Não precisa de corrigir todos os problemas antes de publicar uma atualização. Também pode explicar no que ainda está a trabalhar ou o que gostaria de testar a seguir.

## Continue a fazer playtesting

O playtesting não é algo que se faça apenas uma vez antes do lançamento.

Teste quando introduzir uma nova mecânica, alterar o onboarding, ajustar a dificuldade ou adicionar novo conteúdo. Mesmo pequenos playtests podem ajudá-lo a descobrir problemas importantes antes de afetarem mais jogadores.

**O objetivo não é provar que o seu jogo funciona. É descobrir como torná-lo melhor.**

## Recursos

* [Módulo Scene Feedback e guia de instalação](https://github.com/decentraland/sdk7-test-scenes/tree/main/scenes/0,-1-scene-feedback)
* [Discord de Decentraland](https://discord.gg/decentraland)
* [Decentraland Events](https://decentraland.org/events/)
* [Método RITE: Rapid Iterative Testing and Evaluation](https://handbook.gitlab.com/handbook/upstream-studios/experience-research/rite/)


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-pt/scenes-sdk7/conceber-a-experiencia/playtests.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
