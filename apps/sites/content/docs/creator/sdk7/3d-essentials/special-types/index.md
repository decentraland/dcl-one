> Learn what special types exist, including Vector, Quaternions, and more.

# Special Types

## Vector3

Decentraland uses _vector3_ data to represent paths, points in space, and directions. Vectors can also be used to define rotation orientations, as a friendlier alternative to _quaternions_. A Vector3 object contains numerical values for each of the _x_, _y_, and _z_ axis.

```ts
const myVector: Vector3 = { x: 8, y: 1, z: 8 }
```

The `Vector3` namespace contains a series of handy methods that you can call to avoid having to deal with most vector math operations. Write `Vector3.`, and VS Studio will display a dropdown with all of the available functions.

Below are a few lines showing the syntax for some basic operations with vectors.

```ts
// Create a vector object
let myVector = Vector3.create(3, 1, 5)

// Alternative syntax to create a vector object
let myOtherVector: Vector3 = { x: 8, y: 1, z: 8 }

// Edit one of its values
myVector.x = 5

// Call functions from the Vector3 namespace,
// All these functions require passing Vector3 objects in their parameters

let normalizedVector = Vector3.normalize(myVector)

let distance = Vector3.distance(myVector, myOtherVector)

let midPoint = Vector3.lerp(myVector, myOtherVector, 0.5)
```

Vector3 objects are often required in the fields of several components. For example, the `Transform` component contains `Vector3` values for the _position_ and _scale_ of the entity.

To create a [custom component](/docs/creator/sdk7/architecture/custom-components/) with parameters that require Vector3 values, set the type of these parameters as `Schemas.Vector3`.


> [!WARNING]
> **📔 Note**: `Vector3` must be imported via
> 
> > `import { Vector3 } from "@dcl/sdk/math"`
> 
> See [Imports](/docs/creator/sdk7/getting-started/coding-scenes/#imports) for how to handle these easily.


### Shortcuts for writing direction vectors

The following shortcuts exist for defining generic vectors:

* `Vector3.Zero()` returns _(0, 0, 0)_
* `Vector3.Up()` returns _(0, 1, 0)_
* `Vector3.Down()` returns _(0, -1, 0)_
* `Vector3.Left()` returns _(-1, 0, 0)_
* `Vector3.Right()` returns _(1, 0, 0)_
* `Vector3.Forward()` returns _(0, 0, 1)_
* `Vector3.Backward()` returns _(0, 0, -1)_

## Quaternions

Quaternions are used to store rotation information for the Transform component. A Quaternion is composed of four numerical values between -1 and 1: _x_, _y_, _z_, _w_.

```ts
const myQuaternion: Quaternion = { x: 0, y: 0, z: 0, w: 1 }
```

Quaternions are different from [_Euler_ angles](https://en.wikipedia.org/wiki/Euler_angles), the more common _x_, _y_ and _z_ notation with numbers that go from 0 to 360 that most people are familiar with. The engine expresses all rotations as Quaternions, so it makes sense to avoid computations to convert to and from euler whenever possible.

The `Quaternion` namespace contains a series of handy methods that you can call to avoid having to deal with many math operations. Write `Quaternion.`, and VS Studio will display a dropdown with all of the available functions.

Below are a few lines showing the syntax for some basic operations with Quaternions.

```ts
// Create a quaternion object
let myQuaternion = Quaternion.create(0, 0, 0, 1)

// Edit one of its values
myQuaternion.x = 1

// Call functions from the quaternion namespace
let midPoint = Quaternion.slerp(myQuaternion1, myQuaternion2, 0.5)

// Get the rotation needed to turn from facing one direction to another,
// passing two direction Vector3 values
let rotation = Quaternion.fromToRotation(Vector3.Forward(), Vector3.Up())
```

Since it's a lot easier to think in terms of Euler degrees, the SDK includes a couple of functions to convert to and from Quaternions and Euler.


> [!NOTE]
> **💡 Tip**: Avoid running these conversions as part of recurrent logic inside a system, that run on every tick, as that can get expensive. These conversions are mostly useful for one-time operations, like setting the rotation of a new entity.


```ts
// From euler to Quaternion
let myQuaternion = Quaternion.fromEulerDegrees(90, 0, 0)

// From quaternion to Euler
let myEuler = Quaternion.toEulerAngles(myQuaternion)
```

Quaternion objects are often required in the fields of components. For example, the `Transform` component contains `Quaternion` values for rotation of the entity.

To create a [custom component](/docs/creator/sdk7/architecture/custom-components/) with parameters that require Quaternion values, set the type of these parameters as `Schemas.Quaternion`.


> [!WARNING]
> **📔 Note**: `Quaternion` must be imported via
> 
> > `import { Quaternion } from "@dcl/sdk/math"`
> 
> See [Imports](/docs/creator/sdk7/getting-started/coding-scenes/#imports) for how to handle these easily.


## Scalars

A scalar is nothing more than a number. For that reason, it doesn't make much sense to instantiate a `Scalar` object to store data, as you can do the same with a number. The functions in the `Scalar` namespace however expose several handy functions (similar to those in the _Vector3_ namespace), that can be used on numbers.

```ts
// Call functions from the Scalar class
let random = Scalar.randomRange(1, 100)

let midPoint = Scalar.lerp(number1, number2, 0.5)

let clampedValue = Scalar.clamp(myInput, 0, 100)
```


> [!WARNING]
> **📔 Note**: `Scalar` must be imported via
> 
> > `import { Scalar } from "@dcl/sdk/math"`
> 
> See [Imports](/docs/creator/sdk7/getting-started/coding-scenes/#imports) for how to handle these easily.

