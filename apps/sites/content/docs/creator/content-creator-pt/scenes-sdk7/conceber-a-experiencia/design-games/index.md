> O que você precisa ter em mente ao projetar um jogo para o Decentraland.

# Desenvolver Jogos

Este documento aborda alguns pontos-chave a considerar ao desenhar um jogo para Decentraland. Considerações como a adjacência de outras scenes e a propriedade distribuída do LAND fazem de Decentraland um lugar único que exige que repenses pressupostos que possas ter de jogos anteriores.

Por exemplo, tens de compreender que, ao contrário de outras plataformas de jogos, os jogos de Decentraland não existem num vácuo. Não tens controlo sobre o que existe nas scenes adjacentes, e não tens controlo sobre certos detalhes, como os avatars dos jogadores ou itens que eles possam trazer de outros jogos. Isto abre a porta a possibilidades interessantes e exige que penses de forma diferente sobre as mecânicas de jogo.

O mais próximo que existe nos jogos mais populares atualmente é o Roblox, onde o conteúdo gerado por utilizadores da comunidade pode depois tornar-se um local de encontro para outros explorarem, jogarem e interagirem. Ao contrário do Roblox, navegas pelas scenes não através de um menu de experiências não relacionadas, mas explorando fisicamente um terreno onde todas as scenes são adjacentes umas às outras. Decentraland também faz uso da blockchain como forma de gerir a propriedade de terrenos, avatars, ativos, etc.

Estamos continuamente a melhorar o SDK, por isso algumas das limitações seguintes podem ser removidas em futuras atualizações.

## Limites da scene

**O teu jogo tem de caber por completo no \_LAND**\_\*\* sobre o qual a tua scene é construída.\*\* Para scenes pequenas, pensa em jogos como futebol, onde as regras do jogo mantêm a interação relevante dentro de um espaço confinado, mesmo que os jogadores possam sair do campo de jogo. Os jogadores podem andar para fora dos limites de uma scene, mas qualquer asset ou entities que pertençam à scene têm de permanecer dentro da scene.

Os jogadores que saem da tua scene continuam a renderizá-la enquanto esta estiver num alcance visível. Se se afastarem demasiado, deixarão de a renderizar por completo.

Podes também construir um jogo que se espalha por vários terrenos desconectados e desconhecidos dos jogadores, e onde a exploração do resto do mundo se torna parte da jogabilidade. Um jogo assim seria composto por várias *scenes*, que poderiam partilhar dados entre si através de um server.

Se o teu jogo for publicado para um [Decentraland World](/docs/creator/content-creator-pt/scenes-sdk7/publicacao/publishing-options/#decentraland-worlds), tens mais controlo sobre o que os jogadores veem para além dos limites da tua scene. Os Worlds são normalmente rodeados por uma paisagem auto-gerada de relva, árvores e mar, mas podes [desativar esta paisagem](/docs/creator/content-creator-pt/scenes-sdk7/tipos-de-projetos/scene-metadata/#landscape-terrain) se ela entrar em conflito com o ambiente do teu jogo, por exemplo um jogo que decorre em mar aberto ou no espaço.

### Inventário do utilizador

**Atualmente não existe um inventário onde os jogadores possam guardar itens do jogo enquanto caminham entre scenes.** Atualmente estão disponíveis as seguintes alternativas:

* Podes armazenar informação de inventário na própria scene e ligá-la ao endereço Ethereum de cada jogador (isto pode ser usado como um ID persistente). Esta informação só seria legível a partir da tua scene.
* Podes usar um armazenamento externo personalizado e sincronizar todas as tuas scenes com ele. Esta é uma solução mais robusta que consegue lidar com volumes maiores de jogadores. Também pode estender o acesso a este inventário a várias scenes separadas que tu ou outros possuam.
* Usa tokens na blockchain para tratar da propriedade dos itens.

Ao ganhar um item do jogo, ele pode ser armazenado como um token especial na Wallet Ethereum de um jogador. Quando um jogador que possui o token entra na tua scene, a tua scene pode conceder ao jogador certas características dentro do jogo.

Outras scenes também poderiam responder ao mesmo token de formas diferentes, o que pode criar uma interação interessante entre jogos.

A desvantagem de usar a blockchain para armazenar itens do inventário é que todas as transações têm um custo para o jogador e não são imediatas. Lê mais sobre a blockchain numa secção especializada mais abaixo.

Em futuras versões, os jogadores terão um inventário que levam consigo para todo o lado e que incluirá ativos on-chain e off-chain.

### Experiências portáteis


> [!NOTE]
> **💡 Dica**: Vê [Experiências portáteis](/docs/creator/content-creator-pt/scenes-sdk7/tipos-de-projetos/portable-experiences/) e [Smart wearables](/docs/creator/content-creator-pt/scenes-sdk7/tipos-de-projetos/smart-wearables/) para saber como os criar.


As experiências portáteis são partes da jogabilidade que os jogadores levam consigo à medida que se movem pelo metaverse. Estas não estão ligadas a parcelas de terreno; por vezes estão ligadas a tokens, ou por vezes são lançadas pelo Explorer. Por exemplo, um jogador poderia levar uma bola de neve da tua scene, ir para outra scene e atirar a bola de neve a outro jogador que também esteja a jogar o mesmo jogo.

Os smart wearables são um tipo de experiência portátil que está ligado a um wearable token e é ativado quando o jogador veste a peça de roupa. Os smart wearables podem conceder aos jogadores novas capacidades, como uma jetpack que lhes permite voar, ou adicionar uma nova camada de conteúdo por cima do resto do mundo, como colocar aleatoriamente moedas a serem recolhidas ao longo de todo o Genesis City.

Tem em conta que os jogadores podem estar a usar a experiência portátil de outra pessoa enquanto estão na tua scene. Vê [User Data](/docs/creator/content-creator-pt/scenes-sdk7/interatividade/user-data/#get-portable-experiences) para saber como verificar quais as experiências portáteis que um jogador tem atualmente ativadas.

## Persistência do jogo

**Decentraland é um mundo persistente, a tua scene pode ser visitada por jogadores a qualquer momento.** A tua scene não tem fase de arranque nem fim, por isso deves desenhar as mecânicas do jogo de forma a permitir que os jogadores que entram ou saem a qualquer momento também possam participar.

A tua scene pode ter um mecanismo de reinício que a coloque num estado inicial, mas deves ter cuidado para não perturbar o jogo dos jogadores que já estão a jogar.

### Sincronizar o estado da scene

**Por defeito, os estados das scenes não são partilhados entre jogadores.** Cada jogador executa a sua própria cópia local da scene. Esta é a forma mais simples de construir uma scene, mas não é ideal para experiências sociais.

A forma mais fácil de partilhar o estado entre jogadores é marcar entities como sincronizadas, usando a `syncEntity` função. Vê [Serverless multiplayer](/docs/creator/content-creator-pt/scenes-sdk7/rede/serverless-multiplayer/). Com esta abordagem, as alterações de estado não são armazenadas em lado nenhum: se não houver jogadores atualmente perto da scene e a carregá-la, a scene será reiniciada para um estado predefinido da próxima vez que for carregada.

**Também podes usar um server para armazenar informação sobre a tua scene e manter todos os jogadores sincronizados com ela.** Isto mantém o estado da scene consistente e persistente mesmo quando não há jogadores por perto, e permite validar alterações para que os jogadores não possam fazer batota. A opção recomendada é usar um [Multiplayer Server](/docs/creator/content-creator-pt/scenes-sdk7/rede/authoritative-servers/), que Decentraland aloja e implementa por ti como parte da publicação da tua scene. Também podes alojar [o teu próprio server](/docs/creator/content-creator-pt/scenes-sdk7/rede/third-party-servers/), o que te permite manter certa informação, como tokens de segurança, apenas no server, sem expor essa informação para o exterior.

### Tempo do jogo

**Os jogos que usam a arquitetura de comunicações padrão devem ter em conta que pode haver lag entre jogadores** e não devem depender de reações rápidas entre as ações de jogadores diferentes. Recomendamos jogos por turnos, ou jogos baseados maioritariamente em interações jogador versus ambiente.

Para jogos em que o timing das ações entre jogadores é crítico, como um first-person shooter, deves implementar o teu próprio server como uma fonte de verdade autoritária em tempo real entre todos os jogadores da tua scene.

## Jogadores na scene

**Os jogadores são identificados em Decentraland através do endereço da sua Wallet Ethereum.** Esta Wallet é usada como um ID persistente que já está associado a todos os tokens que o jogador possui.

**Atualmente não existe forma de limitar quantos jogadores podem estar presentes em Decentraland ao mesmo tempo.** Ao contrário de muitos outros jogos, em que podem existir diferentes sessões de jogo alojadas em servers separados, existe apenas uma instância de Decentraland partilhada por todos os jogadores, pelo menos por agora.

Tens de ter em conta que pode haver vários jogadores a circular pela tua scene em qualquer momento. Alguns deles podem estar apenas de passagem e não participar no jogo. Garante que as mecânicas do jogo não possam ser facilmente perturbadas por isto.

**O loop de jogo da tua scene não pode afetar os jogadores diretamente**, a scene tem uma abordagem reativa às ações do player. Se um player estiver em cima de uma entity e a entity se mover ou rodar, o player mover-se-á com essa entity. Isto é especialmente útil para elevadores, plataformas flutuantes e afins.

Como proprietário de uma scene, não podes empurrar à força nem fazer teleport a um player infrator para fora da tua scene. No entanto, poderás colocar players numa blacklist no signaling server. Também podes implementar uma blacklist no código da tua scene e negar certos serviços a players na blacklist.

## Constrói interiores maiores do que na vida real

A maioria dos jogadores explora Decentraland na câmara de terceira pessoa padrão, que fica atrás e acima do avatar. Essa câmara precisa de espaço. Um interior construído nas dimensões do mundo real aparece no ecrã como um corredor apertado: a câmara encosta-se ao teto, o avatar ocupa o enquadramento e o jogador não consegue ver para onde vai.

Constrói a estrutura de uma sala com aproximadamente **1,5 a 2 vezes** a altura livre real:

| Elemento                                         | Mundo real                                           | Constrói-o com                                                  |
| ------------------------------------------------ | ---------------------------------------------------- | --------------------------------------------------------------- |
| Altura do teto                                   | 2,5 a 3 m                                            | **5 a 6 m**                                                     |
| Passagem ou abertura numa rota principal         | cerca de 2,1 m x 0,9 m                               | **cerca de 4 m de altura, 3 m ou mais de largura**              |
| Largura do corredor                              | 1,2 a 1,8 m                                          | **4 m ou mais de largura**                                      |
| Qualquer espaço que o avatar tenha de atravessar |                                                      | **1,2 m de espaço livre ou mais**                               |
| Escadas                                          | cerca de 0,18 m de altura por degrau, 0,28 m de piso | **0,25 m de altura por degrau ou menos, 0,3 m de piso ou mais** |
| Rampas                                           |                                                      | **30 graus ou menos**                                           |

A câmara de terceira pessoa orbita a cerca de 4,5 m de distância do avatar, com um campo de visão de 60 graus. Quando uma parede ou teto fica entre a câmara e o avatar, a câmara avança para manter o avatar à vista, e esse salto repentino é o que faz com que uma sala pareça apertada. Por isso, as salas em que os jogadores se viram ficam melhor com cerca de **9 m de largura ou mais**, o dobro da distância da câmara. Para os limites do motor em degraus, inclinações e altura livre, vê [Restrições espaciais do player](/docs/creator/content-creator-pt/modelacao-3d-e-animacoes/colliders/#player-spatial-constraints).

Algumas coisas a ter em conta:

* **Aumenta a sala, não o seu conteúdo.** Cadeiras, secretárias, balcões, corrimões e sinalética mantêm-se próximos do tamanho real. Se aumentares a escala do mobiliário juntamente com as paredes, recrias o mesmo sentimento de aperto numa escala maior, e o avatar deixa de ser percecionado como uma pessoa na sala.
* **Aplica isto quando planeares a tua área do piso**, não depois de teres colocado tudo. Salas maiores precisam de mais parcelas, por isso estes números alteram o teu [orçamento de parcelas](/docs/creator/content-creator-pt/scenes-sdk7/tipos-de-projetos/scene-metadata/).
* **Julga o resultado a partir da câmara normal de terceira pessoa.** Uma free camera ou o viewport do Creator Hub favorecem os interiores, porque podem ficar fora da geometria e olhar para dentro. Percorre a rota como um player o faria. Se a câmara cortar o teto, ou se a cabeça do avatar ocupar demasiado o topo do enquadramento, a sala é demasiado baixa.


> [!NOTE]
> **💡 Dica**: Esta é uma orientação de design retirada da prática, não um limite do motor. Considera os números como ponto de partida e ajusta-os a olho.


## Limitações do conteúdo da scene

**Constrói a tua scene tendo um cuidado extra com a eficiência do teu código.** Decentraland precisa de correr em navegadores web e dispositivos móveis, e os jogadores vão renderizar várias scenes ao mesmo tempo à medida que percorrem o metaverse.

**Deves também tentar manter a scene leve.** Ao contrário de outros jogos online em que as mesmas textures e assets são convenientemente repetidos ao longo de um grande open world, em Decentraland cada scene pode ter o seu próprio conjunto completamente diferente de assets. À medida que os jogadores atravessam várias scenes, devem conseguir descarregar a totalidade do conteúdo da scene, incluindo textures, ficheiros de som, etc., a uma velocidade razoável.

Por causa disto, impusemos alguns limites para evitar o uso excessivo de recursos computacionais. Vê [limitações da scene](/docs/creator/content-creator-pt/scenes-sdk7/otimizacao/scene-limitations/) para detalhes sobre quais são estes limites.

## Acesso às scenes

**O mapa de Decentraland foi concebido para que existam estradas e praças públicas,** estas garantem acesso fácil a várias partes do mapa, independentemente do que outras pessoas construam. As parcelas de terreno que não estejam adjacentes a nenhuma estrada ou praça correm o risco de ficarem bloqueadas por scenes vizinhas, embora esperemos que a maioria das scenes seja percorrível a pé e não bloqueie as outras.

Os novos jogadores começarão a sua experiência em Genesis Plaza, no centro do mapa, onde serão incentivados a seguir algumas atividades de tutorial e depois a explorar o mundo.

Os jogadores também podem escrever manualmente um URL para uma coordenada específica no mapa de Decentraland para fazer spawn nessa localização. Também podes partilhar links para URLs que tenham coordenadas iniciais codificadas.

Tem em conta que, se um player começar numa localização bloqueada ou abaixo do nível do terreno, não será uma experiência agradável. Para evitar isso, há uma forma de definires um conjunto de localizações específicas na tua scene que seja seguro para fazer spawn. Vê [metadata da scene](/docs/creator/content-creator-pt/scenes-sdk7/tipos-de-projetos/scene-metadata/) para detalhes.

Os jogadores também podem navegar rapidamente pelo world usando o mapa e as listas de locais e eventos populares. A tua scene pode incluir teleports que transportam jogadores para outras partes do mundo, vê [Teleports](/docs/creator/content-creator-pt/scenes-sdk7/interatividade/external-links/#teleports).

## UI do utilizador

**A UI sobreposta padrão que os jogadores veem ao entrar em Decentraland tem apenas o essencial.** Podes adicionar elementos extra a essa UI enquanto um player estiver na tua scene. Tem em conta que a UI padrão de Decentraland é apresentada acima de qualquer coisa da tua scene, por isso desenha a tua UI de modo a que não se sobreponha a esta.

Quando um player sai da scene, todos os elementos de UI são removidos para não interferirem com outras scenes. Os jogadores também têm um botão disponível no ecrã para desativar todos os elementos de UI da scene; isto é principalmente útil para evitar comportamentos abusivos de scenes que possam querer cobrir todo o campo de visão do jogador.

## Physics

Tem em conta que o SDK não fornece um motor de física completo para entities. Podes aplicar forças e impulsos ao avatar do player, vê [Player physics](/docs/creator/content-creator-pt/scenes-sdk7/interatividade/player-physics/). Para física entre entities, como colisões ou gravidade, podes importar uma library ou programar o comportamento tu mesmo.

## Entradas do controlador

**Os controlos do teu jogo devem limitar-se ao conjunto de input actions suportadas pelo SDK**: as teclas de movimento, saltar, point and click, os botões primário (E) e secundário (F), e os botões de ação numerados (1 a 4). Estas entradas são abstraídas das teclas físicas, para que também possam mapear para os controlos no ecrã da app móvel ou para outros dispositivos; não assumas que toda a gente tem um teclado. Vê [Pointer buttons](/docs/creator/content-creator-pt/scenes-sdk7/interatividade/eventos-de-botao/click-events/#pointer-buttons) para a lista completa, e [Input on mobile](/docs/creator/content-creator-pt/criar-para-mobile/desenvolver/input-on-mobile/) para o que é preferível quando te diriges a jogadores em mobile.

Todas estas entradas suportam tanto eventos globais de *button up* e *button down* como eventos hit que te permitem identificar se uma entity estava na mira do player.

## Avatars

**Os jogadores podem criar os seus avatars a partir de um grande catálogo de wearable items**, incluindo wearables criados e vendidos por criadores da comunidade no Decentraland Marketplace. A tua scene pode ler o que um player está a usar e reagir a isso, vê [User data](/docs/creator/content-creator-pt/scenes-sdk7/interatividade/user-data/#get-player-data).

## Comunicação entre jogadores

**Os jogadores podem conversar entre si, falar por voice chat e transmitir linguagem corporal ao fazerem emotes** como dançar, aplaudir ou acenar, incluindo emotes vendidos por criadores da comunidade.

A tua scene pode detetar quando um player faz um emote e também pode fazer com que o avatar de um player reproduza uma animação. Vê [Player plays animation](/docs/creator/content-creator-pt/scenes-sdk7/interatividade/event-listeners/#player-plays-animation) e [Avatar Animations](/docs/creator/content-creator-pt/scenes-sdk7/interatividade/avatares/avatar-animations/).

## Notificações do jogo

**Atualmente não existe um sistema de notificações entre scenes.** Qualquer jogo que exija notificações apresentadas fora da scene atual terá de as implementar usando um serviço externo.

## Usar a blockchain

**Em Decentraland, a blockchain é usada para armazenar informação sobre propriedade.** Hoje isto refere-se sobretudo à propriedade de LAND, mas também pode ser usado para a propriedade de itens do jogo, wearables, avatars especiais, emotes e tokens que podem garantir certos privilégios de jogo ou acesso a jogos.

A blockchain não é usada para armazenar o estado do jogo, a posição do player ou qualquer coisa que precise de mudar em tempo real.

### LAND e MANA

**Os jogadores não precisam de possuir quaisquer parcelas de terreno para participar no metaverse.** Na verdade, a grande maioria dos jogadores não o fará. Os avatars dos jogadores e os tokens LAND que possuem não estão ligados de forma direta.

**Os jogadores não precisam de já possuir uma Wallet Ethereum ou tokens MANA para entrar em Decentraland.** Se a tua jogabilidade depender fortemente da posse de tokens, estarás a excluir a maioria dos jogadores. Um modelo de jogo freemium pode ser uma forma ideal de servir tanto uma base de utilizadores como a outra.

### Outros NFTs

**Podes usar special non-fungible tokens (NFTs) para representar itens do jogo, avatars personalizados ou wearables.** Se um player possuir um destes tokens, a tua scene pode responder-lhe de formas diferentes.

Lê sobre o que são NFTs em [este artigo do blog](https://decentraland.org/blog/technology/what-are-nfts/).

### Transações no jogo

**A tua scene pode suportar transações em blockchain para os jogadores comprarem ou ganharem tokens.**

As transações em blockchain não são imediatas; requerem tempos de verificação e têm um custo em Ether. Tanto o tempo como o custo variam consoante a utilização atual da rede.

Decentraland usa a side-chain Polygon para tratar as transações de forma mais rápida e mais barata do que a rede Ethereum. Esta side-chain é ideal para transações no jogo, uma vez que as alterações podem ocorrer mais perto do tempo real e a um custo muito baixo. A main chain Ethereum continua a ser recomendada para transações que exijam maior segurança e que possam ser mais caras e demorar mais tempo. Vê [Segunda camada](/docs/creator/content-creator-pt/scenes-sdk7/blockchain/second-layer/).

O player tem sempre de aprovar explicitamente estas transações no seu cliente Ethereum. Por exemplo, ao usar Metamask, o Metamask pede ao player para aceitar cada transação antes de ela ser processada.

Os jogadores também podem assinar um contracto que aprova automaticamente todas as transações pedidas por um endereço específico ou dentro de certas restrições, para evitar interrupções na aprovação de transações.

Também podes usar smart contracts para condicionar transações com base em condições personalizadas. Por exemplo, os jogadores poderiam apostar no resultado de um jogo, e os pagamentos correspondentes ocorreriam automaticamente assim que o resultado fosse conhecido.

Para implementar interações com a blockchain no código da tua scene, tens de usar libraries externas que interagem com a rede Ethereum. Versões futuras do SDK fornecerão uma API personalizada para expor estas funcionalidades de uma forma mais simples.


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-pt/scenes-sdk7/conceber-a-experiencia/design-games.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
