> Entenda como os components de um item funcionam

# Entities e Components

Selecione um item clicando nele no canvas ou na Entity tree. Em seguida, você verá os componentes dele exibidos no painel de propriedades, à direita da tela. Itens diferentes têm componentes diferentes, e cada um exibe configurações específicas.

![](/docs/_remote/8c1945559d13f38d8a485e7a.webp)

A maioria dos itens não interativos tem os seguintes componentes:

* **Transform**: Define a posição, rotação e escala do item. Se o item for filho de outro item na [Entity Tree](/docs/creator/content-creator-pt/scene-editor/primeiros-passos/scene-editor-essentials/#the-entity-tree), esses valores são relativos aos do pai.
* **GLTF**: Qual modelo 3D carregar. Inclui o caminho local para o arquivo desse modelo 3D. Também inclui algumas propriedades para configurar [colliders](/docs/creator/content-creator-pt/scenes-sdk7/fundamentos-de-conteudo-3d/colliders/#colliders-on-3d-models) no modelo.

Os itens na sua cena são todos **Entities**. Tudo em uma scene é uma Entity; elas são os blocos básicos das scenes. Itens são Entities que têm pelo menos uma posição e uma forma visível.

## Adicionar componentes

Para adicionar Components a qualquer Entity, clique no **+** sinal na parte superior da aba de propriedades e selecione o Component na lista. Veja [Transforme qualquer item em um smart item](/docs/creator/content-creator-pt/scene-editor/interatividade/make-any-item-smart/)

![O menu de adicionar componentes se abre em uma Entity, listando components agrupados em Conteúdo 3D e Interação.](/docs/_remote/e0e52c0c060b63e97e2466dd.webp)

Você pode excluir qualquer Component de uma Entity clicando no ícone de três pontos à direita e selecionando **Excluir Component**.

## Crie uma Entity do zero

Para criar uma Entity novinha em folha, clique com o botão direito na raiz **Scene** Entity na Entity tree, ou em qualquer outra Entity, e selecione **Add Child**

![](/docs/_remote/a6edc747060b504b764ee35b.webp)

Isso cria uma Entity vazia, com apenas um **Transform** componente. A nova entity é filha da entity pai em que você clicou. Depois, você pode adicionar a ela quaisquer outros componentes que quiser para moldá-la em qualquer coisa que desejar.

## Componentes disponíveis

Os seguintes Components podem ser adicionados a qualquer Entity pela UI do Scene Editor:

* **Mesh Renderer**: Dá à Entity uma forma visível com base em uma forma primitiva (cubo, plano, cilindro ou esfera).
* **Mesh Collider**: Dá à Entity uma geometria de collider invisível. Isso pode impedir o player de atravessar o item e/ou torná-lo clicável. Veja [collider](/docs/creator/content-creator-pt/scenes-sdk7/fundamentos-de-conteudo-3d/colliders/).
* **Material**: Define a cor, a textura e outras propriedades de uma Entity que tenha um **Mesh Renderer** componente. Veja [materials](/docs/creator/content-creator-pt/scenes-sdk7/fundamentos-de-conteudo-3d/materials/).

  <div data-gb-custom-block data-tag="hint" data-style="warning" class="hint hint-warning"><p>**📔 Nota**: o item deve ter um componente **Mesh Renderer**. Isso não afeta itens com uma forma visível **GLTF**.</p></div>
* **Visibility**: Define se uma Entity está invisível.
* **Light Source**: Adiciona uma luz à Entity.
* **Swap Material**: Troca o material de uma Entity que tenha um **GLTF** componente. Se o modelo 3D tiver vários meshes, você pode trocar o material de cada mesh individualmente.
* **Audio Source**: Toca um som a partir de um arquivo de som na localização da Entity. Veja [Sounds](/docs/creator/content-creator-pt/scenes-sdk7/fundamentos-de-conteudo-3d/sounds/).
* **Text Shape**: Exibe texto no espaço 3D. Veja [Text](/docs/creator/content-creator-pt/scenes-sdk7/fundamentos-de-conteudo-3d/text/).
* **Pointer Events**: Marca uma Entity como clicável, exibindo uma dica de hover.


> [!WARNING]
> **📔 Nota**: O **Pointer Events** Component fornece apenas feedback. Para executar ações quando uma Entity for interagida, veja [Transforme qualquer item em um smart item](/docs/creator/content-creator-pt/scene-editor/interatividade/make-any-item-smart/)


* **Multiplayer**: Compartilha quaisquer alterações que aconteçam com a Entity para que todos os players na scene também as vejam. Pode ser configurado para compartilhar alterações apenas em certos components. Veja [Serverless Multiplayer](/docs/creator/content-creator-pt/scenes-sdk7/rede/serverless-multiplayer/#mark-an-entity-as-synced) para mais detalhes.
* **Animator**: Controla a reprodução de animações em uma Entity com um **GLTF** modelo 3D. Veja [animações de modelo 3D](/docs/creator/content-creator-pt/scenes-sdk7/fundamentos-de-conteudo-3d/3d-model-animations/).
* **Tween**: Faz a Entity mover-se, girar ou escalar gradualmente ao longo de um período de tempo. Veja [Mover entities](/docs/creator/content-creator-pt/scenes-sdk7/fundamentos-de-conteudo-3d/move-entities/).
* **Billboard**: Faz a Entity girar sempre para ficar de frente para o player. Veja [Encarar o usuário](/docs/creator/content-creator-pt/scenes-sdk7/fundamentos-de-conteudo-3d/entity-positioning/#face-the-player).
* **Avatar Attach**: Anexa a Entity ao avatar do player, para que ela o acompanhe. Veja [Anexar uma entity a um avatar](/docs/creator/content-creator-pt/scenes-sdk7/fundamentos-de-conteudo-3d/entity-positioning/#attach-an-entity-to-an-avatar).
* **Video Player**: Toca um vídeo dentro da scene, que pode ser exibido na Entity por meio de seu **Material**. Veja [reprodução de vídeo](/docs/creator/content-creator-pt/scenes-sdk7/media/video-playing/).
* **NFT Shape**: Exibe a imagem de um NFT dentro de uma moldura. Veja [Exibir um NFT](/docs/creator/content-creator-pt/scene-editor/construir/nfts/).
* **Particle System**: Emite partículas a partir da localização da Entity, para efeitos como fumaça, fogo ou brilhos. Veja [sistemas de partículas](/docs/creator/content-creator-pt/scenes-sdk7/fundamentos-de-conteudo-3d/particle-system/).
* **Virtual Camera**: Define um ângulo de câmera personalizado para o qual a scene pode alternar a visão do player. Veja [Camera](/docs/creator/content-creator-pt/scenes-sdk7/fundamentos-de-conteudo-3d/camera/).
* **Avatar Modifier Area**: Altera como os avatars se comportam ou aparecem para players dentro de uma região centrada nesta entity. A escala da entity define o tamanho da região. Veja [Modifier Areas](/docs/creator/content-creator-pt/scene-editor/interatividade/modifier-areas/).
* **Camera Modifier Area**: Força a câmera do player para primeira ou terceira pessoa dentro de uma região centrada nesta entity. A escala da entity define o tamanho da região. Veja [Modifier Areas](/docs/creator/content-creator-pt/scene-editor/interatividade/modifier-areas/).
* **Script**: Anexa um módulo de código personalizado à Entity, com parâmetros que você pode configurar pela UI. Veja [componente Script](/docs/creator/content-creator-pt/scene-editor/expandir-com-codigo/script-component/).


> [!WARNING]
> **📔 Nota**: Outros components existem no SDK e atualmente só podem ser usados por código. Você também pode criar seus próprios [componentes personalizados](/docs/creator/content-creator-pt/scenes-sdk7/arquitetura/custom-components/) por código; eles não terão representação na UI, mas podem ser adicionados e editados por código.
> 
> Veja [Combine com código](/docs/creator/content-creator-pt/scene-editor/expandir-com-codigo/overview/) para saber como editar o código da sua scene.
> 
> Observe também que uma Entity só pode conter **um** de cada Component. Não é possível atribuir uma segunda instância de um Component que já exista na entity. Por exemplo, você não pode adicionar dois **Actions** components à mesma Entity.


## Configurações que vale a pena conhecer

A maioria dos campos dos components corresponde um a um à página do SDK vinculada acima. Alguns são fáceis de passar despercebidos ou se comportam de um jeito que não fica óbvio pelo nome do campo.

### Audio Source

**Pitch** multiplica a velocidade de reprodução e desloca o pitch junto com ela. `1` toca o som original, `2` toca duas vezes mais rápido (uma oitava acima), `0.5` metade da velocidade (uma oitava abaixo). Deve ser maior que 0 e, por padrão, é `1`.

### Video Player

* **Playback Rate**: mesma ideia de Pitch, para vídeo. `1` é a velocidade normal. Deixe vazio para o padrão.
* **Start Position (seconds)**: onde a reprodução começa quando o vídeo inicia. Deixe vazio para começar do início.
* **Spatial Audio**: ative **Spatial** para tornar o som do vídeo posicional, fazendo-o enfraquecer à medida que o player se afasta. **Spatial Distance** então define o alcance: o áudio é reproduzido com volume total até **Min** metros e vai diminuindo até o silêncio em **Max** metros. Os padrões são 0 e 60.

Com **Spatial** desligado, o áudio é reproduzido com o mesmo volume em toda a scene.

### Text Shape

* **Size**: a largura (**W**) e a altura (**H**) da caixa de texto, em metros. O alinhamento e o padding são medidos em relação a essa caixa.
* **Text Wrapping**: quebra o texto em novas linhas quando ele alcança a largura definida em **Size**. Com ele desligado, o texto permanece em uma única linha.

### Material

* **Color Alpha**: a opacidade da principal **Color**, de 0 a 1. Isso não tem efeito enquanto **Transparency Mode** estiver **Opaque**.
* **Alpha test**: o ponto de corte em que um pixel se torna invisível. Só aparece quando **Transparency Mode** estiver **Alpha test** ou **Alpha test & blend**, já que esses são os únicos modos que o utilizam.
* **Nenhum**: os dropdowns de cor agora oferecem uma entrada **Nenhum** que limpa a cor, e o dropdown de Type da textura oferece **Type** para remover uma textura. **Nenhum** a opção

### Particle System

A **Rotation** seção controla como as partículas são orientadas e como giram:

* **Billboard**: cada partícula sempre fica de frente para a câmera. Ativado por padrão. Desative para partículas que devem girar livremente em 3D.
* **Face Travel Direction**: cada partícula começa apontando para a direção em que está se movendo, como uma seta.
* **Initial Rotation (deg)**: como cada partícula é rotacionada ao nascer, por eixo.
* **Rotation Over Time (deg/sec)**: quão rápido ela gira, por eixo. Os valores têm limite de 180 por eixo.

Quando **Use Texture** está ativado, aparecem mais dois dropdowns:

* **Wrap Mode**: como a textura se comporta além de suas bordas. **Clamp** (o padrão) estica os pixels da borda, **Repeat** a repete em mosaico, **Mirror** a repete em mosaico com inversões alternadas.
* **Filter Mode**: como a textura é amostrada. **Bilinear** (o padrão) é suave, **Point** é pixelado e bom para pixel art, **Trilinear** suaviza entre os níveis de mipmap.

### Tween

**Tween Type** oferece cinco modos:

| Modo                                                 | O que faz                                                                          |
| ---------------------------------------------------- | ---------------------------------------------------------------------------------- |
| **Mover item**, **Girar item**, **Dimensionar item** | Animar de um valor inicial até um valor final ao longo da **Duração**.             |
| **Mover continuamente**                              | Continuar se movendo em uma direção fixa, para sempre, com velocidade constante.   |
| **Girar continuamente**                              | Continuar girando em torno de um eixo fixo, para sempre, com velocidade constante. |

Os dois modos contínuos não têm um valor final, então **Início**, **Fim**, **Duração**, **Função de suavização** e **Repetir** ficam ocultos para eles. Em vez disso, você obtém:

* **Mover continuamente**: uma **direção** vetorial e uma **velocidade (metros/seg)**. Use uma direção de comprimento 1 para que a velocidade seja exatamente metros por segundo. Uma velocidade negativa move na direção oposta.
* **Girar continuamente**: uma **Eixo de rotação (graus)** e uma **velocidade (graus/seg)**. Só a direção do eixo importa, não o tamanho dos ângulos.

Ao alternar **Tween Type** isso redefine esses valores para padrões adequados ao novo modo, porque os números significam coisas diferentes em cada um.

A **Em execução** o checkbox (anteriormente chamado "Auto start") decide se o tween é executado assim que a scene carrega. Com ele desligado, o tween é criado pausado em seu valor inicial e só é executado quando o seu código o define para tocar.


> [!NOTE]
> **💡 Dica**: algumas propriedades dos components só podem ser definidas por código. Editar uma entity no inspector não as descarta mais, então você pode definir um valor no código da sua scene e continuar usando o editor nessa entity sem perdê-lo.


## Os smart items

[Os smart items](/docs/creator/content-creator-pt/scene-editor/interatividade/smart-items/) também podem incluir components especiais que controlam a interatividade da Entity. Normalmente, eles são:

* **Actions**: Lista todas as ações possíveis que o item pode realizar.
* **Triggers**: Determina quando as ações do component Actions são executadas.
* **States**: Mantém o controle do estado atual do item. O state pode ser usado para lógica condicional, para acionar apenas certas ações se o item estiver em um determinado state.
* **Counter**: Mantém o controle de um counter. O counter pode ser usado para lógica condicional, para acionar apenas certas ações se o valor do counter for igual/maior/menor que um valor dado.

Veja [Smart items avançados](/docs/creator/content-creator-pt/scene-editor/interatividade/smart-items-advanced/) para mais detalhes.

## Sobre Entities e components

Tudo em uma scene é uma Entity. Todos os itens e smart items na scene são Entities.

Todos os atributos de uma Entity são determinados pelos seus components. Eles definem o que a Entity é, onde ela está, como soa e como se comporta. Por exemplo, um **Transform** component armazena as coordenadas, rotação e escala da Entity. Um **MeshRenderer** component dá à Entity uma forma visível (como um cubo ou uma esfera), e um **Material** component dá à Entity uma cor ou textura.

Os valores nos components podem mudar com o tempo. No Scene Editor, você configura os valores iniciais desses components. Mas, quando sua scene estiver em execução, as ações do player ou a passagem do tempo podem alterar esses valores.

Por exemplo, um Smart Item de plataforma em movimento tem uma posição inicial que você define por meio de seu **Transform** component, mas depois que as ações desse item fazem com que ele se mova, o seu **Transform** terá valores diferentes.

Veja [Entities e components](/docs/creator/content-creator-pt/scenes-sdk7/arquitetura/entities-components/) para uma análise detalhada desse conceito e de como ele é usado pelas scenes do Decentraland.


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-pt/scene-editor/construir/components.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
