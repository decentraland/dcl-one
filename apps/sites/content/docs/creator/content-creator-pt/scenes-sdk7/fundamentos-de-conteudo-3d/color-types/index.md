> Como definir valores de cor

# Tipos de Cor

Os valores de cor podem ser passados como propriedades de diferentes componentes, como [Materiais](/docs/creator/content-creator-pt/scenes-sdk7/fundamentos-de-conteudo-3d/materials/), [Text](/docs/creator/content-creator-pt/scenes-sdk7/fundamentos-de-conteudo-3d/text/) ou diferentes propriedades de [UI entities](/docs/creator/content-creator-pt/scenes-sdk7/ui-2d/onscreen-ui/). Os valores de cor podem ser do tipo `Color3` ou `Color4`.

`Color3` contém três propriedades, *vermelho*, *verde*, e *azul*. `Color4` tem essas mesmas três mais `valor de alpha`, para valores de transparência.

## Defina uma cor

Você pode criar uma cor usando as `Color3.create()` ou no `Color4.create()` funções.

```ts
// Cor vermelha
const red3 = Color3.create(1, 0, 0)
const red4 = Color4.create(1, 0, 0, 1)
```

Você também pode criar certas cores predefinidas que fazem parte dos `Color3` e `Color4` namespaces.

```ts
const red = Color3.Red()
const blue = Color3.Blue()
const black = Color3.Black()
```


> [!NOTE]
> **💡 Dica**: Escreva `Color3.` ou `Color4.` e o Visual Studio deve sugerir todos os valores possíveis em um dropdown inteligente.


Caso contrário, você pode escolher uma cor aleatória usando a seguinte função:

```ts
const randomColor = Color3.Random()
```

Se preferir descrever uma cor em hexadecimal, use `Color3.fromHexString()`.

```ts
const red = Color3.fromHexString('FD350A')
const blue = Color3.fromHexString('0A0CFD')
```

Qualquer valor de objeto que inclua valores numéricos para `r`, `g`, e `b` pode ser interpretado como um `Color3` valor. Da mesma forma, qualquer objeto que inclua essas propriedades mais um `a` valor pode ser interpretado como um `Color4` valor. Isso permite que você também use a seguinte sintaxe:

```ts
// Cor vermelha
const red3 = { r: 1, g: 0, b: 0 }
const red4 = { r: 1, g: 0, b: 0, a: 1 }
```

O exemplo a seguir usa uma propriedade de cor como parte de um `TextShape` component, para definir a cor do texto.

```ts
const myEntity = engine.addEntity()

Transform.create(myEntity, {
  position: Vector3.create(4, 1, 4),
})

TextShape.create(myEntity, {
  text: 'este texto é VERMELHO',
  textColor: Color4.create(1, 0, 0, 1),
})
```


> [!WARNING]
> **📔 Nota**: `Color3` e `Color4` devem ser importados via
> 
> > `import { Color3, Color4 } from "@dcl/sdk/math"`
> 
> Veja [Imports](/docs/creator/content-creator-pt/scenes-sdk7/primeiros-passos/coding-scenes/#imports) para saber como lidar com isso facilmente.


## Transparência

Use o seletor *valor de alpha* propriedade de `Color4` para tornar uma cor translúcida.

Se alpha for 1, a cor ficará completamente opaca; se for 0, ficará completamente invisível. Qualquer valor intermediário resulta em um tom parcialmente transparente.

```ts
// Completamente opaco
const opaque = Color4.create(1, 1, 1, 1)

// Meio-termo
const half = Color4.create(1, 1, 1, 0.5)

// Quase invisível
const almostInvisible = Color4.create(1, 1, 1, 0.1)
```

## Lerp

Use o seletor `Color3.lerp()` ou no `Color4.lerp()` função para definir uma cor que fica em algum ponto entre outras duas cores. Estas funções funcionam de forma semelhante à `Vector3.lerp()` .

Ambos `Color3.lerp()` ou no `Color4.lerp()` recebem os seguintes argumentos:

* `left`: A primeira cor a ser usada como referência
* `right`: A segunda cor a ser usada como referência
* `amount`: Um número de 0 a 1 para definir quanto da *right* cor usar na mistura. Quanto mais próximo de 0, mais próxima da *left* cor.

```ts
const orange = Color3.lerp(Color3.Red(), Color3.Yellow(), 0.3)
```

Você pode usar um system para mudar gradualmente a `amount` parâmetro, para criar uma transição suave.

```ts
// Systems
var pulseState: number = 0
const color1 = Color4.Red()
const color2 = Color4.Yellow()

export function PulseSystem(dt: number) {
  pulseState += dt
  // oscila entre 0 e 1
  const amount = (Math.sin(pulseState) + 1) / 2
  const entitiesWithMaterial = engine.getEntitiesWith(Material)

  // iterar pelas entidades do grupo
  for (const [entity] of entitiesWithMaterial) {
    const material = Material.getMutable(entity)
    if (material.material?.$case === 'pbr') {
      material.material.pbr.albedoColor = Color4.lerp(color1, color2, amount)
    }
  }
}

engine.addSystem(PulseSystem)
```


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-pt/scenes-sdk7/fundamentos-de-conteudo-3d/color-types.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
