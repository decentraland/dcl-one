> Saiba quais tipos especiais existem, incluindo Vector, Quaternions e muito mais.

# Tipos Especiais

## Vector3

Decentraland usa *vector3* dados para representar caminhos, pontos no espaço e direções. Vetores também podem ser usados para definir orientações de rotação, como uma alternativa mais amigável a *quaternions*. Um objeto Vector3 contém valores numéricos para cada um dos *x*, *y*, e *z* eixo.

```ts
const myVector: Vector3 = { x: 8, y: 1, z: 8 }
```

A `Vector3` namespace contém uma série de métodos úteis que você pode chamar para evitar lidar com a maioria das operações matemáticas com vetores. Escreva `Vector3.`, e o VS Studio exibirá um menu suspenso com todas as funções disponíveis.

Abaixo estão algumas linhas mostrando a sintaxe de algumas operações básicas com vetores.

```ts
// Criar um objeto vetor
let myVector = Vector3.create(3, 1, 5)

// Sintaxe alternativa para criar um objeto vetor
let myOtherVector: Vector3 = { x: 8, y: 1, z: 8 }

// Editar um dos seus valores
myVector.x = 5

// Chamar funções do namespace Vector3,
// Todas essas funções exigem passar objetos Vector3 em seus parâmetros

let normalizedVector = Vector3.normalize(myVector)

let distance = Vector3.distance(myVector, myOtherVector)

let midPoint = Vector3.lerp(myVector, myOtherVector, 0.5)
```

Objetos Vector3 são frequentemente necessários nos campos de vários components. Por exemplo, o `Transform` component contém `Vector3` valores para o *position* e *scale* da entity.

Para criar um [componente personalizado](/docs/creator/content-creator-pt/scenes-sdk7/arquitetura/custom-components/) com parâmetros que requerem valores Vector3, defina o tipo desses parâmetros como `Schemas.Vector3`.


> [!WARNING]
> **📔 Nota**: `Vector3` deve ser importado via
> 
> > `import { Vector3 } from "@dcl/sdk/math"`
> 
> Veja [Imports](/docs/creator/content-creator-pt/scenes-sdk7/primeiros-passos/coding-scenes/#imports) para saber como lidar com isso facilmente.


### Atalhos para escrever vetores de direção

Os atalhos a seguir existem para definir vetores genéricos:

* `Vector3.Zero()` retorna *(0, 0, 0)*
* `Vector3.Up()` retorna *(0, 1, 0)*
* `Vector3.Down()` retorna *(0, -1, 0)*
* `Vector3.Left()` retorna *(-1, 0, 0)*
* `Vector3.Right()` retorna *(1, 0, 0)*
* `Vector3.Forward()` retorna *(0, 0, 1)*
* `Vector3.Backward()` retorna *(0, 0, -1)*

## Quaternions

Quaternions são usados para armazenar informações de rotação para o component Transform. Um Quaternion é composto por quatro valores numéricos entre -1 e 1: *x*, *y*, *z*, *w*.

```ts
const myQuaternion: Quaternion = { x: 0, y: 0, z: 0, w: 1 }
```

Quaternions são diferentes de [*Euler* ângulos](https://en.wikipedia.org/wiki/Euler_angles), a notação mais comum *x*, *y* e *z* a notação com números que vai de 0 a 360 com a qual a maioria das pessoas está familiarizada. O engine expressa todas as rotações como Quaternions, então faz sentido evitar cálculos para converter de e para Euler sempre que possível.

A `Quaternion` namespace contém uma série de métodos úteis que você pode chamar para evitar lidar com muitas operações matemáticas. Escreva `Quaternion.`, e o VS Studio exibirá um menu suspenso com todas as funções disponíveis.

Abaixo estão algumas linhas mostrando a sintaxe de algumas operações básicas com Quaternions.

```ts
// Criar um objeto quaternion
let myQuaternion = Quaternion.create(0, 0, 0, 1)

// Editar um dos seus valores
myQuaternion.x = 1

// Chamar funções do namespace quaternion
let midPoint = Quaternion.slerp(myQuaternion1, myQuaternion2, 0.5)

// Obter a rotação necessária para girar de olhar para uma direção para outra,
// passando dois valores Vector3 de direção
let rotation = Quaternion.fromToRotation(Vector3.Forward(), Vector3.Up())
```

Como é muito mais fácil pensar em termos de graus de Euler, o SDK inclui algumas funções para converter de e para Quaternions e Euler.


> [!NOTE]
> **💡 Dica**: Evite executar essas conversões como parte de uma lógica recorrente dentro de um system, que roda a cada tick, pois isso pode ser caro. Essas conversões são mais úteis para operações únicas, como definir a rotação de uma nova entity.


```ts
// De Euler para Quaternion
let myQuaternion = Quaternion.fromEulerDegrees(90, 0, 0)

// De quaternion para Euler
let myEuler = Quaternion.toEulerAngles(myQuaternion)
```

Objetos Quaternion são frequentemente necessários nos campos de components. Por exemplo, o `Transform` component contém `Quaternion` valores de rotação da entity.

Para criar um [componente personalizado](/docs/creator/content-creator-pt/scenes-sdk7/arquitetura/custom-components/) com parâmetros que requerem valores Quaternion, defina o tipo desses parâmetros como `Schemas.Quaternion`.


> [!WARNING]
> **📔 Nota**: `Quaternion` deve ser importado via
> 
> > `import { Quaternion } from "@dcl/sdk/math"`
> 
> Veja [Imports](/docs/creator/content-creator-pt/scenes-sdk7/primeiros-passos/coding-scenes/#imports) para saber como lidar com isso facilmente.


## Escalares

Um escalar nada mais é do que um número. Por esse motivo, não faz muito sentido instanciar um `Scalar` objeto para armazenar dados, já que você pode fazer o mesmo com um número. As funções no `Scalar` namespace, no entanto, expõem várias funções úteis (semelhantes às do *Vector3* namespace), que podem ser usadas em números.

```ts
// Chamar funções da classe Scalar
let random = Scalar.randomRange(1, 100)

let midPoint = Scalar.lerp(number1, number2, 0.5)

let clampedValue = Scalar.clamp(myInput, 0, 100)
```


> [!WARNING]
> **📔 Nota**: `Scalar` deve ser importado via
> 
> > `import { Scalar } from "@dcl/sdk/math"`
> 
> Veja [Imports](/docs/creator/content-creator-pt/scenes-sdk7/primeiros-passos/coding-scenes/#imports) para saber como lidar com isso facilmente.



---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-pt/scenes-sdk7/fundamentos-de-conteudo-3d/special-types.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
