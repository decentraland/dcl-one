> Saiba como os systems são usados para atualizar o estado da scene

# Systems

As scenes da Decentraland dependem de *systems* para atualizar quaisquer dados ao longo do tempo, incluindo informações armazenadas em cada Entity [components](/docs/creator/content-creator-pt/scenes-sdk7/arquitetura/entities-components/).

![](/docs/_remote/0b852c135fff08d1f4c9243d.webp)

*systems* são o que tornam as scenes dinâmicas; são funções executadas periodicamente a cada tick do loop do jogo da scene, alterando o que será renderizado.

O exemplo a seguir mostra uma declaração básica de System:

```ts
// Defina o System
function mySystem() {
  console.log("Executado a cada tick. O meu System está em execução")
}
// Adicione o System ao engine
engine.addSystem(mySystem)
```

A função num System pode fazer o que você quiser. Normalmente, ela atuará sobre todas as entities que atendam a certas [consulta](/docs/creator/content-creator-pt/scenes-sdk7/arquitetura/querying-components/), seguindo uma lógica específica para alterar os valores armazenados nos components da Entity.

```ts
function moveSystem(dt: number) {
  // iterar sobre todas as entities com um Transform
  for (const [entity] of engine.getEntitiesWith(Transform)) {

  // obter um componente Transform mutável
  const transform = Transform.getMutable(entity)

  // atualizar o valor da posição
    transform.position.z += 0.01
  }
}

engine.addSystem(moveSystem)
```

No exemplo acima, o System `MoveSystem` é uma função que é executada a cada tick do loop do jogo, alterando a posição de cada Entity na scene que tenha um Transform.

![](/docs/_remote/7e85b65f2626d96c8f892499.webp)

Você pode ter vários Systems na sua scene para desacoplar comportamentos diferentes, tornando o seu código mais limpo e mais fácil de escalar e reutilizar. Por exemplo, um System pode lidar com physics, outro pode fazer uma Entity obstáculo mover-se para a frente e para trás continuamente, e outro pode lidar com a IA dos personagens.

Vários Systems podem atuar sobre uma única Entity. Por exemplo, um personagem não jogável pode mover-se por conta própria com base numa IA, mas também ser afetado pela gravidade ao andar acidentalmente para fora de um penhasco. Nesse cenário, os Systems de physics e de IA nem precisam de saber um do outro. Eles reavaliam independentemente o seu estado atual a cada tick do loop do jogo e implementam a sua própria lógica separada.

## A função do System

A função de um System é executada periodicamente, uma vez a cada tick do loop do jogo. Isso acontece automaticamente; você não precisa chamar explicitamente essa função de lugar nenhum no seu código.

Numa scene da Decentraland, você pode pensar no loop do jogo como a agregação de todas as funções dos Systems na sua scene.


> [!WARNING]
> **📔 Nota**: Você não pode adicionar a mesma função de System ao engine mais de uma vez. Se tentar adicionar um System que já foi adicionado, o engine lança um erro: `System "<name>" já foi adicionado ao engine`.


## Lide com entities por referência

Alguns components e Systems destinam-se a ser usados apenas numa única Entity na scene. Por exemplo, numa Entity que armazena a pontuação de um jogo ou talvez num portão principal que é único na scene. Para aceder a uma dessas entities dentro de um System, você pode simplesmente referir-se à Entity ou aos seus components pelo nome nas funções do System.

```ts
// declare a referência da Entity no escopo do módulo para que os Systems possam aceder a ela
let game: Entity

export function main(){
	// crie uma nova Entity
	game = engine.addEntity()

	// adicione um component a essa Entity
	ScoreComponent.create(game)
}

// Defina o System
export function UpdateScore() {

  // chame a referência para a Entity individual
  const points = ScoreComponent.get(game).points
  console.log(points)
}

// Adicione o System ao engine
engine.addSystem(UpdateScore)
```

Para projetos maiores, recomendamos manter as definições de Systems em ficheiros separados da instanciação de entities e components.

## Itere sobre uma query de components

Muitas vezes, a sua scene terá várias entities do mesmo tipo com comportamentos semelhantes. Por exemplo, muitas portas que podem ser abertas, ou muitos inimigos que podem atacar o jogador. Faz sentido tratar todas essas entities semelhantes num único System, iterando pela lista e fazendo as mesmas verificações em cada uma.

Você não quer que a função de um System itere sobre *todo o* conjunto de entities da scene, pois isso pode ser muito caro em termos de poder de processamento. Para evitar isso, você pode [consultar componentes](/docs/creator/content-creator-pt/scenes-sdk7/arquitetura/querying-components/), para iterar apenas sobre as entities relevantes.

Por exemplo, a sua scene pode ter um `PhysicsSystem` que calcula o efeito da gravidade sobre as entities da sua scene. Algumas entities na sua scene, como árvores, não devem mover-se nunca; por isso, seria sensato evitar calcular os efeitos da gravidade sobre elas. Você pode definir um `HasPhysics` component para marcar entities que podem ser afetadas pela gravidade, e depois fazer com que `PhysicsSystem` trate apenas das entities devolvidas por esta query.

```ts
// Defina o System
export function PhysicsSystem() {
  // iterar sobre todas as entities com um HasPhysics
  for (const [entity] of engine.getEntitiesWith(HasPhysics)) {

  // obter um componente Transform mutável
  const transform = Transform.getMutable(entity)

  // Calcular o efeito da physics
  }
}

// Adicione o System ao engine
engine.addSystem(PhysicsSystem)
```

## Tempo delta entre frames

A função num System pode, opcionalmente, incluir um argumento chamado `dt`, do tipo `number` (representando *tempo delta*).

```ts
function MySystem(dt: number) {

  // Atualize a scene
  console.log("tempo desde o último tick: ", dt)
}

engine.addSystem(MySystem)
```

*tempo delta* representa o tempo decorrido desde o último tick do loop do jogo, em segundos.

A `dt` o argumento é o tempo real decorrido desde o último tick do loop do jogo, medido em segundos. O SDK não impõe uma taxa de tick fixa: o host runtime passa esse valor, e ele varia com o framerate da scene.

Quanto mais suave for a execução de uma scene, mais frequentemente os ticks ocorrem e menor é cada `dt` valor. Quando um tick demora mais a processar, passa mais tempo antes do próximo e `dt` aumenta proporcionalmente.

Esta taxa de frames do lado da scene é limitada a 30 ticks por segundo, caso em que o valor de `dt` é *1/30* (0.0333...).

![](/docs/_remote/d1bc4cb88306d00b1e56431f.webp)

Quando o processamento de um frame demora mais, a renderização desse frame é atrasada, e o `dt` valor passado no tick seguinte reflete esse intervalo mais longo. O engine não compensa o atraso retroativamente; `dt` sempre reporta o tempo realmente decorrido para que os seus Systems se possam ajustar a ele.

![](/docs/_remote/5ee6f60ba5826b88ac1603b7.webp)

Idealmente, você deve evitar que a sua scene perca frames, pois isso afeta a qualidade da experiência do jogador. Como isso depende do poder de processamento da máquina do jogador, existe sempre a possibilidade de a sua scene ter de estar preparada para lidar com isso de forma graciosa.

A `dt` A variável é útil quando o processamento do frame excede o tempo padrão. Assumindo que o frame atual levará tanto tempo quanto o anterior, esta informação pode ser usada para calcular quanto ajustar uma mudança gradual, de modo que a taxa de mudança pareça constante e proporcional ao atraso entre frames.

Veja [posicionamento de entities](/docs/creator/content-creator-pt/scenes-sdk7/fundamentos-de-conteudo-3d/entity-positioning/) para exemplos de como usar `dt` para tornar o movimento mais suave.

## Loop em intervalo temporizado

Se quiser que um System execute algo num intervalo de tempo regular, pode fazê-lo combinando o `dt` argumento com um temporizador.

```ts
let timer: number = 10

function LoopSystem(dt: number) {
  timer -= dt
  if (timer <= 0) {
      timer = 10
      // FAÇA ALGUMA COISA
    }
}

engine.addSystem(LoopSystem)
```

Existe também uma função de atalho `setInterval` e `clearInterval` que usa Systems em segundo plano. Isso permite uma versão mais fácil e curta quando a intenção é executar uma determinada função a cada X unidades de tempo.

```ts
import { timers } from '@dcl/sdk/ecs'

const intervalId = timers.setInterval(() => {
    console.log('A imprimir isto a cada 10 segundos')
}, 10000)
```

Em que o primeiro argumento, `callback`, é a função a ser executada (neste caso, a `console.log()`), e o segundo argumento, `ms`(10000 neste caso), são os milissegundos a aguardar entre cada execução da função.

Para parar o `setInterval` função, `clearInterval` é usado.

```
timers.clearInterval(intervalId)
```

Em que `intervalId` é a referência ao `setInterval` retorno definido anteriormente.

Para casos de uso mais complexos, em que podem ser criados dinamicamente vários atrasos e loops, pode valer a pena definir um component personalizado para armazenar um valor de temporizador individual para cada Entity. Veja [Components personalizados](/docs/creator/content-creator-pt/scenes-sdk7/arquitetura/custom-components/).

## Ordem de execução dos Systems

Em alguns casos, quando tem vários Systems em execução, pode interessar-lhe saber que System é executado primeiro pela sua scene.

Por exemplo, pode ter um *física* System que atualiza a posição das entities na scene, e outro *limites* System que garante que nenhuma das entities fica posicionada fora dos limites da scene. Neste caso, você quer garantir que o *limites* System é executado por último. Caso contrário, o *física* System poderia mover entities para fora dos limites da scene, mas o *limites* System não ficará a saber disso até ser executado novamente no frame seguinte.

Ao adicionar um System ao engine, defina um `priority` campo opcional para determinar quando o System é executado em relação a outros Systems.

```ts
engine.addSystem(PhysicsSystem, 5)
engine.addSystem(BoundariesSystem, 1)
```

Systems com um número de prioridade mais alto são executados primeiro, por isso um System com prioridade de *5* é executado antes de um com prioridade *1*.

Systems que não recebem uma prioridade explícita têm uma prioridade padrão de *100000*. Como os números mais altos correm primeiro, estes Systems são executados antes dos que têm uma prioridade explícita mais baixa.

Se dois Systems tiverem o mesmo número de prioridade, não há como saber com certeza qual deles será executado primeiro.

## Remover um System

Uma instância de um System pode ser adicionada ou removida do engine para o ligar ou desligar.

Se um System estiver definido mas não for adicionado ao engine, a sua função não é chamada pelo engine.

Para remover um System, primeiro tem de lhe dar um nome ao adicioná-lo ao engine, para que possa referir-se ao System mais tarde.

```ts
// declarar System
function mySystem(dt: number){
  console.log("atraso desde o último tick: ", dt)
}

// adicionar System (dando-lhe uma prioridade e um nome)
engine.addSystem(mySystem, 1, "DelaySystem")

// remover System
engine.removeSystem("DelaySystem")
```

A string que passa para `engine.removeSystem()` deve corresponder ao nome que atribuiu ao System quando o adicionou.

Outra forma de remover um System é passar a própria função do System para o `engine.removeSystem()` método. Desta forma, não precisa de dar um nome ao System ao adicioná-lo.

```ts
// declarar System
function mySystem(dt: number){
  console.log("atraso desde o último tick: ", dt)
}

// adicionar System
engine.addSystem(mySystem)

// remover System passando a sua função
engine.removeSystem(mySystem)
```

`engine.removeSystem()` aceita quer a string do nome que atribuiu ao adicionar o System, quer uma referência à própria função do System.

Pode usar o método abaixo para fazer com que um System se termine a si próprio quando o seu objetivo estiver concluído.

```ts
   const mySystem = function(dt: number){
        time += dt
        if(time > 3){
		engine.removeSystem(mySystem)
        }
    }
    engine.addSystem(mySystem)
```


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-pt/scenes-sdk7/arquitetura/systems.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
