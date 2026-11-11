> Os administradores da scene têm controle especial sobre o que acontece na scene em tempo real.

# Administração da Scene

Conceda a alguns jogadores a função especial de **admin** na sua scene.

Durante um evento ao vivo, um admin pode controlar espontaneamente o que acontece na scene diretamente dentro da Decentraland, sem precisar agendar ações com antecedência ou depender de um serviço de terceiros. Comece a tocar a música quando já houver uma multidão suficiente, solte confetes ou faça uma nave espacial aparecer quando o momento parecer certo.



<div class="video-embed"><button type="button" class="video-play" data-video-src="https://www.youtube-nocookie.com/embed/efjJN7Jr7Qo" aria-label="Play YouTube video"><svg viewBox="0 0 64 64" width="64" height="64" aria-hidden="true"><circle cx="32" cy="32" r="30" fill="currentColor"/><path d="m26 20 20 12-20 12Z" fill="#181214"/></svg><span>Play video</span></button></div>
<p class="video-source"><a href="&lt;https://www.youtube.com/watch?v=efjJN7Jr7Qo>">Watch on YouTube</a></p>



Quando um admin da scene visita a sua scene, vê um painel de admin especial no canto superior direito que só ele consegue ver. O painel usa icon tabs no topo para alternar entre secções (Video, Announcements, Smart Items, Permissions). Através desta UI, pode reproduzir videos ou live streams, enviar announcements, banir players ou ativar qualquer smart item que esteja configurado para ser ativado dessa forma. Estas ações são vistas por todos os outros players na scene que estejam ligados à mesma comms island que o admin.

![](/docs/_remote/4a423dfb13080da0e3f0dd27.webp)

## Configurando admins

Para atribuir admins, precisa adicionar o **Admin Tools** smart item à sua scene.

![](/docs/_remote/360f0190e175f35ce4c17c72.webp)


> [!WARNING]
> **📔 Nota**: Atualize a sua scene para usar as dependências mais recentes.
> 
> <img src="/docs/_remote/83f4693cc61cdbab26246a7d.webp" alt="" data-size="original">


Enquanto estiver a desenvolver a scene e a testá-la localmente, é sempre um admin. Quando a scene é publicada, qualquer pessoa com permissões de publicação para a scene também é automaticamente um admin. Isto inclui:

* O proprietário dos parcels LAND ou do World NAME onde a scene é publicada
* Qualquer pessoa a quem sejam concedidos **Operator rights** nestes parcels ou name. Consulte [Conceder permissões](/docs/player/marketplace/land-manager/#give-permissions).
* Qualquer utilizador a alugar essa LAND. Consulte [Arrendamentos](/docs/player/marketplace/rentals/).

Para atribuir admins adicionais que não têm permissão de publicação, mas podem fazer live ops na scene:

1. Publique a scene e visite a versão em direto como admin
2. Abra o **Permissions** tab (através das icon tabs no topo do admin panel).

   ![](/docs/_remote/ed908a8f1e3f969689af965f.webp)
3. Escreva o endereço da wallet da pessoa que pretende adicionar ao lado de **Adicionar um admin** e clique em **Adicionar**.

Pode ver quem é admin na scene clicando no **Ver lista de admins** button. A partir deste ecrã também pode **Remover** pessoas da lista de admins.

![](/docs/_remote/983d682c49d98b1ce3fb7227.webp)


> [!WARNING]
> **📔 Nota**: Só é possível remover a função de admin de players que foram adicionados manualmente à lista através do **Permissions & Moderation** tab. Players que são owners, operators ou renters da scene são apresentados nesta lista, mas não podem ser removidos das suas funções de admin através desta UI. Para remover a função de admin de um operator, primeiro tem de remover a sua função de operator.


Sempre que um player admin estiver na scene, verá uma UI especial no canto superior direito. Players que não são admin não veem esta UI.

![](/docs/_remote/4a423dfb13080da0e3f0dd27.webp)

### Verificar o estado de admin via código

Também é possível saber se um Player é um Admin de uma scene usando código. Isto permite disponibilizar outros comportamentos (ou não) para um Player, por exemplo, permitindo interações com um Entity específico da scene.

```ts
import { isAdmin } from "@dcl/asset-packs/dist/admin";

async function onPlayerSpawn() {
  const isAdminUser = await isAdmin();
  if (isAdminUser) {
    // Mostrar UI apenas para admin, teleportar para o palco, mostrar Entity, etc.
  }
}
```


> [!NOTE]
> **💡 Dica**: Para mais informações sobre async functions, consulte [Funções assíncronas](/docs/creator/content-creator-pt/scenes-sdk7/padroes-de-programacao/async-functions/).


## Reprodução de vídeo

Uma das ações mais comuns que os admins fazem é reproduzir vídeos. O admin panel inclui uma secção de Video Player onde podem controlar tudo o que estiver relacionado com vídeos.

Para ativar isto, precisa adicionar um **Video Player** smart item à sua scene e ligá-lo ao Admin Tools smart item.

1. Adicione um **Video Player** smart item à sua scene

   ![](/docs/_remote/bb8266cdd73eaeb440e3fe4d.webp)

   Veja [Video Screen](/docs/creator/content-creator-pt/scene-editor/interatividade/video-screen/) para mais detalhes sobre como pode configurar a media source predefinida e outras definições do smart item Video Player. A maioria destas configurações pode ser substituída pelo admin assim que a scene estiver a correr.

   <div data-gb-custom-block data-tag="hint" data-style="warning" class="hint hint-warning"><p><strong>📔 Nota</strong>: Um admin só pode gerir vídeos que tocam no smart item Video Screen, não em screens adicionados via código SDK.</p><p>Pode incluir tantas video screens quantas quiser. Em geral, evite ter mais do que um vídeo diferente a tocar ao mesmo tempo, pois isso prejudica muito o desempenho.</p></div>
2. Abra o Admin Tools Smart Item e certifique-se de que a **Video Screens** checkbox está ativada para que esta secção seja mostrada. Depois selecione o screen numa dropdown list e atribua-lhe um nome amigável para apresentar na Admin UI. Pode adicionar tantas Video Screens quantas quiser; cada screen é controlado de forma independente.

   <img src="/docs/_remote/34ed9d039629babf7b485b20.webp" alt="Nome da scene" width="300">

Depois de o acima estar configurado, os utilizadores admin na sua scene podem abrir o admin panel e selecionar o tab de vídeo (através das icon tabs no topo) para controlar estes video screens.

<img src="/docs/_remote/48468cefefacba6545b47b45.webp" alt="Nome da scene" width="300">

Se a sua scene tiver múltiplos video screens independentes, o **screen** dropdown permite escolher qual video screen controlar. A lista mostra os nomes que atribuiu a cada video screen na configuração do smart item Admin Tools.


> [!NOTE]
> **💡 Dica**: Para mostrar o mesmo vídeo em múltiplos screens que podem ser controlados como um só, consulte [Multiple Video Screens](/docs/creator/content-creator-pt/scene-editor/interatividade/video-screen/#multiple-video-screens).


## Media Sources

Um controlo segmentado no topo da secção de vídeo permite alternar entre três media sources. A fonte atualmente ativa é indicada por uma etiqueta em forma de cápsula. Mudar de tab mostra apenas as definições dessa fonte; isso não interrompe o que está a tocar até clicar em **Activate**.

* **Video URL**: Reproduza um vídeo a partir de um URL. Cole um URL de vídeo no campo e clique em **Activate**. O vídeo começará a tocar no screen selecionado para todos os players. Também pode parar, pausar, reiniciar e ajustar o volume.

  <img src="/docs/_remote/77179ebe47d00636f626ed06.webp" alt="Nome da scene" width="300">

  <div data-gb-custom-block data-tag="hint" data-style="warning" class="hint hint-warning"><p><strong>📔 Nota</strong>: Nem qualquer URL de vídeo funciona. Vídeos de alguns sites têm políticas rigorosas sobre o seu conteúdo e bloquearão o acesso a partir da Decentraland. Consulte <a href="/docs/creator/content-creator-pt/scene-editor/interatividade/video-screen/#streaming-from-other-sources">Streaming from other sources</a> para mais informações sobre o que pode e não pode reproduzir na Decentraland.</p></div>
* **DCL Cast**: Use a aplicação web gratuita de streaming da Decentraland para partilhar facilmente a sua câmara ou ecrã com outros players na scene, sem necessidade de configurar software de streaming. Os controlos de apresentação (Previous, Next, Play video, Stop) são mostrados inline quando uma presentation está ativa.

  <img src="/docs/_remote/0d1340c42e272abcc25bf603.webp" alt="DCL Cast" width="300">
* **Stream**: Reproduza um live stream usando a infraestrutura gratuita de streaming da Decentraland e um software de streaming como o OBS ou o StreamYard.

  <img src="/docs/_remote/79d537ec46382a7153d0fb2f.webp" alt="Nome da scene" width="300">

  Veja [Live Streaming](/docs/creator/content-creator-pt/scene-editor/operar-ao-vivo/live-streaming/) para mais informações sobre como configurar um live stream.

Cada secção de media source inclui um **slider de volume** que permite definir o volume de 0% (silenciado) a 100%.

![](/docs/_remote/6fa0aca94ae7997a1988bcad.webp)

## Announcements

No **Announcements** tab do admin panel, os admins podem escrever mensagens que são vistas por todos os players na scene. Mensagens assim só podem ser enviadas por admins, por isso os outros players irão considerá-las mais legítimas do que uma mensagem no chat de alguém que alega ser admin.

Selecione o tab Announcements do admin panel. Escreva uma mensagem e clique em **Share**. A mensagem pode ter até 90 caracteres. O campo de entrada é limpo automaticamente após o envio, para que possa escrever uma nova mensagem de imediato.

![](/docs/_remote/a7a34d2c60e405a1f61bb7f6.webp)

## Ban players

Pode banir players da sua scene selecionando o **Permissions** tab do admin panel, escrevendo o nome ou o endereço da wallet do player que pretende banir e clicando no **Ban** button.

![](/docs/_remote/ed908a8f1e3f969689af965f.webp)


> [!NOTE]
> **💡 Dica**: Para obter o endereço da wallet de um player, clique no seu avatar para abrir o perfil, depois clique no **Para copiar uma string para a área de transferência do jogador, use** button ao lado do endereço da wallet.


Players banidos não conseguirão carregar a sua scene nem interagir com qualquer um dos seus conteúdos. Os outros players não os verão na scene, nem lerão nenhuma das suas mensagens de chat.


> [!WARNING]
> **📔 Nota**: Os efeitos do seu ban são imediatos e permanentes. Depois de um player ser banido, permanecerá banido até que o ban seja levantado. Enquanto estiver banido, um player não pode ver ou interagir com a sua scene, não pode conversar no canal Nearby enquanto estiver nela, e os outros players na scene não o podem ver.
> 
> Se um player sair dos limites da sua scene, deixa de ser afetado pelas regras de ban da sua scene e voltará a ver players banidos.


Clique em **View Ban List** para ver a lista de players atualmente banidos. A partir desta lista também pode **Unban** players.

## Trigger smart items

Para Trigger uma ação de qualquer smart item na scene:

* Adicione um smart item à sua scene
* Abra as definições do **Admin Tools** Smart Item no Creator Hub
* No **secção de ações do smart item** adiciona o smart item a partir do dropdown, atribua-lhe um nome personalizado e selecione uma ação predefinida

Depois de o acima estar configurado, os admins podem acionar a ação abrindo a **Smart Items** tab do admin panel e selecionando um item da dropdown list. Depois podem escolher uma das ações do item no **Actions** dropdown e clicar em **Play Action** para a acionar. Também existe um button para **Hide/Show** o item selecionado.

<img src="/docs/_remote/078360766f2efe8ad3431ea0.webp" alt="Nome da scene" width="300">

Também pode mostrar ou ocultar qualquer smart item desta lista, mesmo que não inclua uma action para fazer isso.


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-pt/scene-editor/operar-ao-vivo/scene-admin.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
