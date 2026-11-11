> O Scene Editor é uma ferramenta visual simples que permite criar e publicar scenes do Decentraland.

# Editando Scenes

O Creator Hub inclui um poderoso Scene Editor que combina uma interface simples sem código com a capacidade de escrever código para personalizar ainda mais suas cenas.

![Creator Hub](/docs/_remote/9100f2b541affb1c6fecfe89.webp)

Veja [Instalação do Creator Hub](/docs/creator/content-creator-pt/scene-editor/primeiros-passos/editor-installation/) para começar.



<div class="video-embed"><button type="button" class="video-play" data-video-src="https://www.youtube-nocookie.com/embed/tK5-fyBVnK0" aria-label="Play YouTube video"><svg viewBox="0 0 64 64" width="64" height="64" aria-hidden="true"><circle cx="32" cy="32" r="30" fill="currentColor"/><path d="m26 20 20 12-20 12Z" fill="#181214"/></svg><span>Play video</span></button></div>
<p class="video-source"><a href="&lt;https://www.youtube.com/watch?v=tK5-fyBVnK0>">Watch on YouTube</a></p>



## Criar uma cena

Para criar uma nova cena, abra o Creator Hub e clique na seta ao lado do **Criar** botão. No menu suspenso, selecione **Nova cena**.

![O menu suspenso do botão Criar, aberto, com as opções Nova cena e Importar cena.](/docs/_remote/727cb68c3c3feea0e6221431.webp)

Em seguida, você será solicitado a nomear sua cena e escolher um local para salvá-la. Para começar a partir de um projeto com algum conteúdo inicial em vez de uma cena vazia, clique em **Modelos** na **Cenas** aba e escolha um.

Veja [Gerenciar cenas](/docs/creator/content-creator-pt/scene-editor/primeiros-passos/manage-scenes/) para mais detalhes.

Depois que a cena abrir, você verá a árvore de entities à esquerda, o canvas no centro, as propriedades do item selecionado à direita e os asset packs na parte inferior.

![O Scene Editor com uma pequena cena aberta: a árvore de entities à esquerda lista Player, Camera, Scene e alguns itens, o canvas mostra um chão de grama com uma churrasqueira e uma fonte de pássaro, e o painel de propriedades à direita mostra os componentes Transform e GLTF da Bird Fountain selecionada.](/docs/_remote/e758b583939953f86e7202c8.webp)

Veja [Essenciais do Scene Editor](/docs/creator/content-creator-pt/scene-editor/primeiros-passos/scene-editor-essentials/) para um tour por cada seção.

## Movendo-se

Para se orientar no Scene Editor:

* Usa **W** e **S** para aproximar ou afastar. Você também pode usar a roda de rolagem do mouse, ou **+** e **-** teclas
* Usa **A** e **D** para mover para os lados.
* Usa **Q** e **E** para mover para cima e para baixo.
* Use o **Botão esquerdo do mouse** para clicar e selecionar itens e movê-los.
* Use o **Botão direito do mouse** e arraste para girar a câmera.


> [!NOTE]
> **💡 Dica**: Você também pode girar a câmera pressionando **Alt** no Windows, ou **Option** no Mac enquanto arrasta. Isso é especialmente útil quando você usa um trackpad em vez de um mouse.


* Pressione **barra de espaço** para redefinir a câmera para a posição padrão

## Adicionar itens

Navegue pelas categorias temáticas dos asset packs no menu na parte inferior para encontrar diferentes itens que você pode colocar na sua cena.

![](/docs/_remote/c5121b56db02ac73b992d99e.webp)

Para colocar um item, clique e arraste-o do menu do asset pack para um local na sua cena no canvas.

![](/docs/_remote/b20b9f38d71f91b4c4270633.webp)

Clique e arraste um item selecionado para movê-lo livremente pela cena ao nível do chão. Veja [Essenciais do Scene Editor](/docs/creator/content-creator-pt/scene-editor/primeiros-passos/scene-editor-essentials/#position-items) para mais detalhes.


> [!NOTE]
> **💡 Dica**: Alguns itens são **Smart items**, eles vêm com comportamentos interativos integrados. Veja [Smart items](/docs/creator/content-creator-pt/scene-editor/interatividade/smart-items/) para mais detalhes.


![](/docs/_remote/4401edd6b0ae269ede4c9467.webp)

Silenciar áudio da cena

### Animações de avatar

A **Animações de avatar** o pack contém animações para o avatar do jogador em vez de itens para colocar: poses sentadas, pesca, carregar uma caixa, dançar e muito mais. Elas são organizadas nas mesmas **Actions**, **Sentado** e **Expressões** categorias que o restante.

Passe o mouse sobre um card para ver a animação sendo reproduzida. Arraste uma para a visualização 3D para copiar o arquivo dela para os Local Assets da sua cena. Nenhuma Entity é adicionada, porque uma animação não é algo que fica na cena: você a executa a partir do código. Veja [Animações de avatar](/docs/creator/content-creator-pt/scenes-sdk7/interatividade/avatares/avatar-animations/).

Cards marcados com um **Integrado** badge funcionam de forma diferente. Essas animações já vêm com o Decentraland, então toda cena pode reproduzi-las sem nenhum arquivo a adicionar. Você não pode arrastá-las. Em vez disso, clique com o botão direito em um card e escolha **Copiar nome** para obter o nome a usar no seu código.

No **Local Assets** aba, o **Animações de avatar** filtro reúne os arquivos de animação do seu projeto, junto com uma pasta somente leitura de **animações integradas** listando as que não precisam de arquivo.

## Pré-visualização

Para testar sua cena e vivenciá-la como um jogador, clique no **Play** botão no canto superior direito. Isso abrirá uma nova janela com o Decentraland Desktop Explorer, executando apenas a sua cena. Lá você pode se mover pela cena e interagir com itens interativos.


> [!WARNING]
> **📔 Nota**: Se você não o tiver instalado na sua máquina, baixe o **Decentraland Launcher** em [Decentraland.org](https://decentraland.org).


![](/docs/_remote/ca7e6550cdb1bb947ce74b28.webp)

Clique na seta ao lado do **Play** botão para abrir o **Play Options** menu, onde você escolhe se quer visualizar no cliente desktop, em um navegador ou em um celular. Veja [Pré-visualize sua cena](/docs/creator/content-creator-pt/scenes-sdk7/primeiros-passos/preview-scene/) para uma lista completa de todas as opções disponíveis.

## Renderizador da cena

O Scene Editor usa **Babylon** como seu renderizador padrão para o canvas de edição. Você pode mudar para **Bevy (experimental)**.

Para alterar o renderizador:

1. Clique no logo do Creator Hub no canto superior esquerdo e selecione **Configurações**.
2. Vá para a aba **EXPERIMENTAL** tab.
3. Ative **Ativar o Renderizador de Cena do Bevy**.

![A caixa de diálogo Preferências do app na aba EXPERIMENTAL, com o botão Ativar o Renderizador de Cena do Bevy.](/docs/_remote/ef0ed302a5aa995c466b03ac.webp)

O renderizador Bevy é um engine alternativo para o canvas de edição. Ele afeta a aparência da sua cena durante a edição, não a forma como ela aparece para os jogadores depois da publicação.

Ao usar o renderizador Bevy, a barra de ferramentas ganha um menu suspenso de modo da câmera, botões de reproduzir e parar para executar a cena no canvas, um **Interagir** alternador (o ícone de gamepad) e um **Silenciar áudio da cena** alternador (o ícone de alto-falante).

![A barra de ferramentas do editor sob o renderizador Bevy: desfazer e refazer, as ferramentas de gizmo, um menu suspenso da câmera mostrando Livre, botões de reproduzir e parar, o alternador de gamepad Interagir, a engrenagem de Preferências e o botão Inspector.](/docs/_remote/851c27272919db1fa6fcf53e.webp)

Use o **Interagir** alternador para testar uma cena em execução diretamente no canvas do editor, sem abrir uma janela de pré-visualização separada. Isso permite verificar rapidamente as interações sem parar a cena. أثناء interagir, os Smart items mostram suas dicas de hover, como a tecla a pressionar, diretamente no canvas.

Clique no botão de alto-falante, rotulado **Silenciar áudio da cena**Silenciar áudio da cena [Audio Source](/docs/creator/content-creator-pt/scenes-sdk7/fundamentos-de-conteudo-3d/sounds/) e streams de áudio na cena, tanto enquanto você edita quanto enquanto a cena é executada no canvas, e a cena permanece sem som se recarregar. Clique nele novamente, agora rotulado **Reativar áudio da cena**para trazer o som de volta. O botão só aparece sob o renderizador Bevy, porque o renderizador Babylon não reproduz o áudio da cena.

O editor Bevy também carrega [itens personalizados](/docs/creator/content-creator-pt/scene-editor/interatividade/custom-items/), mostra marcadores de espaço reservado para assets quebrados ou ausentes e fornece mensagens de erro amigáveis com ações de correção quando as dependências do SDK da sua cena estão desatualizadas. O alternador da câmera na barra de ferramentas mostra **Livre** ou **Jogador**, dependendo do modo em que você estiver.


> [!WARNING]
> **📔 Nota**: a aba de métricas da cena não está disponível no Bevy. Execute uma pré-visualização da cena no cliente desktop ou mobile e veja as estatísticas por lá.



> [!WARNING]
> **Nota:** O renderizador Bevy é experimental e talvez não suporte todos os recursos de edição que o renderizador Babylon padrão suporta.


## Configurações da cena

Clique nos **Ícone de lápis** no canto superior direito da tela. Isso abre uma série de propriedades no nível da cena para editar, incluindo nome, miniatura, tamanho da cena e muito mais.

![](/docs/_remote/a64bc0ae30e4d852f97ccd78.webp)

Veja [Configurações da cena](/docs/creator/content-creator-pt/scene-editor/configurar/scene-settings/) para mais detalhes.

## Publique sua cena

Quando estiver satisfeito com sua cena, pressione o **Publicar** button.

![](/docs/_remote/75db93f7a2fdd17bec3b4907.webp)

Veja [Publicar cena](/docs/creator/content-creator-pt/scene-editor/publicar/publish-scene/) para mais detalhes.

## Ver também

* Veja [Essenciais do Scene Editor](/docs/creator/content-creator-pt/scene-editor/primeiros-passos/scene-editor-essentials/) para mais detalhes sobre a interface do Scene Editor.
* Veja [Smart items](/docs/creator/content-creator-pt/scene-editor/interatividade/smart-items/) sobre como adicionar interatividade simples à sua cena.
* Veja [Combine com código](/docs/creator/content-creator-pt/scene-editor/expandir-com-codigo/overview/) sobre como editar o código da sua cena.
* Veja [Publicar cena](/docs/creator/content-creator-pt/scene-editor/publicar/publish-scene/) sobre como publicar sua cena no Decentraland.


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-pt/scene-editor/primeiros-passos/about-editor.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
