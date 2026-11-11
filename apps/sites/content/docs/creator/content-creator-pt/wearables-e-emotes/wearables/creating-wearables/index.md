> Dicas e diretrizes para criar Wearables do Decentraland

# Criando Wearables

![](/docs/_remote/a3778599667aedc8c1fc29ed.webp)

## Introdução

Este guia apresenta o básico para criar modelos 3D personalizados para Wearables da Decentraland. Ele explica como o sistema de avatar da Decentraland funciona e ilustra como modelar corretamente seus próprios Wearables.

*Nota: este guia pressupõe que você já tenha algum conhecimento básico a intermediário de modelagem 3D. Se você é novo em modelagem 3D,* [*comece aqui*](/docs/creator/3d-modeling/3d-models/)*.*


> [!NOTE]
> **💡 Dica**: Instale o [plugin Blender Decentraland Tools](https://extensions.blender.org/add-ons/decentraland-tools/). Ele inclui várias funções úteis para ajudar você a editar e exportar modelos 3D, Wearables e emotes.


Antes de começar, baixe os arquivos de exemplo para referência de meshes e texturas: [**Modelos de referência de Wearables**](https://drive.google.com/drive/u/1/folders/12hOVgZsLriBuutoqGkIYEByJF8bA-rAU)

## O Sistema de Avatar da Decentraland

O "sistema de avatar" da Decentraland é o amplo conjunto de diferentes componentes e subcomponentes do corpo que podem ser decorados com Wearables personalizados. Esses componentes são:

* Forma do corpo
* Cabeça
  * Forma da cabeça
  * Sobrancelhas
  * Olhos
  * Boca
* Parte superior do corpo
* Parte inferior do corpo
* Handwear
* Pés
* Acessórios

### **Forma base do corpo**

Depois de baixar o arquivo de exemplo do avatar base, carregue o modelo no seu editor 3D, como o Blender.

Você vai notar que cada modelo contém 8 meshes diferentes relacionadas a uma armature. Essas meshes representam a cabeça, as sobrancelhas, os olhos, a boca, a parte superior do corpo, a parte inferior do corpo, as mãos e os pés. Você pode usar esses modelos de exemplo como referência e ponto de partida para o seu próprio Wearable personalizado.

Atualmente, há duas formas de corpo: A ou B. A Forma A (`AvatarShape_A.blend`) é o **corpo feminino** usado para a `BaseFemale` representação. A Forma B (`AvatarShape_B.blend`) é o **corpo masculino** usado para `BaseMale`. Os nomes dos arquivos não deixam isso óbvio, então verifique com atenção antes de construir: um Wearable que se ajusta ao corpo, criado no arquivo errado, acaba com suas duas representações trocadas.

![](/docs/_remote/15019e8948fa233ffdf5f7bb.webp)

### **Cabeça**

![](/docs/_remote/1108d14609f042d64adb1c53.webp)

*A cabeça base inclui diferentes meshes anexadas que podem ser personalizadas como Wearables: Sobrancelhas, Olhos e Boca funcionam como máscaras de transparência renderizadas na frente do rosto.*

### **Parte superior do corpo**

![](/docs/_remote/9ce8e60a3286071a888c6bd9.webp)

*A parte superior do corpo, ou torso, de um avatar. Não inclui as mãos.*

### **Parte inferior do corpo**

![](/docs/_remote/f12c5382a0abf31077a0dfda.webp)

*A parte inferior do corpo inclui a pelve e as pernas de um avatar.*

### **Mãos**

![](/docs/_remote/c61c01c445051846535b37fd.webp)

*As mãos são as mesmas para as Formas A e B e começam nos punhos de um avatar.*

### **Pés**

![](/docs/_remote/4e09b05d153e3455e46bc344.webp)

*Os pés incluem tornozelos e o pé.*


> [!WARNING]
> **Importante: não modifique os vértices de "cortes/costuras" entre a cabeça, a parte superior e a parte inferior do corpo.**


Cada parte do corpo tem tampas, deixando-as "estanque". Essas tampas existem para evitar falhas visuais desagradáveis caso haja problemas de clipping de animação devido a uma má distribuição de skinning. O ideal é não remover essas tampas ao editar a mesh.

![](/docs/_remote/6d31c3917522878384fcbbf7.webp)

## Construindo Modelos 3D para Wearables

#### **Limitações de tris, materiais e texturas**

Para garantir que a Decentraland funcione sem problemas para todos os jogadores, é importante criar modelos de Wearables sem usar triângulos, materiais e texturas em excesso. O objetivo é manter os modelos 3D o mais simples possível para que possam ser renderizados facilmente, sem sacrificar demais o nível de detalhe.

Existem limites para a quantidade de triângulos e texturas que podem ser usados em cada Wearable ou acessório:

* No máximo 1,5K triângulos por slot de Wearable: chapéu, capacete, parte superior do corpo, parte inferior do corpo, pés e cabelo.
* No máximo 500 triângulos por categoria: mask, eyewear, earring, tiara, top\_head e facial hair.
* Para acessórios de mão, o orçamento é de 1k tris. Se o handwear esconder a mão base do avatar, o orçamento é de 1,5k tris.
* No máximo 2 texturas (em resolução de 1024x1024px ou menor) por Wearable. Todas as texturas devem ser quadradas, com resolução de 72 pixels/polegada.
* No máximo 2 materiais (sem contar o AvatarSkin\_MAT)
* No caso de Wearables de skin, a quantidade permitida é de 5k tris e 5 texturas.


> [!WARNING]
> **Combinação de tris de Wearables:**
> 
> *Se o Wearable ocultar outros Wearables, o criador pode combinar os tris por slot. Por exemplo: se você quiser fazer um macacão, poderia criá-lo usando a categoria parte superior do corpo, ocultando a parte inferior do corpo; nesse caso, você poderia ter 1,5K*2= 3K triângulos.\*
> 
> No caso do capacete, se você ocultar todos os Wearables da cabeça (cabeça, brincos, eyewear, tiara, hat, facial\_hair, hair e top\_head, você pode chegar a 4k tris, 2 materiais e 2 texturas)


#### Dimensão máxima de largura, altura e profundidade dos Wearables

Há um limite de distância para os Wearables, para garantir que eles não obstruam a visibilidade das telas de outros jogadores nem invadam o espaço da cena de forma excessiva.

A dimensão dos Wearables não pode exceder:

> `Altura: 2,42 m`, `Largura: 2,42 m`, `Profundidade: 1,4 m`

![](/docs/_remote/0c4ec0aad4c553469f608e3f.webp) ![](/docs/_remote/74d98bf9b871410fa2fac083.webp)

#### **Mapas**

Atualmente, os Wearables da Decentraland suportam 3 tipos de mapas, que são:

1. **Cor base**: Esta é a textura principal, com as cores e os detalhes do seu modelo.
2. **Emissão**: Este mapa é para as partes que estão brilhando no seu modelo. O mapa de emissão vai em um material separado especificamente para emissão.
3. **Alpha**: Este mapa serve para lidar com a transparência. Ele usa o canal de opacidade da textura. (*É sempre preferível usar Alpha Clip em vez de Alpha Blend, usando uma textura em preto e branco para o recorte*)

![](/docs/_remote/d9bad6330bf2adc9dd0f39cd.webp)

Como o cliente de referência da Decentraland usa um Toon Shader para os materiais do avatar, alguns mapas **não são necessários** como:

* **Mapas de normal:** texturas usadas para simular detalhes de alta resolução em modelos de baixa contagem de polígonos, codificando as normais da superfície como valores RGB.
* **Mapas de rugosidade:** texturas usadas para definir a rugosidade da superfície de objetos 3D.

Para usar esses mapas na Decentraland, a solução é fazer bake deles em uma única textura. Você pode encontrar mais informações sobre baking de texturas aqui: <https://docs.blender.org/manual/en/latest/render/cycles/baking.html>

#### **Normais**

**O motor da Decentraland renderiza normais apenas de um lado.** (Isso significa que um plano só será visível de um lado; o outro lado não será renderizado.) Então, para garantir que seu 3D esteja absolutamente correto em relação às normais, podemos verificar isso de duas formas diferentes.

A primeira é ativar "Backface culling" nas configurações das propriedades de Material; essa é uma boa prática para identificar normais invertidas, como nesta imagem:

![](/docs/_remote/0c50bdc39625df77edb130ed.webp) ![](/docs/_remote/19bd475725e7d546031c764d.webp)

A segunda forma de verificar se as normais estão corretas é ativar "Face orientation" nas opções de sobreposição do viewport. Isso deixará seu modelo azul, mas não se preocupe. As faces azuis são as corretas e as vermelhas são as que precisam ser corrigidas; você pode encontrar essa opção aqui:

![](/docs/_remote/05e89b0f38d24052c2543b41.webp)

#### **Armature**

Lembre-se de não alterar nenhuma das especificações, convenções de nomenclatura, hierarquia ou transforms da armature fornecida. Alterar qualquer um desses itens fará com que o Wearable pare de funcionar no client após a exportação.

![](/docs/_remote/8d4e69ae0e0d37a5a75dd0ec.webp)

Certifique-se de que a armature importada não tenha nenhum *end* ou *neutral* bones; caso contrário, o Wearable não vai funcionar após exportar para o builder. Se você estiver importando um .fbx que tenha esse problema, pode ativar ***Ignore Leaf Bones*** ao importar a armature.

![](/docs/_remote/9bb431a3abead38036195f21.webp)


> [!NOTE]
> **Spring Bones:** Wearables podem incluir ossos extras além da armature base para física dinâmica (por exemplo, cabelo balançando, brincos pendentes). Esses ossos devem conter `springbone` no nome. Veja [Spring Bones](/docs/creator/wearables-and-emotes/wearables/spring-bones/) para todos os detalhes.


#### **Sobrancelhas, Olhos e Boca**

Essas meshes funcionam com um shader transparente, então você não precisa fazer nada além de criar sua própria textura png para o novo estilo de sobrancelha, olho ou boca que deseja e posicioná-la corretamente no UV map. Essas texturas devem ser de 256x256px e precisam ter um canal alpha para transparência.

Aqui estão algumas texturas png de exemplo:

![](/docs/_remote/5574fe5585ace1ba7b8fc724.webp) ![](/docs/_remote/65a2cf2f85f2d9c2eceaf72b.webp) ![](/docs/_remote/a7245593b361bbcdb12ba303.webp)

*Olhos e Eyewbrows usam a mesma mesh e o mesmo UV map.*

![](/docs/_remote/769020e9eaad71f8b782e641.webp) ![](/docs/_remote/2b2a15a2fadd5b4fa790102b.webp)

*Mesh e UV map da Mouth.*

Para visualizar o resultado final, você precisará usar estes nós (no Blender):

![](/docs/_remote/477f9ad97ab6c5847c84ba3a.webp) ![](/docs/_remote/7205c2f5e0ce6b631e182dbf.webp)

**Máscaras:** O Avatar Editor tem diferentes opções de cor que os jogadores podem escolher para personalizar seus avatares.

![](/docs/_remote/689357b1313432170c554166.webp)

Essas escolhas de cor são aplicadas a uma máscara específica no Wearable.

![](/docs/_remote/859d860ec5e8ae3859344474.webp) ![](/docs/_remote/bc69b3aeb4af2645c8f66ddd.webp)

A área preta na imagem à esquerda (Eyes Mask) indica a área da textura à direita (Eyes Base) que será colorida. É importante lembrar que as íris sempre precisam estar em escala de cinza (se a íris for totalmente preta, a tint não vai funcionar. Pelo contrário, se a íris for totalmente branca, ela será totalmente tingida pela cor selecionada no editor).

#### **Handwear**

Existem dois tipos de **Handwear** que você pode fazer na mesma categoria.

1. **Substituindo as mãos:**

![](/docs/_remote/9affc36f5fddb68dcafbd2b3.webp)

Se você quiser criar handwear que substitua as duas mãos, provavelmente precisará substituir a ***mão*** mesh base. Fazendo isso, o limite para o Wearable de handwear será de 1500 tris.

2. **Acessórios de mão:**

![](/docs/_remote/f9dc813cad31b498851c90f6.webp)

Você também pode criar acessórios de mão como relógios, braceletes, anéis etc. Nesse sentido, a mão não precisa ser substituída e o limite é de 1000 tris por item.


> [!WARNING]
> Nota: a **handwear** categoria é para acessórios de mão, itens segurados na mão (como ferramentas ou props) e itens que substituem as mãos. Todos eles devem seguir a armature do avatar com skinning adequado. Enviar um item que não siga essas especificações pode resultar em rejeição pelo comitê de curadoria.


#### **Cabelo e pelos faciais**


> [!TIP]
> **Dica:** Cabelo e outros acessórios pendentes agora podem usar **spring bones** para se mover dinamicamente em resposta ao movimento do avatar e à gravidade. Em vez de cabelo estático, você pode adicionar cadeias de spring bones para fazer rabos de cavalo, tranças e cabelo longo balançarem naturalmente. Veja [Spring Bones](/docs/creator/wearables-and-emotes/wearables/spring-bones/) para um guia completo.


Há duas coisas importantes para lembrar ao criar Wearables de cabelo personalizados.

Primeiro, tente seguir o formato da cabeça. Você sempre pode consultar a mesh da cabeça fornecida nos arquivos de exemplo, se precisar de um ponto de partida.

![](/docs/_remote/370336fb19510d9325816b5a.webp)

Segundo, se você quiser que os usuários possam alterar a cor do cabelo ou dos pelos faciais usando o editor de avatar, então você precisa pintar o cabelo em escala de cinza e usar "Hair" na nomenclatura do material (exemplo "M\_Hair\_Short"). Se você quiser incluir outro objeto que não seja influenciado pela tint, basta não adicionar essa convenção de nomenclatura.

![](/docs/_remote/6c65a6133c0e05d828652d8e.webp) ![](/docs/_remote/cb8f9f3d94259277831a5b96.webp) ![](/docs/_remote/4b874f015973cb5a36ecc963.webp)

*Tons mais escuros de cinza aparecerão mais escuros e tons mais claros de cinza aparecerão mais brilhantes, multiplicados pela cor selecionada pelo usuário no editor de avatar.*

### **Materiais e Texturas Base**

Há três materiais básicos para modelos de avatar. Um é o material usado para o próprio Wearable, outro é usado para a pele e outro para as sobrancelhas, olhos e boca.

![](/docs/_remote/d83fc01227d8770cbf81986b.webp)

Cada mesh base vem com sua própria textura de pele.

![](/docs/_remote/930d22b040b6df838d2d26f2.webp) ![](/docs/_remote/8dfa78f550fa2b8b6f32dfd0.webp)

A textura de pele é feita em escala de cinza, o que permite que o motor de renderização tinga a pele do avatar usando o editor de acordo com a preferência do usuário. Para poder tingir a cor da pele usando o editor, o nome do material deve ser *AvatarSkin\_MAT*.

![](/docs/_remote/51dce26a37033db0987f4cbe.webp)


> [!WARNING]
> Importante: preserve sempre o mapeamento UV de qualquer parte do corpo que fique exposta por um Wearable, como as pernas expostas por shorts ou saias.


\src="../images/wearables-and-emotes/creating-wearables/19\_skin\_uv.png"width="600"/>

Você pode criar texturas personalizadas para seus Wearables! No entanto, o ideal é sempre usar um único arquivo de textura, bem pequeno, para cada Wearable. Usar a textura padrão AvatarWearable\_MAT fornecida nos arquivos de exemplo vai garantir que seus Wearables tenham bom desempenho!

![](/docs/_remote/d821fd219afb4cca8fe745d3.webp)


> [!WARNING]
> Para evitar disparar shaders específicos de feições faciais, não inclua '\_mouth', '\_eyebrows' ou '\_eyes' na nomenclatura de nenhuma mesh. Esses termos são reservados para feições faciais, e usá-los de forma inadequada aplicará o shader errado à mesh.


✨ Caso você queira criar suas próprias texturas para o modelo, recomendamos os seguintes addons para um melhor UV Unwrapping:

**UVPacker**

UvPacker é um addon gratuito que ajuda você a organizar e empacotar seus UVs com apenas alguns cliques. Como a Decentraland trabalha com texturas de até 1024x para os Wearables, essa ferramenta é uma grande ajuda para criar texturas melhores e mais organizadas.

Você pode baixar diretamente no site aqui:

[**https://www.uv-packer.com/download/**](https://www.uv-packer.com/download/)

**UVToolKit**

UvToolKit é um addon que ajuda você a expandir suas configurações e opções de UV para criar ótimos UVs no Blender.

Aqui está o link para download: [**https://alexbel.gumroad.com/l/NbMya**](https://alexbel.gumroad.com/l/NbMya)

### **Skin Weighting**

Skin weighting é o processo de determinar quais ossos do rigging do avatar afetam quais Wearables durante uma animação.

Ao fazer skin weighting dos nossos novos Wearables, há várias considerações que precisamos ter em mente.

Cada asset deve ser weighted para o esqueleto completo. Por exemplo, um asset de parte superior do corpo ficará assim ao aplicar skin weights:

![](/docs/_remote/18e214351b78092a3436da29.webp)

Wearables que se encontram nas interseções entre partes do corpo devem ser totalmente weighted para o mesmo osso. Por exemplo, nessas duas zonas verdes, os vértices no pescoço precisam ser totalmente weighted apenas para o osso "Neck".

![](/docs/_remote/e5d072fd17af31865e1e1c4a.webp)

#### **Ossos-chave**

Os ossos "chave" a serem usados no skin weighting são:

**Osso da cabeça:** para o cabelo, brincos, tiaras, olhos, sobrancelhas, boca e qualquer acessório que precise acompanhar o movimento da cabeça.

**Osso do pescoço:** para os vértices de interseção da cabeça e da parte superior do corpo.

**Osso do quadril:** para os vértices de interseção da parte superior e da parte inferior do corpo.

**Ossos da perna direita e da perna esquerda:** para os vértices de interseção da parte inferior do corpo e dos pés.

**Antebraço direito e antebraço esquerdo** para os vértices de interseção das mãos.


> [!WARNING]
> ⚠️ **Dica**
> 
> * Lembre-se: você pode usar qualquer osso para influenciar os vértices de qualquer mesh! Por exemplo, você pode criar uma nova mesh de pé para um par de botas altas e fazer skin weight da parte superior da bota para os "Leg Bones". Ou você pode criar um cabelo longo e usar os ossos de "Shoulder" ou "Spine" para influenciar o cabelo quando o avatar se move.
> * É sempre recomendável manter simetria em ambos os lados do rig; esquerdo e direito devem ter influências ósseas semelhantes.
> * Como orientação geral, um vértice não pode ser influenciado por mais de 4 ossos ou joints.
> * Tenha em mente exportar a armature exatamente como a fornecida na documentação. Se ela tiver quaisquer outros ossos como *"\_end\_bones"* ou similar, não vai funcionar no client.


#### **Partes rígidas**

Para partes que não devem dobrar, como uma mochila, um jetpack ou placas de armadura, distribua o peso de cada vértice da parte em 100% para um único osso. Por exemplo, use `Avatar_Spine2` para qualquer coisa nas costas. Depois, junte a parte à mesh do Wearable, para que o Wearable continue sendo uma única mesh com skinning.

`Avatar_Spine2` fica no mesmo lugar em ambas as formas do corpo, então uma parte influenciada por ele pode usar a mesma posição em ambas.

Uma parte rígida nas costas pode parecer que está flutuando, porque as costas se curvam para longe de uma parte plana. Meça o espaço sob toda a área ocupada pela parte e posicione sua face frontal a cerca de 4 mm do ponto mais afastado das costas. Como alternativa, adicione um pequeno bloco de suporte entre a parte e as costas e distribua o peso dos vértices frontais do suporte como os vértices do corpo que eles tocam, para que o suporte dobre com a spine.

#### **Exportando Wearables**

Ao exportar Wearables, certifique-se de que não haja outros ossos fora da armature fornecida, **exceto pelos spring bones** (ossos com `springbone` springbone no nome, usados para física dinâmica). Um problema comum ao importar armatures entre softwares diferentes é o aparecimento de *"\_end"* ou *"\_neutral"* bones. Certifique-se de removê-los antes de exportar. Caso contrário, é muito provável que os Wearables não funcionem depois no client.

![](/docs/_remote/c6477cfc83e060c6483735ac.webp)

Para exportar o Wearable, selecione o objeto e depois a armature. Certifique-se de não exportar mais nada, como câmeras, luzes ou objetos vazios.

Um Wearable válido feito de uma única mesh tem 64 nós em seu arquivo exportado: a `Armature`, seus 62 ossos e a mesh com skinning. Spring bones adicionam um nó cada.

Em seguida, exporte o Wearable em *glTF2.0* formato. Certifique-se de exportar apenas o Wearable com suas propriedades de skinning e sem outros recursos desnecessários, como animação ou shape keys.

![](/docs/_remote/f71be0c8670ef1406a96e0f4.webp)

## Boas práticas para modelagem

### **Mude sua mesh de T-Pose para A-Pose**

Ao criar Wearables, a melhor forma de visualizar o resultado final e facilitar o trabalho com a topologia e a posição do Wearable é trabalhar com o modelo em A-Pose. Para isso, você precisa seguir estes passos simples:

1. Primeiro, selecione a parte superior do corpo; depois, você deve alternar "**Edit mode**" e "**On cage**" no modificador da armature.

![](/docs/_remote/9dd9e778254d10a239d518c8.webp)

2. Agora, no pose mode, você pode girar os braços em 60°.

![](/docs/_remote/41758037910791f5efcffe51.webp)

3. Você pode editar sua mesh em A-Pose em vez de T-pose.

![](/docs/_remote/71276e7651ff3507da8ee9fe.webp)

4. Mas também é bom lembrar que você pode alternar facilmente de A-Pose para T-Pose apenas voltando a ativar "Edit mode" e "On cage" no modificador da armature.

![](/docs/_remote/a731a1c3b9056b0a3f73203e.webp)

### **Deformação das articulações:**

Para obter os melhores resultados na topologia do Wearable quando se trata de juntas (braços ou pernas, por exemplo), é importante ter boas práticas ao criar loops. Aqui podemos ver a diferença na deformação da mesh para diferentes loop cuts:

![](/docs/_remote/c18bb25e36a973c241938296.webp)

Uma boa maneira de garantir que tudo esteja se deformando corretamente é fazer um weight paint como no exemplo a seguir:

![](/docs/_remote/0e4eb4a84b57c9c5a3f1d079.webp) ![](/docs/_remote/a6d3efa7b12801dd5b29ed8b.webp)

### **Saias**

Uma dica útil e uma boa prática ao modelar saias/vestidos é adicionar loopcuts adicionais nas interseções das dobras. Isso será muito útil quando você precisar pintar os weights do rig.

![](/docs/_remote/20d43358a97c94b9db32014e.webp)

Com esses loopcuts, a influência dos vértices fica muito mais suave e traz resultados melhores quando você estiver animando uma saia/vestido.

Aqui está um exemplo de como a influência dos ossos deve ser:

![](/docs/_remote/3a762bb515429e86217673cd.webp)

### **Chapéus**

Uma boa prática ao criar chapéus é adicionar um cabelo à mesh base do chapéu e depois ocultar a categoria *hair* usando o editor. Fazer isso vai impedir que o chapéu clip com outros cabelos e reduzir resultados inesperados.

![](/docs/_remote/52f717e012928949c63883f0.webp)

### **Adicionar contagem de polígonos**

Uma dica valiosa é sempre acompanhar a contagem de polígonos dos seus modelos. Para isso, no Blender, você precisa ativar as estatísticas no painel de sobreposições do viewport.

![](/docs/_remote/444dbdc043e94bf60c83af44.webp)

### Recursos

Nesta pasta compartilhada você pode encontrar modelos base, texturas e vários outros recursos, incluindo exemplos de Wearables totalmente criados. Sinta-se à vontade para aproveitar esses recursos ao criar os seus próprios.

[**Modelos de referência de Wearables**](https://drive.google.com/drive/u/1/folders/12hOVgZsLriBuutoqGkIYEByJF8bA-rAU)


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-pt/wearables-e-emotes/wearables/creating-wearables.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
