> Saiba que propriedades de mesh de chapéu são suportadas em modelos 3D importados para o Decentraland.

# Meshes

Modelos 3D têm uma *mesh* composta por faces triangulares *faces*. Essas faces se encontram nas *arestas* (as linhas ao longo das quais elas se tocam) e *vértices* (os pontos onde seus cantos se unem).


> [!NOTE]
> **💡 Dica**: Instale o [plugin Blender do Decentraland Tools](https://extensions.blender.org/add-ons/decentraland-tools/). Ele inclui várias funções úteis para ajudar você a editar e exportar modelos 3D, wearables e emotes.


## Limitações de espaço

Todos os modelos 3D na sua cena devem caber dentro dos limites dos seus parcels. Se qualquer parte dos seus modelos se estender além desses limites ao executar uma prévia, essas partes que excederem serão cortadas e não renderizadas.

![](/docs/_remote/9ba86deac82448c396d45bba.webp)

No editor do Creator Hub, você verá o modelo tingido de vermelho e sua caixa delimitadora marcada em vermelho.

![](/docs/_remote/ac6b1ca6eff1a48fc7dfe75d.webp)


> [!NOTE]
> **📔 Nota**: No antigo cliente Web do Decentraland, por motivos de desempenho, se qualquer parte de um modelo 3D estivesse fora dos limites, mesmo que fosse apenas a bounding box, então o modelo inteiro não era renderizado. No cliente desktop do Decentraland 2.0, apenas as partes de um modelo que estão fora dos limites são cortadas. As partes do modelo que estão dentro da cena são renderizadas.


## Bounding Boxes

Se você tiver um modelo com todos os seus vértices bem dentro da área da cena, mas que tenha caixas delimitadoras grandes, em sua maioria vazias, e que se estendam além dos limites da cena, o modelo inteiro será marcado como fora dos limites da cena no Creator Hub. As partes do modelo que estiverem dentro dos seus parcels serão renderizadas corretamente, e você poderá publicar sua cena.

Toda mesh tem uma bounding box, que envolve os limites da forma. Para tornar um modelo 3D mais utilizável dentro do Decentraland, certifique-se de que suas bounding boxes não se estendam além do modelo mais do que o necessário.

Para evitar esse problema, você pode limpar seus modelos 3D para redefinir as posições e rotações dos meshes, de modo que as bounding boxes não se estendam além dos meshes que elas envolvem.

![](/docs/_remote/7c3c5e5108be6fd7e142dbf6.webp)

*No Blender, você pode fazer isso selecionando os objetos, pressionando Ctrl+A e depois aplicando “All transforms”.*

Por exemplo, tenha cuidado ao girar um sub-mesh perto da borda do seu modelo. Como as bounding boxes são cubos, mesmo que a mesh seja redonda, os cantos da sua bounding box podem acabar se projetando depois de girá-la em 45°.

![](/docs/_remote/15f51a636f43103faeefa105.webp)

Recomendamos que você faça bake da rotação e da escala de cada mesh no modelo, para garantir que não haja bounding boxes indesejadas se estendendo além do tamanho que precisam ter.

## Limitações de triângulos

Como o Decentraland é um mundo aberto cheio de diferentes cenas e objetos que são baixados dinamicamente (no mesmo espaço 3D), é importante otimizar nossos meshes para ter um bom desempenho enquanto jogamos. Nesse sentido, há algumas limitações da cena que sempre precisamos ter em mente quando se trata de meshes:

> n representa o número de parcels que uma cena ocupa.

* **Triangles:** n x 10000 Quantidade total de triângulos para todos os modelos na cena.
* **Altura:** 330 metros, independentemente do número de parcels.
* **Tamanho do arquivo:** 15 MB por parcel - 300 MB no máximo Tamanho total dos arquivos enviados para o servidor de conteúdo. Inclui modelos 3D e áudio. Não inclui arquivos que não são enviados, como pacotes node.js.
* **Tamanho máximo do arquivo: 50 MB por arquivo** Nenhum arquivo individual de qualquer tipo na cena pode exceder 50 MB, não importa quantos parcels a cena tenha.

Para mais informações, confira todas as limitações da cena [aqui.](/docs/creator/sdk7/optimizing/scene-limitations/)


> [!WARNING]
> **🔥Dica de otimização🔥: Adicionar contagem de polígonos** Uma dica valiosa é sempre acompanhar a contagem de polígonos dos seus modelos. Para fazer isso no Blender, você precisa ativar as estatísticas no painel de sobreposições do viewport.
> 
> <img src="/docs/_remote/11ef35913355fa3c25f5a19a.webp" alt="" data-size="original">


## Meshes em cenas grandes

Ao criar meshes, devemos ter em mente estas 2 boas práticas:

* **Modularidade:** Divida meshes grandes em partes menores e modulares. Por exemplo, em vez de ter uma única mesh para um edifício inteiro, o edifício poderia ser dividido em meshes separadas para cada parede, piso e seção da construção. Isso permite que o frustum culling do Unity funcione de forma mais eficaz, já que apenas as partes visíveis precisam ser consideradas para renderização. Isso também melhorará o uso de memória, porque uma mesh pode ser reutilizada várias vezes para o mesmo objeto em locais diferentes (para conseguir isso, você precisará referenciar a mesma mesh na definição da sua cena SDK7 ou usar instances em vez de objetos duplicados ao exportar a cena do Blender).
* **Volume delimitador**: Garanta que o volume delimitador de cada objeto se encaixe o mais firmemente possível. Espaço excedente no volume delimitador pode fazer com que objetos sejam renderizados quando não estiverem visíveis.

![](/docs/_remote/85cd1390893917a4f9f84647.webp)

### O que é Frustum Culling?

Frustum Culling é uma técnica de otimização que desativa os renderers (meshes) dos objetos que estão fora da área de visão da câmera. Veja estes 2 exemplos:

![](/docs/_remote/aa272a10f2f7825fafd9b7e8.webp)

*Um nível interno em estilo labirinto. Esta visualização normal da cena mostra todos os Game Objects visíveis.*

![](/docs/_remote/2581f915e835c33d658e7101.webp)

*O frustum culling normal renderiza apenas os objetos dentro da visão da câmera. Isso é automático e sempre acontece.*

## Instanciando objetos vs duplicando objetos

No Blender, duplicar um objeto cria uma cópia completamente separada do objeto, enquanto instanciar um objeto cria uma duplicata que compartilha os mesmos dados do objeto original.

Quando um objeto é duplicado, ele cria um novo objeto com um conjunto de dados totalmente independente, incluindo toda a sua geometria, materiais e animações. Isso significa que quaisquer alterações feitas no objeto original não serão refletidas no objeto duplicado, e vice-versa.

Por outro lado, quando um objeto é instanciado, quaisquer alterações feitas no objeto original serão refletidas em todas as suas instâncias, e vice-versa. Isso acontece porque todas as instâncias compartilham os mesmos dados subjacentes.

Em termos de desempenho, instanciar pode ser muito mais eficiente do que duplicar, especialmente ao trabalhar com cenas complexas ou grandes quantidades de objetos. Isso acontece porque instanciar usa menos memória do que duplicar, já que não cria novos dados para cada instância. Além disso, instanciar pode permitir que o Blender otimize o processo de renderização tratando todas as instâncias como um único objeto, em vez de renderizar cada duplicata separadamente.

No geral, instanciar pode ser uma ferramenta poderosa para melhorar o desempenho e a eficiência do fluxo de trabalho no Blender e na sua cena do Decentraland, especialmente ao trabalhar com cenas grandes e complexas.

![](/docs/_remote/403e1a531d34d0b12ef576b8.webp)

Em alguns casos, ao duplicar objetos como árvores, plantas e postes de luz, instanciar pode melhorar significativamente o desempenho em comparação com a duplicação. Por exemplo, esta cena tem quatorze postes de luz, e todos eles são exatamente iguais. Sabemos que cada poste de luz tem duas texturas diferentes, uma para o poste e outra para a luz. Então, nesse caso, teremos dois draw calls, um por textura.

![](/docs/_remote/b4811452757e72b24eb01716.webp)

No Blender, quando você está na etapa de clonar e posicionar elementos no espaço, você tem três opções. Duplicar, Instanciar ou Mesclar objetos.

Vamos analisar os prós e contras de cada procedimento.

### Duplicar objetos

| Menu:   | Object ‣ Duplicate Objects |
| ------- | -------------------------- |
| Atalho: | Shift-D                    |

Isso criará uma cópia visualmente idêntica do objeto selecionado, mas eles serão tratados como objetos diferentes. Então, ao exportar, ele terá 2 draw calls por objeto, 2 \* 14 = 28 draw calls, e um uso total em disco de 320 KB.

![](/docs/_remote/4a2e11a5d69b78b82bfaeffb.webp)

### Instanciando objetos

| Painel: | Toolbar ‣ Tools ‣ Edit ‣ Duplicate Linked |
| ------- | ----------------------------------------- |
| Menu:   | Object ‣ Duplicate Linked                 |
| Atalho: | Alt-D                                     |

Isso criará um novo objeto com todos os seus dados vinculados ao objeto original. Se você modificar um dos objetos vinculados no Edit Mode, todas as cópias vinculadas serão modificadas. As propriedades de Transform continuam sendo cópias, não vínculos, então você ainda pode girar, escalar e mover livremente sem afetar as outras cópias.

![](/docs/_remote/2366fb1c01bf3e7b09b2e7f4.webp)

*Quando exportamos esses modelos, ainda teremos o mesmo número de draw calls (28). No entanto, observe que agora temos apenas uma mesh, o que reduz significativamente o uso em disco (41 KB).*

![](/docs/_remote/8af76e55acae51d4209ab721.webp)

*Para manter tudo organizado, você pode simplesmente instanciar collections na sua cena; dessa forma, você pode controlar todas as instâncias a partir de uma única pasta e origem.*


> [!NOTE]
> **💡 Dica**: A mesma lógica se aplica do lado do SDK. Quatorze entities que todas apontam seus `GltfContainer` para o mesmo *.glb* arquivo funcionam como instâncias: o arquivo é baixado uma vez, convertido uma vez, e seus meshes e texturas ficam na memória uma única vez, não importa quantas entities o usem. Dividir o mesmo conteúdo em quatorze *.glb* arquivos quase idênticos é o equivalente de duplicar, e custa a você quatorze downloads e quatorze cópias na memória.


### Mesclar objetos

| Menu:   | Object ‣ Join |
| ------- | ------------- |
| Atalho: | Ctrl-J        |

Isso une os objetos selecionados em um único objeto com uma única mesh. Funciona no Object Mode, mescla tudo no último *active* objeto selecionado e exige que todos os objetos selecionados sejam do mesmo tipo. Os Materials são mantidos e mesclados, então nossos quatorze postes de luz deixam de ser quatorze objetos renderizáveis e se tornam um só, e o engine os desenha com 2 draw calls em vez de 28 — um por material, uma vez.

Essa é a única das três opções que realmente reduz draw calls, e vale deixar claro por quê: o engine do Decentraland constrói sua cena em runtime a partir de conteúdo transmitido, então ele não pode agrupar ou fazer batch de objetos repetidos para você, como um engine pode fazer com conteúdo que foi preparado previamente. Se você precisa de menos draw calls, a mesclagem precisa acontecer no Blender.

Essa redução não é gratuita. Mesclar abre mão de várias coisas ao mesmo tempo:

* **O uso em disco volta a aumentar.** A mesh mesclada armazena a geometria dos quatorze postes, então você perde a economia que a instanciação lhe dava.
* **O frustum culling se torna tudo-ou-nada.** Um objeto mesclado é desenhado por completo sempre que qualquer parte dele está na tela. Quatorze postes espalhados ao longo de uma rua quase sempre terão um poste visível, então você acaba desenhando os quatorze o tempo todo — o que pode facilmente custar mais do que os draw calls que você economizou.
* **As partes deixam de ser acessíveis individualmente.** Você não pode mais mover, animar, clicar ou ocultar um único poste, porque só resta um objeto.

Use a mesclagem de forma limitada: para muitos pequenos props que sempre são vistos juntos em um único ponto, que nunca são interagidos e apenas depois de confirmar que o número de objetos é o que realmente está prejudicando o desempenho. Para todo o resto, instanciar é a melhor opção padrão.


> [!WARNING]
> **📔 Nota**: Exportar quatorze objetos separados em um único *.glb* arquivo é **não** mesclagem. O engine ainda vê quatorze objetos e ainda emite 28 draw calls. Apenas unir as meshes muda a contagem.


## Nomeação de meshes

Use nomes significativos para os seus meshes. O nome deve dar contexto de onde o asset é usado ou a que parte do objeto ele se relaciona.

Para nomear assets, use uma mistura de `PascalCase` e `snake_case`, que podemos chamar de `PascalSnake_case`. As regras básicas são - uma nova palavra ou uma palavra após um separador (`_`) começa com **letra maiúscula.**

* Use nomes significativos para os seus meshes **Exemplos:** 🟢 **Prefira** nomes - `Teatro`, `Bonde` 🔴 **Evite** nomes - `Sem título.008`, `primitiva(1)`, `Cilindro.091`, `_esfera-AB`
* Use underscore `_` como separador para dar mais contexto ao nome. Coloque primeiro as coisas mais comuns e, no final, os detalhes específicos. **Exemplos:** 🟢 **Prefira** nomes - `EstatuaFonte`, `ParedePrincipalTeatro`, `RodaEsquerdaDoBonde` 🔴 **Evite** nomes - `Norte_ParedePrincipal_Teatro`, `Bonde_RodaFrontalEsquerda`

Se houver mais de um objeto com o mesmo nome, adicione um número depois da palavra: `EstatuaFonte01, EstatuaFonte02, etc.`

## Ferramentas para criar modelos

Existem muitos addons e ferramentas externas que facilitam o trabalho ao criar assets, tornando o pipeline mais rápido e eficiente; alguns são gratuitos e outros pagos, para citar alguns:

### Decimate

Esse é um modificador bem conhecido que pode ser usado para reduzir a quantidade de tris da sua mesh, mantendo a estrutura da superfície do modelo. Embora seja uma ferramenta muito poderosa para otimizar modelos, leve em conta que, uma vez que o modelo é decimado, a topologia pode ser afetada, causando uma má topologia (porque ela quebra a geometria em partes imprevisíveis). Por outro lado, a decimação converterá os quads da sua mesh em triângulos, tornando difícil modificá-la depois de usada. Nesse sentido **a decimação pode ser útil para modelos estáticos, mas não é a opção preferida para modelos animados.**

![](/docs/_remote/5c1b597b93cffd4692e756c8.webp) ![](/docs/_remote/ffc83dd73f6105e78f5d4ed1.webp)

Outra maneira de otimizar seus meshes usando decimação é decimar o modelo e depois fazer uma retopologia adequada. Assim, você pode garantir um resultado mais previsível e uma topologia limpa.

Alguns addons de retopologia que podem ser úteis são (alguns deles são pagos):

* **Simplygon:** <https://www.simplygon.com/>
* **Speedretopo:** <https://blendermarket.com/products/speedretopo?ref=2>
* **Retopoflow:** <https://blendermarket.com/products/retopoflow>

### Mirror Modifier

O Mirror Modifier no Blender é uma ferramenta usada para criar modelos simétricos espelhando uma metade de um objeto em qualquer direção (X,Y,Z). É uma ótima ferramenta quando você tem modelos simétricos porque ela reduz o tempo de modelagem 3D, do UV Unwrapping (porque você precisará desdobrar apenas uma parte do modelo e o resto compartilhará as mesmas coordenadas UV) e também otimiza suas texturas, evitando desperdiçar resolução de textura em partes que são essencialmente espelhadas.

![](/docs/_remote/d2acc97f8cfac981e2ef4ed3.webp)

### Batch Rename

Uma ferramenta muito útil para mudar convenções de nomes de forma fácil é usar o Batch Rename que o Blender tem integrado ao seu toolkit. Para acessá-lo, vá para *Edit* e depois *Batch Rename*.

![](/docs/_remote/b586dae5ffc349455527b2fc.webp)

Selecione os objetos cujo nome você quer alterar e simplesmente substitua o nome padrão do objeto pelo novo. Essa ferramenta oferece diferentes métodos e tipos de asset para renomear, então ela é muito flexível para mudar os nomes também de meshes, animações, bones etc.

![](/docs/_remote/ff8b93deb811edcecb68aeba.webp) ![](/docs/_remote/7934176b00dd96ce2f519b23.webp)

## Boas práticas para geometrias [#](/docs/creator/3d-modeling/meshes/#best-practices-for-geometries)

* Tenha cuidado com quantas faces você adiciona aos seus modelos 3D, pois mais faces tornam sua renderização mais exigente. Veja [**limitações da cena**](/docs/creator/sdk7/optimizing/scene-limitations/) para os limites impostos por uma cena.
* Certifique-se de que não há faces ocultas que não podem ser vistas, mas que aumentam a contagem de triângulos.
* Para formas que devem ter lados arredondados, defina-as como *smooth* em vez de adicionar faces extras.
* Certifique-se de que as *normais* de todas as faces estejam voltadas para fora, em vez de para dentro. Se houver faces no seu modelo que parecem não estar lá quando você o renderiza, essa é provavelmente a causa.
* Faça bake da rotação e da escala dos seus meshes, para que suas caixas delimitadoras não se estendam desnecessariamente.


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-pt/modelacao-3d-e-animacoes/meshes.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
