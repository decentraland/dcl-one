> Diretrizes para adicionar props e sons aos emotes.

# Props e Sons

Para levar seus Emotes do Decentraland para o próximo nível, você pode adicionar props (geometria 3D) e/ou sons a eles, tornando os emotes muito mais divertidos e envolventes! Neste guia, você encontrará tudo o que precisa saber para exportá-los corretamente!


> [!NOTE]
> **💡 Dica**: Instale o [plugin Blender do Decentraland Tools](https://extensions.blender.org/add-ons/decentraland-tools/). Ele inclui várias funções úteis para ajudar você a editar e exportar modelos 3D, wearables e emotes.


## **Os fundamentos e limitações**

Para começar a adicionar os props aos seus emotes, é importante usar o [Arquivo de Template do Decentraland](https://raw.githubusercontent.com/decentraland/documentation-creators/main/images/emotes/BaseMale_Rig_1.0.blend) que terá o rig para o avatar e também a Ground Reference para manter seu trabalho dentro dos limites do espaço permitido.

**Atualmente, as animações dos props só funcionam com Armature/Bones Animations, o que significa que animações \_transform**\_\*\* não são permitidas.\*\*

O emote com seus props deve ser exportado tudo junto em um único *.glb* arquivo (Avatar\_Armature + Props\_Armature com suas animações).

* No máximo 3 MB no total.
* No máximo 3k tris no total para os props.
* No máximo 2 materials e 2 textures para os props.
* No máximo 62 bones para a armature do prop.
* O emote deve ter uma animação para o avatar e uma animação para o prop. *Atualmente, múltiplas animações não são permitidas.*
* Ambas as animações (Avatar e Prop) devem ter o mesmo comprimento de keyframes.
* As animações não podem exceder 300 frames ou 10 segundos.
* Os limites de espaço são 4 metros quadrados. Props e partículas devem permanecer dentro do cubo de referência fornecido no Avatar File. Para o movimento do avatar, consulte [a seção Ground Reference e Animation Area](/docs/creator/content-creator-pt/wearables-e-emotes/emotes/creating-emotes/#ground-reference-and-animation-area).

## **Convenções de nomenclatura:**

As convenções de nomenclatura devem ser seguidas rigorosamente para que os emotes funcionem! Caso contrário, eles não serão reproduzidos corretamente nem no Builder nem no world.

#### Convenções de nomeação das Armatures:

**Para o Avatar:**

`Armature`

**Para os Props:**

`Armature_Prop`

#### Convenções de nomeação das animações:

**Para o Avatar:**

`AnimationName_Avatar`

* Exemplo: `TennisServe_Avatar`, `GunShoot_Avatar`

**Para os Props:**

`AnimationName_Prop`

* Exemplo: `TennisServe_Prop`, `GunShoot_Prop`

## **Primeiros passos**

Antes de começar sua animação, você terá que criar um rig para o prop. Se você não estiver familiarizado com o processo, consulte [Create a Rig](/docs/creator/content-creator-pt/modelacao-3d-e-animacoes/create-a-rig/) para mais informações sobre como fazer isso.

Certifique-se de que o objeto do prop e a armature tenham suas origens localizadas na posição 0,0 no Blender. Além disso, aplique as transformações ao objeto do prop e à armature, garantindo que fiquem travados em uma escala de 1,1,1. Isso é crucial para evitar quaisquer possíveis problemas com o comportamento do prop quando ele for utilizado dentro do world ou durante as animações.

#### Fazendo o Prop acompanhar o rig do avatar

Alguns props podem precisar ser presos a certas partes do corpo, como uma raquete de tênis na mão. Isso pode ser feito simplesmente adicionando uma constraint. Para fazer isso, em ***Pose Mode***, selecione o osso do prop (o da raquete de tênis, por exemplo), pressione ***CTRL + Shift + C*** no seu teclado e selecione ***Child of*** ou apenas clique na aba ***Bone Constraint Properties*** e, no menu suspenso, selecione ***Child of***.

*Adicione uma constraint pressionando `Ctrl + Shift + C` no seu teclado.*

Então, em ***Target***, selecione a armature do avatar e em ***Bone*** selecione o osso que você quer que o prop siga. Para manter a posição original do prop, clique em ***Set Inverse*** depois de adicionar a constraint. Se a influence for 1, o prop seguirá totalmente o osso selecionado; se for 0, a constraint será desativada. Você pode definir keyframes na Influence para ligá-la e desligá-la ao longo da animação. Para fazer isso, basta pressionar I enquanto o cursor estiver sobre ***Influence***.

***Child of** no menu de constraint. Faça keyframe da Influence para ligá-la e desligá-la.*


> [!NOTE]
> **💡Dica de animação!** Se você usar o slider para desativar a Influence, o prop não manterá sua posição anterior, tornando difícil manter a animação fluida. Para evitar ter que corrigir a posição manualmente, em vez de usar o slider, clique no X ao lado de Influence, defina um keyframe nela e outro em todos os atributos de transform. Dessa forma, o prop manterá a mesma posição de quando a Influence estava ativa!



> [!NOTE]
> **💡Dica de animação**
> 
> Não deixe o prop visível desde o começo! Para evitar revelar o que está prestes a acontecer e uma transição abrupta, comece a animação com o prop reduzido para 0.001 e só o aumente para 1 quando quiser que ele apareça. Lembre-se de reduzi-lo novamente para 0 ao final da ação. Isso fará com que as transições fiquem muito mais fluidas e legais!


### Animation Slots

O Blender 4.4 introduziu um novo recurso: animation slots. De acordo com a documentação do Blender, “o objetivo dos slots é permitir que uma action armazene dados de animação distintos para múltiplos data-blocks”. Em resumo, os slots tornam possível armazenar a animação de várias coisas na mesma Action. Como isso afeta os emotes 2.0?

Novo recurso do Blender 4.4: animation slots.

Embora seja possível que o avatar e o prop compartilhem o mesmo action clip, por causa da convenção de nomenclatura e do número de clips de animação envolvidos no Emotes 2.0, isso não funcionará. Então, o fluxo para isso seria:

1. Crie um clip de animação para o avatar, ou renomeie o fornecido (***Starting\_Pose***). Ele já tem um animation slot, mas sinta-se à vontade para usá-lo (***Avatar\_Animation***) ou criar um novo.
2. Renomeie o clip de animação ***AnimationName\_Avatar***
3. Crie um clip de animação para o prop e renomeie-o ***AnimationName\_Prop***
4. Clique em ***New*** botão para criar um animation slot para ele (ele receberá um nome automático: ***Armature\_Prop***)
5. Anime como faria em versões anteriores do Blender.

Criando um action clip e um slot para a animação do prop.

## **NLA Tracks**

Para que todas as animações sejam exportadas, os clips devem ser adicionados às NLA Tracks. Certifique-se de que haja apenas um clip de animação para o avatar e outro para o prop, **eles devem ter exatamente o mesmo número de frames.**

No ***Object Mode***, selecione a armature do avatar, vá para ***Pose Mode***, selecione o respectivo clip de animação no menu Browse Action, clique em ***Action*** e depois em ***Push Down*** opção.

Depois, volte para ***Object Mode***, selecione a armature do prop, vá para ***Pose mode***, selecione o respectivo clip de animação no menu Browse Action, clique em ***Action*** e depois em ***Push Down*** opção.

Empurrando ações para as NLA tracks.


> [!WARNING]
> ⚠️ Tenha cuidado ao empurrar ações para baixo. Certifique-se de selecionar a armature desejada com a respectiva animação. Não mude apenas a animação e a empurre para baixo antes de selecionar a outra armature, ou você acabará atribuindo duas actions a uma armature e nenhuma à outra.


As NLA tracks devem ficar assim: uma animação para cada armature.


> [!NOTE]
> **🔥 Dica de otimização**
> 
> **Antes desta etapa, certifique-se de fazer um backup do seu projeto.**
> 
> Se você tiver objetos diferentes para os seus props, pode mesclá-los em uma única mesh. Você pode fazer isso simplesmente selecionando os objetos e pressionando o atalho ctrl + J.
> 
> Isso ajudaria a reduzir os draw calls no jogo, tornando o emote mais performático.
> 
> Lembre-se de que isso não funcionará para particles, no entanto.
> 
> *Selecione os objetos e pressione `Ctrl+J` para mesclá-los.*


## **Exportação**

Os Emotes 2.0 são exportados da mesma forma que os emotes comuns. Certifique-se de que apenas a armature do avatar, a armature do prop e as meshes do prop estejam visíveis e oculte todo o resto.

Deixe visíveis apenas a armature do avatar, a armature do prop e a mesh do prop para exportar.

Para exportar, vá para File > Export > glTF2.0 (.glb, .gltf)

Nas configurações de exportação, expanda Include e, em Limit to, ative Visible Objects. Em seguida, expanda a aba Data, expanda Armature e ative Export Deformation Bones Only.

|   |   |
| - | - |

Clique em Export e pronto!

## **Adicionar áudio aos Emotes**

### Formato e limitações para clips de áudio

* O formato correto para exportar sons para os seus emotes é `.mp3` e `.ogg`.
* O clip de áudio deve ter a mesma duração do emote.
* Embora não haja limite de tamanho para o áudio, o emote com props e sons não pode ser maior que 3 MB.


> [!NOTE]
> **📔 Nota**Se o emote tiver som (mp3 ou ogg), ele deve ser compactado em .zip junto com o .glb. Depois disso, basta arrastar e soltar o .zip no Builder. Mais detalhes podem ser encontrados aqui: [Enviando emote com som](/docs/creator/content-creator-pt/wearables-e-emotes/gerir-colecoes/uploading-emotes/#uploading-emotes-using-a-zip-file)



> [!NOTE]
> **💡 Atenção!** Leve em consideração que os clips de áudio usados no emote devem ser IP original (Propriedade Intelectual), com os direitos de reprodução, e devem seguir os [Content Policy](https://decentraland.org/content/)critérios.


### Edição de sons

Para adicionar sons aos seus emotes, você pode fazer isso de diferentes maneiras:

1. **Edite seus sons diretamente no Blender**

Uma forma de adicionar sons aos seus emotes é usando o editor de Video Sequencer que o Blender oferece.

Para começar a adicionar sons, vá para *Editor Type> Video Sequencer.*

Arraste e solte seus sons na interface dos canais.

Pressione o atalho `N` para ver mais opções para lidar com seus sons, como exibir a waveform, deixar seus sons em Mono ou alterar o volume.


> [!NOTE]
> Se quiser fazer fade in e fade out, basta fazê-lo adicionando keyframes de 0 a 1 e vice-versa à propriedade volume.


Depois de terminar de editar seus sons, você pode exportá-los indo para *Render> Render Audio*. Na opção de exportação, você precisa selecionar o `.mp3` ou `.ogg` formato na *Container* seção e depois *Mixdown*. **Apenas o áudio dentro do intervalo de frames será exportado.**

2. **Renderize a animação e adicione som com um software de edição de áudio**

Embora editar sons diretamente no Blender possa ser conveniente, isso não é muito flexível, porque o software não é focado principalmente em edição de som. As ferramentas disponíveis são bem básicas. Se você quiser dar um toque mais profissional aos seus sons, recomendamos usar um software dedicado de edição de áudio de sua preferência.

Há várias opções de software que você pode usar, como [Audacity](https://www.audacityteam.org/) (gratuito e de código aberto), Adobe Audition, Ableton Live ou ProTools. Usar um software dedicado de edição de áudio oferecerá uma gama muito mais ampla de ferramentas, funcionalidades e efeitos sonoros, permitindo que você aprimore seus sons e dê a eles um aspecto mais profissional.

Para renderizar seu emote, você pode simplesmente adicionar uma câmera à sua cena no Blender e posicioná-la de forma que você consiga ver todos os elementos da maneira mais clara possível para depois ter uma boa referência para adicionar sons.

Ao renderizar um emote, é importante incluir apenas o intervalo de frames do seu emote e nada mais. Escolha uma proporção de aspecto que se adapte às suas necessidades e selecione a pasta de saída onde você quer que o vídeo ou a sequência de imagens seja salva.


> [!NOTE]
> **Dica!**
> 
> *Antes de renderizar, certifique-se de fazer uma renderização com baixa amostragem para economizar tempo no seu render!*


Depois que esta etapa for concluída, use seu vídeo como referência para criar os sons correspondentes usando o software de edição de áudio de sua preferência. **Certifique-se de que a sequência de vídeo corresponda ao framerate da animação de 30 frames por segundo (fps)**


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-pt/wearables-e-emotes/emotes/props-and-sounds.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
