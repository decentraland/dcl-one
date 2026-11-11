> Como usar o Scene Editor

# Essenciais do Scene Editor

A UI do Scene Editor está dividida em várias secções diferentes, com finalidades diferentes.

![](/docs/_remote/ee9a6732ea7a70cc625e7341.webp)

* **Canvas**: Manipule itens diretamente e veja como a sua cena fica.
* **Entity tree**: Contém uma lista de todos os itens na cena e da sua hierarquia.
* **Propriedades**: Mostra detalhes sobre o item atualmente selecionado.
* **Recursos**: Mostra os recursos disponíveis para usar.

## Movendo-se

Para se orientar no Scene Editor:

* Usa **W** e **S** para aproximar ou afastar. Você também pode usar a roda de rolagem do mouse, ou **+** e **-** teclas
* Usa **A** e **D** para mover para os lados.
* Usa **Q** e **E** para mover para cima e para baixo.
* Clique nos **Botão direito do mouse** e arraste para girar a câmera.
* Pressione **barra de espaço** para redefinir a câmera para a posição padrão
* Usa **Botão esquerdo do mouse** para clicar e selecionar itens e movê-los.

## Pré-visualizar em mobile

A **Play Options** menu ao lado do **Play** botão tem uma **Mostrar código QR para telemóvel** opção. Digitalize o código QR com um telemóvel na mesma rede Wi‑Fi para abrir a sua cena na [app móvel Decentraland](/docs/creator/content-creator-pt/criar-para-mobile/cliente-movel/overview/). Esta é a forma recomendada de validar a UI, os controlos e o desempenho para jogadores mobile. Veja [Pré-visualizar em mobile](/docs/creator/content-creator-pt/criar-para-mobile/desenvolver/preview-on-mobile/) o guia completo.

## Definir o Ground

O ground da cena pode usar várias texturas diferentes. Pode encontrá-las nos diferentes asset packs temáticos no menu de itens.

Itens do tipo **Ground** têm um ícone de balde de tinta. Se arrastar um deles para a sua cena, ele cobre todo o ground da sua cena com cópias deste item.

![](/docs/_remote/e120bb99677fe9cb301458fb.webp)

Também pode adicionar uma única cópia do item mantendo **Shift** premida a tecla enquanto arrasta o ground para a cena.

![](/docs/_remote/df027a655d783fcc8276005c.webp)

A coleção de itens ground aparece na [entity tree](#the-entity-tree) dentro de uma pasta. Cada um deles está bloqueado para evitar seleções acidentais. [Desative](#lock-or-hide-items) os itens para os mover ou editar.

## Adicionar itens

Navegue pelas categorias temáticas dos asset packs no menu na parte inferior para encontrar diferentes itens que você pode colocar na sua cena.

![](/docs/_remote/c5121b56db02ac73b992d99e.webp)

Também pode usar a caixa de pesquisa. Note que, quando está dentro de um asset pack, a pesquisa só procura nesse asset pack.

Para colocar um item, clique e arraste-o do menu do asset pack para um local na sua cena no canvas.

![](/docs/_remote/b20b9f38d71f91b4c4270633.webp)


> [!NOTE]
> **💡 Dica**: As suas alterações são guardadas automaticamente sempre que adiciona, move ou edita as propriedades de qualquer um dos itens na sua cena.


Para duplicar um item, selecione-o e prima **Ctrl + C** e depois **Ctrl + V**. Também pode encontrar o item no [entity tree](#the-entity-tree) para clicar com o botão direito e selecionar a opção **Duplicar**. O novo item ficará perfeitamente sobreposto ao original.

Para eliminar um item da cena, selecione-o e prima a *Delete* tecla.

Veja [Importar itens](/docs/creator/content-creator-pt/scene-editor/construir/import-items/) para adicionar os seus próprios modelos 3D personalizados a partir do disco.


> [!WARNING]
> **📔 Nota**: Assim que arrastar um modelo 3D para a sua cena, ele é descarregado para a pasta do seu projeto e permanece lá mesmo que o elimine. Estes modelos não utilizados podem aumentar o tamanho da sua cena.
> 
> Abra o **Local Assets** tab para eliminar quaisquer modelos não utilizados.


## Posicionar itens



<div class="video-embed"><button type="button" class="video-play" data-video-src="https://www.youtube-nocookie.com/embed/cNl02PFPdcQ" aria-label="Play YouTube video"><svg viewBox="0 0 64 64" width="64" height="64" aria-hidden="true"><circle cx="32" cy="32" r="30" fill="currentColor"/><path d="m26 20 20 12-20 12Z" fill="#181214"/></svg><span>Play video</span></button></div>
<p class="video-source"><a href="&lt;https://www.youtube.com/watch?v=cNl02PFPdcQ>">Watch on YouTube</a></p>



Clique com o **Botão esquerdo do mouse** e arraste um item selecionado para o mover livremente pela cena ao nível do chão.

Também pode usar as ferramentas no menu superior:

![](/docs/_remote/c6834cebdebce2e0fb6608ba.webp)

* **Move tool**: Clique e arraste cada seta para mover o item num único eixo de cada vez. Com esta ferramenta também pode posicionar coisas acima do nível do chão.
* **Rotate tool**: Clique e arraste cada um dos anéis à volta do item para rodar o item num eixo de cada vez.
* **Scale tool**: Clique no centro do gizmo e arraste para dentro ou para fora para aumentar. Esta ferramenta também permite esticar um item num único eixo para alterar as suas proporções; para isso, clique num dos eixos do gizmo e arraste.

![](/docs/_remote/11bce750b05bad2b59a8c3b8.webp)

Para ter maior precisão ao mover, rodar ou redimensionar um item, prima e mantenha *Shift* a tecla enquanto faz ajustes. Isto evitará o snapping à grelha.

Para alterar a granularidade do movimento e outras definições, clique na seta para baixo à direita das ferramentas. As seguintes definições estão disponíveis:

* **Snap**: Ative ou desative a grelha. Quando desativada, o comportamento de **Shift** é invertido: por predefinição não segue a grelha; segue-a se mantiver **Shift**.
  * **Posição**: O tamanho dos incrementos de movimento em metros quando **Snap** está ativada.
  * **Rotação**: O tamanho dos incrementos de rotação em graus quando **Snap** está ativada.
  * **Escala**: O tamanho dos incrementos de escala quando **Snap** está ativada.
* **Align to world**: Uma única caixa de seleção que se refere ao eixo usado pelos gizmos. Quando marcada, o eixo da Move tool e da Rotate tool alinha-se sempre com o mundo e não muda com a orientação do objeto. Quando desmarcada, alinha-se com a orientação do objeto.

Para selecionar vários itens ao mesmo tempo, prima e mantenha *Controlar* a tecla enquanto os seleciona. Depois pode mover, rodar, redimensionar, duplicar ou eliminar todos numa única ação.

### Introduza uma quantidade exata

Quando arrastar não for suficientemente preciso, introduza a quantidade em vez disso:

1. Prima uma tecla da ferramenta: **M** ou **G** para mover, **R** para rodar, **X** para redimensionar.
2. Pressione **X**, **Y** ou **Z** para escolher um eixo. A escala não precisa de um: sem um eixo, escala todos os eixos pelo mesmo fator.
3. Introduza o número. Use **.** para decimais, e **-** para alternar entre positivo e negativo.
4. Pressione **Enter** para aplicar, ou **Esc** para cancelar. **Backspace** apaga o que escreveu.

Um chip na parte inferior do Canvas mostra a entrada à medida que a escreve, indica o que ainda falta e confirma o que foi aplicado.

Alguns exemplos:

* **R** **X** **1** **5** **Enter** roda a seleção 15 graus em torno de X.
* **X** **2** **Enter** faz a seleção ficar duas vezes maior.
* **M** **Y** **-** **3** **Enter** move-a 3 metros para baixo.

Algumas coisas que vale a pena saber:

* A quantidade é sempre relativa à posição atual da seleção, não uma coordenada absoluta.
* Os valores digitados ignoram a grelha de snap.
* Com vários itens selecionados, todos se movem, rodam ou redimensionam em torno do centro partilhado, como um único passo de undo.
* Premir **M**, **G** ou **R** uma segunda vez ainda desativa o gizmo. **X** não, porque, enquanto escreve, significa o eixo X.


> [!NOTE]
> **💡 Dica**: O ícone do teclado no canto inferior direito do Canvas abre a lista completa de atalhos.


## Smart items

Smart items são itens especiais que vêm com comportamentos interativos incorporados. Veja [Smart items](/docs/creator/content-creator-pt/scene-editor/interatividade/smart-items/) para mais detalhes.

![](/docs/_remote/4401edd6b0ae269ede4c9467.webp)

## A entity tree

Na margem esquerda, verá uma estrutura em árvore com todas as Entities na cena. Isto inclui todos os itens que adiciona, bem como algumas Entities predefinidas.


> [!NOTE]
> **💡 Dica**: Tudo numa cena é uma Entity; elas são os blocos básicos de construção das scenes. Os itens são Entities que têm pelo menos uma posição e uma forma visível.


Em vez de selecionar um item clicando nele na vista 3D da cena, pode selecioná-lo na vista em árvore. Clique com o botão direito do rato numa entity para revelar mais opções: pode renomear, eliminar ou duplicar, criar também uma entity filha, transformá-la num item personalizado ou adicionar um component à entity.

![A entity tree com o menu do botão direito aberto na entity Bird Fountain, listando Renomear, Adicionar filho, Duplicar, Eliminar, Criar item personalizado e Adicionar component.](/docs/_remote/42bfc23b5b7dbda3a5127c7a.webp)

### Pesquisar na entity tree

Uma caixa de pesquisa no topo da entity tree permite encontrar Entities pelo nome. Escreva parte do nome de uma Entity e a árvore filtra para mostrar apenas Entities correspondentes e a sua hierarquia pai, para que possa ver onde cada correspondência fica na árvore. A pesquisa não distingue maiúsculas de minúsculas. As Entities-pai das correspondências são automaticamente expandidas para que cada resultado fique visível.

Pressione **Escape** ou clique no ícone de limpar para remover o filtro e voltar à árvore completa. Se uma Entity estiver selecionada, limpar a pesquisa faz a vista voltar a essa Entity selecionada para não perder o seu lugar.

![A entity tree filtrada pelo termo de pesquisa fount: apenas as Entities Camera, Scene e Bird Fountain permanecem visíveis, com a Entity pai Scene expandida para mostrar a correspondência.](/docs/_remote/462138368861e2d4fd3aba3d.webp)

### Hierarquia de Entities

As Entities seguem uma hierarquia que pode ter tantos níveis quantos quiser. Estabeleça uma relação pai-filho entre duas Entities arrastando um item para cima de outro na árvore. Uma entity filha herda a posição da parent entity, pelo que, quando a parent se move, leva consigo quaisquer filhos. Isto pode ser prático ao construir uma cena; por exemplo, pode definir os óculos e os pratos como filhos de uma mesa e depois mover a mesa sem precisar de reajustar mais nada. Também pode ser importante, ao interagir com a cena, para que os itens se movam em conjunto.

![](/docs/_remote/2ba2f0d9e5656796ad572f00.webp)

Também pode minimizar ou expandir os filhos de uma Entity para manter a vista simples; esta ação não tem efeito na cena.

### Entities especiais

A cena inclui algumas Entities especiais que pode ver na entity tree.

* **Cena**: Isto refere-se à root entity; tudo o que adicionar na cena é filho desta entity. Pode abri-la para ver [as definições da cena](#scene-settings).
* **Jogador**: O avatar do jogador. Pode adicionar components especiais a esta entity que podem alterar mecânicas de jogabilidade. Também pode arrastar outras Entities para serem filhos do avatar. Se uma Entity for filha do avatar, a sua posição ficará fixa ao jogador. Use isto, por exemplo, para adicionar um marcador flutuante acima da cabeça do jogador, que o segue por todo o lado.
* **Camera**: A Camera do jogador. Pode arrastar outras Entities para serem filhos da Camera. Se uma Entity for filha da Camera, a sua posição ficará fixa no ecrã. Use isto, por exemplo, para mostrar uma arma num jogo de tiro, que fica sempre à vista mesmo que o jogador aponte para cima ou para baixo.

### Bloquear ou ocultar itens

Pode dar jeito bloquear ocasionalmente um item para evitar selecioná-lo e movê-lo acidentalmente. Isto é especialmente útil para itens de fundo, como o ground ou um edifício. Para bloquear um item, procure-o na entity tree à esquerda, passe o rato sobre ele e selecione o ícone de bloqueio. Pode ativar ou desativar este comportamento através do mesmo ícone.

Também pode querer ocultar um item que possa obstruir a sua vista enquanto coloca outros. Isto é especialmente útil para ocultar o telhado ou um edifício enquanto trabalha nos interiores. Os itens ocultos só ficam ocultos na janela Canvas do Scene Editor, não para os jogadores que entram na cena. Para ocultar um item, procure-o na entity tree à esquerda, passe o rato sobre ele e selecione o ícone do olho. Pode ativar ou desativar este comportamento através do mesmo ícone.

![](/docs/_remote/1360d2763c98c1ac75eca0d4.webp)

## Properties panel

Selecione um item clicando nele no Canvas ou na entity tree. Depois verá os seus components apresentados no properties panel, à direita do ecrã. Diferentes items têm diferentes components que apresentam cada um definições específicas.

![](/docs/_remote/8c1945559d13f38d8a485e7a.webp)

A maioria dos items não interativos tem os seguintes components:

* **Transform**: Define a posição, rotação e escala do item.
* **GLTF**: Que modelo 3D carregar.

[Smart items](/docs/creator/content-creator-pt/scene-editor/interatividade/smart-items/) pode incluir outros components.

Veja [Components](/docs/creator/content-creator-pt/scene-editor/construir/components/) para saber mais.

## Limitações da cena

As cenas de Decentraland precisam de seguir certas limitações, para poderem ser executadas lado a lado. Existe um número máximo de materials, textures, triangles, etc., proporcional ao número de parcels na cena. Veja [limitações da cena](/docs/creator/content-creator-pt/scenes-sdk7/otimizacao/scene-limitations/) para mais detalhes.

Se o conteúdo na sua cena exceder algum destes limites, o Scene Editor irá notificá-lo no canto inferior esquerdo.

![](/docs/_remote/ab87e37107598ee524831639.webp)

Pode expandir este menu para ver detalhes.

![](/docs/_remote/8d4619b1778683c8d72d3c61.webp)


> [!NOTE]
> **💡 Dica**: Se estiver a criar um Decentraland World, pode sempre aumentar o [tamanho da cena](/docs/creator/content-creator-pt/scene-editor/configurar/scene-settings/#layout) para aumentar os seus limites.


O conteúdo numa cena de Decentraland também deve evitar invadir parcels vizinhos. Se alguma parte dos models na sua cena se estender para além dos limites, ao abrir a pré-visualização da cena verá essas partes cortadas. O Scene Editor marcará o modelo inteiro a vermelho, mas só precisa realmente de se preocupar com as partes do modelo que se estendem para além dos limites da cena.

![](/docs/_remote/8dcb8f74f41494a20d11ba4d.webp)


> [!NOTE]
> **💡 Dica**: Se os models que quer mostrar não couberem, talvez queira aumentar o tamanho da sua cena. Veja [tamanho da cena](/docs/creator/content-creator-pt/scene-editor/configurar/scene-settings/#layout) para ampliar a sua cena.


Mesmo que toda a geometria do modelo 3D caiba na sua cena, um modelo pode ser marcado a vermelho se a Bounding Box do modelo se estender para além da área. Se for esse o caso, pode ignorar os avisos, pois o modelo inteiro será apresentado corretamente. Saiba mais sobre [Bounding Boxes](/docs/creator/content-creator-pt/modelacao-3d-e-animacoes/meshes/#bounding-boxes).

## Limpar assets

Mantenha o projeto arrumado removendo assets que já não estão a ser usados. Abra o **Local Assets** tab na parte inferior do ecrã e clique no ícone da escova.

![](/docs/_remote/98ff3e388967f6b98ae40ab1.webp)

Abre-se uma janela de diálogo listando todos os assets na sua cena e destacando os que não são referenciados por nenhum item.

![](/docs/_remote/a20d113518b454ef45385873.webp)

Selecione os assets que quer eliminar usando as caixas de seleção e depois clique em **Remover selecionados** para os remover permanentemente.

Eliminar um item da cena não remove os seus ficheiros. Modelos, texturas ou sons importados permanecem no seu projeto até os limpar, por isso reveja periodicamente os assets não utilizados.


> [!WARNING]
> **Importante:** Se a sua cena contiver código que referencia assets, alguns assets em uso podem aparecer como não utilizados. Esta janela de diálogo apenas deteta assets referenciados por components na UI do Creator Hub. Depois de clicar em **Remover selecionados**, os ficheiros são eliminados da pasta do projeto; esta ação é permanente e não pode ser desfeita.


## Configurações da cena

Clique nos **Ícone de lápis** no canto superior direito do ecrã. Isto abre uma série de propriedades ao nível da cena para editar.

![](/docs/_remote/a64bc0ae30e4d852f97ccd78.webp)

Aqui pode configurar várias propriedades, incluindo o título e a miniatura, o tamanho da cena, a categoria da cena e a classificação etária, os locais de spawn dos jogadores e os toggles de funcionalidades.

Veja [Configurações da cena](/docs/creator/content-creator-pt/scene-editor/configurar/scene-settings/).

## Ver também

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
GET https://docs.decentraland.org/creator/content-creator-pt/scene-editor/primeiros-passos/scene-editor-essentials.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
