> Saiba quais assets e components são suportados em modelos 3D externos e como configurá-los antes de importá-los para o Decentraland.

# Modelos 3D

![](/docs/_remote/62bf81e2e6497cde955d408a.webp)

Ao importar modelos 3D para a Decentraland, é importante garantir que estão num formato glTF. Estes modelos podem incluir uma variedade de funcionalidades suportadas. Esta secção explica como torná-los compatíveis com a Decentraland e seguir as melhores práticas.


> [!NOTE]
> **💡 Dica**: Instale o [plugin Blender do Decentraland Tools](https://extensions.blender.org/add-ons/decentraland-tools/). Ele inclui várias funções úteis para ajudar você a editar e exportar modelos 3D, wearables e emotes.


Note que todos os modelos, shaders e texturas devem cumprir com as [**limitações da cena**](/docs/creator/sdk7/optimizing/scene-limitations/).

## **Formatos de Modelos 3D Suportados**

Todos os modelos 3D na Decentraland devem estar em formato glTF. [**glTF**](https://www.khronos.org/gltf) (GL Transmission Format) é um projeto aberto da Khronos que fornece um formato comum e extensível para assets 3D, eficiente e altamente interoperável com as tecnologias web modernas.

Os modelos glTF podem ter uma *.gltf* ou um *.glb* extensão. Os ficheiros glTF são legíveis por humanos, pode abrir um num editor de texto e lê-lo como um ficheiro JSON. Isto é útil, por exemplo, para verificar se as animações estão corretamente associadas e para confirmar os seus nomes. Os ficheiros glb são binários, por isso não são legíveis, mas são consideravelmente menores, o que é bom para o desempenho da cena.

Recomendamos usar *.gltf* enquanto estiver a trabalhar numa cena, mas depois mudar para *.glb* ao fazer o upload.

Os seguintes aspetos de um modelo 3D podem ser incorporados num *glTF* ficheiro ou referenciados externamente:

* As texturas podem ser incorporadas ou referenciadas a partir de um ficheiro de imagem externo.
* Os dados binários sobre geometria, animações e outros aspetos relacionados com buffers do modelo podem ser incorporados ou referenciados a partir de um ficheiro externo *.bin* arquivo.


> [!WARNING]
> **📔 Nota**: Animações *devem* ser incorporadas dentro do *glTF* ficheiro para ser usado na Decentraland. Além disso, cada modelo só pode ter 1 mapa UV.


## **Porque Usamos glTF**

Usamos glTF porque fornece um conjunto poderoso de funcionalidades que inclui:

* Objetos hierárquicos
* Estrutura esquelética e animação
* Pipeline robusto para materiais e shaders
* Informação da cena, como fontes de luz e câmaras (que podem ser usadas em desenvolvimentos futuros).

Comparado com *COLLADA*, as funcionalidades suportadas são muito semelhantes. No entanto, como o glTF se foca em fornecer um “formato de transmissão” em vez de um formato de editor, é mais interoperável com as tecnologias web.

## **Exportar e Converter glTF**

### **Exportar para glTF a partir do Blender**

O Blender tem um exportador glTF nativo.

Para exportar, selecione os modelos que quer que fiquem contidos no ficheiro .glb e depois vá a File - Export - glTF 2.0

![](/docs/_remote/1f95091f3ce5ddb4219f6eea.webp) ![](/docs/_remote/3edef4656ec7f2427b2d9dbe.webp)

*No painel lateral marque “Selected Objects” caso queira exportar apenas os objetos selecionados na cena. Além disso, no menu suspenso Mesh selecione Apply Modifiers (se existirem), e depois Export.*

### **Exportar para glTF a partir do 3D Studio Max**

O 3D Studio Max não suporta exportar para glTF por predefinição, mas pode instalar um plugin para o ativar.

1. Descarregue o plugin a partir de [esta ligação](https://github.com/BabylonJS/Exporters/tree/master/3ds%20Max).
2. Instale o plugin seguindo [estas instruções](http://doc.babylonjs.com/resources/3dsmax#how-to-install-the-3ds-max-plugin).
3. Exporte ficheiros glTF usando o plugin seguindo [estas instruções](http://doc.babylonjs.com/resources/3dsmax_to_gltf).

### **Exportar para glTF a partir do Maya**

O Maya não suporta exportar para glTF por predefinição, mas pode instalar um plugin para o ativar.

1. Instale o plugin seguindo [estas instruções](http://doc.babylonjs.com/resources/maya).
2. Exporte ficheiros glTF usando o plugin seguindo [estas instruções](http://doc.babylonjs.com/resources/maya_to_gltf#pbr-materials).


> [!WARNING]
> **📔 Nota**: Como alternativa, pode experimentar [este outro plugin](https://github.com/WonderMediaProductions/Maya2glTF) também.


### **Exportar para glTF a partir do Unity**

O Unity não suporta exportar para glTF por predefinição, mas pode instalar um plugin para o ativar.

Descarregue o plugin a partir de [esta ligação](https://github.com/sketchfab/Unity-glTF-Exporter).


> [!WARNING]
> **📔 Nota**: Como alternativa, pode experimentar [este outro plugin](https://assetstore.unity.com/packages/tools/utilities/collada-exporter-for-unity2017-99793) também.


### **Exportar para glTF a partir do SketchUp**

O SketchUp não suporta exportar para glTF por predefinição, mas pode instalar um plugin para o ativar.

Descarregue o plugin a partir de [esta ligação](https://extensions.sketchup.com/en/content/gltf-exporter).

### **Converter FBX em glTF**

*.fbx* é um padrão muito popular para modelos 3D. Não é suportado pelo nosso engine, mas pode facilmente exportar um *.fbx* modelo para *.gltf* formato.

Recomendamos usar estas ferramentas:

* [**Blender**](https://www.blender.org/download/): Uma forma fácil de converter .fbx para glTF é importar o .fbx para uma cena usando o Blender e depois exportá-lo a partir daí.
* [**a ferramenta CLI do Facebook**](https://github.com/facebookincubator/FBX2glTF): esta é a alternativa mais robusta, mas requer a utilização da linha de comando.
* [**Blackthread**](https://blackthread.io/gltf-converter): Esta é a ferramenta web mais completa. Menos robusta do que a CLI, mas muito mais fácil de usar.
* [**Modelconverter**](https://modelconverter.com/convert.html): Outra ferramenta web fácil de usar.

## **Pré-visualizar um Modelo glTF**

Uma forma rápida e fácil de pré-visualizar o conteúdo de um modelo glTF antes de o importar para uma cena é usar o [**Babylon.js Sandbox**](https://sandbox.babylonjs.com/). Basta arrastar e largar o ficheiro glTF (e o seu *.bin* ficheiro, se aplicável) para o canvas para ver o modelo.

No sandbox também pode pré-visualizar as animações incorporadas no modelo, selecionando quais mostrar a partir de um menu suspenso.

![](/docs/_remote/34159024016bd56402873a74.webp)

## **Analisar glTF**

Uma ótima ferramenta para analisar os seus glTFs é usar [**glTF Sample Viewer**](https://github.khronos.org/glTF-Sample-Viewer-Release/). Esta ferramenta permite-lhe inspecionar canais, texturas, a reação a diferentes ambientes de luz, pré-visualizar animações e muito mais! Ideal para modelos únicos.

![](/docs/_remote/173cb3c41baf0f4c374aea20.webp)

Outra ferramenta útil é [**gltf.report/**](https://gltf.report/). Basta arrastar e largar os seus modelos para o site para ver o conteúdo do pacote glTF e avaliar que partes do modelo podem ser otimizadas.

Esta ferramenta pode ser especialmente útil para ficheiros grandes que contenham vários modelos, texturas e animações. Por exemplo, ao organizar as texturas por tamanho, pode identificar facilmente quais estão a consumir mais recursos.

![](/docs/_remote/40647d97e437f4808c189320.webp)

## **Otimizar um glTF**

A ferramenta seguinte oferece algumas otimizações que tornarão os modelos 3D mais leves e mais rápidos de descarregar para os jogadores na sua cena.

[glTF pipeline](https://github.com/AnalyticalGraphicsInc/gltf-pipeline)

Entre outras coisas, converte *.gltf* o formato em *.glb*, que é binário e, por isso, ocupa muito menos. Também coloca os ficheiros de textura fora do modelo 3D, o que permite usar a mesma textura em vários modelos.


> [!WARNING]
> **📔 Nota**: *.glb* O formato por predefinição tem sempre as texturas incorporadas no ficheiro. O engine não consegue reconhecer duas texturas incorporadas como sendo a mesma; precisam de ser ficheiros externos que partilhem o mesmo hash.


## Ver também

As páginas seguintes também abordam tópicos relacionados com modelos 3D para a Decentraland:

* [Meshes](/docs/creator/content-creator-pt/modelacao-3d-e-animacoes/meshes/)
* [Materials](/docs/creator/content-creator-pt/modelacao-3d-e-animacoes/materials/)
* [Textures](/docs/creator/content-creator-pt/modelacao-3d-e-animacoes/textures/)
* [Colliders](/docs/creator/content-creator-pt/modelacao-3d-e-animacoes/colliders/)
* [Animations](/docs/creator/content-creator-pt/modelacao-3d-e-animacoes/animations/)
* [Recursos Úteis](/docs/creator/content-creator-pt/scenes-sdk7/primeiros-passos/useful-resources/): Add-ons do Blender, bibliotecas de recursos e ferramentas de IA para acelerar o seu fluxo de trabalho 3D


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-pt/modelacao-3d-e-animacoes/3d-models.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
