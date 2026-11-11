> Saiba como criar animações que podem ser incorporadas em modelos 3D importados para o Decentraland.

# Animações

Animação é a arte de dar vida às coisas. E não há maneira melhor de tornar a sua scene mais dinâmica do que adicionar algumas animações aos seus modelos 3D.


> [!NOTE]
> **💡 Dica**: Instala o [plugin Decentraland Tools para Blender](https://extensions.blender.org/add-ons/decentraland-tools/). Inclui várias funções úteis para te ajudar a editar e exportar modelos 3D, wearables e emotes.


Há algumas maneiras de fazer isso: por meio de **animação de objetos**, por meio de um **rig (animação esquelética)**, ou usando **shape keys (animação de morph targets)**.

**Animação de objetos** é melhor para modelos simples, como uma bola quicando, um globo girando ou uma cadeira flutuante, e não precisa de uma armature. É importante mencionar que a animação de objetos é diferente de **vertex animation**. Na animação de objetos, o modelo será animado como um todo, enquanto na vertex animation cada vertex do objeto pode ser animado separadamente.

**shape keys (também conhecidas como morph targets ou blend shapes)** são uma forma de vertex animation que é **suportada pelo Engine do Decentraland**. Elas são especialmente úteis para expressões faciais, lip sync ou qualquer deformação que seja difícil de alcançar apenas com bones. Animações de shape keys incorporadas em um arquivo glTF/GLB serão reproduzidas in-world da mesma forma que animações esqueléticas ou de objetos.

Se você tiver um modelo mais complexo, como uma pessoa, criatura ou maquinário, então precisará de um rig. **rig**. Um rig nada mais é do que um esqueleto digital que moverá e deformará o mesh. O processo de vincular o mesh ao esqueleto chama-se Skinning, no qual você define qual bone afetará cada vertex group e quão forte essa influência será, garantindo que o model se deforme da melhor maneira possível.

Para qualquer um dos métodos, no entanto, todas as animações de um modelo 3D devem estar incorporadas dentro do seu *glTF* arquivo, já que você não pode referenciar animações em arquivos separados.

## Criando uma Animação

Veja como criar animações usando Blender.

### Animação de Objeto

* Certifique-se de que você tenha a ***Dope Sheet*** > ***Action Editor*** aba aberta e clique em ***New*** para adicionar um clipe de animação.
* Renomeie-o como preferir e certifique-se de ativar o ***Fake User*** (o ícone de escudo) para que sua animação seja salva.
* Defina um keyframe no primeiro frame selecionando o objeto e pressionando ***I***. Em seguida, mova o objeto, gire-o ou redimensione-o e, em outro frame na timeline, pressione ***I*** novamente para definir outro keyframe com os transforms atuais.
* O frame final da animação deve ser o mesmo que o primeiro, então basta copiar o primeiro frame e colá-lo no último.

![](/docs/_remote/da0a0df92981981477fc8dbd.webp)

*Criando um clipe de animação e adicionando keyframes.*

### Animação de Rig

Para uma animação de rig, você precisará de uma armature. Se quiser fazer isso você mesmo, veja [Criar um Rig](https://www.notion.so/686e6f59a1604585b059f990a36b2d55) para instruções sobre como configurar uma armature consistente.

Quando o rigging estiver pronto e o skinning estiver com boas deformações, você estará pronto para começar sua animação! O processo é muito semelhante à Object Animation, mas em vez de fazê-lo em ***Object Mode***, você irá animá-lo em ***Pose Mode***.

* Vá para ***Pose Mode*** e certifique-se de que você tenha o ***Dope Sheet*** > ***Action Editor*** aba aberta e clique em ***New*** para adicionar um clipe de animação.
* Renomeie-o como preferir e certifique-se de ativar o ***Fake User*** (o ícone de escudo) para que sua animação seja salva.
* Com o mouse sobre o ***Viewport***, pressione ***Um*** para selecionar tudo e então ***I*** para definir um keyframe para toda a armature.
* Vá para um frame diferente na timeline e manipule o bone como quiser para obter a pose desejada. Você pode alterar a localização, rotação e escala de um bone dependendo de como configurou seu rig.
* Quando tiver um bom resultado, defina outro keyframe para toda a armature. Continue fazendo isso até terminar sua animação. Lembre-se de manter o primeiro e o último frames iguais se a animação for entrar em loop.

![](/docs/_remote/aa4ecd75ce5fa54f9c60066e.webp)

Se você é novo no processo de animação, confira este tutorial com algumas dicas úteis para iniciantes:

[![Video Preview](/docs/_remote/629c65271f6a14386ffd42f7.webp)](https://youtu.be/-iWslh4uQIk)

## Animações com Skinning vs Animações de Transform

**As animações de Transform geralmente são melhores do que as animações de skinned mesh em desempenho, porque envolvem menos sobrecarga em termos de recursos computacionais necessários para exibir a animação.**

A animação de skinned mesh envolve usar um mesh com um skeleton de bones com weights atribuídos aos vertices do mesh. O skeleton é animado, o que por sua vez anima o mesh. Esse processo pode ser bastante intensivo em termos computacionais, especialmente quando há um grande número de vertices para animar.

As animações de Transform, por outro lado, envolvem animar diretamente as propriedades de Transform de um objeto (como sua posição, rotação e escala). Isso pode ser feito com menos recursos computacionais do que a animação de skinned mesh, já que normalmente há menos propriedades de Transform para animar do que vertices em um mesh.

Além disso, as animações de Transform muitas vezes podem ser pré-calculadas e armazenadas em um formato mais compacto, como dados de keyframe, que podem ser acessados e reproduzidos rapidamente sem a necessidade de cálculos intensivos em tempo real.

No geral, embora a animação de skinned mesh possa produzir animações mais detalhadas e realistas, as animações de Transform costumam ser preferidas para aplicações críticas de desempenho, como videogames.

## Criando e Exportando Vários Clipes de Animação

Você pode ter quantos clipes de animação quiser para o seu model. Você pode conferir como criar, navegar e excluir animações em [esta seção aqui](https://www.notion.so/5e962e5d54a24bcb9b906748007eb4cc).

No vídeo abaixo, você aprenderá como exportar várias animações do Blender em um único arquivo GLB.

[![Video Preview](/docs/_remote/eb23ea78b0ffa9a0778e60d6.webp)](https://www.youtube.com/watch?v=YxAB4bujO_w&ab_channel=Decentraland)

## Exportando Muitas Animações como Uma Só

Ao contrário de uma animação esquelética, em que você pode rigar vários objects e combinar suas animações em um único clipe de animação, na object animation você não pode fazer isso. As animações serão exportadas separadamente e apenas um objeto será reproduzido por vez. Uma solução para isso é mesclar várias animações em uma só, para que você tenha um clipe de animação com todos os objetos se movendo ao mesmo tempo.

Para fazer isso, envie para a ***Nonlinear Animation Track*** a animação de cada objeto, como mostrado abaixo.

![](/docs/_remote/03413441d7a33b6e5298b18f.webp)

*Envie a animação de cada objeto para a NLA Track.*

Depois que todas as animações estiverem listadas no NLA Editor, selecione todos os objects e vá para **File > Export > glTF2.0**. Expanda Include e marque Selected Objects. Expanda Animation, expanda Animation novamente e desmarque **Group by NLA Track**. Você pode renomear o clipe de animação em Merged Animation Name e então basta pressionar **Export glTF 2.0**.

![](/docs/_remote/9ba538ff6074491ba9311c8c.webp)

*Configurações para exportar várias animações como uma só.*

Tenha em mente que isso só funcionará se os objects tiverem uma única animação cada. Se os objects tiverem vários animation clips, é melhor exportá-los separadamente.


> [!NOTE]
> **💡 Dica**: Em vez de criar suas próprias animações, você também pode baixar animações genéricas e aplicá-las ao seu modelo. Por exemplo, para personagens 3D com características humanas, você pode baixar animações gratuitas ou pagas do [Mixamo](https://www.mixamo.com/#/).


## Amostragem

Às vezes, o seu arquivo de animação pode acabar ficando pesado demais devido à quantidade de animações ou à combinação animação+modelo.

A taxa de amostragem é uma boa forma de otimizar a animação. A taxa de amostragem define com que frequência um keyframe será gerado na animação. Por exemplo, se a taxa de amostragem estiver definida para 2, isso significa que será criado um keyframe a cada dois frames. Uma taxa de amostragem de 3 irá gerar um keyframe a cada três frames, e assim por diante. Quanto maior a taxa de amostragem, mais leve fica o ficheiro.

A desvantagem, porém, é que a animação começará a ficar cada vez menos fluida, já que perde alguns keyframes importantes (eles são distribuídos pela animação de forma desigual). Também é importante observar que **sampling NÃO é dividir o número de frames da animação pela taxa de sampling**.

Normalmente, uma **taxa de amostragem de 2 ou 3** é suficiente. Esses números podem otimizar a animação sem comprometer a qualidade.

Você pode encontrar o ***Taxa de amostragem*** nas configurações de exportação, em ***Animation***.

![](/docs/_remote/942221d28977584ca4c825da.webp)


> [!NOTE]
> **💡 Dica**: Se o número de frames da animação puder ser dividido pela taxa de sampling, isso é ótimo! Isso significa que o frame final será baked, preservando a transição do fim para o início da animação.


## Implementando Animações

Este documento cobre como adicionar animações em um modelo 3D. Veja [**handle animations**](/docs/creator/sdk7/3d-essentials/3d-model-animations/) para instruções sobre como ativar e lidar com animações em uma scene.

## Melhores Práticas para Animações

* Mantenha a armature o mais simples possível, criando bones apenas para as partes do modelo que você pretende animar. Bones podem afetar o desempenho, então certifique-se de adicionar apenas o que é realmente necessário.
* Se a animação for entrar em loop na sua scene, certifique-se de que a pose final seja idêntica à inicial para uma transição melhor.
* Nunca deixe um character sem animação, mesmo que ele não esteja realmente fazendo nada. Crie uma animação "idle" para quando o character estiver parado. O idle pode incluir movimentos sutis como respirar e talvez olhares ocasionais.
* Não deixe bones sem pose, como hands, fingers, head ou neck. Os detalhes são realmente importantes em uma boa animação e movimentos rígidos só a tornarão menos convincente.
* Evite keyframes demais, a menos que seja extremamente necessário. Quanto mais keyframes você adicionar, maiores as chances de obter uma interpolação ruim e uma animação mais pesada.
* Selecione todos os seus bones (incluindo os de deformação) e defina um keyframe no primeiro e no último frame da sua animação. Isso evitará bones sem informação, fazendo com que uma animação afete a outra involuntariamente.
* Certifique-se de que seu arquivo tenha apenas uma armature quando você o exportar. Ao importar animações, uma armature também será importada com ela. Todas as animações devem ser executadas pela mesma armature base, então exclua tudo o que você não precisar.
* Sempre renomeie seus bones e animações. Mantenha tudo organizado!
* Ao exportar o *glTF* modelo, confirme que está exportando todos os objects e animações. Alguns exporters exportarão apenas o ***atualmente selecionado*** por padrão.
* Depois de exportar o modelo, inspecione-o em [Babylon Sandbox](https://sandbox.babylonjs.com/) e verifique se todas as animações estão funcionando e se estão nomeadas como esperado.


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-pt/modelacao-3d-e-animacoes/animations.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
