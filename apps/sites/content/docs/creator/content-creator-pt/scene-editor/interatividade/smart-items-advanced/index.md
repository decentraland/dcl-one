> Use smart items na sua scene para adicionar interatividade.

# Smart Items - Avançado

A maioria dos smart items tem um módulo básico onde podes configurar apenas as definições mais comuns de forma simples, mas podes descer para além do **Avançado** marcador para personalizares quase tudo sobre o comportamento do item.



<div class="video-embed"><button type="button" class="video-play" data-video-src="https://www.youtube-nocookie.com/embed/m_xWCSDDxpQ" aria-label="Play YouTube video"><svg viewBox="0 0 64 64" width="64" height="64" aria-hidden="true"><circle cx="32" cy="32" r="30" fill="currentColor"/><path d="m26 20 20 12-20 12Z" fill="#181214"/></svg><span>Play video</span></button></div>
<p class="video-source"><a href="&lt;https://www.youtube.com/watch?v=m_xWCSDDxpQ>">Watch on YouTube</a></p>



O item seguinte tem um component Transform e um módulo básico que expõe apenas os campos básicos para configurar um botão. Mas, se desceres para além do **Avançado** marcador, encontrarás todas as definições disponíveis.

![](/docs/_remote/6f2502ce1e009c232a69e7cb.webp)


> [!NOTE]
> **📔 Nota**: A maioria das definições no módulo básico também está disponível nos components mais abaixo. As alterações feitas no módulo básico refletem-se nos components mais abaixo e vice-versa, exceto em alguns casos em que as definições básicas são uma abstração de várias definições mais abaixo. Nesses casos, alterar as definições avançadas para valores que não sejam suportados pelo módulo básico fará com que o campo no módulo básico seja marcado como indefinido.


## Configuração avançada

As propriedades são agrupadas em [**components**](/docs/creator/content-creator-pt/scenes-sdk7/arquitetura/entities-components/). Diferentes smart items podem ter components diferentes, dependendo da sua funcionalidade.

O comportamento da maioria dos items é controlado por:

* [**Actions**](#actions): O component Actions define o que o item pode fazer. Por exemplo, reproduzir um som, reproduzir uma animação, mover-se para cima ou tornar-se invisível.
* [**Triggers**](#triggers): O component Triggers atribui que eventos fazem essas ações acontecer. Por exemplo, quando o player clica no item, quando o player entra numa área ou quando a scene carrega pela primeira vez.

Por exemplo, num smart item de porta, o **Actions** component inclui as ações "Open" e "Close". O **Triggers** component nesse item inclui um **On Click** trigger que ativa a ação "Open" quando o player clica na porta.

Os triggers de um smart item podem ativar ações em qualquer smart item da scene, não apenas nesse mesmo smart item. Por exemplo, um smart item de botão pode ter um **Triggers** component que ativa a ação "move up" definida no **Actions** component de uma plataforma flutuante.

Os triggers também podem acontecer condicionalmente. Por exemplo, os smart items de porta incluem dois **On Click** triggers no seu component Triggers: um abre a porta se essa porta estiver fechada, o outro fecha a porta se estiver aberta. Para mais detalhes, consulta [Estados e lógica condicional](/docs/creator/content-creator-pt/scene-editor/interatividade/states-and-conditions/).

## Interações entre items

Para fazer com que os items interajam entre si:

* Um item precisa de ter pelo menos uma ação definida num [Actions](#actions) component.
* O outro item precisa de um trigger no [Triggers](#triggers) component que aponta para essa ação.

Por exemplo, para fazer um botão abrir uma porta:

1. Adiciona qualquer smart item de botão, abre o seu **Triggers** component. Tem um evento trigger predefinido que reproduz um som e uma animação para o próprio botão.
2. Clica no **+** sinal junto de **Assigned Actions**, para adicionar uma terceira ação nesse mesmo evento trigger.
3. Seleciona o smart item da porta no primeiro dropdown.
4. No segundo dropdown, seleciona a ação "Open".

![](/docs/_remote/564f4fe0ae016962310472b6.webp)


> [!NOTE]
> **💡 Dica**: Em alternativa, podes criar um novo evento Trigger que trate apenas da ação da porta. Ambos os eventos trigger são chamados sempre que o botão é clicado.
> 
> <img src="/docs/_remote/4c4d3311baeb14d82a595558.webp" alt="" data-size="original">


Qualquer item pode desencadear qualquer ação de qualquer outro item, desde que a ação esteja definida. Vê [Triggers](#triggers) para mais formas de acionar uma ação.

Podes usar [states and conditional logic](/docs/creator/content-creator-pt/scene-editor/interatividade/states-and-conditions/) para acionar uma ação apenas se uma condição for satisfeita. A condição pode até verificar o estado de um terceiro smart item. Por exemplo, um botão só abre a porta se o smart item personalizado "power generator" estiver com o seu estado definido como "On".

## Actions

O **Actions** O component lista as ações que o item pode executar. Cada smart item inclui um conjunto de ações predefinidas. Podes personalizar ações existentes ou adicionar novas. Estão disponíveis os seguintes tipos de ações:

* **Play Animation**: Reproduz uma animação no modelo 3D do item. Consulta [About playing animations](#about-playing-animations)
* **Stop Animation**: Para todas as animações que estão a ser reproduzidas pelo modelo 3D do item.
* **Play Sound**: Reproduz um som a partir de um ficheiro, na localização do item. Consulta [About playing sounds](#about-playing-sounds)
* **Stop Sound**: Para todos os sons que estão a ser reproduzidos a partir do item.
* **Start Tween**: Faz uma alteração gradual na posição, rotação ou escala durante um determinado período. Consulta [Mover, rodar ou escalar](#moving-rotating-or-scaling).
* **Set Visibility**: Torna o item visível ou invisível.
* **Attach To Player**: Define o item como filho do avatar do jogador. Por exemplo, para o transportar na mão ou acima da cabeça.
* **Detach From Player**: Desanexa o item do avatar do jogador.
* **Open Link**: Abre uma ligação para um site externo num separador do navegador. Os jogadores são questionados se confiam no domínio antes de ele abrir.


> [!NOTE]
> **📔 Nota**: Esta ação só pode acontecer como resultado de clicar num item. Não pode ser acionada ao entrar numa trigger area.


* **Move Player**: Altera a posição do jogador para um conjunto de coordenadas locais dentro da scene. Só é possível mover o jogador dentro da mesma scene.
* **Teleport Player**: Teleporta um jogador para outra localização em Decentraland. Usa o **Teleport Mode** dropdown para os enviar **To coordinates** de outra scene em Genesis City, ou **To World**, indicando o nome de um World. Os jogadores aparecerão no ponto de spawn da scene de destino.
* **Play Emote**: Faz o avatar do jogador executar uma das animações padrão do avatar (por exemplo: acenar ou bater palmas).
* **Play Custom Emote**: Faz o avatar do jogador executar uma animação personalizada, a partir de um ficheiro carregado para a scene.
* **Show Text**: Exibe texto na UI do ecrã, para ser escondido após alguns segundos. Ideal para dicas, falas de diálogo, notificações, etc.
* **Hide Text**: Oculta qualquer texto da UI que possa estar atualmente apresentado.
* **Start Delay**: Atrasa outra ação do mesmo item durante tantos segundos quantos precisares.
* **Stop Delay**: Cancela quaisquer ações atrasadas no item.
* **Start Loop**: Repete uma ação do mesmo item recorrentemente num intervalo definido.
* **Stop Loop**: Cancela quaisquer ações em loop no item.
* **Play Video**: Reproduz um vídeo como material numa forma primitiva.
* **Stop Video**: Para quaisquer vídeos que estejam atualmente a ser reproduzidos.
* **Play Audio Stream**: Reproduz um stream de áudio.
* **Stop Audio Stream**: Para quaisquer streams de áudio que estejam atualmente a ser reproduzidos.
* **Clone**: Duplica um item na posição designada.
* **Spawn Entity**: Cria uma nova cópia de um item na scene, numa posição relativa ao item que executa a ação. O item a ser criado não precisa de ser colocado na scene. [Sobre a geração de entities](#about-spawning-entities).
* **Remove**: Elimina um item da scene.
* **Show Image**: Exibe uma imagem na UI, potencialmente por tempo limitado. Também pode incluir legenda.
* **Hide Image**: Oculta qualquer imagem atualmente apresentada na UI através da ação Show Image.
* **Damage**: Reduz a vida de qualquer healthbar que esteja perto. A *Layer* propriedade pode determinar se só atua sobre healthbars no player, ou noutros items.
* **Move player here**: Altera a posição do player para a posição deste item.
* **Place on Player**: Altera a posição do item para a do player.
* **Rotate as Player**: Altera a rotação do item para a do player.
* **Place on Camera**: Altera a posição do item para a da camera.
* **Rotate as Camera**: Altera a rotação do item para a da camera.
* **Set Position**: Altera a posição do item para uma posição específica. Pode ser absoluta ou relativa à sua posição atual.
* **Set Rotation**: Altera a rotação do item para uma rotação específica. Pode ser absoluta ou relativa à sua rotação atual.
* **Set Scale**: Altera a escala do item para uma escala específica. Pode ser absoluta ou relativa à sua escala atual.
* **Follow Player**: Começa a mover-se e a virar-se na direção da posição do player. Ignora quaisquer obstáculos no caminho. Podes definir a velocidade e fazer com que só se mova em certos eixos. A Min Distance determina a que distância se aproximará do player.
* **Stop Following Player**: Para a ação Follow Player.
* **Random Action**: Uma das ações listadas aqui será reproduzida aleatoriamente com igual probabilidade sempre que a random action for chamada. Podes listar qualquer uma das ações que pertençam ao item.
* **Batch Actions**: Todas as ações listadas aqui serão reproduzidas em simultâneo sempre que a batch action for chamada. Podes listar qualquer uma das ações que pertençam ao item.
* **Heal Player**: Restaura a vida da health bar do player.
* **Player Face Item**: Faz com que o avatar do player se vire para o item.
* **Freeze Player**: Impede o player de se mover, saltar ou executar emotes.
* **Unfreeze Player**: Restaura a capacidade do player de se mover após uma ação Freeze Player.
* **Lights On**: Liga o **Light Source** component.
* **Lights Off**: Desliga o **Light Source** component.
* **Lights Modify**: Altera propriedades do **Light Source** component, como a sua cor ou intensidade.
* **Select Camera**: Muda a vista do player para uma **Virtual Camera**, seja neste item seja noutra entity.
* **Change Text**: Altera o texto de um item com um **Text Shape** component. Também pode alterar o tamanho da fonte e a cor.
* **Stop Tween**: Interrompe qualquer movimento tween atualmente em curso no item.
* **Slide Texture**: Desliza continuamente a textura no material do item numa determinada direção, para efeitos como água corrente ou tapetes rolantes.
* **Change Collisions**: Ativa ou desativa os colliders do item, tanto para física como para pointer events.
* **Change Skybox**: Altera o skybox da scene para uma hora fixa do dia, expressa em segundos desde a meia-noite.
* **Reset Skybox**: Restaura o skybox da scene para as suas definições predefinidas.
* **Log to Console**: Imprime uma mensagem na console, útil para depuração enquanto desenvolves a scene.
* **Eliminar**: Remove o item e todos os seus filhos da scene.

Veja [states and conditional logic](/docs/creator/content-creator-pt/scene-editor/interatividade/states-and-conditions/) para saberes mais sobre outras ações relacionadas com condições lógicas, como **Set State** e as ações de contador.

O **Actions** component define ações possíveis, mas estas não fazem nada na scene a menos que sejam acionadas. Actions são ativadas por um [trigger](#triggers), seja do mesmo smart item seja de outro diferente.

Para adicionar uma nova action a um item, clica no **Add New Action** button na parte inferior do component Action. Depois dá um nome à action, seleciona um tipo e preenche quaisquer campos adicionais específicos desse tipo de action.

![](/docs/_remote/af4f04d249dfe05512b5fb47.webp)

### Triggers

O **Triggers** component define eventos trigger; estes ativam actions quando acontece um determinado evento. Existem os seguintes tipos de eventos trigger:

* **On Click**: Quando o player clica no item. Consulta [About click triggers](#about-click-triggers)
* **On Input Action**: Quando o player prime um botão de input enquanto aponta para o item.
* **On Global Click**: Quando o player clica com o pointer em qualquer lugar, sem precisar de apontar para o item.
* **On Global Primary**: Quando o player prime o botão Primary (E) em qualquer lugar.
* **On Global Secondary**: Quando o player prime o botão Secondary (F) em qualquer lugar.
* **Player Enters Area**: Quando o player entra numa área. Consulta [Trigger Area](/docs/creator/content-creator-pt/scene-editor/interatividade/trigger-area/)
* **Player Leaves Area**: Quando o player sai de uma área. Consulta [Trigger Area](/docs/creator/content-creator-pt/scene-editor/interatividade/trigger-area/)
* **On Spawn**: Quando a scene começa, ou o item é criado na scene. Consulta [Trigger on spawn](#trigger-on-spawn)
* **On Delay**: Quando uma **Start Delay** action no item termina a sua contagem decrescente.
* **On Loop**: Em cada iteração de uma **Start Loop** action no item.
* **On Clone**: Quando o item é clonado através de uma **Clone** action, a nova cópia dispara este trigger.
* **On Click Image**: Quando o player clica numa imagem exibida por uma **Show Image** action.
* **On Damage**: Quando o item recebe damage de uma **Damage** action.
* **On Heal Player**: Quando o player é curado por uma **Heal Player** action.
* **On Tick**: Em cada tick da scene, uma vez por frame. Usa com cautela, pois as actions acionadas por ele correm com muita frequência.

Veja [states and conditional logic](/docs/creator/content-creator-pt/scene-editor/interatividade/states-and-conditions/) para saberes mais sobre outros triggers relacionados com condições lógicas, como **On State Change**, **On Counter Change** e **On Tween End**.

Para adicionar um novo trigger, clica em **Add New Trigger Event** na parte inferior do component Trigger. Depois seleciona o tipo de trigger, a entity que queres ativar e uma action dessa entity.

![](/docs/_remote/4c0c7ee0b8ac3b2401bebd31.webp)


> [!NOTE]
> **📔 Nota**: Uma action precisa de estar definida no [Actions](#actions) component da entity antes de a poderes acionar. Os triggers só podem afetar entities que tenham um component Actions.


## About Playing Animations

Usa uma action do tipo **Play Animation** para executar uma animação no modelo 3D do smart item. A animação precisa já de existir como parte do ficheiro do modelo 3D. O **Select Animation** dropdown apresenta uma lista de todas as animações disponíveis no modelo 3D.

O **Play Mode** campo permite escolher se uma animação deve ser reproduzida apenas uma vez, ou se deve continuar em loop.

![](/docs/_remote/b62f8f6fe8ab5cf4a5ed6eba.webp)

Assim que a action é criada, podes ativá-la através do [Triggers](#triggers) component desse mesmo item ou de qualquer outro item.

Use o seletor **Stop Animation** action para parar todas as animações do item, tanto as que estão em loop como as que não estão.


> [!NOTE]
> **💡 Dica**: Para verificares facilmente o conteúdo de um modelo 3D, para veres que animações inclui e como são, uma boa ferramenta é o [Babylon Sandbox](https://sandbox.babylonjs.com/). Basta arrastares o ficheiro do modelo 3D para a janela. Deverá aparecer na parte inferior um dropdown com uma lista das suas animações.


Para saberes mais sobre animações e como podes criar as tuas próprias como parte de um modelo 3D, consulta [Animações](/docs/creator/content-creator-pt/modelacao-3d-e-animacoes/animations/).

## About Playing sounds

Usa uma action do tipo **Play Sound** para reproduzir um ficheiro de som. Podes reproduzir qualquer ficheiro de som desde que esteja importado para o projeto da scene. O som é ouvido de forma posicional, a partir da localização do item, o que significa que soa mais alto se o player estiver mais perto.


> [!NOTE]
> **💡 Dica**: Em vez de escreveres o caminho para o ficheiro de som, podes arrastá-lo para o **Path** field a partir do menu de navegação de ficheiros na parte inferior do Scene Editor.


Use o seletor **Play Mode** campo para escolher se reproduzir o som uma vez ou em loop continuamente.

![](/docs/_remote/eae4449cca4301e0122d9583.webp)

Assim que a action é criada, podes ativá-la através do [Triggers](#triggers) component desse mesmo item ou de qualquer outro item.

Use o seletor **Stop Sound** action para parar todos os sons do item, tanto os que estão em loop como os que não estão. Isto também para os sons do **AudioSource** component.

Para fazer um item reproduzir sempre um som em loop, por exemplo para ambiente ou música, é mais fácil usar o **AudioSource** component, em vez de usares Actions e Triggers. Este component só requer que forneças um caminho para um ficheiro e marques as caixas **Start Playing** e **Loop**.

![](/docs/_remote/efba578789bf705bd7ec2955.webp)


> [!NOTE]
> **📔 Nota**: Um smart item só pode reproduzir um som de cada vez. Chamar um segundo som irá interromper quaisquer outros sons que estejam atualmente a soar. Isto também se aplica aos sons do **AudioSource** component. Se precisares que dois sons soem em simultâneo, considera adicionar uma entity invisível na mesma localização para conter um **Play Sound** action.


Veja [sons](/docs/creator/content-creator-pt/scenes-sdk7/fundamentos-de-conteudo-3d/sounds/) para saberes mais sobre reproduzir sons em Decentraland.

## Mover, rodar ou escalar

Usa uma **Start Tween** action para alterar a **posição**, **escala**, ou **rotação**, do item ao longo de um período de tempo. Todas as **Start Tween** actions partem do estado original do item e mudam para um estado final ao longo de um período de tempo.

Os tweens de posição podem ser relativos ou absolutos. Um tween absoluto de posição move o item para uma posição fixa em relação à scene. O item mover-se-á de onde quer que esteja para essa posição. Se já lá estiver, não parecerá mover-se. Um tween relativo de posição move o item uma certa distância a partir de onde está agora, por exemplo um tween para uma posição relativa de `1, 0, 0` move o item 1 metro para a frente, na direção para onde está atualmente virado. Se executares a action tween uma segunda vez, o item mover-se-á mais um metro para a frente.

Os tweens de rotação também podem ser relativos ou absolutos. Uma rotação relativa é adicionada à rotação atual do item. Um tween absoluto de rotação fará com que o item fique virado numa direção específica, em relação à scene.

Use o seletor **Duration** campo para definir quanto tempo todo o movimento deve demorar, em segundos. Nota que o slider vai até 100 segundos, mas também podes escrever manualmente um número maior se precisares.

![](/docs/_remote/04196395f61a96c50fbd476c.webp)

Assim que a action é criada, podes ativá-la através do [Triggers](#triggers) component desse mesmo item ou de qualquer outro item.

Os tweens podem seguir diferentes **Curve Types** que afetam a taxa de mudança ao longo do tempo. Uma **linear** curva (predefinição) significa que a velocidade da mudança é constante do início ao fim. Há muitas opções por onde escolher, que desenham curvas de formato diferente consoante o início e/ou o fim comecem devagar, e em que medida. Uma **easeinexpo** curva começa devagar e termina depressa, aumentando a velocidade exponencialmente; pelo contrário, uma **easeoutexpo** curva começa depressa e termina devagar.

![](/docs/_remote/8451a60c562d729a69cf2138.webp)


> [!NOTE]
> **💡 Dica**: Experimenta diferentes curvas de movimento. As diferenças são muitas vezes subtis, mas interpretamos subconscientemente informação sobre a forma como as coisas se movem, como peso, fricção ou até personalidade.


Use **On Tween End** eventos trigger no **Triggers** component para ativar uma action depois de um tween terminar. Usa [states and conditional logic](/docs/creator/content-creator-pt/scene-editor/interatividade/states-and-conditions/) para descrever um caminho em loop para uma plataforma flutuante, para que se mova constantemente entre dois locais.

Quando um item realiza um tween, isso afeta tudo o que diz respeito ao item. Por exemplo, se mudar a escala, muda a escala do seu modelo 3D visível e também da geometria invisível dos colliders, o tamanho do texto, etc. Se o item tiver quaisquer filhos (aninhados na entity tree à esquerda), essas entities filhas também são afetadas pelo tween.


> [!NOTE]
> **📔 Nota**: Cada entity só pode executar um tween de cada vez. Por exemplo, não podes fazer com que um item se mova para o lado e rode ao mesmo tempo. Como solução alternativa, podes usar entities parented. Por exemplo, podes ter uma entity pai invisível que se move para o lado, com um filho visível que roda.


## Sobre a geração de entities

Usa uma **Spawn Entity** action para criar uma nova cópia de um item na scene enquanto esta está em execução. Ao contrário da **Clone** action, o item que vais criar não precisa de ser colocado na scene primeiro.

### Tornar um item spawnable

Antes de um item poder ser criado, tem de estar disponível nos ficheiros do teu projeto. Para adicionar um item sem o colocar na scene:

1. No **Assets** painel, encontra o item no catálogo ou nos teus **Itens Personalizados**.
2. Clica com o botão direito no item e seleciona **Add to filesystem**.

Isto copia o item para o teu projeto para que possa ser criado mais tarde. Não adiciona nada visível à scene. O item aparece então no **Source** dropdown de qualquer **Spawn Entity** action.

### Configurar a ação

Um **Spawn Entity** a ação tem dois campos:

* **Source**: O item a ser gerado. Escolha entre os itens adicionados aos arquivos do seu projeto.
* **Posição**: As coordenadas X, Y e Z onde a nova cópia aparece, relativas ao item que executa a ação.

Se o item que você gerar for, ele próprio, um smart item, cada cópia gerada mantém suas próprias actions e triggers funcionando de forma independente.


> [!NOTE]
> **💡 Dica**: Para gerar items a partir do código, veja [Composites](/docs/creator/content-creator-pt/scenes-sdk7/arquitetura/composites/). Os items adicionados aos arquivos do seu projeto são armazenados como composites, no mesmo formato usado pela função spawn do SDK.


## About click triggers

Para acionar uma action ao clicar em um item, crie um **On Click** trigger. A action será ativada toda vez que o player clicar na entity.

![](/docs/_remote/342cba6acbd2ae9ce5e8ce5f.webp)

Veja [Tornar qualquer item smart](/docs/creator/content-creator-pt/scene-editor/interatividade/make-any-item-smart/#interactivity) para mais detalhes.


> [!NOTE]
> **📔 Nota**: Ao usar modelos 3D personalizados, o modelo deve ter uma geometria de collider invisível para poder ser clicável. Veja [colliders](/docs/creator/content-creator-pt/scenes-sdk7/fundamentos-de-conteudo-3d/colliders/#pointer-blocking).
> 
> Como alternativa, você pode configurar o **GLTF** component do item, para que seu **Visible Layer** de colisão seja definida como **Pointer**.
> 
> Outra alternativa é adicionar um **Click Area** smart item, para desenhar um cubo que se sobreponha ao item que você deseja clicar. O smart item Click Area é um [item invisível](#invisible-items).


## Trigger on spawn

Triggers do tipo **On Spawn** ativam uma action quando a Scene é carregada. Em vez de esperar que o player interaja com um item, a action é executada imediatamente.

Por exemplo, use isso para fazer uma plataforma se mover continuamente. Use um **On Spawn** trigger para ativar uma action tween. Em seguida, use **On State Change** triggers para mantê-la se movendo entre duas ou mais posições.

![](/docs/_remote/b027f05ed8b8aee085690f4b.webp)

## Multiplayer

Todos os smart items são multiplayer por padrão. Veja [Smart Items - Basic](/docs/creator/content-creator-pt/scene-editor/interatividade/smart-items/) para mais detalhes.

Você pode alterar ou ajustar esse comportamento multiplayer para sincronizar apenas certos components do item.

No item, **Multiplayer** component, marque as caixas dos components que você deseja compartilhar.

Por exemplo, uma porta compartilha seu `Animator` para que todos vejam as animações de abertura, seu `AudioSource` para que todos ouçam o seu som, e seu `State` para que todos acompanhem se ela está aberta ou fechada no momento. A porta não compartilha seu `Visibility` component, porque a porta normalmente está sempre visível. Se você incluir actions para ativar e desativar sua visibilidade, talvez também queira marcar esse component, para que as mudanças sejam sincronizadas entre todos os players.

## Itens invisíveis

Alguns items não foram feitos para serem vistos pelo player, mas ficam visíveis enquanto você edita sua Scene para facilitar o gerenciamento. É o caso de items como **Ambience**, **Trigger Area**, **Click Area**, etc.

No modo avançado, esses items têm um **Visibility** component definido como invisível. Esse component não afeta a visibilidade dos items no Scene Editor, mas qualquer item definido como invisível não é visto pelos players ao executar um preview.

## Veja também

* [Smart items - Basics](/docs/creator/content-creator-pt/scene-editor/interatividade/smart-items/)
* [States and conditions](/docs/creator/content-creator-pt/scene-editor/interatividade/states-and-conditions/)
* [Tornando qualquer item smart](/docs/creator/content-creator-pt/scene-editor/interatividade/make-any-item-smart/)
* [Combinar com código](/docs/creator/content-creator-pt/scene-editor/expandir-com-codigo/overview/)


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-pt/scene-editor/interatividade/smart-items-advanced.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
