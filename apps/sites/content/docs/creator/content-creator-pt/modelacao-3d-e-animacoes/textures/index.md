> Saiba como as texturas funcionam no Decentraland

# Texturas

As texturas são uma parte fundamental do pipeline de arte 3D para alcançar o aspeto e a sensação que deseja para a sua scene. Nesta secção encontrará tudo o que precisa de saber para criar as suas texturas, limitações, nós no Blender e como otimizá-las para terem o melhor desempenho!


> [!NOTE]
> **💡 Dica**: Instale o [plugin Blender do Decentraland Tools](https://extensions.blender.org/add-ons/decentraland-tools/). Ele inclui várias funções úteis para ajudar você a editar e exportar modelos 3D, wearables e emotes.


## Limitações

### Restrições de Tamanho das Texturas

Atualmente, o Decentraland Explorer comprime as texturas para um máximo de 1024px por motivos de otimização, usando um Asset Bundle Converter depois de a cena ser carregada nos content servers. Certifique-se de ter em conta esta limitação ao criar os seus assets!

Outro ponto importante a ter em conta é que as texturas devem ser sempre potências de dois; quaisquer texturas que não sigam esta especificação podem causar problemas ao renderizar a cena.

Os tamanhos das Textures devem usar números de largura e altura (em pixels) que correspondam aos seguintes números:

```
1, 2, 4, 8, 16, 32, 64, 128, 256, 512, 1024
```

> Esta sequência é composta por potências de dois: `f(x) = 2 ^ x` . **1024 é o número máximo que permitimos para o tamanho de uma textura.** Este é um requisito bastante comum noutros rendering engines; existe devido a otimizações internas dos processadores gráficos.

A largura e a altura não precisam de ter o mesmo número, mas ambas precisam de pertencer a esta sequência.

**O tamanho recomendado para Textures é 1024x1024**, constatámos que este é o tamanho ideal para ser transportado através de redes domésticas e para proporcionar experiências razoáveis de carregamento/qualidade.

Exemplos de outros tamanhos válidos:

```
32x32px
64x32px
512x256px
512x512px
1024x1024px
```

Por outro lado, existe um limite para texturas por parcel:

```
log2(n+1) x 10 Quantidade de texturas por parcel. Inclui texturas importadas como parte de modelos.
```

## Mapeamento UV

UV Mapping é o processo de desembrulhar as faces do seu modelo 3D numa coordenada 2D que será usada mais tarde para adicionar os diferentes maps aos seus assets. É uma parte fundamental do processo de criação. Fazer um unwrap correto dos seus modelos é um fator chave para aproveitar ao máximo a resolução dos seus modelos e também organizar os seus maps para serem flexíveis a modificações.

Para saber mais sobre UV Unwrapping pode ver este vídeo incrível feito pela Blender Foundation:

[![Video Preview](/docs/_remote/b25b76a5b0e4c1d0cec817e8.webp)](https://youtu.be/Y7M-B6xnaEM?si=qMnWTMsXxC-vxZAH)

Há outro ótimo tutorial de nível intermédio para unwrap de UVs feito por \[Blender Guru] (<https://www.youtube.com/@blenderguru>) que explica como fazer unwrap de um modelo mais complexo:

[![Video Preview](/docs/_remote/2b79a21fed4d0ba2abc23bfa.webp)](https://youtu.be/scPSP_U858k?si=Uw0xHbv9jtqVstDS)

## Mapas

No [secção de materiais](/docs/creator/3d-modeling/materials/) explicámos como o Decentraland funciona com os shaders PBR. Nesta secção vamos mostrar-lhe como cada texture map modifica o shader e o aspeto e sensação do objeto 3D que mais tarde vai ser exportado para o mundo.

## Mapas Diffuse

Esta é a cor base da superfície do objeto. Ter uma paleta de cores equilibrada entre os seus modelos é fundamental para alcançar um aspeto e sensação coesos para a sua experiência.

![](/docs/_remote/3a9f1ec299edbbb69e0531e0.webp)

Aqui tem um gerador de paletas gratuito e interessante, caso precise!

* **Coolors:** <https://coolors.co/>

Ou alguma inspiração de paletas fornecida por filmes incríveis:

* **Movies in Color:** <https://moviesincolor.com/>

## Mapas Metallic

Num mapa Metallic, o mapa em tons de cinzento representa o grau de metalicidade que um objeto possui, sendo branco totalmente metálico e preto não metálico. No exemplo seguinte vemos como a luz afeta o modelo e como interage com o ambiente.

![](/docs/_remote/d7d680ed47a91de7cef0901a.webp)

*Na imagem podemos ver como as partes escuras da textura afetam o modelo. As faixas escuras são opacas, enquanto a faixa branca está a comportar-se de acordo com as propriedades do shader metálico e o cinzento sendo uma mistura entre ambos.*

## Mapas Roughness

Num mapa de roughness, as áreas mais escuras correspondem a superfícies mais suaves, enquanto as áreas mais claras correspondem a superfícies mais rugosas. Esta representação em tons de cinzento é usada pelos rendering engines para determinar como a luz deve ser dispersa ou refletida em diferentes pontos da superfície.

![](/docs/_remote/ab1077956ce52917b01f28a4.webp) ![](/docs/_remote/cf4b607b56f34ffc9787473a.webp)

## Mapas Transparent

### Alpha Clip

Alpha Clip no Blender, quando usado com uma textura, envolve usar o canal alpha da textura para determinar que partes do material devem estar visíveis. Pixels com valores alpha acima de um limiar especificado são mostrados, enquanto os abaixo do limiar são descartados, criando um efeito de recorte baseado nas informações de transparência da textura.

No exemplo seguinte usamos um material com Alpha Clip, usando 2 texturas, a cor diffuse e a textura alpha em preto e branco, ligando a cor ao canal alpha.

![](/docs/_remote/a6eb0c3607b3ac754dd99f4a.webp) ![](/docs/_remote/7efa7539bc36fae5c645b2f7.webp)

![](/docs/_remote/85b8e0c6b3f5f217698536fc.webp)

*Como resultado, podemos ver que as áreas pintadas a preto são descartadas enquanto a área branca está a ser renderizada*

### Alpha Blend

*Alpha Blend* permite-lhe escolher valores intermédios por região.

Alpha Blend no Blender, quando usado com uma textura, envolve misturar as áreas transparentes e opacas da textura com base no seu canal alpha. Isto permite que a textura combine de forma suave as partes visíveis e as partes transparentes, interagindo naturalmente com o fundo ou com outros objetos na cena.

![](/docs/_remote/46cdac0bd41b0f225bd498e4.webp)

*Enquanto o Alpha Clip renderiza valores 0 ou 1 (num limiar específico), o Alpha Blend interpola os valores entre 0 e 1. No exemplo acima, o material de alpha blend mostra a transição completa em gradiente, enquanto o alpha clip exclui parte da textura definida pelo limiar de corte*


> [!WARNING]
> **🔥Dica de otimização🔥**
> 
> * Tenha em conta que texturas transparentes (RGB+A) são sempre mais caras em termos de desempenho do que usar um valor em tons de cinzento para transparências.
> * A transparência é sempre uma operação dispendiosa ao renderizar a cena. Tente manter as transparências sempre no mínimo e use Alpha Blend apenas quando necessário; caso contrário, Alpha Clip é preferível em vez de Alpha Blend.


## Mapas Emissive

Um emissive map é um tipo de texture map usado para controlar a autoiluminação ou a luz emitida por uma superfície numa cena 3D. É um componente do shader que determina quanta luz (e de que cor) uma determinada parte de um modelo 3D emite, independentemente de fontes de luz externas. Os emissive maps são normalmente usados para simular materiais ou objetos que parecem emitir a sua própria luz.

![](/docs/_remote/e4c3fc1cd832dcc33edb3f1c.webp)

*Neste exemplo podemos ver a utilização de um emissive map combinado com emissive strength num ambiente que usa glow postprocessing para testar aproximadamente como o emissive se comporta no world.*

## Mapas Normal

Um normal map é um tipo de textura usada em gráficos 3D para simular detalhes finos da superfície e criar a ilusão de geometria complexa sem alterar realmente a geometria subjacente de um modelo. É normalmente usado para melhorar o realismo de modelos low-poly, adicionando a aparência de saliências, cavidades e outras irregularidades na superfície. Também permite manter os próprios objetos mais leves, já que muitos detalhes podem ser fornecidos na camada do normal map em vez de geometria complexa.

![](/docs/_remote/76188ce5daf39c2ac1f11691.webp) ![](/docs/_remote/c3f4920500591fa40e453fd3.webp)

Para adicionar um normal map ao seu material usando o *Shader Editor*, terá de conectar o ***Normal Map*** node entre a textura e o *Principled BSDF* shader.

![](/docs/_remote/813d6e575903a5c3a19e8e2e.webp)


> [!WARNING]
> **⚠️Importante⚠️:** Nunca use uma textura como albedo e normal ao mesmo tempo, porque isso pode criar problemas ao renderizar a cena.


## Otimização de Texturas

O processo de otimizar texturas traz muitos benefícios ao renderizar a cena no Explorer, mas também é uma boa forma de manter o estilo da sua cena consistente e mais flexível no processo de design. Alguns desses benefícios são:

* Ter texturas otimizadas em tamanho e compressão fará com que a cena corra muito mais suavemente e rapidamente, tornando o download mais rápido e a renderização mais fácil (especialmente para jogadores com uma ligação à internet lenta).
* Reduz a quantidade de memória e poder de processamento necessários para renderizar a sua experiência, resultando numa melhor experiência de utilizador para os seus jogadores.
* Poupa armazenamento nos content servers da comunidade.
* Usar **Texture Atlases** e/ou **Trim Sheets** dar-lhe-á mais flexibilidade para iterar na criação da sua cena e consistência de estilo entre os objetos. Com estas técnicas, pode facilmente trocar texturas, ajustar cores ou padrões em vez de o fazer individualmente para cada modelo.
* Partilhar texturas entre modelos permite ter menos texturas por cena, reduzindo drasticamente os draw calls no jogo. Se estiver a trabalhar com GLBs (com texturas embutidas), encontrará um extrator nas seguintes diretrizes para extrair as texturas e redirecionar os modelos para usarem a mesma textura.

## Texturas Partilhadas Entre Modelos glTF

Uma prática sábia e comum para otimizar a sua cena é partilhar texturas e materiais entre modelos ao longo da cena. Fazer isto reduzirá drasticamente os draw calls e a sua cena Decentraland correrá muito mais suavemente.

A seguinte ferramenta baseada em [**glTF pipeline**](https://github.com/AnalyticalGraphicsInc/gltf-pipeline)**,** oferece algumas otimizações que tornarão os modelos 3D mais leves e mais rápidos de descarregar para os jogadores na sua cena.

**Mac:** [MAC GLB Extractor](https://github.com/decentraland/docs/blob/main/creator/images/3d-models-and-animations/glb-extractor/texture_extractor.sh)

**PC:** [PC GLB Extractor](https://github.com/decentraland/docs/blob/main/creator/images/3d-models-and-animations/glb-extractor/glb_texture_extract.bat)

Converte o formato .gltf em .glb, que é binário e, por isso, ocupa muito menos. Também coloca os ficheiros de textura fora do modelo 3D, o que permite usar a mesma textura em vários modelos.

> 📔 Nota: o formato .glb, por omissão, tem sempre texturas incorporadas no ficheiro. O engine não consegue reconhecer duas texturas incorporadas como sendo a mesma; elas precisam de ser ficheiros externos que partilhem o mesmo hash.

### Como Usar o GLB Texture Extractor

Neste exemplo de cena, temos uma cena sci-fi simples no Blender.

![](/docs/_remote/09c63fb13acc8a4914118992.webp)

Esta cena contém o ambiente base para os modelos estáticos, mas há também outros dois assets, um droid e uma nave espacial que queremos exportar separadamente para os mover mais tarde por código, para que possam interagir com os jogadores. Neste caso usamos 4 texturas (uma para o chão, um atlas de mapa de cor para a maior parte dos assets, uma emissive e uma textura sci-fi de UI para os painéis)

![](/docs/_remote/1167ef79495fce28d3f04994.webp) ![](/docs/_remote/a8815581bf426ea4c90234ea.webp)

Depois de exportarmos todos estes assets para a pasta models, temos 3 modelos: o ambiente estático da cena, a nave espacial e o droid.

![](/docs/_remote/ba80d47093d413ede575a657.webp)

Mas temos um problema: os ficheiros .glb têm as texturas incorporadas, por isso, se estiver a exportar diferentes assets que reutilizam as texturas, estas serão duplicadas cada vez que houver um novo asset na pasta. Para evitar ter texturas duplicadas, podemos usar esta ferramenta útil.


> [!WARNING]
> ⚠️ NOTA IMPORTANTE: Antes de usar a ferramenta, faça um BACKUP dos seus modelos, por precaução, caso algo corra mal!!!


**No Mac**

Depois de exportar todos os assets para a sua pasta models, pode arrastar o ficheiro script para ela.

![](/docs/_remote/e18bbaa902da38ea04efdabb.webp)

1 - Abra a pasta no terminal arrastando a pasta para ele.

![](/docs/_remote/b110a995c9095984ab0e64a6.webp)

2 - Arraste o texture-extractor.sh para o terminal.

![](/docs/_remote/21793b724c96391277db0f59.webp)

3 - Execute o comando. Pode demorar alguns segundos a processar todos os assets. Depois disso, verá algo assim:

![](/docs/_remote/98233ac951185a1a694e7d05.webp)

4 - Vá para a sua pasta models e haverá uma nova pasta chamada "out", na qual verá os seus novos assets com as texturas extraídas.

![](/docs/_remote/fb16a7c2d30d3624207c138e.webp)

5 - Substitua todos os assets pelos novos! Além disso, assim que terminar, apague o "texture-extractor.sh" e a pasta vazia "out".

![](/docs/_remote/98e2e796967f27259ce9c137.webp)

**No Windows**

Depois de exportar todos os assets para a sua pasta models, pode arrastar o ficheiro script para ela.

![](/docs/_remote/7a5b21bf0d0ceb24563c52c3.webp)

1 - Faça duplo clique em glb\_texture\_extract.bat para extrair os ficheiros. Se o Windows o avisar sobre uma aplicação não reconhecida, vá a **Mais Informações** e depois **Executar mesmo assim**.

![](/docs/_remote/c0f9074745dc366687e508e5.webp) ![](/docs/_remote/e286200a0e0ba4a6327ffc69.webp)

2 - O script irá gerar uma pasta chamada out, aí encontrará todos os novos ficheiros .glb com as respetivas texturas extraídas.

![](/docs/_remote/c4d9063db0fa909ac05097d1.webp)

5 - Substitua todos os assets pelos novos. Quando terminar, elimine o script "texture-extractor.bat" e a pasta vazia "out".

Se seguir todos os passos, a sua cena será agora muito mais rápida e os assets partilharão a mesma textura! Quando trabalhamos com vários assets e cenas grandes, a melhoria é bastante percetível! Com esta ferramenta, pode poupar bastantes megabytes de informação!


> [!WARNING]
> ⚠️ Depois de completar este passo, é crucial verificar se cada textura segue as diretrizes. Se as texturas não seguirem as diretrizes ou forem demasiado pesadas, otimize-as para ficarem mais leves antes de fazer deploy. O tamanho recomendado de textura para cenas com bom desempenho é 1024x1024. Verifique também a resolução; imagens com mais de 72 DPI afetarão o desempenho e não farão a imagem parecer melhor.


## Texture Atlas/ Trim Sheets

Um texture atlas é um único ficheiro de imagem que contém dados de várias imagens mais pequenas agrupadas. Em vez de ter uma textura para cada mesh, várias meshes partilham uma textura maior.

Pode criar um texture atlas antes de fazer o asset, o que significa que o asset é desdobrado em UV de acordo com o texture atlas. Isto requer algum planeamento antecipado ao criar a textura.

Em alternativa, pode criar o texture atlas depois de o asset estar concluído, fundindo texturas em software de pintura. No entanto, isto também significa que as ilhas UV têm de ser reorganizadas de acordo com a textura.

Abaixo está uma imagem que mostra vários objetos 3D que usam um único conjunto de texturas:

![](/docs/_remote/ed4a55e913cda2f92bcb60db.webp) ![](/docs/_remote/d4e59128a0be5c8f7414ad23.webp)

Outra forma de criar atlases é usar Trim Sheets, uma técnica comum em modelação 3D e desenvolvimento de jogos que grandes estúdios e a indústria dos jogos usam para garantir consistência visual e um pipeline de arte eficiente. Trim Sheets é o uso de uma única imagem ou textura que contém vários pequenos detalhes ou elementos que podem ser aplicados a diferentes partes de um modelo. Esta técnica é muito útil ao criar cenas grandes e deve ser considerada na primeira fase do pipeline de arte.

![](/docs/_remote/6e219acc9852ed30d1a9b768.webp)

*Um exemplo em colagem de um Trim Sheet que usa uma textura diffuse, normal e emissive.*

Há um excelente [tutorial](https://www.artstation.com/blogs/jennifermcgarry/yd4Q/jenns-guide-to-trim-sheets) por [Jennifer McGarry](https://www.artstation.com/jennifermcgarry/blog) que explica a utilização de Trim Sheets usando o Blender!

## Nomeação de Texturas

**É crucial nomearmos corretamente as nossas texturas.** Ter uma nomeação correta para a textura irá:

* Tornar o seu pipeline de arte mais eficiente, flexível, organizado, fácil de direcionar e modificar se necessário.
* Evitar a sobreposição de texturas com o mesmo nome ao usar o texture extractor ou outras ferramentas.
* Evitar problemas de sobreposição de texturas ao usar Asset Bundles.
* Forma mais eficiente de resolver problemas específicos ao analisar a cena.

**Como nomear corretamente as suas Textures:**

* **Nome do asset** deve representar claramente o que a textura é.
* O nome das Textures deve começar com o prefixo `T_`.
* O nome da textura deve terminar com o sufixo que define o tipo de textura:
  * `_D` - **Diffuse/ Color Map**
  * `_A` - **Textura Alpha**
  * `_MT` - **Metallic**
  * `_R` - **Roughness**
  * `_N` **- Normal Map**
  * `_E` - **Emissão**

Exemplo: Se for um diffuse map de textura de tijolo para uma parede, o nome `T_BrickWall_D` pode ser apropriado. Se for um Normal Map do mesmo asset, o nome pode ser `T_BrickWall_N`

**Exemplos:**

* 🟢 **Prefira** a começar o nome da textura com - `T_Parquet_D`, `T_Floor_R`, `T_Pipes_MT`,
* 🔴 **Evite** a começar o nome da textura com - `Image_`, `sprite_`,`Sem título`

## Ferramentas Opcionais

Existem muitos addons e ferramentas externas que facilitam o trabalho ao criar assets, tornando o pipeline mais rápido e eficiente; alguns são gratuitos e outros pagos, para citar alguns:

### UVTools

* **UV Packer (Gratuito)**:<https://www.uv-packer.com/blender/>
* **Zen UV:** <https://blendermarket.com/products/zen-uv>
* **Uvpackmaster 3:** <https://blendermarket.com/products/uvpackmaster>

### Comressores de Imagem

Existem vários compressores de imagem online que pode usar para tornar as suas texturas mais leves. Para citar alguns:

* **CompressPNG:** <https://compresspng.com/>
* **TinyPNG:** <https://tinypng.com/>
* **FreeConvert:** <https://www.freeconvert.com/>


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-pt/modelacao-3d-e-animacoes/textures.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
