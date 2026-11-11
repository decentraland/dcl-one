> Como exportar partículas nos seus Emotes usando Armature

# Partículas em Emotes

Os Emotes 2.0 podem ter sons e props, mas você sabia que também é possível usar partículas? Elas podem ser muito divertidas e adicionar aquele brilho visual extra! E também podem acelerar o processo de animação.


> [!NOTE]
> **💡 Dica**: Instale o [plugin Blender do Decentraland Tools](https://extensions.blender.org/add-ons/decentraland-tools/). Ele inclui várias funções úteis para ajudar você a editar e exportar modelos 3D, wearables e emotes.


Mas, como a animação de objetos e os sistemas de partículas não são suportados em Emotes 2.0, precisamos converter essas partículas em uma armature (que é suportada) para poder adicioná-las aos emotes. Usaremos um add-on do Blender desenvolvido pela Content Team da Foundation para esse processo, que pode ser baixado no link abaixo.

Use o [Addon Particles to Bones para Blender](https://github.com/decentraland/docs/blob/main/creator/images/emotes/particle_system_to_bone_animation.py) para começar!

![](/docs/_remote/8a0401cd85e912317d2b0e9e.webp) ![](/docs/_remote/86f127e163391ec5c9cc99a0.webp)

O processo de adicionar partículas ao seu emote envolverá duas etapas; você pode seguir as orientações junto com estes vídeos:

* **Convertendo partículas em armature**

[![Video Preview](/docs/_remote/26ae9b1db5276e225497fc37.webp)](https://vz-8a0704eb-552.b-cdn.net/bb51a195-4a80-48d3-badd-0bfe6cc9e3b3/playlist.m3u8)

* **Mesclando a armature das partículas com a armature do prop**

[![Video Preview](/docs/_remote/5a12aec98b02dff46c6ea7a5.webp)](https://vz-8a0704eb-552.b-cdn.net/9f512c34-ae83-42fe-a858-165ccea6d0c5/playlist.m3u8)


> [!NOTE]
> **Atenção!**
> 
> ⚠️ A criação de um sistema de partículas não será abordada nesta documentação. O objetivo é ajudar a implementá-lo no seu emote.


## Convertendo Partículas em Armature

Como mencionado anteriormente, os Emotes 2.0 não oferecem suporte à animação de objetos nem a qualquer forma de sistema de partículas. É por isso que precisamos converter as partículas em uma armature.

Para começar, você precisará de um sistema de partículas funcional e do add-on. O sistema de partículas mostrado abaixo é muito simples; é apenas um exemplo para mostrar como o add-on funciona. Certifique-se de que seu sistema de partículas corresponda às animações do seu emote para obter um resultado melhor.

![](/docs/_remote/221d5be3906962eace9e413c.webp)

*Sistema de partículas simples.*

Em seguida, baixe e instale o add-on. Para isso, vá em Edit > Preferences > Add-ons e clique em Install para procurar o arquivo no seu computador.

![](/docs/_remote/7494e551d315ebd81e317174.webp)

*Clique em Edit > Preferences.*

![](/docs/_remote/455940b9766bf96f12df39e3.webp)

*Selecione Add-ons à direita e clique em Install.*

Depois disso, clique na caixa de seleção para ativá-lo. Uma aba chamada Converter aparecerá no lado direito do Viewport.

![](/docs/_remote/4a04786944a55dc6ea9f1b36.webp)

*Clique na caixa de seleção para ativar o add-on.*

![](/docs/_remote/c89bb19680957280f6fbecbf.webp)

*Você encontrará a aba Converter à direita do Viewport.*

Em seguida, selecione o objeto, vá para a aba Converter, renomeie a Output Collection como achar melhor, defina os Start e End Frames e clique em Particle System to Armature Converter, como mostrado abaixo.

![](/docs/_remote/e7e1f6344eb6ec89ec8c469f.webp)

Você notará que uma collection foi criada no Outliner, com a armature das partículas. Também foi criado um animation clip, ao fazer o bake de todo o movimento de cada objeto.

![](/docs/_remote/d4067e87e6fe33342f19de70.webp)

*Armature gerada pelo add-on.*

![](/docs/_remote/b2b2e68b9a2d4d5dc47d2c10.webp)

*Clipe de animação gerado pelo add-on.*

E isso é tudo para converter suas partículas em uma armature! Lembre-se de salvar seu arquivo com um nome diferente (mantenha o original como backup), vamos precisar dele para a próxima etapa.


> [!NOTE]
> **Atenção!**
> 
> ⚠️ As partículas devem ser o último elemento adicionado ao seu emote. Quando o emote estiver pronto, você terá exatamente a duração da animação, o que facilitará sincronizar as partículas com a ação realizada.


## Mesclando a Armature das Partículas com a Armature do Prop

Os Emotes 2.0 podem ter apenas duas armatures e dois animation clips por arquivo. Se simplesmente importarmos as partículas, acabaremos com três armatures e três animation clips. A maneira de corrigir isso é mesclar a armature das partículas na armature do prop. O motivo de mesclá-la ao prop e não à armature do avatar é muito simples: o avatar rig não pode ser editado de forma alguma, adicionando ou excluindo bones. Isso faz da armature do prop a única opção.

## Mesclando a Armature das Partículas com a Armature do Prop

Os Emotes 2.0 podem ter apenas duas armatures e dois animation clips por arquivo. Se simplesmente importarmos as partículas, acabaremos com três armatures e três animation clips. A maneira de corrigir isso é mesclar a armature das partículas na armature do prop. O motivo de mesclá-la ao prop e não à armature do avatar é muito simples: o avatar rig não pode ser editado de forma alguma, adicionando ou excluindo bones. Isso faz da armature do prop a única opção.

A primeira coisa que precisamos fazer é importar ou anexar a collection gerada pelo add-on. Você sempre pode exportar o arquivo glb para isso e importá-lo no seu arquivo de emote. Ou simplesmente anexar a collection, o que é muito mais fácil e rápido, indo para ***File*** > ***Append***, procure o arquivo das partículas no seu computador, selecione a pasta ***Collection*** e escolha a collection desejada.

![](/docs/_remote/9ca230b3c18bf89586ff9347.webp)

*Para anexar uma collection, vá em File > Append.*

![](/docs/_remote/1f0417578e1b9a2dcae76f3e.webp)

*Depois de localizar o arquivo das partículas, selecione a pasta Collection.*

![](/docs/_remote/539369b8db786f772b98368b.webp)

*Escolha a collection gerada pelo add-on (que você teve a oportunidade de renomear). Particles\_Out é apenas o nome dado a ela neste exemplo.*

Em seguida, vamos mesclar as armatures. No Object Mode, selecione primeiro a armature das partículas, depois mantenha Shift pressionado para selecionar a armature do prop e pressione Ctrl+J para juntá-las/mesclá-las, como mostrado abaixo.

![](/docs/_remote/c6adaaf3d6e33639b4ce3f4e.webp)

Depois disso, as partículas podem parecer desorganizadas, mas não se preocupe, isso faz parte do processo. Para corrigir isso, selecione cada particle object no Outliner, um de cada vez, vá em Object Properties (ícone de quadrado laranja) e, em Relations, verifique se o parent é Armature\_Prop. Faça isso para todos os particle objects. Isso significa que todos os objects são filhos de Armature\_Prop.

![](/docs/_remote/bed704b9a320a1befd208977.webp)

*Certifique-se de que Armature\_Pop seja o parent de cada objeto de partícula.*

Depois de fazer isso para todos os objetos de partícula, selecione o primeiro objeto de partícula no Outliner, vá em Modifiers (o ícone de chave inglesa azul) e o campo Object estará vazio. Como esta é a aba Modifiers, isso significa que aqui você escolherá qual armature irá controlar o objeto. Então, basta clicar em Object e selecionar Armature\_Prop, repetindo isso para todos os objetos de partícula. À medida que fizer isso, você notará que os objetos de partículas voltarão aos seus tamanhos originais, corrigindo toda a bagunça criada quando as armatures foram mescladas.

![](/docs/_remote/39489288235ee5b1b5748410.webp)

*Selecione a armature correta para controlar os objetos.*

Agora que a armature está devidamente mesclada e configurada, há mais uma etapa para que isso funcione. Se você tentar reproduzir a animação, notará que as partículas não estão se movendo. Isso acontece porque agora elas fazem parte de Armature\_Prop, e a animação reproduzida por ela é a do prop. Lembra que as partículas tinham seu próprio animation clip? O que precisamos fazer agora é copiar os keyframes do animation clip das partículas e colá-los no animation clip do prop.

Para fazer isso, vá para ***Pose Mode*** e, no Outliner, expanda ***Armature\_Prop*** e depois ***Pose***. Selecione todos os bones que pertencem às partículas.

![](/docs/_remote/10184a7f4c04c033592867a1.webp)

*Selecionando todos os bones das partículas.*

Com os bones selecionados, no Action Editor, selecione o animation clip original das partículas que contém todos os keyframes. Selecione todos os keyframes pressionando A, clique com o botão direito e copie os keyframes. Em seguida, selecione a animação do prop, defina um keyframe no primeiro frame e cole os keyframes do animation clip das partículas.

![](/docs/_remote/b16528d9ab25ee97d3d824e0.webp)

*Copiando frames da animação das partículas para a animação do prop.*

E pronto! Salve o arquivo, certifique-se de deletar o animation clip original das partículas e exporte como faria normalmente.

Se precisar de mais informações sobre como fazer isso, confira a ***Exportação*** seção em [Adicionando Props e Sons aos seus Emotes](/docs/creator/wearables-and-emotes/emotes/props-and-sounds/) documentação.


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-pt/wearables-e-emotes/emotes/particles-in-emotes.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
