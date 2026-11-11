> Aprende qué tipos especiales existen, incluidos Vector, Quaternions y más.

# Tipos especiales

## Vector3

Decentraland usa *Vector3* datos para representar rutas, puntos en el espacio y direcciones. Los vectores también pueden usarse para definir orientaciones de rotación, como una alternativa más amigable a *quaternions*. Un objeto Vector3 contiene valores numéricos para cada uno de los *x*, *y*, y *z* eje.

```ts
const myVector: Vector3 = { x: 8, y: 1, z: 8 }
```

El `Vector3` namespace contiene una serie de métodos útiles que puedes llamar para evitar tener que lidiar con la mayoría de las operaciones matemáticas con vectores. Escribe `Vector3.`, y VS Studio mostrará una lista desplegable con todas las funciones disponibles.

A continuación se muestran unas líneas que muestran la sintaxis de algunas operaciones básicas con vectores.

```ts
// Crear un objeto vector
let myVector = Vector3.create(3, 1, 5)

// Sintaxis alternativa para crear un objeto vector
let myOtherVector: Vector3 = { x: 8, y: 1, z: 8 }

// Edita uno de sus valores
myVector.x = 5

// Llama funciones del namespace Vector3,
// Todas estas funciones requieren pasar objetos Vector3 en sus parámetros

let normalizedVector = Vector3.normalize(myVector)

let distance = Vector3.distance(myVector, myOtherVector)

let midPoint = Vector3.lerp(myVector, myOtherVector, 0.5)
```

A menudo se requieren objetos Vector3 en los campos de varios components. Por ejemplo, el `Transform` component contiene `Vector3` valores para el *position* y *scale* de la entity.

Para crear un [custom component](/docs/creator/content-creator-es/scenes-sdk7/arquitectura/custom-components/) con parámetros que requieran valores Vector3, establece el tipo de estos parámetros como `Schemas.Vector3`.


> [!WARNING]
> **📔 Nota**: `Vector3` debe importarse mediante
> 
> > `import { Vector3 } from "@dcl/sdk/math"`
> 
> Consulta [Importaciones](/docs/creator/content-creator-es/scenes-sdk7/primeros-pasos/coding-scenes/#imports) para saber cómo manejarlos fácilmente.


### Atajos para escribir vectores de dirección

Existen los siguientes atajos para definir vectores genéricos:

* `Vector3.Zero()` devuelve *(0, 0, 0)*
* `Vector3.Up()` devuelve *(0, 1, 0)*
* `Vector3.Down()` devuelve *(0, -1, 0)*
* `Vector3.Left()` devuelve *(-1, 0, 0)*
* `Vector3.Right()` devuelve *(1, 0, 0)*
* `Vector3.Forward()` devuelve *(0, 0, 1)*
* `Vector3.Backward()` devuelve *(0, 0, -1)*

## Quaternions

Los Quaternions se usan para almacenar información de rotación del componente Transform. Un Quaternion está compuesto por cuatro valores numéricos entre -1 y 1: *x*, *y*, *z*, *w*.

```ts
const myQuaternion: Quaternion = { x: 0, y: 0, z: 0, w: 1 }
```

Los Quaternions son diferentes de [*Euler* ángulos](https://en.wikipedia.org/wiki/Euler_angles), la notación más común *x*, *y* y *z* la notación con números que van de 0 a 360 con la que la mayoría de las personas está familiarizada. El Engine expresa todas las rotaciones como Quaternions, así que tiene sentido evitar cálculos para convertir de y hacia Euler siempre que sea posible.

El `Quaternion` namespace contiene una serie de métodos útiles que puedes llamar para evitar tener que lidiar con muchas operaciones matemáticas. Escribe `Quaternion.`, y VS Studio mostrará una lista desplegable con todas las funciones disponibles.

A continuación se muestran unas líneas que muestran la sintaxis de algunas operaciones básicas con Quaternions.

```ts
// Crear un objeto quaternion
let myQuaternion = Quaternion.create(0, 0, 0, 1)

// Edita uno de sus valores
myQuaternion.x = 1

// Llama funciones del namespace quaternion
let midPoint = Quaternion.slerp(myQuaternion1, myQuaternion2, 0.5)

// Obtén la rotación necesaria para girar de una dirección a otra,
// pasando dos valores de dirección Vector3
let rotation = Quaternion.fromToRotation(Vector3.Forward(), Vector3.Up())
```

Como es mucho más fácil pensar en términos de grados Euler, el SDK incluye un par de funciones para convertir hacia y desde Quaternions y Euler.


> [!NOTE]
> **💡 Consejo**: Evita ejecutar estas conversiones como parte de la lógica recurrente dentro de un system, que se ejecuta en cada tick, ya que eso puede resultar costoso. Estas conversiones son útiles sobre todo para operaciones puntuales, como establecer la rotación de una nueva entity.


```ts
// De Euler a Quaternion
let myQuaternion = Quaternion.fromEulerDegrees(90, 0, 0)

// De quaternion a Euler
let myEuler = Quaternion.toEulerAngles(myQuaternion)
```

A menudo se requieren objetos Quaternion en los campos de components. Por ejemplo, el `Transform` component contiene `Quaternion` valores de rotación de la entity.

Para crear un [custom component](/docs/creator/content-creator-es/scenes-sdk7/arquitectura/custom-components/) con parámetros que requieran valores Quaternion, establece el tipo de estos parámetros como `Schemas.Quaternion`.


> [!WARNING]
> **📔 Nota**: `Quaternion` debe importarse mediante
> 
> > `import { Quaternion } from "@dcl/sdk/math"`
> 
> Consulta [Importaciones](/docs/creator/content-creator-es/scenes-sdk7/primeros-pasos/coding-scenes/#imports) para saber cómo manejarlos fácilmente.


## Escalares

Un escalar no es más que un número. Por esa razón, no tiene mucho sentido instanciar un `Scalar` objeto para almacenar datos, ya que puedes hacer lo mismo con un número. Las funciones del `Scalar` namespace, sin embargo, exponen varias funciones útiles (similares a las del *Vector3* namespace), que pueden usarse con números.

```ts
// Llama funciones de la clase Scalar
let random = Scalar.randomRange(1, 100)

let midPoint = Scalar.lerp(number1, number2, 0.5)

let clampedValue = Scalar.clamp(myInput, 0, 100)
```


> [!WARNING]
> **📔 Nota**: `Scalar` debe importarse mediante
> 
> > `import { Scalar } from "@dcl/sdk/math"`
> 
> Consulta [Importaciones](/docs/creator/content-creator-es/scenes-sdk7/primeros-pasos/coding-scenes/#imports) para saber cómo manejarlos fácilmente.



---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-es/scenes-sdk7/fundamentos-del-contenido-3d/special-types.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
