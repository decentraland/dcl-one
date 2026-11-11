> Como definir a posição, rotação e escala de uma Entity numa cena

# Posicionamento de Entity

Você pode definir o *posição*, *rotação* e *escala* de qualquer entidade usando o `Transform` componente. Isso pode ser usado em qualquer entidade no espaço 3D, afetando onde a entidade é renderizada. Isso inclui formas primitivas (cubo, esfera, plano, etc), formas de texto 3D, formas NFT e modelos 3D (`GltfContainer`).

## Use o Scene Editor no Creator Hub

Ao adicionar um item à sua cena via o Scene Editor, ele inclui implicitamente um **Transform** componente. Em seguida, você altera os valores no componente Transform da entidade implicitamente ao alterar a posição, rotação ou escala de uma entidade. Você também pode usar a UI do Scene Editor para fornecer valores numericamente para obter mais precisão.

## Essenciais do código

![](/docs/_remote/d0b3835ca11dd6cc73d8d261.webp)

```ts
// Crie uma nova entidade
const ball = engine.addEntity()

// Dê a esta entidade uma forma, para torná-la visível
MeshRenderer.setSphere(ball)

// Dê a esta entidade um componente Transform
Transform.create(ball, {
	position: Vector3.create(5, 1, 5),
	scale: Vector3.create(1, 1, 1),
	rotation: Quaternion.Identity(),
})
```

Para mover, rotacionar ou redimensionar uma entidade na sua cena ao longo de um período de tempo, altere os valores neste componente incrementalmente, frame a frame. Veja [Mover entidades](/docs/creator/content-creator-pt/scenes-sdk7/fundamentos-de-conteudo-3d/move-entities/) para mais detalhes e boas práticas.

Qualquer campo que você omitir recebe seu valor padrão: position `0, 0, 0`, sem rotação, e uma escala de `1, 1, 1`. Omita um campo em vez de defini-lo como `undefined`. Um campo que está explicitamente `undefined`, por exemplo, de uma variável que não foi definida, substitui o padrão, e a cena então lança um erro durante a próxima atualização do engine, não na linha que chamou `Transform.create()`.

```ts
// ❌ Se savedRotation for undefined, a cena lança um erro mais tarde
Transform.create(ball, { position: Vector3.create(5, 1, 5), rotation: savedRotation })

// ✅ Recorra a um valor real
Transform.create(ball, { position: Vector3.create(5, 1, 5), rotation: savedRotation ?? Quaternion.Identity() })
```


> [!WARNING]
> **📔 Nota**: `Vector3` e `Quaternion` deve ser importado via
> 
> > `import { Vector3, Quaternion } from "@dcl/sdk/math"`
> 
> Veja [Importações](/docs/creator/content-creator-pt/scenes-sdk7/primeiros-passos/coding-scenes/#imports) para saber como lidar com isso facilmente.


## Posição

`posição` é um *vetor 3D*, ele define a posição do centro da entidade nos três eixos, *x*, *y*e *z*. Veja [Tipos de geometria](/docs/creator/content-creator-pt/scenes-sdk7/fundamentos-de-conteudo-3d/special-types/) para mais detalhes.

```ts
// Crie uma nova entidade
const ball = engine.addEntity()

// Crie um transform com uma posição predefinida
Transform.create(ball, {
	  position: Vector3.create(5, 1, 5)
})

// Obtenha uma versão mutável do transform
const mutableTransform = Transform.getMutable(ball)

// Defina a posição com um objeto
mutableTransform.position = { x: 5, y: 1, z: 5 }

// Defina a posição com um objeto (sintaxe alternativa)
mutableTransform.position = Vector3.create(2, 1, 4)

// Defina cada eixo individualmente
mutableTransform.position.x = 3
mutableTransform.position.y = 1
mutableTransform.position.z = 3
```

Ao definir uma posição, tenha em mente as seguintes considerações:

* Os números em um vetor de posição representam *metros* (a menos que a entidade seja filha de uma entidade escalada).
* Uma cena composta por um único parcel mede 16m x 16m. O centro da cena (ao nível do solo) fica em `x:8, y:0, z:8`. Se a cena for composta por vários parcels, então o centro variará dependendo da disposição deles.
* `x:0, y:0, z:0` refere-se ao *sudoeste* canto do parcel base da cena, ao nível do solo.

  > Dica: Ao visualizar uma cena no modo de Preview, um compasso aparece no ponto (0,0,0) da cena com rótulos para cada eixo como referência.

  > Observação: Você pode alterar o parcel base de uma cena editando o `base` atributo de *scene.json*.
* Para se orientar melhor, use sua *esquerda* mão:
  * seu dedo indicador (apontando para frente) é o *z* eixo
  * seu dedo médio (apontando para o lado) é o *x* eixo
  * seu polegar (apontando para cima) é o *y* eixo.
* Se uma entidade for filha de outra, então `x:0, y:0, z:0` refere-se ao centro de sua entidade pai, onde quer que ela esteja na cena.
* Toda entidade na sua cena deve ser posicionada dentro dos limites dos parcels que ocupa o tempo todo. Se uma entidade sair desses limites, será gerado um erro.

  > Dica: Ao visualizar uma cena no modo de preview, as entidades fora dos limites são destacadas em *vermelho*.
* Sua cena também tem limite de altura. A altura máxima é de 330 metros para cada cena, independentemente de quantos parcels ela ocupe. Entidades colocadas acima de 200 metros podem sofrer problemas de sincronização no multiplayer. Veja [limitações da cena](/docs/creator/content-creator-pt/scenes-sdk7/otimizacao/scene-limitations/) para mais detalhes.

## Rotação

`rotação` é armazenada como uma [*quaternion*](https://en.wikipedia.org/wiki/Quaternion), um sistema de quatro números, *x*, *y*, *z* e *w*. Cada um desses números vai de -1 a 1. Veja [Tipos de geometria](/docs/creator/content-creator-pt/scenes-sdk7/fundamentos-de-conteudo-3d/special-types/) para mais detalhes.

```ts
// Crie uma nova entidade
const cube = engine.addEntity()

// Crie um transform com uma rotação predefinida de 0
Transform.create(cube, {
	  rotation: Quaternion.Identity()
})

// Obtenha uma versão mutável do transform
const mutableTransform = Transform.getMutable(cube)

// Defina a rotação com um objeto, a partir de ângulos de Euler
mutableTransform.rotation = Quaternion.fromEulerDegrees(0, 90, 0)

// Defina a rotação com um objeto
mutableTransform.rotation = { x: 0.1, y: 0.5, z: 0.5, w: 0 }

// Defina cada eixo individualmente
mutableTransform.rotation.x = 0
mutableTransform.rotation.y = 1
mutableTransform.rotation.z = 0.3
mutableTransform.rotation.w = 0
```

Você também pode definir o campo de rotação com [*Euler* ângulos](https://en.wikipedia.org/wiki/Euler_angles), a notação mais comum *x*, *y* e *z* com números que vão de 0 a 360 com a qual a maioria das pessoas está familiarizada. Para usar ângulos de Euler, use uma das seguintes notações:

```ts
// Crie um transform com uma rotação predefinida em ângulos de Euler
Transform.create(cube, {
	  rotation: Quaternion.fromEulerDegrees(0, 90, 0)
})

// Obtenha uma versão mutável do transform
const mutableTransform = Transform.getMutable(cube)

// Defina a rotação com um objeto, a partir de ângulos de Euler
mutableTransform.rotation = Quaternion.fromEulerDegrees(0, 90, 0)
```

Ao usar um *vetor 3D* para representar ângulos de Euler, *x*, *y* e *z* representa a rotação nesse eixo, medida em graus. Uma volta completa requer 360 graus.

Ao recuperar a rotação de uma entidade, ela retorna um quaternion por padrão. Para obter a rotação expressa em ângulos de Euler, use `Quaternion.toEulerAngles()`:

```ts
// Obtenha uma versão somente leitura do transform
const transform = Transform.get(cube)

// Obtenha a rotação expressa em ângulos de Euler
const eulerAngle = Quaternion.toEulerAngles(transform.rotation)
```

## Obtendo a posição global e a rotação de uma entidade

O `getWorldPosition` e `getWorldRotation` funções retornam a posição global e a rotação de uma entidade. Isso significa que ela retorna a posição ou rotação percebida em que o jogador verá o item, ignorando quaisquer hierarquias pai.

* `getWorldPosition(engine, entity: Entity): Vector3Type`: Esta função retorna a posição World de uma entidade, levando em consideração todas as posições das entidades pai, se a própria entidade tiver alguma, retornando `{x: 0, y: 0, z: 0}` se a entidade não tiver Transform.

```ts
const worldPos = getWorldPosition(engine, childEntity)
console.log(`World position: ${worldPos.x}, ${worldPos.y}, ${worldPos.z}`)
```

* `getWorldRotation(engine, entity: Entity): QuaternionType`: Esta função retorna a rotação World de uma entidade, levando em consideração todas as rotações das entidades pai, se a própria entidade tiver alguma. Ela retorna um `Quaternion` tipo, retornando um quaternion identidade `{x: 0, y: 0, z: 0, w: 1}` se a entidade não tiver Transform.

```ts
const worldRot = getWorldRotation(engine, childEntity)
console.log(`World rotation: ${worldRot.x}, ${worldRot.y}, ${worldRot.z}, ${worldRot.w}`)
```


> [!NOTE]
> **Nota:** A posição global e a rotação global são relativas às coordenadas dentro da cena e não à Genesis City.


## Encarar o jogador

Adicione um *Billboard* componente a uma entidade para que ela sempre gire para encarar a câmera do jogador.

Billboards eram uma técnica comum usada em jogos 3D dos anos 90, em que a maioria das entidades eram planos 2D que sempre encaravam o jogador. A mesma ideia também pode ser usada para girar um modelo 3D.


> [!WARNING]
> **📔 Nota**: Um billboard encara a **câmera**, não o avatar. Em terceira pessoa, a câmera fica atrás e acima do avatar, então os dois estão em lugares diferentes, e um billboard apontado para a câmera fica visivelmente afastado do avatar. Para encarar o avatar em vez disso, defina `targetEntity` a `engine.PlayerEntity`.


```ts
// Crie uma nova entidade
const cube = engine.addEntity()

// Dê à entidade uma forma visível
MeshRenderer.setBox(cube)

// Crie um transform com uma posição predefinida
Transform.create(cube, {
	  position: Vector3.create(5, 1, 5)
})

// Dê à entidade um componente Billboard
Billboard.create(cube, {})
```

Você pode configurar como o billboard se comporta com os seguintes parâmetros:

* `billboardMode`: Usa um valor de `BillboardMode` para definir quais eixos de rotação giram automaticamente para encarar o jogador:
  * `BillboardMode.BM_ALL`: A entidade gira para encarar o jogador em todos os seus eixos de rotação. Se o jogador estiver muito acima da entidade, a entidade irá apontar para cima.
  * `BillboardMode.BM_NONE`: A entidade não girará de forma alguma.
  * `BillboardMode.BM_X`: A entidade gira apenas em seu *x* eixo de rotação.
  * `BillboardMode.BM_Y`: A entidade gira apenas em seu *y* eixo de rotação. Ele gira apenas para a esquerda e para a direita, não para cima e para baixo. Permanece perpendicular ao chão se o jogador estiver acima ou abaixo da entidade.
  * `BillboardMode.BM_Z`: A entidade gira apenas em seu *z* eixo de rotação.
* `targetEntity`: (opcional) Uma entidade para a qual o billboard gira para encarar, em vez da câmera do jogador. Se a entidade referenciada não existir (por exemplo, ela ainda não foi criada ou foi removida), o billboard para de girar e mantém sua última orientação até a entidade existir novamente. Definir isso como `engine.CameraEntity` é equivalente a deixá-lo sem definir.

```ts
// billboard plano
const perpendicularPlane = engine.addEntity()

Transform.create(perpendicularPlane, {
	position: Vector3.create(8, 1, 8),
})

MeshRenderer.setPlane(perpendicularPlane)

Billboard.create(perpendicularPlane, {
	billboardMode: BillboardMode.BM_Y,
})

// etiqueta de texto
const textLabel = engine.addEntity()

Transform.create(textLabel, {
	position: Vector3.create(6, 1, 6),
})

TextShape.create(textLabel, {
	text: 'This text is always readable',
})

Billboard.create(textLabel)

// placa que encara outra entidade em vez do jogador
const sphere = engine.addEntity()

Transform.create(sphere, {
	position: Vector3.create(4, 2, 4),
})

MeshRenderer.setSphere(sphere)

const sign = engine.addEntity()

Transform.create(sign, {
	position: Vector3.create(8, 1, 8),
})

TextShape.create(sign, {
	text: 'Watching the sphere',
})

Billboard.create(sign, {
	targetEntity: sphere,
})
```


> [!NOTE]
> **💡 Dica**: Billboards são muito úteis para adicionar a *texto* entidades, pois isso as torna sempre legíveis.


Defina `targetEntity` a `engine.PlayerEntity` para seguir o avatar em vez da câmera. Isso é o que você quer para uma etiqueta de nome, um balão de fala ou qualquer outra coisa que pertença ao avatar em vez da visão do jogador:

```ts
// Uma etiqueta que gira para encarar o avatar, não a câmera
Billboard.create(label, {
	targetEntity: engine.PlayerEntity,
	billboardMode: BillboardMode.BM_Y,
})
```

A posição do avatar fica nos seus pés, então uma etiqueta acima da cabeça dele se inclinaria para baixo em direção a eles. `BM_Y` mantém a etiqueta na vertical e só a gira para a esquerda e para a direita.

### Para que lado um billboard encara

Um billboard aponta seu eixo local **-Z** para o que quer que ele esteja encarando. Isso importa quando você torna outras entidades filhas de um billboard:

* Uma entidade filha com um deslocamento local **+Z** situa-se **atrás de** o billboard, escondida por ele.
* Uma entidade filha com um deslocamento local **-Z** situa-se **à frente**, mais perto do observador.

Então, para colocar uma moldura ou um fundo atrás de uma etiqueta com billboard, desloque-o em **+Z**. Para colocar um destaque à frente dela, desloque-o em **-Z**.

O `rotação` valor do `Transform` componente da entidade não muda conforme o billboard segue os jogadores.

Se uma entidade tiver tanto um `Billboard` componente quanto `Transform` componente com `rotação` valores, os jogadores verão a entidade girando como um billboard. Se o billboard não afetar todos os eixos, o eixo restante será girado de acordo com o `Transform` componente.


> [!WARNING]
> **📔 Nota**: Se houver vários jogadores presentes ao mesmo tempo, cada um verá as entidades com o modo billboard encarando-o. As rotações do billboard são calculadas localmente para cada jogador e não afetam o que os outros veem. Isso não se aplica a billboards com um `targetEntity`: como a posição do alvo faz parte da cena, todos os jogadores veem esses billboards encarando a mesma direção.


## Encarar um conjunto de coordenadas

Para a entidade A olhar para a entidade B:

```
1) Subtraia a posição da entidade A da entidade B para obter um vetor que descreve a distância entre elas.
2) Normalize esse vetor, para que tenha comprimento 1, mantendo sua direção.
3) Use `Quaternion.lookRotation` para obter uma rotação Quaternion que descreve a rotação nessa direção.
4) Defina esse Quaternion como a rotação da entidade A
```

```ts
export function turn(entity: Entity, target: Vector3.ReadonlyVector3) {
	const transform = Transform.getMutable(entity)
	const difference = Vector3.subtract(target, transform.position)
	const normalizedDifference = Vector3.normalize(difference)
	transform.rotation = Quaternion.lookRotation(normalizedDifference)
}
```

## Escala

`escala` também é uma *vetor 3D*, armazenada como uma `Vector3` objeto, incluindo o fator de escala no *x*, *y* e *z* eixo. A forma da entidade é escalada de acordo, seja ela uma forma primitiva ou um modelo 3D.

A escala padrão é 1, então atribua um valor maior que 1 para alongar uma entidade ou menor que 1 para reduzi-la.

```ts
// Crie uma nova entidade
const ball = engine.addEntity()

// Crie um transform com uma posição predefinida
Transform.create(ball, {
	  scale: Vector3.create(5, 5, 5)
})

// Obtenha uma versão mutável do transform
const mutableTransform = Transform.getMutable(ball)

// Defina a escala com um Vector3

mutableTransform.scale = Vector3.create(2, 2, 2)

// Defina a posição com um objeto
mutableTransform.scale = { x: 5, y: 1, z: 5 }

// Defina cada eixo individualmente
mutableTransform.scale.x = 3
mutableTransform.scale.y = 3
mutableTransform.scale.z = 2
```

## Herdar transformações do pai

Quando uma entidade está aninhada dentro de outra, as entidades filhas herdam componentes dos pais. Isso significa que, se uma entidade pai for posicionada, escalada ou rotacionada, suas filhas também serão afetadas. Os valores de posição, rotação e escala das entidades filhas não substituem os dos pais; em vez disso, eles são combinados.

Você define uma entidade como pai de outra ao definir o `parent` campo na `Transform` componente.

Se uma entidade pai for escalada, todos os valores de posição de suas filhas também serão escalados.

```ts
// Crie entidades
const parentEntity = engine.addEntity()
const childEntity = engine.addEntity()

// Crie um transform para o pai
Transform.create(parentEntity, {
	position: Vector3.create(3, 1, 1),
	scale: Vector3.create(0.5, 0.5, 0.5),
})

// Crie um transform para a filha e atribua-a como filha
Transform.create(childEntity, {
	position: Vector3.create(0, 1, 0),
	parent: parentEntity,
})
```

Neste exemplo, a entidade filha será reduzida para 0.5, já que sua entidade pai tem essa escala. A posição da entidade filha também será relativa à sua entidade pai. Temos de somar a posição do pai à da filha. Neste caso, como o pai está escalado para metade do seu tamanho, a transformação da filha também é reduzida proporcionalmente. Em termos absolutos, a filha fica posicionada em `{ x: 3, y: 1.5, z: 1 }`. Se a entidade pai tivesse um `rotação`, isso também afetaria a posição final da filha, pois altera o eixo em que a filha é deslocada.

Se uma entidade filha não tiver `posição` em seu Transform, o padrão é `0,0,0`, o que a deixará posicionada na mesma posição que seu pai.

Você pode usar uma entidade invisível sem componente de forma como pai, para agrupar um conjunto de outras entidades. Essa entidade não ficará visível na cena renderizada, mas pode ser usada para agrupar suas filhas e aplicar um transform a todas elas.


> [!WARNING]
> **📔 Nota**: Evite criar um loop na hierarquia, em que uma entidade acaba sendo sua própria ancestral. O SDK detecta esses loops e registra um aviso nomeando as entidades envolvidas. As entidades afetadas não são posicionadas corretamente até que você corrija a relação de parent.


## Anexar uma entidade a um avatar

Há três métodos para anexar uma entidade ao jogador:

* Torne-o filho do **Entidade Avatar**
* Torne-o filho do **Entidade da câmera**
* Use a **componente AvatarAttach**

A maneira mais simples de anexar uma entidade ao avatar é definir o pai como a [entidade reservada](/docs/creator/content-creator-pt/scenes-sdk7/arquitetura/entities-components/#reserved-entities) `engine.PlayerEntity`. A entidade então se moverá junto com a posição do jogador.

```ts
let childEntity = engine.addEntity()

MeshRenderer.setCylinder(childEntity)

Transform.create(childEntity, {
	scale: Vector3.create(0.2, 0.2, 0.2),
	position: Vector3.create(0, 0.4, 0),
	parent: engine.PlayerEntity,
})
```

Você também pode definir uma entidade para a [entidade reservada](/docs/creator/content-creator-pt/scenes-sdk7/arquitetura/entities-components/#reserved-entities) `engine.CameraEntity`. Ao usar a entidade da câmera em primeira pessoa, a entidade anexada seguirá os movimentos da câmera. Isso é ideal para manter algo sempre à vista, por exemplo, para manter o modelo 3D da arma sempre à vista, mesmo quando a câmera aponta para cima.

```ts
let childEntity = engine.addEntity()

MeshRenderer.setCylinder(childEntity)

Transform.create(childEntity, {
	scale: Vector3.create(0.2, 0.2, 0.2),
	position: Vector3.create(0, 0.4, 0),
	parent: engine.CameraEntity,
})
```

Para anexar um objeto a um dos bones do avatar e fazer com que ele se mova junto com as animações do avatar, adicione um `AvatarAttach` componente à entidade.

Você pode escolher diferentes anchor points no avatar; a maioria desses pontos está ligada à armature do jogador e segue as animações do jogador. Por exemplo, ao usar o anchor point da mão direita, a entity anexada se moverá quando o avatar acenar ou balançar os braços enquanto corre, como se o jogador estivesse segurando a entity na mão.

```ts
// Anexar ao jogador principal; se avatarId não estiver definido, engine.PlayerEntity é usado por padrão
AvatarAttach.create(myEntity, {
	anchorPointId: AvatarAnchorPointType.AAPT_NAME_TAG,
})

// Anexar a um jogador por ID
AvatarAttach.create(myEntity, {
	avatarId: '0xAAAAAAAAAAAAAAAAA',
	anchorPointId: AvatarAnchorPointType.AAPT_NAME_TAG,
})
```

Ao criar um `AvatarAttach` component, passe um objeto com os seguintes dados:

* `avatarId`: *Opcional* O ID do jogador ao qual anexar. É o mesmo que o endereço Ethereum do jogador, para aqueles conectados com uma Wallet Ethereum. Se não for especificado, a entity é anexada ao avatar do jogador local.
* `anchorPointId`: Qual anchor point no esqueleto do avatar deve ser usado para anexar a entity, usando um valor do enum `AvatarAnchorPointType`.


> [!WARNING]
> **📔 Nota**: Se você quiser que todos os jogadores na scene vejam um objeto anexado ao mesmo jogador, por exemplo, para que todos vejam que o Player A pegou um objeto e o segura na mão esquerda, então você deve fornecer um valor para `avatarId`. Se não for especificado, então todos os jogadores verão o objeto anexado aos seus próprios avatars.


O exemplo a seguir coloca uma entity anexada a um avatar específico, para que todos os outros jogadores a vejam anexada ao mesmo avatar.

```ts
import { getPlayer } from '@dcl/sdk/src/players'
import { AvatarAnchorPointType, AvatarAttach, engine, Entity } from '@dcl/sdk/ecs'
import { syncEntity } from '@dcl/sdk/network'

async function attachToPlayer(){

 let userData = await getPlayer()
 console.log(userData)

 if (!userData || !userData.wearables) return

  let entity = engine.addEntity()

  AvatarAttach.create(entity, {
    avatarId: userData.userId,
    anchorPointId: AvatarAnchorPointType.AAPT_RIGHT_HAND,
  })

  // Outros components

  syncEntity(entity, [AvatarAttach.componentId])

}
```

Os seguintes anchor points estão disponíveis no `AvatarAnchorPointType` enum:

* `AAPT_RIGHT_HAND`: Fixo na mão direita do jogador
* `AAPT_LEFT_HAND`: Fixo na mão esquerda do jogador
* `AAPT_HEAD`: Fixo no centro da cabeça do jogador.
* `AAPT_NECK`: Fixo na base do pescoço do jogador.
* `AAPT_SPINE`: Fixo na seção superior da coluna vertebral.
* `AAPT_SPINE1`: Fixo na seção média da coluna vertebral.
* `AAPT_SPINE2`: Fixo na seção inferior da coluna vertebral.
* `AAPT_HIP`: Fixo no osso do quadril.
* `AAPT_LEFT_SHOULDER`: Fixo no ombro esquerdo.
* `AAPT_LEFT_ARM`: Fixo no primeiro osso do braço esquerdo, na altura do ombro.
* `AAPT_LEFT_FOREARM`: Fixo no osso do antebraço esquerdo.
* `AAPT_LEFT_HAND_INDEX`: Fixo na ponta do dedo indicador esquerdo.
* `AAPT_RIGHT_SHOULDER`: Fixo no ombro direito.
* `AAPT_RIGHT_ARM`: Fixo no primeiro osso do braço direito, na altura do ombro.
* `AAPT_RIGHT_FOREARM`: Fixo no osso do antebraço direito.
* `AAPT_RIGHT_HAND_INDEX`: Fixo na ponta do dedo indicador direito.
* `AAPT_LEFT_UP_LEG`: Fixo no osso da parte superior da perna esquerda.
* `AAPT_LEFT_LEG`: Fixo no osso da parte inferior da perna esquerda.
* `AAPT_LEFT_FOOT`: Fixo no tornozelo da perna esquerda.
* `AAPT_LEFT_TOE_BASE`: Fixo na ponta do dedo do pé da perna esquerda.
* `AAPT_RIGHT_UP_LEG`: Fixo no osso da parte superior da perna direita.
* `AAPT_RIGHT_LEG`: Fixo no osso da parte inferior da perna direita.
* `AAPT_RIGHT_FOOT`: Fixo no tornozelo da perna direita.
* `AAPT_RIGHT_TOE_BASE`: Fixo na ponta do dedo do pé da perna direita.
* `.AAPT_NAME_TAG`: Flutua logo acima da name tag do jogador e não é afetado pelas animações do jogador.

  > Nota: A altura da name tag é ajustada dinamicamente com base na altura dos Wearables que o jogador está usando. Portanto, um jogador usando um hat alto terá sua name tag um pouco mais alta que a dos outros.
* `AAPT_POSITION` *OBSOLETO*: A posição geral do jogador. Isso aparece a uma altura de 0.8 acima dos pés do jogador.

  >


> [!WARNING]
> \> \*\*📔 Nota\*\*: \`AAPT\_POSITION\` está obsoleto. Para seguir a posição geral do jogador, é melhor tornar a entity filha do Avatar Entity. Veja o início desta seção para um exemplo. >



> [!NOTE]
> **💡 Dica**: Para usar esses valores, escreva `AvatarAnchorPointType.` e o VS Code exibirá a lista completa de opções em um dropdown.


![](/docs/_remote/a3894bbddd0ce5df1254538a.webp)

A renderização de entity é determinada localmente em cada instância da scene. Anexar uma entity a um jogador não a torna visível para outros jogadores que estão vendo esse jogador. Se uma entity estiver anexada ao jogador local padrão, cada jogador perceberá a entity como anexada ao seu próprio avatar.


> [!WARNING]
> **📔 Nota**: As entities anexadas a um avatar devem permanecer dentro dos limites da scene para serem renderizadas. Se um jogador sair da sua scene, quaisquer entities anexadas deixarão de ser renderizadas até que o jogador volte. Smart wearables não têm essa limitação.


O `AvatarAttach` component substitui os valores no `Transform` component. Quaisquer valores que você colocar no `Transform` component são substituídos pela posição relativa da entity em relação à do Transform do jogador; esses valores são atualizados quadro a quadro à medida que o jogador se move e anima.

Se você precisar posicionar uma entity com um deslocamento a partir do anchor point no avatar, ou uma rotação ou escala diferente, faça isso por meio de uma entity pai.

1. Crie uma entity invisível com apenas um `Transform` e um `AvatarAttach` component. Seus `Transform` valores serão substituídos à medida que o jogador se move
2. Defina a entity que você deseja anexar como filha desta entity pai. Seus `Transform` valores podem descrever o deslocamento a partir do anchor point.

```ts
// Criar entity pai
const parentEntity = engine.addEntity()

// Anexar entity pai ao jogador
AvatarAttach.create(parentEntity, {
	anchorPointId: AvatarAnchorPointType.AAPT_NAME_TAG,
})

// Criar entity filha
let childEntity = engine.addEntity()

MeshRenderer.setCylinder(childEntity)

Transform.create(childEntity, {
	scale: Vector3.create(0.2, 0.2, 0.2),
	position: Vector3.create(0, 0.4, 0),
	parent: parentEntity,
})
```


> [!WARNING]
> **📔 Nota**: Se a entity anexada tiver colliders, esses colliders podem bloquear o movimento do jogador ou causar efeitos de tremulação. Você pode querer desativar a camada de física dos colliders da entity anexada. Veja [Camadas de colisão](/docs/creator/content-creator-pt/scenes-sdk7/fundamentos-de-conteudo-3d/colliders/#collision-layers)



> [!WARNING]
> **📔 Nota**: Se você tiver uma entity filha e quiser saber a Position e/ou Rotation globais dela, pode usar as `getWorldPosition` e `getWorldRotation` funções. Você pode consultá-las na [Obter posição e rotação globais](#getting-global-position-and-rotation-of-an-entity) seção.


### Anexar a outros jogadores

Você pode usar o `AvatarAttach` component para anexar uma entity a outro jogador. Para fazer isso, você deve conhecer o id do jogador.

Para anexar uma entity ao avatar de outro jogador, você deve fornecer o ID do usuário no campo `avatarId`. Existem [várias maneiras](/docs/creator/content-creator-pt/scenes-sdk7/interatividade/user-data/#get-player-data) de obter esses dados.


> [!WARNING]
> **📔 Nota**: Para aqueles jogadores conectados com uma Wallet Ethereum, seu `userId` é o mesmo que o endereço Ethereum deles.


Obtenha o `userId` de todos os outros jogadores próximos via `getPlayer()`

```ts
executeTask(async () => {
	for (const [entity, data] of engine.getEntitiesWith(PlayerIdentityData)) {
		console.log('Player id: ', data.address)
	}
})
```

Usando-o em conjunto com `AvatarAttach`, você poderia usar o código a seguir para adicionar um cubo flutuando sobre a cabeça de todos os outros jogadores na scene:

```ts
executeTask(async () => {
        for (const [entity, data] of engine.getEntitiesWith(PlayerIdentityData)) {
            const myEntity = engine.addEntity()
            MeshRenderer.setBox(myEntity)
            AvatarAttach.create(myEntity, {
                anchorPointId: AvatarAnchorPointType.AAPT_LEFT_HAND,
                avatarId: data.address,
            })
        }
    })
```

Veja outras maneiras de obter os IDs de outros usuários em [Obter dados do jogador](/docs/creator/content-creator-pt/scenes-sdk7/interatividade/user-data/#get-player-data).


> [!WARNING]
> **📔 Nota**: Não defina a entity de outro jogador como a `parent` de sua entity. O engine grava o `Transform` para sua scene ler, mas ela não é uma entity pai da qual sua entity possa depender, então a entity filha volta para a raiz da scene e permanece na origem da scene. Use `AvatarAttach` com o endereço do jogador em vez disso, como mostrado acima.


Os IDs de entity de jogador são reutilizados conforme as pessoas entram e saem, então a mesma entity pode pertencer a outra pessoa mais tarde. Se você mantiver um marcador por jogador, armazene o endereço para o qual ele foi criado e compare-o a cada frame. Quando o endereço em uma entity mudar, exclua o marcador antigo e crie um novo:

```ts
import { AvatarAttach, AvatarAnchorPointType, engine, Entity, PlayerIdentityData } from '@dcl/sdk/ecs'

// endereço para o qual o marcador foi criado, para que possamos identificar um ID de entity reutilizado
const markers = new Map<Entity, { address: string; marker: Entity }>()

function markerSystem() {
	// descartar marcadores cujos jogadores saíram, ou cujo ID de entity agora pertença a outra pessoa
	for (const [player, entry] of markers) {
		if (PlayerIdentityData.getOrNull(player)?.address === entry.address) continue
		engine.removeEntity(entry.marker)
		markers.delete(player)
	}

	for (const [player, data] of engine.getEntitiesWith(PlayerIdentityData)) {
		if (markers.has(player)) continue
		const marker = engine.addEntity()
		AvatarAttach.create(marker, {
			avatarId: data.address,
			anchorPointId: AvatarAnchorPointType.AAPT_NAME_TAG,
		})
		markers.set(player, { address: data.address, marker })
	}
}

engine.addSystem(markerSystem)
```

## Limites da scene

Todas as entities na sua scene devem caber dentro dos limites da scene, pois o que estiver fora desses limites são parcels de LAND pertencentes a outros jogadores.

Se qualquer parte dos seus modelos ultrapassar esses limites ao executar um preview, essas partes excedentes serão cortadas e não serão renderizadas, tanto durante o preview quanto na scene publicada.

A posição das entities na sua scene é verificada constantemente à medida que elas se movem; se uma entity sair da scene e depois retornar, ela será removida e então renderizada normalmente novamente.

Uma grade no ground da scene mostra os limites da scene, que por padrão vão de 0 a 16 no eixo *x* e *z* eixo, e até 20 no *y* eixo. Você pode colocar entities no subsolo, abaixo de 0 no *y* eixo.


> [!NOTE]
> **💡 Dica**: Se sua scene precisar de mais parcels, você pode adicioná-los no `scene.json` arquivo do projeto. Veja [Metadados da scene](/docs/creator/content-creator-pt/scenes-sdk7/tipos-de-projetos/scene-metadata/) para instruções. Depois de adicionados, você deverá ver a grade se estender para cobrir os parcels adicionais.



---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-pt/scenes-sdk7/fundamentos-de-conteudo-3d/entity-positioning.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
