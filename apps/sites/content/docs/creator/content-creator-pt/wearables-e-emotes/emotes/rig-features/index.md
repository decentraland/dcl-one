> Recursos sobre o rig do avatar e o arquivo para download.

# Recursos do Rig

Esta documentação explica a configuração do Rig 1.0, seus controles e funcionalidades.


> [!NOTE]
> **💡 Dica**: Instale o [plugin Decentraland Tools para Blender](https://extensions.blender.org/add-ons/decentraland-tools/). Ele inclui várias funções úteis para ajudar você a editar e exportar modelos 3D, wearables e emotes.


### Transforms da Armature

Estes são os transforms da armature em Object Mode com a configuração dos controles. **Não edite isto de forma alguma**. O rig deve ser manipulado apenas em Pose Mode. Para evitar edições indesejadas, os transforms foram bloqueados em Object Mode.

*Transforms do Rig 1.0.*


> [!WARNING]
> ⚠️ **Aviso**: **Nunca edite o rig em Object Mode.**


## Orientação dos Bones

Esta é a orientação dos bones para o Rig 1.0.

*Eixos para a orientação dos bones.*

*Comportamento ao espelhar poses.*

### Espelhar uma pose

Os bones esquerdo e direito do rig não estão dispostos da mesma forma, então um único truque de espelhamento não funciona para o corpo inteiro. Comece com **Pose > Copy Pose**, depois **Paste X-Flipped**, e finalize a parte superior do corpo manualmente.

**As pernas e os pés são feitos depois da colagem.** **Paste X-Flipped** produz o resultado certo em `CTRL_FK_Avatar_UpLeg`, `CTRL_FK_Avatar_Leg`, `CTRL_FK_Avatar_Foot`, `CTRL_FK_Avatar_ToeBase`, `CTRL_IK_Foot`, `CTRL_IK_Avatar_ToeBase` e `CTRL_Avatar_Knee`. É exato para as pernas IK e fica a cerca de 3 cm do pé nas pernas FK, porque os bones de repouso esquerdo e direito têm rolls ligeiramente diferentes. Ajuste o pé FK visualmente se precisar que fique exato.

**Ombros, braços, mãos e dedos precisam de mais um passo.** **Paste X-Flipped** sozinho coloca a outra mão de 40 a 70 cm longe de onde você quer. Depois de colar, passe por cada um desses controles e **inverta os valores de X e Z, mantendo Y**:

* `CTRL_Avatar_Shoulder`
* `CTRL_FK_Avatar_Arm`, `CTRL_FK_Avatar_ForeArm`, `CTRL_FK_Avatar_Hand`
* `CTRL_IK_Avatar_Hand`, `CTRL_IK_Avatar_Elbow`
* Todos os 30 `CTRL_Avatar_Hand*` controles dos dedos

Aplique isso à rotação, quer você trabalhe em quaternions ou ângulos de Euler. Os sinais são os mesmos em ambos os casos. Nos controles IK da mão e do cotovelo, aplique também à localização.

Por exemplo, um braço colado que mostra X -0.5, Z 0.1 passa a X 0.5, Z -0.1.

Isso deixa toda a cadeia do braço exatamente correta.

**Por que a parte superior do corpo é diferente:** os frames dos bones da perna direita são um espelho simples dos da esquerda, que é o que **Paste X-Flipped** assume. Os frames do braço direito espelham os da esquerda com o **Z** local invertido em vez de X, então a colagem espelhada sai rotacionada na direção errada. Inverter X e Z corrige exatamente isso.

#### Definindo os valores diretamente em vez disso

Se você definir os valores do lado direito a partir dos valores brutos do lado esquerdo, em vez de colar, use estas inversões de sinal:

| Controles               | Rotação `(w, x, y, z)` | Location     |
| ----------------------- | ---------------------- | ------------ |
| Parte superior do corpo | `(w, -x, -y, z)`       | `(x, y, -z)` |
| Pernas                  | `(w, x, -y, -z)`       | `(-x, y, z)` |

Este é o caminho aproximado: cerca de 1 mm de precisão para os braços, mas com erro de 5 a 10 cm nas pernas FK. **Paste X-Flipped** também corrige as pequenas diferenças de roll entre os bones de repouso esquerdo e direito, e uma simples inversão de sinal não corrige. Prefira o método de colar e depois ajustar acima.


> [!WARNING]
> **📔 Nota**: A pose inicial não é perfeitamente simétrica desde o início, então espelhar uma pose de um lado para o outro sobrescreve essa pequena assimetria. Verifique o resultado visualmente antes de criar keyframes.


### Rotacionando os braços

O controle do braço superior, `CTRL_FK_Avatar_Arm`, tem um eixo X local inclinado. Rotacionar em torno dele varre o braço pelo peito, que é o motivo mais comum de os braços ficarem cruzados quando a intenção era levantá-los.

* Para **direcionar** o braço, rotacione em torno dos eixos mundiais em vez disso: o X mundial o levanta para a frente, e o Z mundial o move para dentro e para fora.
* Para **girar** o braço, para que a palma vire para cima ou para baixo e o cotovelo aponte para dentro ou para fora, rotacione em torno do Y local do próprio bone.

### Em qual eixo cada junta se dobra

Cotovelos e joelhos se dobram no X local. Pulsos e dedos não: eles se curvam no Z local. Rotacionar um dedo em torno do X local o move para o lado, que é o motivo mais comum de os dedos dobrarem na direção errada em um emote finalizado.

| Control                                                        | Dobre-o em                                | Outros eixos                                                   |
| -------------------------------------------------------------- | ----------------------------------------- | -------------------------------------------------------------- |
| Cotovelo `CTRL_FK_Avatar_ForeArm`, joelho `CTRL_FK_Avatar_Leg` | X local                                   | Y e Z estão bloqueados                                         |
| Pulso `CTRL_FK_Avatar_Hand`                                    | Z local flexiona a mão em direção à palma | X local o inclina para o lado                                  |
| Dedos `CTRL_Avatar_Hand{Index,Middle,Ring,Pinky}{1,2,3}`       | Z local curva o dedo em direção à palma   | X local o move para o lado no plano da palma. Y está bloqueado |
| Base do polegar `CTRL_Avatar_HandThumb1`                       | X local move o polegar ao longo da palma  | Y está bloqueado                                               |
| Polegar `CTRL_Avatar_HandThumb2`, `CTRL_Avatar_HandThumb3`     | Z local curva o polegar                   | Y está bloqueado                                               |

As curvas acontecem do mesmo jeito em ambas as mãos. A flexão do cotovelo, a inclinação do pulso e o movimento do polegar ao longo da palma acontecem no sentido oposto nos lados esquerdo e direito, então um valor que funciona de um lado precisa ter o sinal invertido no outro.

### Bone Collections

Para evitar acidentes e facilitar a identificação dos controles, este rig está organizado em bone collections, que podem ser acessadas na *Data Properties* aba no Blender. A visibilidade dessas collections pode ser ativada e desativada clicando no *ícone do olho.* Por padrão, todas estão visíveis, exceto as collections DON'T TOUCH.

aba Armature Data Properties.

Veja como os bones foram separados nas collections:

* Global/Switch: controles globais, como os do root e da spine, assim como os ombros. Controles com quaisquer custom attributes também estão nesta collection.
* FK Upper: todos os controles de configuração FK da parte superior do corpo.
* FK Lower: todos os controles de configuração FK da parte inferior do corpo.
* IK Upper: todos os controles de configuração IK da parte superior do corpo.
* IK Lower: todos os controles de configuração IK da parte inferior do corpo.
* Fingers: controles para os dedos de ambas as mãos.
* Deformation Bones: é aqui que os bones de deformação são armazenados.


> [!WARNING]
> **⚠️ Atenção!**
> 
> As collections DON'T TOUCH guardam as configurações de IK e de outras constraints do rig e devem permanecer ocultas. Editar esses bones pode quebrar a funcionalidade do rig.


## Controles e agrupamento

Os controles são bones que não deformam e que comandam o esqueleto base. Eles têm cores diferentes dependendo da categoria:

* Amarelo: controles globais e controles com custom attributes
* Verde: quadril (mais fácil de identificar na hierarquia da spine)
* Azul: controles com comportamento FK
* Vermelho: controles IK
* Rosa: controles do lado esquerdo
* Laranja: controles do lado direito

*Todos os controles e suas cores.*

## Custom Attributes e configuração

### Mistura FK/IK

Embora os braços normalmente sejam configurados como FK e as pernas como IK, há certas situações que exigem uma configuração diferente. Se a mão precisar manter uma certa posição, como durante flexões ou ao escalar, o IK será a melhor escolha. Já para as pernas, quando estiverem no ar, nadando ou girando, o FK funciona melhor. Para mais flexibilidade e liberdade na animação, este rig tem uma mistura FK/IK no controle UpperBody, sendo 0 totalmente FK e 1 totalmente IK. Qualquer outro valor intermediário será uma mistura dos dois.

![Mistura FK/IK para braços e pernas.](/docs/_remote/efb0c0633809e34cc3499adb.webp)

*Mistura FK/IK para braços e pernas.*

![Como funciona a troca FK > IK.](/docs/_remote/b7bd249527c142b1c626e967.webp)

*Como funciona a troca FK > IK.*

Uma regra simples cobre a maioria dos casos: se um membro mantém contato com algo, mantenha-o em IK; se ele perde o contato e se move livremente, troque-o para FK. Para uma pose sentada:

* **Ambos os pés no chão**: mantenha as pernas em IK. Mova os pés com `CTRL_IK_Foot`, depois abaixe `CTRL_Avatar_UpperBody` para baixar o corpo. Os pés permanecem apoiados. Aponte os joelhos com `CTRL_Avatar_Knee`.
* **Uma perna cruzada sobre a outra**: troque apenas a perna levantada para FK.
* **Pés pendurados**: troque as duas pernas para FK.

Use `CTRL_Avatar_UpperBody` para mover o corpo inteiro para cima e para baixo. É o centro de gravidade. `CTRL_Avatar_Hips` move apenas a pelve e as pernas, o que estica o torso, então este é o controle errado para sentar.


> [!WARNING]
> **📔 Nota**: Evite trocar um membro entre FK e IK no meio de um clipe. A menos que as duas cadeias estejam alinhadas exatamente naquele frame, o membro dá um salto. Se você realmente precisar fazer a troca, crie keyframes da própria propriedade FK > IK para que a mudança aconteça onde você quiser.


#### Toque o rosto ou um alvo com IK

Para fazer uma mão tocar algo, como o rosto, um prop ou a outra mão, troque esse braço para IK. Rotacionar juntas FK não consegue posicionar com precisão a ponta do dedo em um alvo. Lembre-se de que o controle da mão IK fica no pulso, então colocar o controle sobre o alvo posiciona o pulso ali, não a ponta do dedo. Mova o controle até que a ponta do dedo chegue onde você quer.

A propriedade FK > IK faz a mistura gradualmente entre os keyframes. Nos keyframes em que você faz a troca, pose o braço em FK perto da pose IK. Caso contrário, um braço em FK deixado pendurado ao lado atravessa o peito enquanto a mistura acontece.

Use estas medidas da cabeça do avatar padrão, tiradas com o avatar em sua pose idle, para posicionar uma mão no rosto:

| Referência         | Altura acima do chão               | Posição relativa ao `Avatar_Head` bone                             |
| ------------------ | ---------------------------------- | ------------------------------------------------------------------ |
| `Avatar_Head` bone | 1,72 m                             | Este é o meio do rosto, não a base do crânio                       |
| Ponta do nariz     | 1,66 m (1,64 m no corpo masculino) | 0,19 m para frente e 0,06 m para baixo (0,08 m no corpo masculino) |
| Queixo             | 1,60 m                             | 0,12 m para baixo                                                  |
| Topo da cabeça     | 1,90 m                             | 0,18 m para cima                                                   |

A cabeça tem cerca de 0,30 m de largura. Uma mão posicionada 0,12 m acima do bone da cabeça chega à testa. Se a cabeça estiver rotacionada, aplique esses deslocamentos no frame rotacionado da cabeça.

### Mistura FK do Isolate Rotation

Outro custom attribute no controle UpperBody é o isolate rotation, que permite escolher se o bone vai herdar ou não a rotação do pai (enquanto estiver em FK). Em 0, o bone não herda a rotação; em 1, ele segue completamente o comportamento do pai. Qualquer valor intermediário será uma mistura dos dois. Esta é uma ferramenta interessante porque faz o bone FK manter sua posição, comportando-se um pouco como um IK.

![Custom attribute de isolate rotation para os braços.](/docs/_remote/627c80a7f1680d2c4d02c6d4.webp)

*Custom attribute de isolate rotation para os braços.*

![Como funciona o atributo IsoRot para os braços.](/docs/_remote/517ca0821f2dd011b2c186f1.webp)

*Como funciona o atributo IsoRot para os braços.*

O controle da Head também tem esse atributo. Ele é muito útil para ciclos de caminhada, por exemplo, porque a cabeça manterá sua rotação mesmo com o torso girando, garantindo que ela esteja sempre olhando para a frente. Sem essa opção, o animador teria que rotacionar manualmente a cabeça toda vez que o torso girasse para que ela ficasse reta e olhasse para frente.

![Custom attribute de isolate rotation para a cabeça.](/docs/_remote/4afc891b59733db2550998de.webp)

*Custom attribute de isolate rotation para a cabeça.*

![Como funciona o atributo IsoRot para a cabeça.](/docs/_remote/4c87d322918f5fad671253d0.webp)

*Como funciona o atributo IsoRot para a cabeça.*


> [!WARNING]
> ⚠️ **Aviso**: Em versões mais antigas do Blender, mesmo que todos os controles tenham sido selecionados e com keyframes definidos, esses custom attributes não serão keyframed automaticamente. Certifique-se de inserir manualmente um keyframe em cada atributo para não perder a pose/movimento que você criou. No Blender 4.4, ao pressionar I, um keyframe é definido em todos os attributes e custom properties.


![Em versões anteriores do Blender, certifique-se de criar keyframes para todos os controles e custom attributes!](/docs/_remote/a66e6f888ab4d3128c2395ee.webp)

*Em versões anteriores do Blender, certifique-se de criar keyframes para todos os controles e custom attributes!*

*No Blender 4.4, pressione I para definir automaticamente um keyframe em Location, Rotation, Scale e Custom Properties..*

Outra solução para criar keyframes em custom properties é selecionar ***Keying*** embaixo na aba Timeline e em ***Active Keying Set*** selecione Location, Rotation, Scale e Custom Properties, como mostrado no gif abaixo. Dessa forma, toda vez que você pressionar I, um keyframe será criado sem o menu pop-up. Como alguns animadores preferem o menu, por padrão essa opção não está ativada. Mas sinta-se à vontade para escolher o método que melhor lhe convier.

![Criação de keyframes com a opção Keying.](/docs/_remote/296a3a5045569233102e0836.webp)

*Criação de keyframes com a opção Keying.*

### Configuração invertida do pé IK

Animar um pé em FK é bem simples: basta pegar qualquer um dos controles e rotacioná-lo. Como há um controle para o pé e outro para os dedos, o animador tem controle total sobre os movimentos. No entanto, para o IK não é tão simples. O pé precisa permanecer preso ao chão, mas também precisa poder rotacionar no ball e no calcanhar, além de para os lados.

Este rig foi configurado de forma a dar ao animador liberdade de movimento do pé sem perder as vantagens do sistema IK. Ele consiste em quatro controles:

* Foot roll: este controle rotaciona o pé para frente e para trás e para os lados. Para evitar dobrar demais no calcanhar ou demais no ball, foi definido um limite para que o rig do pé não quebre. Quando esse limite é alcançado, o pé para de rotacionar.

![Foot roll: rotacione em X e ele se move para frente e para trás; rotacione em Z e ele se move de um lado para o outro.](/docs/_remote/28fab31b129fde23a5c5f90d.webp)

*Foot roll: rotacione em X e ele se move para frente e para trás; rotacione em Z e ele se move de um lado para o outro.*

* Toe tip roll: rotaciona o pé pela ponta dos dedos. Ele só rotaciona para frente.

![O toe tip roll só rotaciona em X positivo.](/docs/_remote/68e7725b2019cbf56dd0f028.webp)

*O toe tip roll só rotaciona em X positivo.*

* Toes control: rotaciona os dedos a partir do ball.

![Os dedos podem ser rotacionados em qualquer direção.](/docs/_remote/9c173333785f1d51960159db.webp)

*Os dedos podem ser rotacionados em qualquer direção.*

* Foot control: este é um controle global que move o pé como um todo. Como ele é o pai de todos os outros controles do pé, ele mantém quaisquer transforms e também pode ser selecionado e rotacionado.

![Como funciona o controle do pé.](/docs/_remote/aa913f0950d1fe1e5d55b393.webp)

*Como funciona o controle do pé.*

### Transforms Bloqueados

Alguns controles podem ter um símbolo de cadeado ao lado dos parâmetros de transforms, o que significa que esses valores não podem ser alterados. Isso foi feito em controles que só devem se comportar de uma certa maneira e para evitar qualquer transformação indesejada. Por exemplo, cotovelos e joelhos devem rotacionar em apenas um eixo, que neste caso é X, então os outros eixos foram bloqueados. Outros exemplos de controles com attributes bloqueados são cotovelos e joelhos IK, dedos, foot roll e toe tip.

É recomendável mantê-los bloqueados, mas, caso você queira mais liberdade de movimento, basta clicar no ícone de cadeado para desbloqueá-los.

Os bloqueios só se aplicam às ferramentas interativas do Blender. Um script em Python que escreve diretamente a rotação de um bone os ignora, então, se você posear o rig com um script, mantenha você mesmo cada rotação nos eixos livres da junta.

![Transforms bloqueados em um controle.](/docs/_remote/52bbc9e002370ab201046aeb.webp)


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-pt/wearables-e-emotes/emotes/rig-features.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
