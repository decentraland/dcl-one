> Use raycasting para traçar uma linha no espaço e consultar colisões com entities na scene.

# Raycasting

Raycasting é uma ferramenta fundamental no desenvolvimento de jogos. Com raycasting, pode traçar uma linha imaginária no espaço e verificar se alguma entidade é intersectada por essa linha. Isto é útil para calcular linhas de visão, trajetórias de balas, algoritmos de pathfinding e muitas outras aplicações.

Quando um jogador carrega no botão do ponteiro, ou no botão primário ou secundário, é traçado um ray a partir da posição do jogador na direção para a qual está a olhar, veja [eventos de botão](/docs/creator/content-creator-pt/scenes-sdk7/interatividade/eventos-de-botao/click-events/) para mais detalhes sobre isto. Este documento cobre como traçar um ray invisível a partir de qualquer posição e direção arbitrárias, independentemente das ações do jogador, o que pode usar em muitos outros cenários.

Tenha em atenção que os raycasts só atingem objetos com colliders. Por isso, se quiser detetar impactos de ray contra um modelo 3D, ou:

* O modelo deve conter [malhas de colisão](/docs/creator/content-creator-pt/modelacao-3d-e-animacoes/colliders/).
* O `GLTFContainer` deve ser configurado para usar a [geometria visível com máscaras de colisão](/docs/creator/content-creator-pt/scenes-sdk7/fundamentos-de-conteudo-3d/colliders/#colliders-on-3d-models).
* Adicione um [componente MeshCollider](/docs/creator/content-creator-pt/scenes-sdk7/fundamentos-de-conteudo-3d/colliders/).

Também é boa prática atribuir [camadas de colisão](/docs/creator/content-creator-pt/scenes-sdk7/fundamentos-de-conteudo-3d/colliders/#collision-layers) máscaras de colisão personalizadas a modelos 3D, para que os rays só precisem de calcular colisões contra as entidades relevantes, em vez de contra tudo o que tenha um collider.

## Criar um ray

Todos os rays têm um ponto de origem e uma direção. O ponto de origem baseia-se na posição de uma entidade, tomando os valores do componente Transform dessa entidade. A direção de um ray pode ser definida de 4 formas diferentes:

* **movimentos locais**: Uma direção relativa à direção para a frente da entidade, também afetada pela transformação de quaisquer entidades pai. Isto é útil para detetar obstáculos à frente de veículos, respeitando o seu rumo.
* **global**: Ignora a rotação da entidade e aponta numa direção como se a rotação da entidade fosse 0. Isto é útil, por exemplo, para apontar sempre para baixo.
* **alvo global**: Traça uma linha entre a posição da entidade e uma posição global de destino na cena. Ignora a rotação da entidade. Útil, por exemplo, para criar jogos de tower defense, em que a torreta de cada torre pode apontar para uma coordenada precisa no espaço.
* **entidade-alvo**: Traça uma linha entre a posição da entidade e a posição de uma segunda entidade-alvo. Ignora a rotação de qualquer uma das entidades.

O código seguinte cria um raycast com uma direção local:

```ts
const myEntity = engine.addEntity()
Transform.create(myEntity, {
  position: Vector3.create(4, 1, 4),
})

raycastSystem.registerLocalDirectionRaycast(
  {
    entity: myEntity,
    opts: { direction: Vector3.Forward() },
  },
  function (raycastResult) {
    // função de callback
  }
)
```

Use as seguintes funções para criar raycasts fornecendo a direção de formas diferentes:

* `raycastSystem.registerLocalDirectionRaycast()`: cria um raycast com uma **movimentos locais** direção. O `direction` campo espera um `Vector3` que descreve um vetor relativo à entidade e à sua rotação (por ex., `Vector3.Forward()` acabaria por usar o vetor forward do Transform da entidade)
* `raycastSystem.registerGlobalDirectionRaycast()`: cria um raycast com uma **global** direção. O `direction` campo espera um `Vector3` que descreve a direção global.
* `raycastSystem.registerGlobalTargetRaycast()`: cria um raycast com uma direção definida por uma **alvo global** posição. O `alvo` campo espera um `Vector3` que descreve uma posição global na cena.
* `raycastSystem.registerTargetEntityRaycast()`: cria um raycast com uma direção definida para uma **entidade-alvo** posição. O `entidade-alvo` campo espera uma referência a uma entidade; a posição desta entidade será usada como alvo do ray.

Os seguintes campos opcionais estão disponíveis ao criar um ray com qualquer um dos métodos acima:

* `maxDistance`: *number* para definir o comprimento com que este ray será traçado. Se não for definido, o valor predefinido é 16 metros.
* `queryType`: *RaycastQueryType* valor enum, para definir se o ray irá devolver todas as entidades atingidas ou apenas a primeira. As seguintes opções estão disponíveis:
  * `RaycastQueryType.RQT_HIT_FIRST`: *(predefinição)* apenas devolve a primeira entidade atingida, a partir do ponto de origem.
  * `RaycastQueryType.RQT_QUERY_ALL`: devolve todas as entidades atingidas, desde a origem até à distância máxima do ray.
* `originOffset`: Em vez de iniciar o raycast a partir da posição de origem da entidade, adicione um offset para iniciar a query a partir de uma posição relativa. Pode, por exemplo, usar um pequeno offset para evitar que o ray colida com o próprio collider da entidade. Se não for definido, o valor predefinido é `Vector3.Zero()`.
* `collisionMask`: Apenas deteta colisões com certas collision layers. Use isto em conjunto com uma collision layer personalizada, ou para detetar apenas a camada de physics ou de pointer events. Veja [camadas de colisão](/docs/creator/content-creator-pt/scenes-sdk7/fundamentos-de-conteudo-3d/colliders/#collision-layers). Se não for definido, a camada predefinida usada é `ColliderLayer.CL_PHYSICS`.
* `continuous`: Se true, continuará a executar uma query de raycast em cada frame. Se false, o ray será usado apenas no frame atual. Se não for definido, o valor predefinido é false.
* Quando definir a direção com uma direção local ou global, o `direction` campo assume por defeito `Vector3.Forward()`.
* Quando definir a direção com um alvo global, o `alvo` campo assume por defeito `Vector3.Zero()`.
* Quando definir a direção com uma entidade-alvo, o `entidade-alvo` campo assume por defeito a entidade raiz da cena, localizada em `Vector3.Zero()`.


> [!WARNING]
> **📔 Nota**: A `continuous` a propriedade deve ser usada com cautela, pois executar uma query de raycast em cada frame pode ser muito dispendioso para o desempenho. Quando possível, use um system (ou a `interval` função na biblioteca Utils) para executar queries de raycast a intervalos regulares mais espaçados, veja [raycasting recorrente](#recurrent-raycasting).


Abaixo estão exemplos que usam cada um dos quatro métodos para determinar a direção do ray:

```ts
// RAYCAST DE DIREÇÃO LOCAL
raycastSystem.registerLocalDirectionRaycast(
  {
    entity: myEntity,
    opts: {
      queryType: RaycastQueryType.RQT_QUERY_ALL,
      direction: Vector3.Forward(),
      maxDistance: 30,
    },
  },
  function (raycastResult) {
    console.log(raycastResult.hits)
  }
)
// RAYCAST DE DIREÇÃO GLOBAL
raycastSystem.registerGlobalDirectionRaycast(
  {
    entity: myEntity,
    opts: {
      queryType: RaycastQueryType.RQT_QUERY_ALL,
      direction: Vector3.Forward(),
      maxDistance: 30,
    },
  },
  function (raycastResult) {
    console.log(raycastResult.hits)
  }
)
// RAYCAST DE POSIÇÃO DE DESTINO GLOBAL
raycastSystem.registerGlobalTargetRaycast(
  {
    entity: myEntity,
    opts: {
      queryType: RaycastQueryType.RQT_QUERY_ALL,
      target: Vector3.Zero(),
    },
  },
  (raycastResult) => {
    console.log(raycastResult.hits)
  }
)
// RAYCAST DE ENTIDADE-ALVO
const targetEntity = engine.addEntity()
Transform.create(targetEntity, { position: Vector3.create(8, 1, 10) })

raycastSystem.registerTargetEntityRaycast(
  {
    entity: myEntity,
    opts: {
      queryType: RaycastQueryType.RQT_QUERY_ALL,
      targetEntity: targetEntity,
    },
  },
  (raycastResult) => {
    console.log(raycastResult.hits)
  }
)
```


> [!WARNING]
> **📔 Nota**: `raycastSystem`, `RaycastQueryType` e `ColliderLayer` devem ser importados via
> 
> > `import { raycastSystem, RaycastQueryType, ColliderLayer } from "@dcl/sdk/ecs"`
> 
> Veja [Imports](/docs/creator/content-creator-pt/scenes-sdk7/primeiros-passos/coding-scenes/#imports) para saber como lidar com isso facilmente.


## Resultado do Raycast

A função de callback que trata o raycast recebe um objeto que contém dados sobre o próprio ray e sobre quaisquer entidades que tenham sido atingidas.

* `globalOrigin`: A posição onde o ray foi originado, relativa à cena.
* `direction`: A direção global para a qual o ray estava apontado, como um `Vector3`.
* `hits`: Um array com um objeto por cada entidade que foi atingida. Se não houver entidades atingidas, este array está vazio. Se o raycast usou `RaycastQueryType.RQT_HIT_FIRST`, este array conterá apenas um objeto.

Cada objeto no `hits` array inclui:

* `entityId`: Número de Id da entidade que foi atingida pelo ray.
* `meshName`: *String* com o nome interno da malha específica no modelo 3D que foi atingida. Isto é útil quando um modelo 3D é composto por várias malhas.
* `posição`: *Vector3* para a posição onde o ray intersectou a entidade atingida (relativa à cena)
* `length`: Comprimento do ray desde a sua origem até à posição onde ocorreu o impacto na entidade.
* `normalHit`: *Vector3* para a normal da superfície atingida no espaço global.
* `globalOrigin`: *Vector3* para a posição onde o ray se origina (relativa à cena)
* `direction`: A direção global para a qual o ray estava apontado, como um `Vector3`.

O exemplo seguinte itera sobre as entidades que foram atingidas:

```ts
const myEntity = engine.addEntity()
Transform.create(myEntity, {
  position: Vector3.create(4, 1, 4),
})

raycastSystem.registerLocalDirectionRaycast(
  {
    entity: myEntity,
    opts: {
      queryType: RaycastQueryType.RQT_QUERY_ALL,
      direction: Vector3.Forward(),
      maxDistance: 30,
    },
  },
  function (raycastResult) {
    if (raycastResult.hits.length > 0) {
      for (const hit of raycastResult.hits) {
        if (hit.entityId) {
          console.log('entidade atingida ', hit.entityId)
        }
      }
    } else {
      console.log('nenhuma entidade atingida')
    }
  }
)
```


> [!WARNING]
> **📔 Nota**: Pode obter um resultado de raycast ao atingir uma entidade numa cena diferente.


## Tratar entidades atingidas

Quando obtém um resultado de raycast que atingiu uma entidade, pode usar o `entityId` para interagir com a entidade e os seus components. Uma entidade é [nada mais do que um número](/docs/creator/content-creator-pt/scenes-sdk7/arquitetura/entities-components/#overview), por isso o `entityId` próprio valor pode ser interpretado como um tipo `Entity` .

```ts
const hitEntity = hit.entityId as Entity
const transform = Transform.get(hitEntity)
console.log(transform.position)
```

## Collision layers

É boa prática verificar colisões apenas contra entidades relevantes, para tornar a cena mais performante. O `collisionMask` campo permite listar apenas collision layers específicas — a camada de physics (paredes e pisos da cena), a camada de pointer (pointer events), as camadas de player (avatars), ou 8 camadas personalizadas que pode atribuir livremente. Veja [camadas de colisão](/docs/creator/content-creator-pt/scenes-sdk7/fundamentos-de-conteudo-3d/colliders/#collision-layers).

Por defeito, o `collisionMask` campo é definido para `ColliderLayer.CL_PHYSICS`. Pode alterar este valor para listar outras camadas, ou combinar várias com o `|` separador.

```ts
raycastSystem.registerLocalDirectionRaycast(
  {
    entity: myEntity,
    opts: {
      queryType: RaycastQueryType.RQT_QUERY_ALL,
      direction: Vector3.Forward(),
      maxDistance: 30,
      collisionMask:
        ColliderLayer.CL_CUSTOM1 |
        ColliderLayer.CL_CUSTOM3 |
        ColliderLayer.CL_POINTER,
    },
  },
  (raycastResult) => {
    log(raycastResult.hits)
  }
)
```

## Raycasting recorrente

Ao usar as funções da `raycastSystem`, o comportamento predefinido é criar um único ray, que irá verificar colisões uma vez. Como alternativa, pode definir o `continuous` campo para *true* para executar uma query e a função de callback em cada tick do game loop.

O exemplo seguinte continuará a executar a query de raycast a partir deste ponto

```ts
raycastSystem.registerLocalDirectionRaycast(
  {
    entity: myEntity,
    opts: {
      queryType: RaycastQueryType.RQT_QUERY_ALL,
      direction: Vector3.Forward(),
      maxDistance: 30,
      continuous: true,
    },
  },
  function (raycastResult) {
    log(raycastResult.hits)
  }
)
```


> [!WARNING]
> **📔 Nota**: A `continuous` a propriedade deve ser usada com cautela, pois executar uma query de raycast em cada frame pode ser muito dispendioso para o desempenho.


Quando já não for necessário, remova quaisquer raycasts recorrentes. Para o fazer, tem de usar `raycastSystem.removeRaycasterEntity`.

```ts
raycastSystem.removeRaycasterEntity(myEntity)
```

Quando possível, use um system (ou a `interval` função na biblioteca Utils) para executar queries de raycast a intervalos regulares mais espaçados, como apenas uma vez por segundo, ou a cada quinto de segundo.

```typescript
// components personalizados
const CubeOscilator = engine.defineComponent('CubeOscilator', {
  t: Schemas.Float,
})

const TimerComponent = engine.defineComponent('TimerComponent', {
  t: Schemas.Float,
})

const RAY_INTERVAL = 0.1

// verificar rays
engine.addSystem((dt) => {
  for (const [entity] of engine.getEntitiesWith(TimerComponent)) {
    const timer = TimerComponent.getMutable(entity)
    timer.t += dt

    if (timer.t > RAY_INTERVAL) {
      timer.t = 0
      raycastSystem.registerGlobalDirectionRaycast(
        {
          entity: myEntity,
          opts: {
            queryType: RaycastQueryType.RQT_HIT_FIRST,
            direction: Vector3.Forward(),
            maxDistance: 16,
          },
        },
        function (raycastResult) {
          log(raycastResult.hits)
        }
      )
    }
  }
})

TimerComponent.create(engine.addEntity())

// system do cubo oscilante
engine.addSystem((dt) => {
  for (const [entity, cube] of engine.getEntitiesWith(
    CubeOscilator,
    Transform
  )) {
    CubeOscilator.getMutable(entity).t += dt
    Transform.getMutable(entity).position.y = 2 + Math.cos(cube.t)
  }
})

// criar cubo
const cubeEntity = engine.addEntity()
Transform.create(cubeEntity, { position: { x: 8, y: 1, z: 8 } })
CubeOscilator.create(cubeEntity)
MeshRenderer.setBox(cubeEntity)
MeshCollider.setBox(cubeEntity)
```

O exemplo acima executa um raycast recorrente a cada 0,1 segundos. Usa um componente de temporizador e a propriedade `dt` de um system para cronometrar isto de forma uniforme. Também inclui um cubo que oscila para cima e para baixo, controlado por outro system, para entrar e sair do caminho do ray.


> [!NOTE]
> **💡 Dica**: Use a `interval` função na [biblioteca Utils do SDK](https://github.com/decentraland/sdk7-utils) para uma forma mais simples de executar uma função num intervalo fixo.


## Raycasts através de um system

Outra forma de executar raycasts recorrentes é executá-los a partir da função recorrente de um system. Isto permite ter muito mais controlo sobre quando e como isto funciona. Em vez de registar uma função de callback, pode executar uma query de raycast com `raycastSystem.registerRaycast` e depois verificar os dados devolvidos por esta operação, tudo dentro da função do system.

Tenha em atenção que, como o raycast é executado num system, o resultado só ficará disponível no tick seguinte, sendo necessárias duas execuções do system. Uma para registar o raycast para o frame seguinte, e o frame seguinte para processar o seu resultado.

```ts
engine.addSystem((deltaTime) => {
		const result = raycastSystem.registerRaycast(
			myEntity,
			raycastSystem.localDirectionOptions({
				collisionMask: ColliderLayer.CL_CUSTOM1 | ColliderLayer.CL_CUSTOM3 | ColliderLayer.CL_POINTER,
				originOffset: Vector3.create(0, 0.4, 0),
				maxDistance: 16,
				queryType: RaycastQueryType.RQT_HIT_FIRST,
				direction: Vector3.Forward(),
				continuous: true // não use em excesso a propriedade 'continuous', pois raycasting tem impacto no desempenho
			})
		)
		if (result) {
			// faça algo
		}
	})
```

## Colidir com o jogador

Pode detetar avatares diretamente com raycasts incluindo uma das collision layers de avatar na máscara:

* `ColliderLayer.CL_PLAYER`: corresponde a qualquer avatar — o jogador local E qualquer outro jogador renderizado na cena.
* `ColliderLayer.CL_MAIN_PLAYER`: corresponde apenas ao jogador local (principal).

Ambas as camadas podem ser combinadas ou usadas independentemente. O valor predefinido `collisionMask` para um raycast é `CL_PHYSICS`, que NÃO atinge avatares — tem de o ativar explicitamente.

```ts
// Atingir apenas o jogador local (ignorar outros avatares)
raycastSystem.registerLocalDirectionRaycast(
  {
    entity: myEntity,
    opts: {
      direction: Vector3.Forward(),
      collisionMask: ColliderLayer.CL_MAIN_PLAYER,
    },
  },
  (raycastResult) => {
    if (raycastResult.hits.length > 0) {
      console.log('Atingiu o jogador local')
    }
  }
)

// Atingir qualquer avatar (local + remoto)
raycastSystem.registerLocalDirectionRaycast(
  {
    entity: myEntity,
    opts: {
      direction: Vector3.Forward(),
      collisionMask: ColliderLayer.CL_PLAYER,
    },
  },
  (raycastResult) => {
    // raycastResult.hits[i].entityId é 0 para impactos em avatares remotos (sem id de entidade local da cena)
    console.log(raycastResult.hits)
  }
)
```

Quando um raycast atinge o jogador local, o `hit.entityId` está `engine.PlayerEntity`. Os impactos em avatares remotos não transportam um id de entidade local da cena (o campo é `0`) pois o jogador remoto não é uma entidade no mundo da sua cena — mas o impacto continua a ser reportado com o seu `posição`, `length`, `normalHit`, e outros dados geométricos.


> [!NOTE]
> **💡 Dica**: Para detetar apenas **outros** jogadores (excluindo o jogador local), use `CL_PLAYER` e filtre `hit.entityId !== engine.PlayerEntity` na callback.


## Raycasts a partir do jogador

Para traçar um ray a partir da posição do jogador na direção para a qual a câmara está virada, pode traçar um ray usando a câmara ou a avatar [Entidades reservadas](/docs/creator/content-creator-pt/scenes-sdk7/arquitetura/entities-components/#reserved-entities).


> [!NOTE]
> **💡 Dica**: Na maioria dos casos, talvez seja melhor usar [eventos do ponteiro](/docs/creator/content-creator-pt/scenes-sdk7/interatividade/eventos-de-botao/click-events/) em vez de raycasts.


O exemplo seguinte traça um ray a partir da posição da câmara do jogador para a frente, usando a `engine.CameraEntity` entidade.

```ts
raycastSystem.registerGlobalDirectionRaycast(
  {
    entity: engine.CameraEntity,
    opts: {
      queryType: RaycastQueryType.RQT_HIT_FIRST,
      direction: Vector3.rotate(
        Vector3.Forward(),
        Transform.get(engine.CameraEntity).rotation
      ),
    },
  },
  function (raycastResult) {
    console.log(raycastResult)
  }
)
```


> [!WARNING]
> **📔 Nota**: Tenha em mente que, em 3ª pessoa, o cursor pode no futuro não se comportar da mesma forma que em 1ª pessoa. É recomendado usar isto apenas se o jogador estiver em 1ª pessoa.


## Raycast a partir da posição do cursor

Também pode traçar um ray a partir da posição do cursor do jogador para o mundo 3D. Isto pode ser usado para arrastar objetos, shooters, etc.

Neste exemplo, detetamos quando o jogador carrega na tecla E e, em seguida, traçamos um ray a partir da posição do cursor para o mundo 3D. Depois verificamos se o ray atingiu alguma entidade e, se sim, fazemos algo com ela.

```ts
import { engine, Entity, InputAction, inputSystem, PointerEventType, RaycastQueryType, raycastSystem, TextShape, Transform } from '@dcl/sdk/ecs'
import { PrimaryPointerInfo } from '@dcl/sdk/ecs'

let cooldown = 1
let rayFrequency = 0.1
let mousePressed = false

export function main() {
   engine.addSystem(rayCastSystem)
}

const rayCastSystem = (t: number) => {

    if (inputSystem.isTriggered(InputAction.IA_PRIMARY, PointerEventType.PET_DOWN)) {
      mousePressed = true
    }

    if (inputSystem.isTriggered(InputAction.IA_PRIMARY, PointerEventType.PET_UP)) {
      mousePressed = false
    }

    if (!mousePressed) {
      cooldown = 0
      raycastSystem.removeRaycasterEntity(engine.CameraEntity)
      return
    }

    cooldown += t
    if (cooldown < rayFrequency) return
    cooldown = 0

    const pointerInfo = PrimaryPointerInfo.getOrCreateMutable(engine.RootEntity)
    let dir = pointerInfo.worldRayDirection

    raycastSystem.registerGlobalDirectionRaycast(
      {
        entity: engine.CameraEntity,
        opts: {
          queryType: RaycastQueryType.RQT_HIT_FIRST,
          direction: dir,
        },
      },
      function (raycastResult) {
        let result = raycastResult.hits[0]

        // fazer algo na posição atingida
        if (result && result.position) {
          console.log("x:", result.position.x, ", y:", result.position.y, ", z:", result.position.z)
        }

        // fazer algo com a entidade atingida
        const entity = result.entityId as Entity
        if (entity) {
          console.log("entidade: ", entity)
        }
      }
    )
}

```


> [!NOTE]
> **💡 Dica**: Neste exemplo usamos o botão primário (E) para disparar o raycast. Não usamos o botão do ponteiro (clique esquerdo) porque clicar e arrastar também altera o ângulo da câmara por defeito. Se quiser impedir a rotação da câmara enquanto arrasta, pode usar um [Virtual Camera](/docs/creator/content-creator-pt/scenes-sdk7/fundamentos-de-conteudo-3d/camera/) para definir o ângulo da câmara como fixo.


## Sintaxe avançada

### Criar um componente Raycast

Um componente Raycast descreve o ray invisível que é usado para verificar entidades intersectantes. O ray é traçado começando na posição da entidade, conforme definido pelo componente Transform e afetado pelo de quaisquer entidades pai. A direção pode ser definida de várias formas,

Os rays são definidos usando os seguintes dados:

* `direction`: Um objeto que contém um `$case` campo para selecionar o tipo de direção, e um campo adicional que dependerá deste tipo, que determina essa direção. Seguem-se os valores aceites para `$case`:
  * `'localDirection'`: Uma direção relativa à direção para a frente da entidade, também afetada pela transformação de quaisquer entidades pai. Isto é útil para detetar obstáculos à frente de veículos, respeitando o seu rumo. A rotação é definida pelo `localDirection` campo, como um `Vector3` que descreve uma rotação.
  * `'globalDirection'`: Ignora a rotação da entidade e aponta numa direção como se a rotação da entidade fosse 0. Isto é útil, por exemplo, para apontar sempre para baixo. A rotação é definida pelo `globalDirection` campo, como um `Vector3` que descreve uma rotação.
  * `'globalTarget'`: Traça uma linha entre a posição da entidade e uma posição global de destino na cena. Ignora a rotação da entidade. Útil para criar jogos de tower defense, em que a torreta de cada torre pode apontar para uma coordenada precisa no espaço. O alvo é definido pelo `globalTarget` campo, como um `Vector3` que descreve a posição global.
  * `'targetEntity'`: Traça uma linha entre a posição da entidade e a posição de uma segunda entidade-alvo. Ignora a rotação de qualquer uma das entidades. O alvo é definido pelo `entidade-alvo` campo, contendo uma referência à entidade.
* `maxDistance`: *number* para definir o comprimento com que este ray será traçado.
* `queryType`: *RaycastQueryType* valor enum, para definir se o ray irá devolver todas as entidades atingidas ou apenas a primeira. As seguintes opções estão disponíveis:
  * `RaycastQueryType.RQT_HIT_FIRST`: apenas devolve a primeira entidade atingida, a partir do ponto de origem.
  * `RaycastQueryType.RQT_QUERY_ALL`: devolve todas as entidades atingidas, desde a origem até à distância máxima do ray.
* `collisionMask`: Apenas deteta colisões com certas collision layers. Use isto em conjunto com uma collision layer personalizada, ou para detetar apenas a camada de physics ou de pointer events. Veja [camadas de colisão](/docs/creator/content-creator-pt/scenes-sdk7/fundamentos-de-conteudo-3d/colliders/#collision-layers). Por defeito, o valor é `ColliderLayer.CL_POINTER | ColliderLayer.CL_PHYSICS`.
* `originOffset`: Em vez de iniciar o raycast a partir da posição de origem da entidade, adicione um offset para iniciar a query a partir de uma posição relativa. Pode, por exemplo, usar um pequeno offset para evitar que o ray colida com o próprio modelo 3D da entidade.
* `continuous`: Se true, continuará a executar uma query de raycast em cada frame. Se false, o ray será usado apenas no frame atual. Por defeito, este valor é false.


> [!WARNING]
> **📔 Nota**: A `continuous` a propriedade deve ser usada com cautela, pois executar uma query de raycast em cada frame pode ser muito dispendioso para o desempenho. Quando possível, use um system (ou a `interval` função na biblioteca Utils) para executar queries de raycast a intervalos regulares mais espaçados, veja [raycasting recorrente](#recurrent-raycasting).


O exemplo seguinte usa uma rotação global para determinar a direção e apenas devolve a primeira entidade que é atingida no frame em que o ray é enviado.

```typescript
const entity1 = engine.addEntity()

Transform.create(entity1, {
  position: Vector3.create(8, 1, 0)
})

Raycast.createOrReplace(entity1, {
  direction: {
    $case: "globalDirection",
    globalDirection: Vector3.create(0, 0, 1)
  },
  maxDistance: 16,
  queryType: RaycastQueryType.RQT_HIT_FIRST
})
```

O exemplo abaixo lança um raio na direção frontal da Entity, retornando apenas o primeiro item atingido. Ele faz isso continuamente. Também inclui um pequeno deslocamento de 0,5 para evitar que o raio atinja o collider da própria Entity.

```typescript
const entity1 = engine.addEntity()

Transform.create(entity1, {
  position: Vector3.create(8, 1, 0)
})

Raycast.createOrReplace(entity1, {
  direction: {
    $case: "localDirection",
    localDirection: Vector3.Forward()
  },
  maxDistance: 16,
  queryType: RaycastQueryType.RQT_HIT_FIRST,
  originOffset: Vector3.create(0.5, 0, 0),
  continuous: true
})
```

Este exemplo traça um raio entre duas entities. Ele retorna todas as entities atingidas entre elas.

```ts
const entity1 = engine.addEntity()

Transform.create(entity1, {
  position: Vector3.create(8, 1, 0)
})

const entity2 = engine.addEntity()

Transform.create(entity2, {
  position: Vector3.create(0, 1, 8)
})

Raycast.createOrReplace(entity1, {
  direction: {
    $case: "targetEntity",
    targetEntity: entity2
  },
  maxDistance: 16,
  queryType: RaycastQueryType.RQT_QUERY_ALL
})
```

### Componente de resultados do Raycast


> [!WARNING]
> **📔 Nota**: A maneira mais fácil de lidar com os resultados do raycast é usar `raycastSystem`, e registrar uma função de callback como parte da mesma instrução que cria o raio. O`RaycastResult` componente é usado internamente por essa interface, mas também é exposto para permitir lógica personalizada mais avançada.


Depois de criar um componente Raycast, a Entity à qual esse componente é adicionado terá um `RaycastResult` componente. Este componente inclui informações sobre quaisquer acertos do raio. Configure um System para verificar esses dados.

O `RaycastResult` componente contém os seguintes dados:

* `globalOrigin`: A posição onde o ray foi originado, relativa à cena.
* `direction`: A direção global para a qual o ray estava apontado, como um `Vector3`.
* `hits`: Um array com um objeto por cada entidade que foi atingida. Se não houver entidades atingidas, este array está vazio. Se o raycast usou `RaycastQueryType.RQT_HIT_FIRST`, este array conterá apenas um objeto.

Cada objeto no `hits` array inclui:

* `entityId`: Número de Id da entidade que foi atingida pelo ray.
* `meshName`: *String* com o nome interno da malha específica no modelo 3D que foi atingida. Isto é útil quando um modelo 3D é composto por várias malhas.
* `posição`: *Vector3* para a posição onde o ray intersectou a entidade atingida (relativa à cena)
* `length`: Comprimento do ray desde a sua origem até à posição onde ocorreu o impacto na entidade.
* `normalHit`: *Vector3* para a normal da superfície atingida no espaço global.
* `globalOrigin`: *Vector3* para a posição onde o ray se origina (relativa à cena)
* `direction`: A direção global para a qual o ray estava apontado, como um `Vector3`.

O exemplo abaixo mostra como você pode acessar os resultados de uma Entity individual usando um System:

```typescript

const rayEntity = engine.addEntity()

Transform.create(rayEntity, {
  position: Vector3.create(8, 1, 0)
})

// retornar todas as entities
Raycast.createOrReplace(rayEntity, {
  direction: {
    $case: "globalDirection",
    globalDirection: Vector3.create(0, 0, 1)
  },
  maxDistance: 16,
  queryType: RaycastQueryType.RQT_QUERY_ALL
})

engine.addSystem(() => {
  const rayResult = RaycastResult.get(rayEntity)
  console.log(rayResult.hits)
})
```

O próximo exemplo mostra como você pode acessar `RaycastResult` componentes de todas as entities na cena, usando uma [consulta de componente](/docs/creator/content-creator-pt/scenes-sdk7/arquitetura/querying-components/).

```typescript
engine.addSystem(() => {
  for (const [_, result] of engine.getEntitiesWith(RaycastResult)) {
    console.log(result.hits)
  }
})
```


> [!WARNING]
> **📔 Nota**: Os resultados de um raycast não chegam no mesmo tick do game loop em que você criou o raycast. Os resultados podem levar um ou vários ticks para chegar.


Em uma cena em que você usa vários tipos de raios para diferentes propósitos (como para busca de caminho, verificação de linha de visão, rastreamento de projéteis, etc.), talvez você queira usar diferentes [camadas de colisão](/docs/creator/content-creator-pt/scenes-sdk7/fundamentos-de-conteudo-3d/colliders/#collision-layers), para evitar calcular colisões irrelevantes.


> [!NOTE]
> **💡 Dica**: Para um exemplo funcional de raycasting, veja o [`77,-1-raycast-unit-tests`](https://github.com/decentraland/sdk7-test-scenes/tree/main/scenes/77,-1-raycast-unit-tests) test scene, que afirma o comportamento do raycast por meio de `@dcl/sdk/testing` testes unitários em toda a `localDirection`, `globalDirection`, `globalTarget` e `entidade-alvo` formas de direção, incluindo raios lançados a partir de pais transformados. Para ver como se comportam as collider layers que um raio pode atingir, veja [`5,5-collider-layers`](https://github.com/decentraland/sdk7-test-scenes/tree/main/scenes/5,5-collider-layers).



---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-pt/scenes-sdk7/interatividade/raycasting.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
