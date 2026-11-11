> Dicas e diretrizes para criar Emotes no Decentraland.

# Criando Emotes

Esta documentação abordará as especificações do arquivo, o básico de animação no Blender, a maneira correta de exportar um Emote e como importar um no Builder.


> [!NOTE]
> **💡 Dica**: Instale o [plugin Decentraland Tools Blender](https://extensions.blender.org/add-ons/decentraland-tools/). Ele inclui várias funções úteis para ajudar você a editar e exportar modelos 3D, wearables e emotes.


#### Tabela de Especificações de Animação

| Taxa de quadros              | 30 fps                                  |
| ---------------------------- | --------------------------------------- |
| Comprimento máximo           | 10 segundos (300 frames)                |
| Animações por arquivo        | 1                                       |
| Formato de exportação        | .glb                                    |
| Taxa de amostragem           | 1 por padrão (2 ou 3, se necessário)    |
| Tamanho máximo do arquivo    | 3 MB                                    |
| Distância máxima da animação | 1 metro (frente/trás, esquerda/direita) |
| Altura máxima da animação    | 4 metros                                |

Você pode encontrar uma explicação mais detalhada das especificações de animação [**abaixo**](#the-animation-specifications).

### **Recursos**

Esta documentação explica a configuração do Rig 1.0, seus controles e recursos.

[Decentraland Blender Rig](https://github.com/decentraland/docs/tree/main/creator/images/emotes/Avatar_File.blend)


> [!NOTE]
> Se você estiver usando Maya, você pode baixar este [Maya Rig](https://github.com/decentraland/docs/tree/main/creator/images/emotes/DCL_Maya_Rig.ma) e [picker](https://github.com/decentraland/docs/tree/main/creator/images/emotes/emoteAvatar.pkr) fornecido por [SparkleStudios](https://www.sparkles.studio/) ❤️.


## **Antes de começar**

### **Taxa de quadros**

Antes de começar, é importante verificar a taxa de quadros. As animações do Decentraland devem ter uma taxa de quadros de 30 fps. O arquivo do rig vem configurado com 24 fps, então altere para 30 antes de animar (uma taxa de quadros incorreta afetará a velocidade da animação). Essa opção pode ser encontrada em Output Properties (o ícone da impressora), em Format, como mostrado abaixo:

![Certifique-se de que a taxa de quadros esteja definida para 30 fps antes de começar.](/docs/_remote/53650a16525c3ac92e8eef4c.webp)

Certifique-se de que a taxa de quadros esteja definida para 30 fps antes de começar.

### **Pose Mode**

No Blender, um rig pode ser visualizado em três modos diferentes: Object Mode, Edit Mode e Pose Mode. As animações só podem ser feitas em Pose Mode (nesse modo, os controles têm cores). Com o rig selecionado, você encontrará essa opção em um menu suspenso, no canto superior direito.

![Mudando para Pose Mode.](/docs/_remote/2d50e8d239676da8c48b0736.webp)

Mudando para Pose Mode.

### **Interface para animações**

No arquivo do rig, além das duas janelas para o viewport (vista frontal e lateral), há mais três na parte inferior: um ***Graph Editor***, ***um Dope Sheet***, e uma ***Timeline***.

* ***Graph Editor***: Neste editor, é possível editar as curvas de animação de cada propriedade de transform dos controles selecionados. Essas curvas mostram como a interpolação está sendo calculada e podem ser editadas para alcançar o efeito desejado na animação. Tanto aqui quanto no dope sheet, a ferramenta ***Only Show Selected*** está ativada, o que significa que ela incluirá apenas canais relacionados ao controle selecionado. Isso pode ser ativado e desativado simplesmente clicando no ícone de seta.
* ***Dope Sheet***: Aqui você pode editar os keyframes. É também aqui que você pode criar novas animações ou navegar entre as várias que foram criadas. Lembre-se de que, para ter acesso à animação, o ***Action Editor*** deve estar selecionado. Essa opção fica logo ao lado do *Dope Sheet* ícone, em um menu suspenso.
* ***Timeline***: É aqui que a timeline e os controles de reprodução são encontrados. Aqui, o ***Auto Keying*** está ativado, o que significa que toda vez que um controle é manipulado ele cria automaticamente um keyframe. Você sempre pode desativar essa função clicando no ponto ao lado dos controles de reprodução.

Com este workspace, você tem tudo o que precisa para começar a animar!

![](/docs/_remote/4dfc916aa7bf8bcc00b1659f.webp)

Estas são as janelas inferiores. A de cima está no ***Graph Editor,*** a do meio no ***Dope Sheet,*** e a de baixo é a ***Timeline.*** A seta vermelha de cima mostra a ***Only Show Selected*** ferramenta e a de baixo mostra o ***Auto Keying***.


> [!NOTE]
> **💡 Dica!**
> 
> Como o Blender é altamente personalizável, este também é um bom momento para configurar o layout que melhor se adapta a você, adicionando, ajustando ou removendo janelas. Cada animador tem suas próprias preferências, então sinta-se à vontade para editar o layout como quiser!


## Começando

#### **Pose inicial**

No arquivo do rig fornecido, já existe uma action, a ***Starting\_Pose***. Considerando que todas as ações do avatar começam a partir da pose idle, **nós realmente recomendamos começar sua animação a partir dessa pose e também usá-la novamente no último frame**. Isso fará com que a transição de Idle para Emote seja melhor e a animação fique mais fluida.


> [!NOTE]
> **💡 Dica!**
> 
> Se você quiser fazer uma animação em loop, não precisa começar a animação a partir da Starting Pose. Sinta-se à vontade para usar a pose que fizer mais sentido na sua animação!\*\*


Para que um loop funcione de forma suave, o primeiro e o último keyframe devem ser idênticos. Por exemplo, um loop de 4 segundos a 30 fps precisa da mesma pose no frame 1 e no frame 121. Se eles diferirem em qualquer grau, os jogadores verão um salto visível toda vez que o clip repetir.

#### Área de animação <a href="#ground-reference-and-animation-area" id="ground-reference-and-animation-area"></a>

No arquivo de avatar fornecido, você encontrará uma coleção chamada ***Animation\_Area\_Reference***. Ela consiste em três objetos que ajudarão visualmente a definir os limites da animação: ***Animation\_Area\_Reference***, ***Ground\_Reference*** e ***Root\_Animation\_Area***.

* Animation\_Area\_Reference é um cubo 4x4, que é a bounding box. A mesh do avatar e os props devem permanecer dentro dessa área durante toda a animação. Os props podem ser movidos livremente, desde que permaneçam dentro da área do cubo.
* Ground\_Reference é um plano fornecido para ajudar a verificar se há penetração no chão durante a animação. Ele também contém informações importantes: o círculo menor, que tem um diâmetro de 2 metros, define a área onde o root do avatar pode ser movido. Isso significa que, enquanto estiver no chão, o avatar só pode se mover 1 metro em cada direção: frente, trás, esquerda e direita.
* Root\_Animation\_Area é um cilindro que define a área onde você pode mover o root/centro de gravidade do avatar (CTRL\_Avatar\_UpperBody ou Avatar\_Hips). O cilindro tem 4 metros de altura, o que significa que o root pode subir até o topo, desde que nenhuma mesh esteja fora da bounding box. As pernas, braços e outras partes do corpo do avatar podem ficar fora do cilindro, desde que nunca saiam de fora de Animation\_Area\_Reference.

![Referência da área de animação.](/docs/_remote/8ef56331d4f29f23223dc576.webp)

Em resumo, o avatar pode se mover para a direita, esquerda, frente e trás, desde que o root permaneça dentro do cilindro, o que significa 1 metro em qualquer uma das quatro direções.

![Movimento do avatar.](/docs/_remote/7949b480b39476fbab88c60f.webp)

Quanto à altura, ele pode subir até o máximo (4 metros), desde que o avatar permaneça dentro da bounding box. Abaixo estão alguns casos de altura do avatar feitos corretamente (corpo inteiro dentro da bounding box) e outros feitos incorretamente (mesh fora da bounding box).

| ![](/docs/_remote/cbee2350215541a459589276.webp) | ![](/docs/_remote/4774d078d1fff2f442dda8f4.webp) |
| -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |

| ![](/docs/_remote/d51139e66aeafc6d74965579.webp) | ![](/docs/_remote/93bbdf9a30346096af762dea.webp) |
| -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |

![](/docs/_remote/da31c95fa83a05f64398f245.webp)

Aqui estão alguns exemplos de emotes que estão dentro dos limites da área de animação.

| ![](/docs/_remote/989fc61d99ce5203d6197a0e.webp) | ![](/docs/_remote/aa48c499a9817ab4725cfe89.webp) |
| ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |


> [!NOTE]
> **💡 Atenção!**
> 
> Cuidado com esses limites, porque ultrapassá-los pode causar problemas de gameplay.


## **Criando uma animação**

O arquivo blend já possui um clip de animação pronto para ser editado: *StartingPose\_Avatar*. Você pode duplicar e renomear esse clip de animação como quiser. Não há necessidade de criar um do zero!

Na seção *Browse Action* basta clicar no botão ***Create A New Action*** para duplicar a animação atual. Para renomear o clip, basta clicar no texto e digitar outro nome.

O Blender 4.4 introduziu *Slotted Actions*, o ícone à direita da seção *Browse Action* das versões anteriores. Não há necessidade de mexer nisso se você estiver criando um emote sem prop, então você pode simplesmente deixá-lo como está. Se você estiver animando o avatar, certifique-se de que a slotted action seja Avatar\_Animation.

![](/docs/_remote/75912c834d5cbe632654a9f0.webp)

Crie uma nova animação duplicando a existente ou clicando em ***Unlink Action*** e então ***New***.

### **Navegando e excluindo animações**

No Blender, você pode ter múltiplas trilhas de animação no mesmo arquivo. É possível navegar por elas clicando no menu suspenso Browse Action. Toda animação com F (Fake User) será salva. Para excluir uma animação, pressione Shift no teclado e clique no X. Depois disso, a animação mostrará um 0 ao lado, o que significa que ela será excluída na próxima vez que você fechar o Blender ou reabrir o arquivo.

![](/docs/_remote/cf72bdeab774263920c04bb9.webp)

Navegando pelas animações: As que tiverem um F serão salvas, e as que tiverem 0 serão excluídas.

Outra maneira de excluir animações sem precisar recarregar o Blender é alterando o Display Mode de View Layer para Blender File. Expanda Actions e exclua qualquer animação indesejada clicando com o botão direito nelas e selecionando Delete.

![](/docs/_remote/3e2be89fb22f3ee63cc39ef1.webp)

Você pode excluir animações diretamente de Blender File em Display Mode no outliner.


> [!NOTE]
> **💡 Dica!**
> 
> Não edite sempre a mesma trilha de animação. Antes de fazer mudanças importantes, basta duplicar a animação. Assim, você terá uma versão de backup caso se arrependa de deletar ou alterar algo. Esta também é uma boa maneira de acompanhar o progresso feito até agora!


![](/docs/_remote/75912c834d5cbe632654a9f0.webp)

Duplicando clips de animação.

### **Nomeação**

**O nome de uma animação deve começar com letra maiúscula e, se tiver mais de uma palavra, as palavras devem ser separadas por \_.** Não use espaços nem caracteres especiais. Aqui estão alguns exemplos de nomeação:

* Snowfall
* Rainbow\_Dance
* Throw\_Money
* Talk\_To\_Hand

### **Substituições de Emote**

Substituições de Emote acontecem quando os deform bones não têm um keyframe definido em um dos parâmetros. Sem um keyframe, esse osso não terá a informação de onde deve estar, quanto foi rotacionado e escalado, deixando esse canal aberto. A consequência é que, se você reproduzir um emote no world e depois disparar o seu enquanto o anterior ainda estiver tocando, a informação de location, rotation e scale será substituída pelo emote anterior, o que causará uma combinação dos dois. A menos que isso seja feito de propósito, isso afetará sua animação, às vezes com um resultado divertido, mas outras vezes deixando o emote completamente bagunçado. Abaixo está um exemplo de substituição de emote.

![](/docs/_remote/4d3cf50ba093eedb38f43aee.webp)

Para evitar isso, selecione todas as layers com bones nelas (que podem ser encontradas em ***Object Data Properties*** > ***Skeleton*** > ***Layers***). Então, em ***Pose Mode***, deixe o cursor da timeline no primeiro frame da sua animação e, com o mouse em ***Viewport Display***, pressione ***A*** para selecionar tudo. Em ***Graph Editor***, clique duas vezes no ícone ***Eye*** ao lado do canal da armature para tornar todos os canais visíveis. Com todos os bones selecionados, pressione ***I*** para definir um keyframe. Faça o mesmo para o último frame.

**Certifique-se de selecionar os deform bones, isso é especialmente importante!** Os deform bones podem ser encontrados na última layer inferior e são mostrados como bones verdes no ***Viewport***.

![](/docs/_remote/359c9a9a72aea9590225e2c7.webp) Definir keyframes em todos os bones nos primeiros e últimos frames evita substituições de emote.

## **As Especificações de Animação**

### **O Comprimento da Animação**

O comprimento máximo de uma animação é **10 segundos** ou **300 frames**. Lembre-se de definir keyframes para as propriedades de cada controle nos primeiros e últimos frames.


> [!WARNING]
> ⚠️ Canais com a visibilidade desativada no Graph Editor não serão keyframed, excluídos ou sequer mostrados no Action Editor. A menos que isso tenha sido feito intencionalmente, preste atenção redobrada à visibilidade.


![](/docs/_remote/91ee3dde0d4a300d0eb09a66.webp)

Torne os canais visíveis antes de definir keyframes!

### **Número de animações**

Se for um emote padrão (sem prop), o arquivo exportado só pode conter uma animação. Para emotes 2.0, você pode ter um clip para o avatar e um clip para o prop. Se animações foram duplicadas durante o processo, certifique-se de excluí-las todas antes de exportar. Mantenha apenas a versão final. Emotes sequenciais que precisam de muitas animações para funcionar (action start, action loop e action end) não são suportados no momento.

### **Formato**

As animações devem ser exportadas como .**GLB**. O arquivo pode conter apenas o esqueleto deformador e a animação. **Mesh, controles e qualquer outro objeto não devem ser exportados**. Mais detalhes sobre como exportar podem ser encontrados [**abaixo**](#exporting).

### **Amostragem**

Como constraints não podem ser exportadas, a única maneira de exportar o clip de animação é fazendo bake dele, o que significa que todas as posições, rotações e escalas dos deform bones serão keyframed em cada frame da animação. Se o clip for muito longo, como até 300 frames, ele terá 300 keyframes após a exportação e, quanto mais keyframes tiver, mais pesado o arquivo ficará.

A amostragem é uma boa maneira de otimizar a animação. A taxa de amostragem definirá com que frequência um keyframe será baked na animação. Por exemplo, se a taxa de amostragem estiver definida como 2, isso significa que um keyframe será criado a cada dois frames. Uma taxa de amostragem de 3 fará bake de um keyframe a cada três frames e assim por diante. Quanto maior a taxa de amostragem, mais leve o arquivo.

A desvantagem, porém, é que a animação começará a ficar cada vez menos fluida, já que perde alguns keyframes importantes (eles são distribuídos pela animação de forma irregular). Também é importante notar que **a amostragem NÃO é dividir o número de frames da animação pela taxa de amostragem**.

Normalmente, uma **taxa de amostragem de 2 ou 3** dará conta do recado. Esses números podem otimizar a animação sem comprometer a qualidade.


> [!NOTE]
> **💡 Dica!**
> 
> Se o número de frames da animação puder ser dividido pela taxa de amostragem, isso é ótimo! Significa que o frame final será baked, preservando a transição do fim para o começo da animação.


### **Tamanho do arquivo**

O tamanho máximo do arquivo é **3 MB**. Se o arquivo ficar acima disso após a exportação, tente verificar se a mesh não foi exportada por acidente ou se a animação não passa de 10 segundos. Se ainda estiver acima de 3 MB, tente experimentar com a Taxa de amostragem, pois valores mais altos melhorarão a otimização.

Se o emote contiver quaisquer modelos 3D adicionais, as texturas nesses modelos não podem exceder o tamanho de 1024 pixels.

## **Exportando**

Como queremos que apenas a armature e a animação sejam exportadas, desative a visibilidade da mesh e de qualquer objeto que não seja a armature antes de exportar, como mostrado abaixo:

![](/docs/_remote/3b011135dba6d9de9e62a101.webp)

Desative a visibilidade da mesh antes de exportar!

Seu arquivo também deve conter apenas a única action que você está exportando. O rig vem com uma `Starting_Pose` action, então [exclua-a](#browsing-and-deleting-animations) juntamente com qualquer outra action restante, caso contrário o exportado `.glb` carrega dois clips em vez de [uma animação por arquivo](#number-of-animations) que o Decentraland espera. Salve uma cópia editável do seu `.blend` antes de remover qualquer coisa.

Para exportar, vá para *File* > *Export* > *glTF2.0 (.glb, .gltf)*

![](/docs/_remote/3142125b130d9e9715dddd3d.webp)

Nas configurações de exportação, expanda Include e em Limit to ative Visible Objects. Em seguida, expanda a aba Data, expanda Armature e habilite Export Deformation Bones Only.

| ![](/docs/_remote/446d202239ea82992946aa43.webp) | ![](/docs/_remote/eef0447fdcd598e5153a2669.webp) |
| --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |

Se você precisar amostrar a animação, expanda a aba Animation, expanda Sampling Animations e escolha o número de samples desejado.

| ![](/docs/_remote/b5ee46c688eb78bff045e6a1.webp) | ![](/docs/_remote/6813e1cd152caee20fe515fb.webp) |
| ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |

É isso para exportar a animação!

## Referências

Se você ainda não tiver certeza de por onde começar ou precisar de alguma referência ou inspiração, aqui estão alguns clips de animação para ajudar você com isso. Eles podem ser um ótimo material de estudo!

[Idle.glb](https://raw.githubusercontent.com/decentraland/documentation-creators/main/images/emotes/idle.glb)

[Jump.glb](https://raw.githubusercontent.com/decentraland/documentation-creators/main/images/emotes/jump.glb)

[Walk.glb](https://raw.githubusercontent.com/decentraland/documentation-creators/main/images/emotes/walk.glb)

[Run.glb](https://raw.githubusercontent.com/decentraland/documentation-creators/main/images/emotes/run.glb)

[Pose\_Jump.glb](https://raw.githubusercontent.com/decentraland/documentation-creators/main/images/emotes/pose_jump.glb)

[Pose\_Spin.glb](https://raw.githubusercontent.com/decentraland/documentation-creators/main/images/emotes/pose_spin.glb)

[Spotlight.glb](https://raw.githubusercontent.com/decentraland/documentation-creators/main/images/emotes/spotlight.glb)

[Fashionista.glb](https://raw.githubusercontent.com/decentraland/documentation-creators/main/images/emotes/fashionista.glb)

[Chic.glb](https://raw.githubusercontent.com/decentraland/documentation-creators/main/images/emotes/chic.glb)

[Flag\_Emote.glb](https://github.com/decentraland/docs/tree/main/creator/images/emotes/Flag_Emote.glb)

[Flag\_Emote.blend](https://github.com/decentraland/docs/tree/main/creator/images/emotes/Flag_Emote_Final.blend)


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-pt/wearables-e-emotes/emotes/creating-emotes.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
