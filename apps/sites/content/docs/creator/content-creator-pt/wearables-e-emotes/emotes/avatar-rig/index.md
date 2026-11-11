> Noções básicas sobre o rig do avatar.

# Rig do Avatar

Um rig é um esqueleto virtual que permite que um modelo se mova. Ele consiste em uma hierarquia de ossos individuais, muito parecida com a de um esqueleto real, e funciona sob uma relação pai/filho. Este documento abordará alguns conceitos básicos de rigging, como posição dos ossos, orientação dos ossos, ossos deformáveis e não deformáveis, a diferença entre IK e FK e suas finalidades. A estrutura do rig de um avatar, atributos personalizados e a configuração para animar podem ser encontrados em [recursos do rig](/docs/creator/content-creator-pt/wearables-e-emotes/emotes/rig-features/).


> [!NOTE]
> **💡 Dica**: Instale o [plugin Decentraland Tools Blender](https://extensions.blender.org/add-ons/decentraland-tools/). Inclui várias funções úteis para o ajudar a editar e exportar modelos 3D, wearables e emotes.


## **Noções básicas**

#### Posição dos ossos ou pontos de pivô

Embora um rig não seja uma réplica exata do esqueleto humano, é uma boa prática seguir a posição dos ossos reais ao posicionar os digitais. Isso é feito para garantir uma deformação crível e fluida. A posição dos ossos é importante porque define o ponto de pivô (de onde o movimento começará). Um osso posicionado incorretamente causará uma má deformação da mesh. As imagens abaixo mostram a diferença na posição dos ossos.

![head\_pivot\_rig\_1.0.gif](/docs/_remote/19b528a3a24a1ffcdeaf8d5f.webp)

![head\_pivot\_rig\_2.0.gif](/docs/_remote/95fc62ca479c3f614187167d.webp)

#### Orientação dos ossos

A orientação dos ossos definirá em que direção eles irão girar, como X positivo, Z negativo etc. Eles podem ser configurados de muitas maneiras diferentes, desde que isso seja consistente em todo o rig. Por exemplo, se o eixo X for escolhido para o movimento de flexão para frente da coluna, faz sentido que a mesma configuração seja usada para as pernas. A orientação também é importante porque afetará o comportamento de espelhamento. Isso significa que, normalmente, a orientação do lado direito é a versão espelhada da do lado esquerdo, e vice-versa.

![](/docs/_remote/4d39fc26ef0dd267cbd54630.webp)

*Ative os eixos para mostrar a orientação dos ossos.*

![](/docs/_remote/8278804deb8abeeac7cba8ee.webp)

*Os eixos são as direções nas quais o osso irá girar.*

#### Ossos deformáveis e não deformáveis

Os ossos deformáveis são aqueles que deformarão a mesh; eles são responsáveis pela forma como o modelo se move. Esse é o esqueleto base e ele **não deve ser editado de forma alguma**. Alterá-lo de qualquer forma pode quebrar o rig, e a animação não funcionará quando exportada. Por esse motivo, esse rig base foi movido para sua própria camada (a última na parte inferior). Esses ossos são a armature exportada junto com a mesh.

Os ossos não deformáveis são aqueles que não deformarão a mesh, mas ainda são necessários em um rig robusto e são usados para a configuração de [IKs, FKs](#whats-fk-and-ik-in-a-rig) e outras propriedades personalizadas. Exemplos de ossos não deformáveis são controles, ossos de IK e FK, ossos de configuração dos pés. Eles não devem ser exportados, sendo usados apenas para fins de animação.

![](/docs/_remote/4587f7ead5c1973d96247db4.webp)

*Ossos deformáveis.*

![](/docs/_remote/8c4d5b63d6775310aef4709b.webp)

*Ossos não deformáveis.*


> [!WARNING]
> ⚠️ **Atenção!** **Não edite o esqueleto base de forma alguma!**


![](/docs/_remote/7856fc2ef29a54224be410db.webp)

*O esqueleto base.*

#### Controles

Como boa prática, um rig não deve ser animado manipulando os ossos deformáveis, pois isso pode fazer com que o rig quebre. Em vez disso, **as animações devem ser feitas manipulando controles.**

Controles são basicamente ossos não deformáveis, o que significa que eles não afetarão a mesh, tornando-os completamente seguros para serem manipulados e animados. Sua função é controlar o esqueleto base por meio de constraints e drivers, sem tocá-lo diretamente. Eles geralmente têm formatos e cores diferentes como indicação visual de suas funções e propósitos, facilitando para o animador distingui-los. Alguns deles também terão mais do que apenas transformações de posição, rotação e escala, porque é possível adicionar propriedades personalizadas a eles, como uma alternância de IK/FK.

Também é importante notar que não é possível usar a configuração de controles em um software diferente daquele em que ela foi originalmente feita. Cada software tem sua própria lógica e não é possível exportar constraints.

![](/docs/_remote/66b97ad8bec99d92c8bba7a2.webp) \_Controles e seus diferentes formatos e cores.\_


> [!WARNING]
> ⚠️ **Aviso**: O rig tem que ser animado no mesmo software em que foi criado. Não é possível usar uma configuração do Blender, por exemplo, no Maya, e vice-versa.


#### O que são FK e IK em um rig?

Um rig pode usar duas configurações diferentes que influenciarão a forma como ele se move: FK e IK.

#### FK - Cinemática Direta

Na cinemática direta, ou FK, o pai na hierarquia move todos os ossos filhos abaixo dele. Vamos usar o braço como exemplo: quando o ombro gira, o restante do braço também girará; quando o braço gira, o antebraço seguirá seu comportamento. Ao animar em FK, cada osso precisa ser rotacionado individualmente. Essa configuração oferece muito controle sobre o movimento e é ótima para movimentos em arco, que são essenciais para uma animação fluida.

*Direção do movimento na hierarquia.*

![](/docs/_remote/c8dc3dc16dcc58581a238041.webp)

*Em FK, cada osso precisa ser rotacionado individualmente.*

#### IK - Cinemática Inversa

Na cinemática inversa, ou IK, o filho na hierarquia pode influenciar o movimento de seus pais. Nesse caso, usando o braço como exemplo novamente, quando a mão é movida, o restante do braço seguirá o movimento. Isso também significa que, não importa como o ombro se mova, a mão manterá sua posição. Nessa configuração, um pole vector/pole target controlará em que direção os ossos irão dobrar. As pernas geralmente usam IK, e isso é essencial para que os pés permaneçam no nível do chão durante a animação.

![](/docs/_remote/2137414a0b2253ea260c8f89.webp)

*Direção do movimento na hierarquia.*

![](/docs/_remote/9de7bae6abe4d41fe6467d0e.webp)

*Em IK, a mão moverá todo o braço e também manterá sua posição. O pole target determina a direção em que o cotovelo se dobra.*


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-pt/wearables-e-emotes/emotes/avatar-rig.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
