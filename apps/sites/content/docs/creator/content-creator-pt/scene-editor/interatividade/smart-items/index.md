> Use smart items na sua scene para adicionar interatividade.

# Smart Items - Básico

Alguns dos itens no catálogo do Scene Editor são **Smart Items**. Os jogadores podem interagir com eles, eles têm propriedades configuráveis e podem disparar ações em outros smart items. Por exemplo: portas que podem ser abertas e fechadas, plataformas que sobem e descem, ou botões e alavancas que podem ativar outros itens.



<div class="video-embed"><button type="button" class="video-play" data-video-src="https://www.youtube-nocookie.com/embed/z7HF4GR01hE" aria-label="Play YouTube video"><svg viewBox="0 0 64 64" width="64" height="64" aria-hidden="true"><circle cx="32" cy="32" r="30" fill="currentColor"/><path d="m26 20 20 12-20 12Z" fill="#181214"/></svg><span>Play video</span></button></div>
<p class="video-source"><a href="&lt;https://www.youtube.com/watch?v=z7HF4GR01hE>">Watch on YouTube</a></p>



Você pode reconhecer esses itens no explorador de asset pack porque eles têm um ícone de raio e um fundo de cor diferente.

![](/docs/_remote/4401edd6b0ae269ede4c9467.webp)

Você pode reconhecer quais itens na sua cena são smart porque eles têm o ícone de raio ao lado deles na entity tree.

![](/docs/_remote/d3ff99754b5e412ff6b047ee.webp)

## Usando itens

Para usar um smart item, arraste-o para a cena como qualquer outro item. Todos os itens incluem um comportamento padrão; execute uma prévia da cena para testá-lo.

Aqui estão alguns itens comuns e seus comportamentos padrão:

* **Portas**: As portas são abertas ou fechadas quando clicadas. Você pode alterar esse comportamento para que sejam abertas por botões, trigger areas etc.
* **Botões**: Quando clicados, eles reproduzem som e uma animação como feedback. Adicione mais ações aos seus eventos de gatilho para ativar outros smart items.
* **Alavancas**: Quando clicadas, elas alternam entre dois estados. Faça com que cada posição da alavanca execute ações diferentes em outros smart items.
* **Baús**: Eles se comportam como portas; por padrão, são abertos ou fechados quando clicados. Você pode colocar itens menores dentro deles.
* **Plataformas**: Eles se movem entre duas posições. Use suas ações tween para controlar para onde eles vão, a velocidade etc.
* **Área de gatilho**: Um item invisível que pode disparar outros smart items quando o jogador entra em sua área. Veja [Sobre áreas de gatilho](#trigger-areas).
* **Avatar Modifier Area**: Uma área invisível que altera como os avatares se comportam ou aparecem dentro dela; por exemplo, ocultando avatares ou desativando popups de passaporte. Veja [Modifier Areas](/docs/creator/content-creator-pt/scene-editor/interatividade/modifier-areas/).
* **Camera Modifier Area**: Uma área invisível que força a câmera do jogador para a primeira ou terceira pessoa enquanto estiver dentro dela. Veja [Modifier Areas](/docs/creator/content-creator-pt/scene-editor/interatividade/modifier-areas/).
* **Video Player**: Uma tela para mostrar vídeos ou streams ao vivo. Veja [Reproduzindo vídeos](#playing-videos).
* **Audio Stream**: Reproduza áudio de um stream ao vivo. Veja [Reproduzindo streams de áudio](#playing-audio-streams)
* **NFT**: Exiba uma imagem de NFT como um retrato. Veja [Exibindo NFTs](#displaying-nfts)
* **Floor Pressure Button**: Uma placa redonda que afunda enquanto um jogador fica em cima dela e volta a subir quando ele sai. Use seus **On Press** e **On Release** hooks para executar ações, por exemplo, para manter uma porta aberta enquanto alguém estiver em cima dela.
* **Water Plane** e **Water Cylinder**: Superfícies de água que espirram quando um jogador anda ou pula nelas. Veja [Water](#water).

Todos os smart items podem ser configurados para se comportarem de maneiras personalizadas. Por exemplo, até onde uma plataforma se move, ou o que um botão ativa.

## Configurar um item

Selecione um item no Scene Editor para ver todas as suas propriedades à direita.

A maioria dos smart items mantém seu comportamento em um [Script component](/docs/creator/content-creator-pt/scene-editor/expandir-com-codigo/script-component/). As configurações aparecem no **Script** painel, um campo por parâmetro, cada um com uma dica de ferramenta explicando o que faz. Você não precisa escrever nenhum código para usá-los.

Alguns campos típicos que você pode encontrar em muitos itens são:

* **Texto de dica**: Qual texto é exibido na UI como dica quando o jogador passa o cursor sobre o item. Por exemplo, uma porta pode dizer "Open"
* **Interação**: Com qual botão o item é ativado? Em um teclado típico:
  * **Primária** está **E**
  * **Secundária** está **F**
  * **Pointer** está **Botão esquerdo do mouse**
  * **Action3** é a tecla **1**
  * **Action4** é a tecla **2**
  * **Action5** é a tecla **3**
  * **Action6** é a tecla **4**
* **Hooks que reagem ao item**: parâmetros onde você escolhe uma ação para executar em um momento específico. Quais um item possui depende do que ele faz. Um botão tem **onClick**, uma alavanca tem **onActivate** e **onDeactivate**, uma plataforma móvel tem **onReachStart** e **onReachEnd**, um sino tem **onRing**. Deixe um hook vazio e nada acontece naquele momento.

Cada item tem suas próprias configurações específicas, que podem variar de um item para outro.

Alguns itens mantêm o antigo **Config** painel em vez disso. As luzes, por exemplo, ainda definem sua cor e intensidade ali.

Todos os itens têm um **Advanced Mode** que permite configurar quase qualquer coisa sobre eles. Isso inclui coisas como quais sons são reproduzidos, ou em que direção uma plataforma se move. Você também pode adicionar ações personalizadas que incluem todo tipo de coisa, como teletransportar o jogador, reproduzir animações do avatar, anexar um item às mãos do jogador etc. Você também pode adicionar lógica condicional, para ativar algo apenas em certos cenários. Veja [Smart Items - Advanced](/docs/creator/content-creator-pt/scene-editor/interatividade/smart-items-advanced/).

![](/docs/_remote/ff0b8ab0126b0a18e07ec387.webp)

## Chamar uma ação em outro item

Smart items podem disparar ações em outros smart items, para que elas aconteçam toda vez que o item for ativado. Basta selecionar o item que você quer chamar, de uma lista de todos os itens na cena, e então selecionar uma ação. Itens diferentes expõem ações diferentes.

Por exemplo, aqui está um botão que abre ou fecha uma porta. Toda vez que o botão for pressionado, a porta abrirá ou fechará.

![](/docs/_remote/564f4fe0ae016962310472b6.webp)

Aqui está uma alavanca que abre uma porta quando ativada e fecha essa porta quando desativada.

![](/docs/_remote/dc29efe9b4e6df7d2b39a714.webp)

Você pode adicionar quantas ações diferentes de itens diferentes quiser para serem acionadas juntas. Basta clicar **+ Assign Action**.

Remova ações clicando nos três pontos ao lado de uma ação e selecionando *Remover ação*.

As ações que um item expõe mantêm seus nomes familiares, como **Open**, **Close**, **Sit Here**, **Turn On**, ou **Teleport**, então a conexão entre itens funciona da mesma forma que sempre funcionou.

Você também pode encadear ações. Por exemplo, se o **onActivate** abre uma porta, e o próprio hook dessa porta dispara outra coisa, a alavanca aciona toda a cadeia indiretamente.

Se você usar o [Advanced mode](/docs/creator/content-creator-pt/scene-editor/interatividade/smart-items-advanced/) você também pode adicionar lógica condicional a esses tipos de ações.

## Smart items especiais

Alguns smart items têm características únicas que os tornam muito úteis para cenários comuns:

### Áreas de trigger

Use o smart item Trigger Area para disparar uma ação quando o jogador entra em uma área.

![](/docs/_remote/9dcff76b0c7616640d8295ac.webp)

Arraste-o para dentro, redimensione a caixa ou a esfera para cobrir a área desejada e então use os **Quando um jogador entra…** e **Quando um jogador sai…** botões na sua **Reactions** seção para descrever o que deve acontecer. O [AI Assistant](/docs/creator/content-creator-pt/scene-editor/expandir-com-codigo/ai-assistant/) escreve o script para você. Veja [Área de gatilho](/docs/creator/content-creator-pt/scene-editor/interatividade/trigger-area/) para o passo a passo completo.

![](/docs/_remote/83142e91c462787a36ca8b35.webp)

Veja [Área de gatilho](/docs/creator/content-creator-pt/scene-editor/interatividade/trigger-area/) para mais informações.

### Paredes invisíveis

Uma coleção de formas invisíveis que podem bloquear jogadores de atravessar ou clicar através de uma área.

Essas paredes invisíveis podem ser úteis ao importar um modelo 3D que não tenha uma malha de collider, ou quando você quiser criar uma parede que não seja visível para o jogador.

Veja [Colliders](/docs/creator/content-creator-pt/scenes-sdk7/fundamentos-de-conteudo-3d/colliders/) para mais informações.

### Área de clique

Um cubo invisível que pode ser clicado pelos jogadores para disparar ações em qualquer outro smart item. Este item pode ser ativado ou desativado por qualquer outro smart item; quando desativado, ele não poderá ser clicado. Você também pode definir o texto que os jogadores veem quando apontam o cursor para ele.

![](/docs/_remote/43e0c40165045ebe199d3264.webp)

### Assentos

Cadeiras, bancos e outros assentos permitem que um jogador se sente ao clicar neles.

* Um assento só fica livre quando o jogador que o ocupou **se afasta** dele, mais de 1,5 metro.
* Em um item com vários lugares, como um banco, clicar nele faz você se sentar no **lugar livre mais próximo**.
* Quando todos os lugares estão ocupados, o texto da dica diz **O assento está ocupado** e clicar não faz nada.

### Water

Dois itens na categoria **Materiais** oferecem água na qual o jogador pode espirrar:

* **Water Plane**: uma superfície plana de 16 x 16 m. Redimensione-a para cobrir um lago, um canal ou o mar.
* **Water Cylinder**: uma piscina redonda, com 8 m de largura e 2 m de profundidade.

Ambos espirram quando um jogador anda ou pula neles. Você pode definir:

* **Ativado por**: `meu jogador` para que só você veja os próprios respingos, ou `todos os jogadores` para que todo avatar espirre.
* **Profundidade**: quão abaixo da superfície a área de respingo alcança, em metros. Ela nunca vai além do fundo de uma piscina.
* **Tamanho do respingo**: o tamanho do efeito de respingo, em metros.
* **Som**: o nome de um arquivo de áudio dentro da pasta do item, reproduzido a cada respingo. Deixe em branco para água silenciosa.

Eles também expõem **entrar** e **sair** eventos e uma **ação Splash** que faz um respingo no meio da água sob demanda.


> [!WARNING]
> **📔 Nota**: Cada camada de água precisa do seu normal map. Uma camada sem um não fica mais animada e, se não restar nenhuma, a área de respingo encolhe para 1 x 1 m.


### Teleports

O **Teleport** item envia o jogador para outro lugar quando ele interage com ele.

Defina os parâmetros **x** e **y** para enviá-los a um parcel em Genesis City. Para enviá-los a um [World](/docs/creator/content-creator-pt/scenes-sdk7/publicacao/publishing-options/#decentraland-worlds) em vez disso, preencha o **world** parâmetro com o nome do world, como `myname.dcl.eth`, e deixe as coordenadas em branco.


> [!WARNING]
> **📔 Nota**: Antes existia um **World Teleport** item separado. Ele foi removido. Use o **Teleport** item com seu **world** parâmetro em vez disso.


### Configurações de locomoção

O **Locomotion Settings** item altera a velocidade com que os jogadores se movem e a altura que saltam enquanto estão dentro da sua cena. Use-o para fazer uma cena parecer leve, desacelerar os jogadores em um museu ou acelerá-los ao atravessar um mapa grande.

Arraste-o para qualquer lugar da sua cena. Ele aplica seus valores quando a cena começa, e seu cubo de espaço reservado só fica visível no editor, nunca para os jogadores.

Cada campo é um slider, pré-configurado com o valor padrão de Decentraland:

| Campo                 | Padrão  | Faixa  |
| --------------------- | ------- | ------ |
| `walkSpeed`           | 1,5 m/s | 0 a 10 |
| `jogSpeed`            | 8 m/s   | 0 a 30 |
| `runSpeed`            | 10 m/s  | 0 a 50 |
| `jumpHeight`          | 1 m     | 0 a 10 |
| `runJumpHeight`       | 1,5 m   | 0 a 10 |
| `doubleJumpHeight`    | 2 m     | 0 a 15 |
| `glidingSpeed`        | 6 m/s   | 0 a 30 |
| `glidingFallingSpeed` | 1 m/s   | 0 a 10 |
| `hardLandingCooldown` | 0,75 s  | 0 a 5  |

Jogar é a velocidade padrão com que os jogadores se movem. Andar e correr são o que eles obtêm ao segurar as teclas control ou shift.

O item também expõe duas ações que outros smart items podem chamar:

* `apply`: grava os valores atuais dos campos novamente no jogador, por exemplo após uma trigger area ou um botão.
* `restoreDefaults`: limpa seus valores, para que o jogador volte aos padrões de Decentraland.

Veja [Locomotion Settings and Restrictions](/docs/creator/content-creator-pt/scenes-sdk7/interatividade/avatares/locomotion/) para saber em detalhes o que cada configuração faz e como alterar esses valores a partir do código.

### Reproduzindo vídeos

Reproduza vídeos a partir de:

* **Arquivos locais**
* **Stream a partir de uma URL**
* **Transmitir ao vivo de** [**Decentraland Cast**](/docs/creator/content-creator-pt/scene-editor/operar-ao-vivo/live-streaming/#dcl-cast-easy)
* **Transmitir ao vivo de** [**RTMP Software**](/docs/creator/content-creator-pt/scene-editor/operar-ao-vivo/live-streaming/#stream-advanced) **(OBS, XSplit, StreamYard etc.)**


> [!WARNING]
> **📔 Nota**: Reproduzir vídeos exige muito do desempenho, então mantenha baixo o número de vídeos reproduzidos ao mesmo tempo. O engine limita quantos vídeos podem ser reproduzidos simultaneamente com base nas configurações de qualidade de cada jogador (1 no baixo, 5 no médio, 10 no alto) e pausa quaisquer vídeos além desse limite. Veja [Reproduzir vídeos](/docs/creator/content-creator-pt/scene-editor/interatividade/video-screen/#play-videos).


Veja [Reproduzir vídeos](/docs/creator/content-creator-pt/scene-editor/interatividade/video-screen/) para mais informações.

### Reproduzindo streams de áudio

Reproduza um stream de áudio a partir de uma URL, usando o **Audio Stream** smart item.


> [!NOTE]
> **📔 Nota**: Nem todos os serviços de streaming permitem que você reproduza o áudio deles fora do próprio site. A seguir estão alguns exemplos que funcionam em Decentraland:
> 
> ```ts
> GRAFFITI = 'https://n07.radiojar.com/2qm1fc5kb.m4a?1617129761=&rj-tok=AAABeIR7VqwAilDFeUM39SDjmw&rj-ttl=5'
> SIGNS = 'https://edge.singsingmusic.net/MC2.mp3'
> DELTA = 'https://cdn.instream.audio/:9069/stream?_=171cd6c2b6e'
> JAZZ = 'https://live.vegascity.fm/radio/8010/the_flamingos.mp3'
> ```
> 


Você pode ajustar o volume do seu stream. Observe que o áudio do stream não é posicional; ele é ouvido com um volume uniforme em toda a sua cena.

### Exibindo NFTs

Para exibir um NFT em uma moldura, use o **NFT** smart item. Você deve fornecer os seguintes campos:

* Rede


> [!NOTE]
> **📔 Nota**: Atualmente **ethereum** é a única rede suportada.


* Contrato da coleção NFT: o smart contract da coleção de NFT.
* ID do token: o ID do token deste NFT colecionável específico.

<img src="/docs/_remote/9bae5521b60927140f632d59.webp" alt="forma de NFT" width="400">

Você pode obter essas informações em [OpenSea](https://opensea.io), verificando a **Detalhes** aba abaixo da imagem do NFT.

<img src="/docs/_remote/2ef80788a2ffef0f4c7b360b.webp" alt="Detalhes do OpenSea" width="400">


> [!NOTE]
> **📔 Nota**: Você também pode obter essas informações a partir da URL do opensea. Por exemplo, se a URL do NFT for a seguinte:
> 
> > `https://opensea.io/assets/ethereum/0x32b7495895264ac9d0b12d32afd435453458b1c6/1956`
> 
> Você pode completar os campos com o seguinte:
> 
> * Rede: ethereum
> * Contrato: 0x32b7495895264ac9d0b12d32afd435453458b1c6
> * Token: 1956


Você também pode configurar uma cor de fundo, o que é particularmente útil para NFTs com fundo transparente.

Você também pode escolher um **estilo de moldura**, para enquadrar o NFT em uma variedade de estilos diferentes, clássicos e modernos.

Veja [NFTs](/docs/creator/content-creator-pt/scene-editor/construir/nfts/) para mais detalhes.

### Barras de saúde

![](/docs/_remote/4b7b9bba4ad2a27559774acf.webp)

O **Health Bar** smart item é um ótimo bloco de construção para vários mecanismos de jogo. Ele pode ser usado de várias formas:

* Aninhe-o sob o **Player** para exibir a saúde do jogador sobre o avatar

  ![](/docs/_remote/7ec4155f634538c1243f6333.webp)
* Aninhe-o sob o **Camera** para exibi-lo fixo na UI

  ![](/docs/_remote/8f666b7f68658ee843a4304c.webp)
* Aninhe-o sob literalmente qualquer item na cena para acompanhar a saúde desse item

  ![](/docs/_remote/1bc82b628c58c582bc684876.webp)

Outros itens podem interagir com a barra de saúde para adicionar ou subtrair saúde dela.

* Itens como o **Spikes** ou **Robot Enemy** podem reduzir a saúde

  ![](/docs/_remote/8a3aa562c2ae8a2e3b67d7c6.webp)
* itens como **First Aid** ou no **Healing Pad** podem restaurá-la.

  ![](/docs/_remote/4b71eac34757d0b4ca625eca.webp)

Você deve configurar a Health Bar para definir o que acontecerá quando a saúde for igual a 0. Você pode reaparecer o jogador na posição de um **Respawn Pad** smart item, redefinir o contador da pontuação dele, reaparecer quaisquer inimigos, exibir um texto na UI ou o que fizer sentido na lógica do seu jogo.

Você também pode disparar ações quando a saúde for menor que um certo valor; por exemplo, tocar uma música especial ou mostrar uma dica na UI quando a saúde for menor que 3.

As barras de saúde podem ser configuradas para afetar qualquer coisa! Por exemplo, adicione uma barra de saúde aninhada sob o **Wooden Door** smart item. Essa barra pode ter sua saúde reduzida pelo jogador usando o **Sword** smart item, mas também por uma explosão do **Barrel** ou pelo ataque do **Robot Enemy**. Para que isso funcione, configure a barra de saúde para que ela execute uma ação no item pai quando seu valor for 0.

![](/docs/_remote/03f177d0727bf4a6bb82243f.webp)

Armas como o **Sword** podem ser coletadas pelo jogador e depois usadas para causar dano em qualquer outro item com uma barra de saúde que esteja perto do jogador ao executar a ação.

## Multiplayer

Quase todos os smart items têm comportamento multiplayer, de modo que todos os jogadores na cena compartilhem a mesma experiência conforme os itens mudam de estado. Se o jogador A abre uma porta, o jogador B também vê essa porta aberta. Se o jogador C então entra na cena enquanto os outros jogadores ainda estão lá, ela também verá a porta já aberta.

No entanto, se não houver jogadores perto da cena, ela é restaurada ao seu estado padrão. Então, se todos os jogadores saírem, mas depois o jogador A voltar, ela encontrará a porta fechada (se esse era o estado padrão da porta).

Certifique-se de projetar sua cena de modo que as ações de um jogador não sabotem a cena para outros que venham depois. Por exemplo, se a cena for um jogo de quebra-cabeça, você pode usar uma *atraso* ação em um *tools* smart item para fazer com que todos os itens na cena sejam redefinidos ao seu estado inicial alguns segundos depois de o quebra-cabeça ser resolvido.

Você também pode desativar o comportamento multiplayer de um item; veja [Smart Items - Advanced](/docs/creator/content-creator-pt/scene-editor/interatividade/smart-items-advanced/).

## Solução de problemas

* *Um item na minha cena deveria ser clicável, mas não pode ser clicado*.

Certifique-se de que ele não esteja sendo obstruído por outra coisa. Você não pode clicar através de outros itens. Alguns itens têm uma *collider mesh* que tem uma geometria simplificada que pode estar obstruindo seu item, embora sua forma visível não pareça estar fazendo isso. Tente mover o item para ver o que acontece.

## Veja também

* [Smart items - Avançado](/docs/creator/content-creator-pt/scene-editor/interatividade/smart-items-advanced/)
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
GET https://docs.decentraland.org/creator/content-creator-pt/scene-editor/interatividade/smart-items.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
