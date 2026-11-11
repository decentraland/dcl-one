> Transmita vídeo ao vivo para sua scene usando a Video Screen e as Admin Tools.

# Streaming ao Vivo

Use o seletor **Tela de vídeo** smart item junto com o **Ferramentas de admin** smart item para transmitir vídeo ao vivo para a sua scene.

Decentraland oferece diferentes maneiras de transmitir vídeo ao vivo para a sua scene:

* **DCL Cast** *(Modo Fácil)*: Use o app web gratuito de streaming da Decentraland para compartilhar facilmente sua câmera ou tela com outros players na scene, sem precisar configurar um software de streaming. Esse modo tem a menor latência e é o mais fácil de configurar.
* **Stream** *(Modo Avançado)*: Use um software de streaming como [OBS](https://obsproject.com/) para transmitir pela infraestrutura de streaming da Decentraland. Esse modo permite que você tenha mais controle sobre a transmissão, como o layout da tela e as fontes de áudio.
* **URL do vídeo** *(Modo Avançado)*: Aponte para sua própria infraestrutura de streaming colando a URL no **URL do vídeo** campo.

<img src="/docs/_remote/9fe1dc304a4f62e468f108b3.webp" alt="Métodos de streaming" width="400">

O streaming funciona em Worlds e Genesis City, sem limites de público do lado da scene.

## Configurar a scene

As etapas a seguir são comuns tanto ao DCL Cast quanto aos métodos de Stream:

1. Adicione um **Tela de vídeo** smart item para a sua scene.

   ![](/docs/_remote/bb8266cdd73eaeb440e3fe4d.webp)
2. Adicione um **Ferramentas de admin** smart item e habilite a seção **Video Screens** . Selecione cada tela no menu suspenso e dê a ela um nome amigável para a UI de admin.

   ![](/docs/_remote/34ed9d039629babf7b485b20.webp)
3. Publique sua scene (World ou Genesis City) e entre como um usuário com permissões de admin.

   ![](/docs/_remote/522b72b559e9bf32de19bcab.webp)

Depois que sua scene for publicada, você pode entrar como um usuário com permissões de admin e configurar as definições de streaming.


> [!NOTE]
> **💡 Dica**: Se você adicionar várias Video Screens para mostrar o mesmo vídeo, configure a fonte de todas, exceto uma, para apontar para o mesmo video player; veja [Multiple Video Screens](/docs/creator/content-creator-pt/scene-editor/interatividade/video-screen/#multiple-video-screens) para mais detalhes.


## DCL Cast (fácil)

### Compartilhar acesso ao app

Entre na sua scene publicada como um usuário admin e abra o painel de admin. Selecione a aba **Video** , depois selecione a funcionalidade **DCL Cast** .

<img src="/docs/_remote/0d1340c42e272abcc25bf603.webp" alt="DCL Cast" width="400">

Você verá dois links que pode copiar e compartilhar com outras pessoas.

* **Cast Speakers**: Este link é para os speakers usarem para transmitir o vídeo deles para a scene.

  <div data-gb-custom-block data-tag="hint" data-style="danger" class="hint hint-danger"><p><strong>❗Aviso</strong>: Trate o link de streaming como um segredo; compartilhe-o apenas com pessoas em quem você confia. Redefina o link entre apresentadores, se necessário.</p><p>Quando terminar de transmitir, feche a aba do navegador do DCL Cast para liberar o canal.</p></div>
* **Viewers**: Este link é para o público assistir ao vídeo a partir de um navegador ou celular. Isso é útil para players que não estejam atualmente dentro da Decentraland.

Clica no **Copiar link** botão para copiar os links para a área de transferência.

Quando estiver pronto para transmitir, clique no **Activate** botão para tornar o stream visível ao público na scene.

<img src="/docs/_remote/6fa0aca94ae7997a1988bcad.webp" alt="Ativar stream" width="150">

Se, por qualquer motivo, você precisar redefinir a sala, clique no **Reset Room** botão para gerar uma nova. Qualquer pessoa que estiver transmitindo no momento será desconectada.

<img src="/docs/_remote/5daf35dba544baf2c6504619.webp" alt="Redefinir sala" width="150">

### Usando o app DCL Cast

Quando alguém colar o link do speaker em um navegador, verá uma tela assim:

<img src="/docs/_remote/8b74b9e4f571aec33f619bc5.webp" alt="app DCL Cast" width="400">

O navegador solicitará permissão para compartilhar sua câmera e microfone. Você também pode configurar os diferentes dispositivos de entrada a serem usados para o stream.


> [!NOTE]
> **📔 Dica**: Use o Google Chrome ou um navegador baseado no mecanismo Chrome. Esses navegadores oferecem a funcionalidade de compartilhar facilmente vídeo e áudio diretamente de uma aba do navegador.


Os usuários podem inserir um nome (não precisa corresponder ao nome de usuário da Decentraland) e clicar no botão **Join Now** para começar a transmitir.

Depois de começar a transmitir, o app é semelhante a vários apps de videoconferência conhecidos, com botões para silenciar/desmutar, compartilhar câmera e tela, e uma interface de chat.

O chat é somente leitura e escuta todas as mensagens enviadas pelos players dentro da scene na Decentraland. Isso é ótimo para manter contato com o público, mesmo que você esteja transmitindo de um dispositivo diferente.

<img src="/docs/_remote/66b24803959f06d37231b10c.webp" alt="app DCL Cast" width="700">

Na **Na aba** você pode ver três listas:

* **Speakers**: As pessoas que estão transmitindo atualmente para a scene.
* **Viewers**: As pessoas que estão assistindo ao stream atualmente de um navegador.
* **In-world participants**: Os players que estão atualmente dentro da scene, assistindo ao stream in-world.

<img src="/docs/_remote/1ff69ac3627fa7cbbad58fca.webp" alt="aba Participants" width="200">

Se vários speakers estiverem presentes em uma sessão de DCL Cast, os players in-world ouvirão as vozes de todos os speakers, e a visualização do vídeo mudará automaticamente para mostrar quem estiver emitindo som no momento, para sempre mostrar quem está falando.

Para substituir esse comportamento padrão:

* Clica no **Speakers** botão no painel de admin da scene

<img src="/docs/_remote/2e5e3dd8ae5ce493e7c7cc13.webp" alt="aba Participants" width="200">

* Escolha um dos speakers e selecione uma fonte a ser exibida (a câmera ou a tela desse speaker)

<img src="/docs/_remote/7e4c6bd675830ab1e698bcfc.webp" alt="aba Participants" width="200">

Isso forçará essa fonte a ser sempre mostrada na tela, independentemente de outros speakers estarem falando.


> [!NOTE]
> **📔 Dica**: Se você também estiver in-world assistindo ao stream, pode achar desagradável ouvir eco do áudio repetido tanto no app DCL Cast quanto na scene da Decentraland. Você pode facilmente silenciar todo o áudio do app DCL Cast alternando o ícone do speaker no canto inferior esquerdo da tela
> 
> <img src="/docs/_remote/f1c102f46a10285862b60d5a.webp" alt="Participants tab" data-size="original">
> 
> Caso contrário, você pode silenciar o áudio nas configurações da Decentraland.


### Compartilhar apresentações

Você também pode compartilhar o conteúdo de uma apresentação de slides como uma fonte alternativa de imagens.

* No in-world, clique no botão **Share presentation** no in-world na aba DCL Cast. No app DCL Cast, clique no menu suspenso ao lado do botão **Share Screen** e selecione **Share presentation**.

<img src="/docs/_remote/021196a0a453b918bd70cad9.webp" alt="aba Participants" width="200">

* Cole um link do Google Slides, um link para um .pdf hospedado no Drive ou fonte similar, ou carregue um arquivo .pdf.

A apresentação agora será uma fonte que pode ser selecionada para ser exibida na tela, enquanto as vozes de todos os speakers ainda são ouvidas.


> [!WARNING]
> **📔 Nota**: Os arquivos de apresentação devem ter menos de 100 MB. As apresentações do Google Slides devem ser definidas como *públicas*.
> 
> Só pode haver uma apresentação ativa por vez em uma sessão de DCL Cast.


Depois disso, você pode trocar os slides ou até reproduzir e pausar quaisquer vídeos incorporados nesses slides pressionando botões que existem tanto no app DCL Cast quanto in-world na UI do Admin Tools.

## Stream (avançado)

Para usar o recurso de Live Streaming na sua scene, você precisará instalar um software de streaming que possa enviar para um endpoint RTMP (por exemplo, [OBS](https://obsproject.com/), [XSplit](https://www.xsplit.com/), [StreamYard](https://streamyard.com/)).


> [!WARNING]
> **Aviso**: O live streaming não é suportado em worlds Single Player. Se o seu world tiver **Single Player** habilitado em World Settings (ou `fixedAdapter` definido como `"offline:offline"` em `scene.json`), desabilite-o antes de configurar o streaming. O recurso de streaming depende da camada de comunicações, que é desativada no modo Single Player.


### Obter credenciais de streaming

1. Abra a UI de admin na scene (ícone no canto superior direito).

   ![](/docs/_remote/3a78785f1b44cd50b0963098.webp)
2. No **Video** aba, mude para **Stream** e clique em **Get Stream Key**.

   ![](/docs/_remote/548a347a2432d40b40d20705.webp)
3. Copie o **RTMP Server** e **Stream Key** para o seu software de streaming.

   ![OBS configuration](/docs/_remote/683eec84be8a03a78e4ed799.webp)


> [!CAUTION]
> **❗Aviso**: Apenas uma pessoa pode transmitir para uma scene por vez. Quando terminar de transmitir, clique em **Stop Streaming** no seu software para liberar o canal.


### Iniciar e controlar o stream

1. Comece a transmitir pelo seu software.
2. Na UI de admin, clique em **Activate** para mostrar o stream na scene.

   <img src="/docs/_remote/6fa0aca94ae7997a1988bcad.webp" alt="Ativar stream" width="100">

### Chaves de streaming

As chaves de streaming são geradas por scene e são válidas por 4 dias (96 horas). Uma única sessão ao vivo pode durar até 4 horas continuamente.

![](/docs/_remote/8fdba6b9787070f6aacf6827.webp)

* Clique em **Reset Stream Key** para revogar a chave atual e emitir uma nova. Streams em andamento serão interrompidos.
* Cada scene tem seu próprio endereço e chave de streaming. Os admins podem compartilhar a chave com streamers externos.
* Só pode haver um stream ativo por scene por vez; iniciar um novo substituirá o stream atual.


> [!CAUTION]
> **❗Aviso**: Trate as stream keys como segredos. Redefina a chave entre apresentadores, se necessário.


## Streaming a partir de URL (avançado)

Você também pode transmitir configurando o Video Screen para usar a opção **URL do vídeo** e colando uma URL de stream.

Você deve conseguir colar uma URL apontando para um vídeo da maioria dos sites de streaming de vídeo populares. Tenha atenção aos termos de serviço dessas plataformas.

Para transmitir a partir de um arquivo de vídeo que você tem na sua máquina local, o caminho mais fácil é enviar esse vídeo para um Google Drive público e colar o link.

* A URL deve ser `https`. Veja [Sobre Streaming Externo](/docs/creator/content-creator-pt/scenes-sdk7/media/video-playing/#about-external-streaming).
* Os provedores recomendados incluem [Vimeo](https://vimeo.com/), [Bunny](https://bunny.net), [Livepeer Studio](https://livepeer.studio/) e [Serraform](https://serraform.gitbook.io/streaming-docs/guides/decentraland-playback).
* Dicas para a configuração do encoder: [Configurando o OBS para um streaming bem-sucedido](/docs/creator/content-creator-pt/scenes-sdk7/media/video-playing/#setting-up-obs-for-successful-streaming).


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-pt/scene-editor/operar-ao-vivo/live-streaming.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
