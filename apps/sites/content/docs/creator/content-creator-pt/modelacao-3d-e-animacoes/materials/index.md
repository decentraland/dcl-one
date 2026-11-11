# Materiais

Saiba quais propriedades de material e texturas são suportadas em modelos 3D importados para o Decentraland.

Os materiais são incorporados em um *.gltf* ou *.glb* arquivo.


> [!NOTE]
> **💡 Dica**: Instala o [plugin Decentraland Tools para Blender](https://extensions.blender.org/add-ons/decentraland-tools/). Inclui várias funções úteis para te ajudar a editar e exportar modelos 3D, wearables e emotes.


Este documento refere-se a materiais importados em um modelo 3D. Para materiais definidos via código para serem aplicados em formas primitivas, veja [**materials**](https://docs.decentraland.org/creator/development-guide/materials/) .


> [!WARNING]
> **📔 Nota**: No momento, você não pode alterar dinamicamente os materiais de um modelo 3D a partir do código da sua scene, a menos que seja uma forma primitiva.


## Suporte a Shader

Nem todos os shaders podem ser usados em modelos importados para Decentraland. Certifique-se de usar um dos seguintes:

* **Materiais standard:** quaisquer shaders são suportados, por exemplo diffuse, specular, transparência, etc.
* **materiais PBR (Physically Based Rendering)**: Este shader é mais flexível, pois inclui propriedades como diffuse, roughness, metalness e emission que permitem configurar como um material interage com a luz.

A imagem abaixo mostra dois modelos idênticos, criados com as mesmas cores e texturas. O modelo à esquerda usa todos os *PBR* materiais, alguns dos quais incluem *metalness*, *transparência*, e *emissividade*. O modelo à direita usa todos os *standard* materiais, alguns incluindo *transparência* e *emissividade*.

![](/docs/_remote/129845688fefc65d479ded1d.webp)

### Propriedades PBR que atualmente funcionam com o Decentraland Engine

> * Cor Base
> * Metálico
> * Rugosidade
> * Especular
> * Emissivo
> * Intensidade de Emissão
> * Alpha
> * Normal

![](/docs/_remote/9edb0c21919b4803373b3885.webp)

Para visualizar como essas propriedades se comportam no mundo, você pode ir a este [mundo de teste](decentraland://?realm=TestEnvironment.dcl.eth) para encontrar diferentes objetos e materiais e ver como eles interagem com as luzes e o ambiente do mundo.

![](/docs/_remote/1abdb0c1779701d95d2637dc.webp)

### Cor Base/Difusa

Define a cor base da superfície do objeto. Por si só, ela não sofre nenhum efeito da iluminação; por isso, é combinada com outros nós, como roughness, metallic, specular, etc.

### Metálico

Um shader metallic refere-se a um tipo de técnica de renderização usada para simular a aparência de superfícies metálicas. Um shader metallic leva em conta as propriedades físicas dos metais e como eles interagem com a luz para produzir as características brilhantes e reflexivas do metal.

![](/docs/_remote/b601bec72f14836b99d6f451.webp)

### Rugosidade

Materiais de roughness estão relacionados à simulação realista de como a luz interage com a superfície do material. Normalmente, os mapas de roughness são usados para dar aos modelos uma faixa de "suavidade" ou "rugosidade" em suas superfícies. Um mapa de textura em tons de cinza é normalmente usado para fornecer informações desse tipo.

![](/docs/_remote/c4e81d0781e9fcd132dbed8f.webp)

Essa propriedade faz a transição entre um modelo de material não metálico e metálico. Um valor de 1.0 produz uma reflexão especular totalmente com tonalidade da cor base, sem reflexão difusa ou transmissão. Em 0.0, o material consiste em uma camada base difusa ou transmissiva, com uma camada de reflexão especular por cima.

### Especular

Em um shader Physically-Based Rendering (PBR), as propriedades especulares se referem a como a luz interage com uma superfície em termos de refletividade e brilho. A reflexão especular é a reflexão espelhada da luz em uma superfície. Em PBR, essa propriedade é usada para controlar quanta luz uma superfície reflete de maneira espelhada. Materiais como metais normalmente têm alta reflexão especular, criando realces nítidos e brilhantes, enquanto materiais não metálicos, como plásticos, têm menor reflexão especular, resultando em realces mais amplos e suaves.

![](/docs/_remote/92855ba60b2140bd9d3977ad.webp)

### Alpha

Você pode definir um material como *transparente*. Materiais transparentes podem ser vistos através deles em graus variados, dependendo de sua *valor de alpha*. Para isso, ative a propriedade de transparência do material e então defina seu *valor de alpha* para o valor desejado. Um alpha de 1 tornará o material completamente opaco; um alpha de 0 o tornará invisível.

A imagem abaixo mostra dois modelos idênticos criados com materiais standard. O da esquerda usa apenas materiais opacos, enquanto o da direita usa materiais transparentes e opacos em algumas de suas partes.


> [!WARNING]
> 💡 Lembre-se de que usar materiais transparentes é sempre mais caro em termos de desempenho do que materiais diffuse. Tente sempre manter os materiais transparentes no menor nível possível.


![](/docs/_remote/69e874b1001f67ad59d3ac60.webp)

Há dois modos principais de transparência diferentes: *Alpha Clip* e *Alpha Blend*. As principais diferenças são:

* **Alpha Clip:** Alpha Clip renderiza valores absolutos de 0 ou 1 com base em um limiar de corte de um valor em escala de cinza. A cor anterior será substituída pela cor da superfície, mas somente se o valor alpha estiver acima do valor de Clip Threshold.

![](/docs/_remote/6e885f9bbcbfca598d148a89.webp)

* **Alpha Blend:** Alpha Blend interpola os valores entre 0 e 1. Você pode usar alpha blending para sobrepor a cor da superfície sobre a cor anterior.

![](/docs/_remote/293aba2fd75ef42086bd5c31.webp)


> [!WARNING]
> **🔥Dica de otimização🔥** A menos que você queira especificamente poder ter um nível intermediário de transparência, **é sempre mais performático para renderização usar \_Alpha Clip***\*\* em vez de \*\****Alpha Blend**\_**.**



> [!WARNING]
> **⚠️ Problemas de ordenação**
> 
> Quando você usa modos de blend transparentes no seu jogo, é crucial considerar a ordem em que a mesclagem de cores acontece. Isso porque a cor final de saída pode ser significativamente afetada pela ordem da mesclagem. Atualmente, o engine suporta apenas ordenação por objeto, o que significa que ele ordena automaticamente todas as superfícies transparentes com base na origem do objeto. No entanto, ordenação por fragmento (pixel) e ordenação por triângulo não são suportadas no momento.
> 
> Para evitar problemas relacionados à ordenação, é melhor evitar usar objetos com alpha clip e alpha blend no mesmo mesh. Isso pode ajudar a evitar artefatos de mesclagem inesperados e garantir que seu jogo fique no seu melhor.


### Emissivo

Você também pode tornar um material *emissivo*. Materiais emissivos emitem sua própria luz. Observe que, quando renderizados, eles não iluminam realmente os objetos próximos na scene; apenas parecem ter um brilho difuso ao redor.

A imagem abaixo mostra dois modelos idênticos criados com materiais standard. O da direita tem materiais emissivos brilhantes em algumas de suas superfícies.

![](/docs/_remote/6a611f35617386705cb8bcce.webp) ![](/docs/_remote/4deead5b2aeaa81746397cb9.webp)

*Para tornar um material emissivo no Blender, basta adicionar um `emission` shader ao material.*

### Intensidade Emissiva

Intensidade da luz emitida. Um valor de 1.0 garantirá que o objeto na imagem tenha exatamente a mesma cor da Emission Color, ou seja, o tornará ‘shadeless’.

![](/docs/_remote/5392ea458ab888ee365b3678.webp)

*Você pode verificar em* [*mundo de teste*](decentraland://?realm=TestEnvironment.dcl.eth) *como a intensidade da emissão se comporta no mundo*

### Normal

O nó "normal" em um shader PBR é um componente fundamental usado para controlar as normals de superfície de um material. Normals são vetores que definem a direção perpendicular a uma superfície em um ponto específico, e desempenham um papel crucial na determinação de como a luz interage com a superfície.

![](/docs/_remote/c3f4920500591fa40e453fd3.webp)

## Pintura de vértices

A pintura de vértices de modelos 3D não é atualmente suportada pelo engine do Decentraland.

## Limitações de materiais

Leve em conta que os limites de material por parcel são:

> log2(n+1) x 20 Quantidade de materiais na cena. Isso inclui materiais importados como parte dos modelos.

É importante levar em conta que cada material representa uma draw call por objeto, então é crucial manter os materiais o mais reduzidos possível e tentar reutilizá-los ao máximo usando técnicas como Texture Atlases; isso também vai beneficiar a cena, proporcionando um estilo coeso entre os assets da sua scene.

## Nomenclatura de materiais

Para ter um pipeline de arte organizado e saudável, recomendamos nomear seus materiais corretamente. Uma forma de fazer isso é usando este método de convenção.

```
<Object>_<Classification>_<Sub-Classification(optional)>_<_MAT>
```

Então, por exemplo, digamos que fizemos 2 árvores diferentes, uma emissiva e brilhante para a primavera e outra fria e metálica para o inverno. Poderíamos nomear os materiais: *"TreeSpring\_Emissive\_MAT"* e outra *"TreeWinter\_Metallic\_MAT"*

Em conclusão,

* 🟢 **Prefira** usar nomes que comecem com o objeto e a classificação: *"Wood\_Oak\_MAT"*, *"SciFiFence\_Metallic\_MAT"*, etc.
* 🔴 **Evite** usar nomes como *"Material009"*, *"material1"*, o que torna a cena e os modelos muito difíceis de acompanhar e analisar.

## Substituir materiais glTF

Você pode sobrescrever os materiais de um *glTF* modelo usando o [GltfNodeModifiers](/docs/creator/content-creator-pt/scenes-sdk7/fundamentos-de-conteudo-3d/materials/#modify-gltf-materials) component no código da sua scene. Veja [Modificar materiais glTF](/docs/creator/content-creator-pt/scenes-sdk7/fundamentos-de-conteudo-3d/materials/#modify-gltf-materials) para mais detalhes.

## Boas práticas para materiais

* Se a sua scene incluir vários modelos que usam a mesma texture, referencie a texture como um arquivo externo em vez de deixá-la incorporada no modelo 3D.

Texturas incorporadas são duplicadas para cada modelo e aumentam o tamanho da scene. *.glb* arquivos têm suas texturas incorporadas por padrão, mas você pode usar [**glTF pipeline**](https://github.com/AnalyticalGraphicsInc/gltf-pipeline) para extraí-las externamente.

> Nota: Depois de referenciar um arquivo para uma texture que não será incorporada, certifique-se de que esse arquivo não será movido nem renomeado, pois, caso contrário, a referência ao arquivo será perdida. O arquivo também precisa estar dentro da pasta da scene para que seja enviado junto com a scene.

* Ao definir a transparência de um material, tente sempre usar *Alpha clip* em vez de *Alpha blend*, a menos que você realmente precise ter um material parcialmente transparente (como vidro). Isso evitará problemas em que o engine renderiza o modelo errado na frente do outro.
* Como regra geral, lembre-se sempre de definir *backface culling* nos seus materiais. Isso tornará sua scene mais performática, já que o Engine vai renderizar apenas a face visível dos seus modelos. Desative isso apenas no caso de você precisar que um modelo seja renderizado dos dois lados (por exemplo, um grupo de folhas de uma árvore feito com planos 3D). *backface culling* Use as

![](/docs/_remote/83df362d7fc9ab62e2ae130d.webp)

* texturas padrão do Decentraland [**, que são pré-carregadas pelos jogadores, fazendo com que seus assets renderizem muito mais rápido.**](https://github.com/decentraland/builder-assets/tree/master/textures) este artigo
* Leia [**para uma visão detalhada de um pipeline de arte completo que usa texturas PBR em modelos glTF.**](https://www.khronos.org/blog/art-pipeline-for-gltf) para uma visão detalhada de um pipeline de arte completo que usa texturas PBR em modelos glTF.
* Você pode encontrar uma referência detalhada sobre como criar materiais compatíveis com glTF no Blender em [**documentação do Blender**](https://docs.blender.org/manual/en/latest/addons/import_export/scene_gltf2.html) .
* Encontre texturas PBR gratuitas e de alta qualidade em [**cgbookcase**](https://cgbookcase.com/) .


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-pt/modelacao-3d-e-animacoes/materials.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
