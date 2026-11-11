> Reproduza vídeos na sua scene

# Tela de Vídeo

Para reproduzir vídeos pré-gravados ou transmitidos em streaming em uma tela na sua cena, use o Video Player [Smart Item](/docs/creator/content-creator-pt/scene-editor/interatividade/smart-items/).

![](/docs/_remote/bb8266cdd73eaeb440e3fe4d.webp)

## Configurações gerais

Essas configurações são relevantes para todos os cenários, seja se você estiver reproduzindo vídeos ou fazendo streaming.

![](/docs/_remote/6418389e00c134e192913d0e.webp)

Você pode configurar o volume do áudio do vídeo. Observe que o áudio da transmissão não é posicional; ele é ouvido com volume uniforme em toda a sua cena.

A **Fontes de mídia padrão** o menu suspenso permite escolher entre dois tipos diferentes de fontes:

* **URL do vídeo**: Buscar um vídeo ou uma transmissão de uma URL ou de um arquivo de vídeo local
* **Transmissão ao vivo**: Use a infraestrutura de streaming gratuita da Decentraland para exibir uma transmissão. Para usar isso, você também deve incluir um [ferramentas de admin](/docs/creator/content-creator-pt/scene-editor/operar-ao-vivo/scene-admin/) smart item na sua cena.

## Reproduzir vídeos

Você pode reproduzir vídeos pré-gravados a partir de:

* **Arquivos locais**: Envie um arquivo de vídeo como parte da cena e, em seguida, aponte o *URL* campo para o caminho desse arquivo.
* **Transmitir a partir de uma URL**: Aponte para uma transmissão ao vivo ou pré-gravada na web, por exemplo, de um provedor como Bunny. Veja [vídeos em streaming](#streaming-from-other-sources)

O momento em que o smart item Video Player reproduz um vídeo pode depender de diferentes coisas:

* **Automático**: O vídeo começa a ser reproduzido assim que a cena carrega. Para isso, defina o menu suspenso de fonte de mídia padrão como **URL do vídeo** e cole uma URL diretamente no **Caminho do vídeo ou URL .m3u8** .

  ![](/docs/_remote/6418389e00c134e192913d0e.webp)
* **Acionado por um admin**: Um [admin da cena](/docs/creator/content-creator-pt/scene-editor/operar-ao-vivo/scene-admin/) que esteja atualmente na cena pode usar a Admin UI para colar uma URL de vídeo e reproduzi-la para todos os jogadores que estiverem atualmente na cena.
* **Com base nas ações do jogador**: Defina uma Action do tipo **Reproduzir vídeo**. Isso permite que você acione a reprodução do vídeo como resultado da interação com algum outro smart item, como entrar em uma sala ou apertar um botão. Veja [Smart Items - Advanced](/docs/creator/content-creator-pt/scene-editor/interatividade/smart-items-advanced/).

  ![](/docs/_remote/87a1916dbb31dbea1502aad6.webp)

Em todos os casos, você configura o vídeo para repetir em loop ou ser reproduzido uma vez.


> [!WARNING]
> **📔 Nota**: Se muitos vídeos estiverem sendo reproduzidos ao mesmo tempo na sua cena, alguns serão pausados pelo engine. A prioridade é determinada com base na proximidade ao jogador, na direção da câmera e no tamanho da tela. A quantidade máxima de vídeos simultâneos depende das configurações de qualidade do jogador.
> 
> * Baixa: 1
> * Média: 5
> * Alta: 10
> 
> Também recomendamos começar a reproduzir o vídeo quando o jogador estiver perto ou executar uma ação para isso. Começar a reproduzir um vídeo quando sua cena é carregada muito ao longe no horizonte afetará desnecessariamente o desempenho enquanto os jogadores visitam as cenas vizinhas.


## Múltiplas telas de vídeo

Você pode reproduzir o mesmo vídeo em várias telas ao mesmo tempo. Para fazer isso, você deve editar as propriedades avançadas do smart item Video Player.


> [!WARNING]
> **📔 Nota**: Evite ter mais de um vídeo diferente sendo reproduzido ao mesmo tempo, pois isso prejudica muito o desempenho.
> 
> Se você simplesmente colar a mesma URL em dois video players, o engine não saberá que são o mesmo vídeo e reproduzirá os dois separadamente. Siga as etapas abaixo para configurar o segundo video player para reproduzir o mesmo vídeo que o primeiro.


1. Adicione dois smart items Video Player à cena, um para cada tela.
2. Configure o primeiro normalmente, como descrito em [Reproduzir vídeos](#play-videos) seção.
3. No segundo video player, remova o **Video Player** componente.

![](/docs/_remote/a77f00900280e771283a1861.webp)


> [!WARNING]
> **📔 Nota**: Esta etapa é importante; caso contrário, o segundo video player será processado pelo engine, mesmo que não esteja visível.


4. Ainda no segundo video player, abra o **Material** componente, expanda a seção **Texture** e selecione o dropdown **Video Source Entity** para apontar para o primeiro video player.

![](/docs/_remote/ac9747acfdb69510f8b3a247.webp)

Você pode fazer o mesmo para qualquer número de video players, desde que configure cada um para apontar para o mesmo video player.

Ao fazer [live streaming](/docs/creator/content-creator-pt/scene-editor/operar-ao-vivo/live-streaming/), ambas as telas também exibirão a mesma transmissão.


> [!NOTE]
> **💡 Dica**: As etapas acima também podem ser repetidas com um item que tenha um **Swap Material** componente, para transformar qualquer modelo 3D em uma tela de vídeo. Configure a seção **Texture** dentro do **Swap Material** componente para apontar para a entidade do video player.


## Sobre arquivos de vídeo

Os seguintes formatos de arquivo são suportados:

* *.mp4*
* *.ogg*
* *.webm*

Tenha em mente que um arquivo de vídeo aumenta o tamanho total da cena, o que faz com que a cena demore mais para ser baixada pelos jogadores que entrarem nela. O tamanho do vídeo também pode fazer com que você ultrapasse as [limitações da cena](/docs/creator/content-creator-pt/scenes-sdk7/otimizacao/scene-limitations/), já que você tem um máximo de 15 MB por parcel para usar. Recomendamos comprimir o vídeo o máximo possível, para que isso seja menos problemático.

## Streaming ao vivo

Para streaming ao vivo de ponta a ponta, veja [Streaming ao vivo](/docs/creator/content-creator-pt/scene-editor/operar-ao-vivo/live-streaming/).

### Streaming de outras fontes

Você também pode transmitir vídeos usando outras infraestruturas de streaming. Para fazer isso, basta configurar o smart item Video Player para usar a **URL do vídeo** fonte de mídia, e cole a URL da transmissão no **Caminho do vídeo ou URL .m3u8** .

A origem do streaming deve ser uma *https* URL (*http* URLs não são suportadas), e a origem deve ter [políticas CORS (Cross Origin Resource Sharing)](https://en.wikipedia.org/wiki/Cross-origin_resource_sharing) que permitam o acesso externo a ela. Isso significa que você não pode transmitir um vídeo do YouTube ou de sites semelhantes, pois eles só permitem exibir o conteúdo em seu widget HTML com a marca deles. Veja [Sobre Streaming Externo](/docs/creator/content-creator-pt/scenes-sdk7/media/video-playing/#about-external-streaming) para opções e dicas.

Há várias opções para fazer streaming de vídeo. A opção mais simples é usar um provedor de hospedagem gerenciada como [Vimeo](https://vimeo.com/), [Bunny](https://bunny.net), [Livepeer Studio](https://livepeer.studio/) ou [Serraform](https://serraform.gitbook.io/streaming-docs/guides/decentraland-playback) onde você paga uma taxa ao provedor para gerenciar toda a infraestrutura de streaming.

Leia [Configurando o OBS para um streaming bem-sucedido](/docs/creator/content-creator-pt/scenes-sdk7/media/video-playing/#setting-up-obs-for-successful-streaming) para dicas de como transmitir melhor conteúdo para a Decentraland.


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-pt/scene-editor/interatividade/video-screen.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
