> Saiba como criar um rig para personagens no Decentraland.

# Criar um Rig

Este documento mostra como configurar um rig básico no Blender. Se você quiser se familiarizar mais com alguns conceitos de rigging, confira [O Rig: Conceitos Básicos](/docs/creator/wearables-and-emotes/emotes/avatar-rig/).


> [!NOTE]
> **💡 Dica**: Instala o [plugin Decentraland Tools para Blender](https://extensions.blender.org/add-ons/decentraland-tools/). Inclui várias funções úteis para te ajudar a editar e exportar modelos 3D, wearables e emotes.


### Adicionando a Armature

Antes de tudo, você precisará importar seu modelo 3D para o Blender ou, se já o fez no Blender, basta abrir seu arquivo .blend. No Object Mode, pressione `Shift+A` e selecione ***Armature*** no menu, como mostrado abaixo. Depois, em ***Object Data Properties***, em ***Viewport Display***, ative ***In Front***, para que você possa ver o bone através da mesh. Mesh e Armature devem estar alinhadas, então certifique-se de que o modelo esteja bem posicionado no centro do mundo antes de adicionar os bones. A origem da Armature deve estar em 0,0,0 (X,Y,Z).

![](/docs/_remote/c2e0666e9dda208616b7bea9.webp)

*Adicionando uma Armature e exibindo-a através da mesh.*

### Editando a Armature

Depois de adicionar o objeto Armature, selecione-o e vá para ***Edit Mode***. É aqui que você adiciona os outros bones, extrudindo o original (pressione `E`), duplicando-o (`Shift+D`) ou simplesmente adicionando um novo com `Shift+A`. O skeleton deve seguir a forma do modelo, como um esqueleto real. Bones podem ser escalados, movidos e rotacionados, e têm duas partes: a head e a tail. A head é o ponto de pivot, o que significa que rotações e escala começarão a partir dali. Isso também significa que é aí que ocorrerá a deformação da mesh, então posicione os bones o mais centralizados possível na mesh.

![](/docs/_remote/8512ec54c085799325f2d767.webp)

*Estrutura do bone.*

![](/docs/_remote/edf9b89cb68b376886dd222e.webp)

*Bone girando a partir de seu ponto de pivot.*

Não há necessidade de se preocupar em criar bones para os lados direito e esquerdo por enquanto. Foque primeiro em criar apenas um lado.

### Renomeando os Bones

Algo realmente importante a se ter em mente é renomear corretamente todos os seus bones, de acordo com [a convenção de nomenclatura do Blender](https://docs.blender.org/manual/en/latest/animation/armatures/bones/editing/naming.html). Isso não apenas manterá tudo organizado, mas também tornará possível espelhar poses e weight paint. Você poderia simplesmente renomeá-lo como a parte do corpo que ele representa, como Spine\_01, ou ser mais específico e chamá-lo de DEF\_spine.001 (DEF significa deforming bone, então você sabe que este bone faz parte da hierarchy de deformação).

### Orientação do Bone

Antes de seguir em frente, você deve verificar a orientação do seu bone. Você pode fazer isso indo para ***Object Data Properties*** e, em ***Viewport Display***, ative ***Axes***. Isso tornará os axes dos bones visíveis, para que você possa verificar se eles vão rotacionar na direção correta.

Os axes devem estar alinhados com a direção na qual você quer que os bones girem. Se parecer um pouco errado, selecione o bone e, no ***Item Transform*** menu à direita do viewport, ajuste o ***Roll*** até que a orientação pareça correta.

![](/docs/_remote/e985c26fabd10eaadd09c1d3.webp)

*Corrigindo o bone roll.*

![](/docs/_remote/9737a17ebb3d4b75ab40ecbe.webp)

*Ative Axes para que você possa ver a direção na qual o bone irá girar.*

Corrigir o bone roll é especialmente importante para os dedos, para que todas as articulações dobrem na mesma direção correta. Você não precisa fazer isso em todos os bones de um dedo, basta acertar apenas um deles. Então, selecione primeiro os que você quer corrigir e, por último, o que estiver com o roll correto, vá para ***Armature > Bone Roll > Recalculate Roll > Active Bone*** e todos terão o mesmo roll. Ou você pode simplesmente pressionar `Shift+N` > ***Active Bone***. Isso pode ser usado em muitas situações em que você tem uma longa cadeia de bones, como um tentáculo, uma cauda, um rabo de cavalo longo ou um braço mecânico.

![](/docs/_remote/a781e753f3bb59cd494a2002.webp)

*Pressione `Shift+N` para recalcular o bone roll.*

### Espelhando o Rig

Você não precisa criar bones para os dois lados. Faça apenas um lado primeiro e, quando tiver corrigido toda a nomenclatura e a orientação dos bones, selecione todos os bones que deseja espelhar, clique com o botão direito do mouse e selecione ***Symmetrize***. Dessa forma, todos os bones serão espelhados corretamente, com as orientações certas. Lembre-se de que, para essa ferramenta funcionar, os bones precisam ser nomeados de acordo com a convenção de nomenclatura do Blender. Se houver “L”, “R”, “left” ou “right” em maiúsculas ou minúsculas, o Blender lida corretamente com o correspondente.

Alguns exemplos de convenção de nomenclatura adequada:

* Hand\_L –> Hand\_R
* Hand.L –>Hand.R
* HandLeft –> HandRight

**Todos os exemplos acima são válidos, mas escolha apenas um e mantenha-o consistente em todo o rig.**

![](/docs/_remote/de49fec9cde1c419a8de093f.webp)

*Use a opção Symmetrize para espelhar seus bones.*


> [!NOTE]
> **⚠️ Atenção!** Symmetrize só funcionará se os bones forem renomeados corretamente. Se ele não espelhar corretamente, talvez você queira conferir novamente os nomes dos bones.


### Skinning

Skinning é o processo de vincular a mesh à Armature. Para isso, em ***Object Mode***, selecione a mesh, depois a Armature e pressione `CTRL+P` > ***Armature Deform > With Automatic Weights.*** Depois, vá para ***Pose Mode*** e teste diferentes poses para testar a deformação da mesh. Na maioria das vezes, serão necessários alguns ajustes, como visto abaixo.

![](/docs/_remote/6b45f045f5eacbbaf6f518ce.webp)

*Para o Skinning, torne a mesh filha da armature pressionando `CTRL+P` e teste-a em Pose Mode.*

Se você achar que um bone foi posicionado de forma incorreta ou que o ponto de pivot não está preciso, você sempre pode voltar para o ***Edit Mode*** da armature e ajustá-lo. Isso não afetará o skinning em nada, e você sempre pode usar ***Symmetrize*** para espelhar as mudanças.

Para corrigir deformações estranhas, vá para ***Object Mode***, selecione a mesh e vá para ***Weight Paint***. Em ***Object Data Properties*** você encontrará os ***Vertex Groups***.

![](/docs/_remote/a2dbfb8ab2995b61fc35c031.webp)

*Edite a influência de um bone em Weight Paint.*

Antes de começar a editar, para que seu weight paint seja espelhado automaticamente enquanto você pinta, ative ***Mirror Vertex Groups*** e selecione ***X*** em ***Mirror*** em ***Symmetry*** (no Blender 3.4.0) Para editar as influências, selecione o vertex group que você quer e, no ***Tool*** menu, em ***Blend***, alterne entre ***Adicionar*** e ***Subtract*** de acordo com as suas necessidades. Você também pode alterar o tamanho, peso e força do brush.

![](/docs/_remote/3a6194f2b0653962efb783c3.webp)

*Altere o Blend para adicionar ou remover influência. Certifique-se de que a Symmetry esteja correta.*

![](/docs/_remote/7787147101d32c50c282e5c6.webp)

Use o seletor ***Blur*** à esquerda da tela para suavizar o weight paint e deixá-lo mais uniforme.

![](/docs/_remote/94e217fd4c7d693055d4cc02.webp)

*Ferramenta Blur.*

### Configurando o IK

IK é essencial quando você quer que algo permaneça no lugar. O melhor exemplo de seu uso é nas pernas, mas ele pode ser adotado em uma variedade de situações, como nos exemplos abaixo:

![](/docs/_remote/ae34643f4671eb5037f61506.webp)

*Uso de IK em um braço mecânico.*

![](/docs/_remote/af572bd577d62ebeb0784242.webp)

*Uso de IK em cabos.*

Como isso mudará a hierarquia dos bones, é melhor mantê-lo como uma configuração separada. Então, a primeira coisa que você precisa fazer é duplicar a cadeia de bones que será afetada pelo IK. Vamos usar a perna como exemplo. Selecione os bones e pressione `Shift+D` para duplicá-los e movê-los para uma layer diferente pressionando `M` e selecionando outro slot para isso.

Lembre-se de renomear os bones duplicados, adicionando IK ao nome para que você saiba que eles fazem parte da configuração de IK. E como eles não devem deformar a mesh, selecione todos eles e, em ***Bone Property***, pressione e segure `Alt` e desmarque ***Deform***.

![](/docs/_remote/3b93713349f793f06d7f6bcb.webp)

*Duplique os bones e mova-os para uma layer diferente.*

![](/docs/_remote/fb47b25da4bfe5bf1f79c139.webp)

*Selecione todos os bones de IK e, enquanto segura `Alt`, desmarque Deform.*

Em seguida, você precisará criar um bone que conduzirá a cadeia de IK. Selecione a head do bone do pé e extrude-o no eixo Y. Depois, pressione `Alt+P` para desparentá-lo, porque o bone de IK não pode fazer parte da cadeia e não pode ser conectado a outros bones. Você precisará tornar o pé um child do IK, então selecione primeiro o bone do pé e o bone de IK por último e pressione `CTRL+P` > ***Manter Offset***.

![](/docs/_remote/2538a5aac86c9d53322d27e3.webp)

*Criando um bone de IK.*

Em ***Pose Mode***, clique no bone da canela, pressione `CTRL+Shift+C` e selecione ***Inverse Kinematics***. Vai parecer tudo bagunçado, mas não se preocupe, isso será corrigido assim que você mudar algumas configurações. Com a canela selecionada, vá para as ***Bone Constraint Properties*** como mostrado abaixo.

![](/docs/_remote/25208beb5685cd2437384baa.webp)

*Em Bone Constraint Properties você pode editar as configurações de IK.*

Para o ***Target***, selecione ***Armature***. Depois disso, uma opção chamada ***Bone*** aparecerá em Target. Para isso, selecione o bone de IK que você criou. Em ***Chain Length*** você definirá o número de bones que devem ser afetados pelo IK. Como estamos fazendo uma perna, queremos que ele afete a canela e a coxa, então defina-o para ***2***.

### Pole Target

O Pole Target permite controlar a direção em que os bones vão dobrar. Estes são os passos para adicionar corretamente um pole target (ou pole vector) à cadeia de IK:

* Em ***Edit Mode***, duplique o bone da coxa e coloque-o em qualquer lugar.
* Selecione a articulação do joelho (ou a tail do bone da coxa) e pressione `Shift+S` > ***Cursor to Selected.***
* Com o bone da coxa duplicado selecionado, pressione `Shift+S` > ***Selection to Cursor***.
* Em ***Transform Orientation***, altere para ***Normal*** e em ***Tansform Pivot Point*** altere para ***Active Element***.
* Rotacione o bone em X -90 (ou 90, dependendo da orientação que você definiu) para que ele aponte para frente e mova-o em Y até que fique em uma boa posição à frente da perna. Você também pode reduzi-lo um pouco.

![](/docs/_remote/9eb9391b9e9f16ea7b21d708.webp)

*Criando um Pole Target.*

De volta a ***Pose Mode***, selecione a canela novamente e vá para o ***Bone Constraint Properties***. Para ***Pole Target*** selecione ***Armature*** e para ***Bone***, selecione o pole target que você acabou de criar. Se você mostrar os bones de deformação, verá que o pole target girou um pouco o IK.

![](/docs/_remote/a7dfd226eea1cbdac586bafa.webp)

*A cadeia de IK e os bones de deformação não estão mais alinhados por causa do pole target.*

![](/docs/_remote/4c1a015b3348c877c0b7412e.webp)

*A rotação pode ser corrigida alterando o Pole Angle.*

Isso pode ser facilmente corrigido alterando o ***Pole Angle***. Normalmente, -90° resolve, mas você sempre pode ajustá-lo manualmente para garantir que fiquem perfeitamente alinhados.

Por fim, torne tanto o bone de IK quanto o pole target filhos do bone raiz, selecionando ambos e depois a raiz, pressione `CTRL+P` > ***Manter Offset***.

Mova a coxa e a canela para outra layer, pois você não precisará delas para a animação; elas são apenas parte da configuração de IK.

### Vinculando Bones de Deformação a Bones Não Deformantes

A cadeia de IK está toda configurada, mas ela deveria controlar os deform bones e, no momento, isso não está acontecendo; porém, você pode usar constraints para corrigir isso. Em ***Pose Mode***, selecione primeiro um bone da cadeia de IK e, por último, o bone de deformação correspondente, pressione `CTRL+Shift+C` e selecione ***Copy Transforms***. Faça isso para todos os bones, que no exemplo são coxa, canela, pé e dedos. Os bones de deformação terão uma cor verde, o que significa que eles têm um constraint. Se você clicar em ***Bone Constraint Properties***, você pode ver qual constraint está sendo usada e qual bone a está controlando.

![](/docs/_remote/6b79927002f5fe0563c8468a.webp)

*Bones verdes têm constraints. Você pode verificá-las em Bone Constraints Properties.*


> [!NOTE]
> **💡 Dica**: Você não precisa configurar o IK do zero para o outro lado. Em *Edit Mode*, basta apagar todos os bones do lado que não tem o IK, depois selecione todos os bones de deformação e a cadeia de IK que você quer espelhar, clique com o botão direito do mouse e selecione *Symmetrize*. Isso não apenas espelhará os bones, mas também todos os constraints!


![](/docs/_remote/657c90b8853090a842847bce.webp)

*Use a opção Symmetrize para espelhar constraints também!*

### Esqueleto Não Deformante e Controles\*\*

É uma boa prática não animar diretamente os bones de deformação, pois você pode acabar quebrando o rig, e adicionar constraints à Armature de deformação afetará a hierarquia e o comportamento dos bones quando exportados.

A solução para isso é criar um esqueleto não deformante que controlará o esqueleto deformante por meio de constraints, e que possa ser animado com segurança, sem o risco de quebrar nada. Você também pode personalizar sua forma para facilitar a identificação da função de um bone. Eles serão os controles do seu rig.

O processo para isso é praticamente o mesmo feito para a configuração de IK. Duplique todos os bones (exceto a configuração de IK) com `Shift+D` e mova-os para uma layer diferente pressionando `M` e escolhendo outro slot. Renomeie todos eles adicionando **Control\_** ou **CTRL\_** como sufixo, para que você saiba que eles fazem parte da hierarchy de controles. Pressione `Um` para selecioná-los todos e, em ***Bone Property***, pressione e segure `Alt` e desmarque ***Deform***.

Agora você precisará adicionar constraints para vincular os dois skeletons. Para tornar esse processo mais fácil, você pode mudar a forma do bone clicando em ***Object Data Properties*** e, em ***Viewport Display***, altere ***Display As*** de ***Octahedral*** como ***B-Bone***. Depois, pressione `CTRL+Alt+S` para ampliar um pouco os bones, para que eles fiquem maiores que os originais.

![](/docs/_remote/99d8f893af1a418a6af220a9.webp)

*Altere a forma dos bones e amplie-os para que seja mais fácil diferenciar os dois skeletons.*

Para os constraints, selecione primeiro um bone de controle e, por último, o bone de deformação correspondente, pressione `CTRL+Shift+C` e selecione ***Copy Transforms***. Faça isso para todos os bones de controle.

Você pode separar os bones de controle em diferentes grupos e atribuir cores a eles. Vá para ***Pose Mode*** > ***Object Data Properties*** > ***Bone Groups***. Clique no ***+*** ícone para adicionar um novo grupo, renomeie-o como achar melhor e selecione uma cor para ele. Depois, selecione os bones que você quer que façam parte desse grupo e clique em ***Assign***. Você pode criar quantos grupos quiser para manter tudo organizado. Você também pode mover diferentes bone groups para layers diferentes.

![](/docs/_remote/d70aff06bf4143e4711b47e3.webp)

*Bone groups ajudarão você a manter seu rig organizado e mais intuitivo.*

Uma forma extra de melhorar seu rig é personalizar a forma dos bones. Para isso, vá para ***Object Mode***, pressiona `Shift+A` para adicionar uma mesh, como um círculo, por exemplo. Para manter tudo organizado, crie uma collection para suas formas e mova para lá o círculo que você acabou de criar. Volte para Pose Mode e, em ***Object Data Properties*** > ***Viewport Display*** marque ***Shapes***. Selecione o bone desejado e depois, em ***Bone Properties***, clique em ***Viewport Display*** > ***Custom Shape***. Em ***Custom object***, selecione o círculo no outliner.

![](/docs/_remote/a712323c80495b4deba01b97.webp)

*Personalizando a forma do bone.*

Se a forma parecer estranha, você sempre pode editá-la em ***Edit Mode***, como mostrado abaixo. Certifique-se de que a forma tenha a mesma orientação do bone.

![](/docs/_remote/6ea2f3c66b5674c66248f2bc.webp)

*Edite a forma em Edit Mode.*

Crie formas diferentes para diferentes controles de acordo com sua função para tornar seu rig mais intuitivo.

### Spring Bones

Além do skeleton de deformação padrão, wearables podem incluir **spring bones** — bones extras cujos transforms são conduzidos pela simulação física, e não pela animação. Spring bones fazem cabelo, brincos, capas e outros acessórios se moverem dinamicamente em resposta ao movimento do avatar e à gravidade.

Para adicionar spring bones a um rig:

1. Em **Edit Mode**, extrude novos bones a partir do bone pai apropriado (por exemplo, `Avatar_Head` para cabelo).
2. Nomeie cada bone incluindo `springbone` no nome (sem distinção entre maiúsculas e minúsculas), por exemplo: `SpringBone_hair_left`, `springbone_hair_left_01`, `Hair_SpringBone_left_tip` são nomes válidos, desde que incluam `springbone`.
3. Compilar **linear chains** de pelo menos **2 bones** (um root e uma tip). Um único spring bone, sozinho, não produzirá nenhum movimento — a simulação precisa de uma cadeia com um bone raiz que conduza a física e um bone tip que defina o ponto final. Cada bone na cadeia deve ter no máximo um child spring bone.
4. Faça skin weight da mesh nos spring bones como de costume.
5. Exporte como `.glb` — os spring bones serão incluídos na Armature.

Os parâmetros de física (stiffness, gravity, drag) são configurados no Builder depois do upload, não no Blender. Para o guia completo, veja [Spring Bones](/docs/creator/wearables-and-emotes/wearables/spring-bones/).

E é isso! Com todas essas informações, você está pronto para fazer um rig básico!


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-pt/modelacao-3d-e-animacoes/create-a-rig.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
