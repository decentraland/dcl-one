> Cómo definir valores de color

# Tipos de color

Los valores de color pueden pasarse como propiedades de diferentes componentes, como [Materials](/docs/creator/content-creator-es/scenes-sdk7/fundamentos-del-contenido-3d/materials/), [Text](/docs/creator/content-creator-es/scenes-sdk7/fundamentos-del-contenido-3d/text/) o diferentes propiedades de [entidades de UI](/docs/creator/content-creator-es/scenes-sdk7/ui-2d/onscreen-ui/). Los valores de color pueden ser de tipo `Color3` o `Color4`.

`Color3` contiene tres propiedades, *rojo*, *verde*, y *azul*. `Color4` tiene esas mismas tres más `alpha`, para valores de transparencia.

## Definir un color

Puedes crear un color usando las `Color3.create()` o el `Color4.create()` funciones.

```ts
// Color rojo
const red3 = Color3.create(1, 0, 0)
const red4 = Color4.create(1, 0, 0, 1)
```

También puedes crear ciertos colores predefinidos que forman parte de los `Color3` y `Color4` espacios de nombres.

```ts
const red = Color3.Red()
const blue = Color3.Blue()
const black = Color3.Black()
```


> [!NOTE]
> **💡 Consejo**: Escribe `Color3.` o `Color4.` y Visual Studio debería sugerir todos los valores posibles en un dropdown inteligente.


De lo contrario, puedes elegir un color aleatorio usando la siguiente función:

```ts
const randomColor = Color3.Random()
```

Si prefieres describir un color en hexadecimal, usa `Color3.fromHexString()`.

```ts
const red = Color3.fromHexString('FD350A')
const blue = Color3.fromHexString('0A0CFD')
```

Cualquier valor de objeto que incluya valores numéricos para `r`, `g`, y `b` puede interpretarse como un `Color3` valor. Asimismo, cualquier objeto que incluya esas propiedades más un `a` valor puede interpretarse como un `Color4` valor. Esto te permite usar también la siguiente sintaxis:

```ts
// Color rojo
const red3 = { r: 1, g: 0, b: 0 }
const red4 = { r: 1, g: 0, b: 0, a: 1 }
```

El siguiente ejemplo usa una propiedad de color como parte de un `TextShape` componente, para definir el color del texto.

```ts
const myEntity = engine.addEntity()

Transform.create(myEntity, {
  position: Vector3.create(4, 1, 4),
})

TextShape.create(myEntity, {
  text: 'this text is RED',
  textColor: Color4.create(1, 0, 0, 1),
})
```


> [!WARNING]
> **📔 Nota**: `Color3` y `Color4` debe importarse mediante
> 
> > `import { Color3, Color4 } from "@dcl/sdk/math"`
> 
> Consulta [Importaciones](/docs/creator/content-creator-es/scenes-sdk7/primeros-pasos/coding-scenes/#imports) para saber cómo manejarlos fácilmente.


## Transparencia

Usa la *alpha* propiedad de `Color4` para hacer un color translúcido.

Si alpha es 1, el color será completamente opaco; si es 0, será completamente invisible. Cualquier valor intermedio da como resultado un tono parcialmente transparente.

```ts
// Completamente opaco
const opaque = Color4.create(1, 1, 1, 1)

// A mitad de camino
const half = Color4.create(1, 1, 1, 0.5)

// Casi invisible
const almostInvisible = Color4.create(1, 1, 1, 0.1)
```

## Lerp

Usa la `Color3.lerp()` o el `Color4.lerp()` función para definir un color que se encuentre entre otros dos colores. Estas funciones funcionan de manera similar a la `Vector3.lerp()` función.

Ambos `Color3.lerp()` o el `Color4.lerp()` toman los siguientes argumentos:

* `left`: El primer color a usar como referencia
* `right`: El segundo color a usar como referencia
* `amount`: Un número de 0 a 1 para definir cuánto del *right* color usar en la mezcla. Cuanto más cerca de 0, más cerca del *left* color.

```ts
const orange = Color3.lerp(Color3.Red(), Color3.Yellow(), 0.3)
```

Puedes usar un System para cambiar gradualmente el `amount` parámetro, para crear una transición suave.

```ts
// Sistemas
var pulseState: number = 0
const color1 = Color4.Red()
const color2 = Color4.Yellow()

export function PulseSystem(dt: number) {
  pulseState += dt
  // oscilar entre 0 y 1
  const amount = (Math.sin(pulseState) + 1) / 2
  const entitiesWithMaterial = engine.getEntitiesWith(Material)

  // iterar sobre las entidades del grupo
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
GET https://docs.decentraland.org/creator/content-creator-es/scenes-sdk7/fundamentos-del-contenido-3d/color-types.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
