# Sistema de partículas

Añade efectos de partículas como fuego, lluvia, chispas y auras mágicas a tu escena

Los sistemas de partículas te permiten crear efectos visuales dinámicos al emitir y animar grandes cantidades de pequeños sprites. Úsalos para crear fuego, humo, lluvia, nieve, chispas, auras mágicas, explosiones y muchos otros efectos que sería poco práctico lograr con Meshes estáticas.

## Añadir un sistema de partículas

Para añadir un sistema de partículas a tu scene, crea una entity y adjunta el `ParticleSystem` componente.

```ts
import { engine, Transform, ParticleSystem } from '@dcl/sdk/ecs'
import { Vector3 } from '@dcl/sdk/math'

const emitter = engine.addEntity()

Transform.create(emitter, {
	position: Vector3.create(8, 1, 8),
})

ParticleSystem.create(emitter, {})
```

Sin propiedades establecidas, el component usa todos los valores predeterminados: un emitter de punto que emite 10 partículas por segundo, con una vida útil de partículas de 5 segundos.

## Formas del emitter

La forma del emitter determina dónde aparecen las partículas. Hay cuatro formas disponibles mediante los `ParticleSystem.Shape` helpers.

### Point

Las partículas aparecen desde un solo punto (la posición de la entity). Este es el valor predeterminado cuando no se especifica ninguna forma.

```ts
ParticleSystem.create(emitter, {
	shape: ParticleSystem.Shape.Point(),
})
```

### Sphere

Las partículas aparecen desde la superficie o el interior de una esfera.

```ts
ParticleSystem.create(emitter, {
	shape: ParticleSystem.Shape.Sphere({ radius: 2 }),
})
```

### Cono

Las partículas aparecen desde la base de un cono y se desplazan hacia afuera en la dirección del cono. `angle` es el semiángulo en grados; `radius` es el radio de la base en metros. Para orientar la dirección del cono, establece la rotación de la entity's `Transform`.

```ts
ParticleSystem.create(emitter, {
	shape: ParticleSystem.Shape.Cone({ angle: 25, radius: 1 }),
})
```

### Box

Las partículas aparecen desde cualquier punto dentro de un volumen de caja.

```ts
ParticleSystem.create(emitter, {
	shape: ParticleSystem.Shape.Box({ size: Vector3.create(3, 1, 3) }),
})
```

## Propiedades de emisión

Controla cuántas partículas se crean y cuánto tiempo viven.

| Propiedad      | Predeterminado | Descripción                                                   |
| -------------- | -------------- | ------------------------------------------------------------- |
| `rate`         | `10`           | Partículas emitidas por segundo (continuo).                   |
| `maxParticles` | `1000`         | Límite máximo absoluto de partículas activas simultáneamente. |
| `lifetime`     | `5`            | La vida útil de cada partícula en segundos.                   |
| `active`       | `true`         | Si el system está emitiendo activamente nuevas partículas.    |

```ts
ParticleSystem.create(emitter, {
	rate: 50,
	maxParticles: 500,
	lifetime: 3,
})
```

## Movimiento

### Gravedad

El `gravity` La propiedad gravity es un multiplicador aplicado a la gravedad de la scene (-9.81 m/s²). Establécelo en `0` para hacer que las partículas floten en su lugar, un valor negativo para empujarlas hacia arriba, o un valor positivo para acelerarlas hacia abajo.

```ts
ParticleSystem.create(emitter, {
	gravity: -0.5, // las partículas flotan lentamente hacia arriba
})
```

### Velocidad inicial

`initialVelocitySpeed` es un `FloatRange` que establece un mínimo y un máximo para el rango aleatorio de velocidad con el que se lanza cada partícula (en m/s). Ambos valores tienen como valor predeterminado `1`.

```ts
ParticleSystem.create(emitter, {
	initialVelocitySpeed: { start: 2, end: 8 },
})
```

### Fuerza adicional

Un vector de fuerza constante aplicado a todas las partículas en cada frame, además de la gravedad. Útil para efectos de viento o magnéticos.

```ts
ParticleSystem.create(emitter, {
	additionalForce: Vector3.create(0.5, 0, 0), // empuje lateral constante
})
```

### Limitación de velocidad

Usa `limitVelocity` para limitar la velocidad máxima de las partículas y, opcionalmente, amortiguar su velocidad excesiva en cada frame.

```ts
ParticleSystem.create(emitter, {
	limitVelocity: {
		speed: 5, // velocidad máxima permitida en m/s
		dampen: 0.8, // fracción de la velocidad excesiva eliminada por frame
	},
})
```

## Propiedades visuales

### Color

`initialColor` establece el color de la partícula al nacer, elegido aleatoriamente dentro del rango dado. `colorOverTime` hace que el color de la partícula transicione gradualmente desde el `inicio` hasta la `fin` valor a lo largo de la vida útil de la partícula. Ambos aceptan un `ColorRange` con un `inicio` y `fin` valor.

```ts
import { Color4 } from '@dcl/sdk/math'

ParticleSystem.create(emitter, {
	initialColor: {
		start: Color4.create(1, 0.5, 0, 1), // naranja
		end: Color4.create(1, 1, 0, 1), // amarillo
	},
	colorOverTime: {
		start: Color4.create(0.5, 0, 0, 0.5), // rojo tenue, medio transparente
		end: Color4.create(0, 0, 0, 0), // totalmente transparente (desvanecer)
	},
})
```

Ten en cuenta que el cuarto valor en un `Color4` es el *alpha*. Si estableces el color final en uno con un alpha de 0, las partículas se desvanecerán gradualmente y se volverán invisibles, lo cual suele ser un buen efecto.

### Tamaño

`initialSize` y `sizeOverTime` son `FloatRange` más bajos. `initialSize` controla la escala de la partícula al nacer, elegida aleatoriamente dentro del rango dado. `sizeOverTime` hace que la escala de la partícula transicione gradualmente desde el `inicio` hasta la `fin` valor a lo largo de la vida útil de la partícula. Un valor de `1` es igual al tamaño original de la textura.

```ts
ParticleSystem.create(emitter, {
	initialSize: { start: 0.1, end: 0.3 },
	sizeOverTime: { start: 0.5, end: 1.0 }, // crecer a lo largo de la vida útil
})
```


> [!WARNING]
> **📔 Nota**: El `scale` de la entity's `Transform` no afecta la escala de las partículas.


### Texture

Por defecto, las partículas se renderizan como cuadrados blancos. Proporciona una textura para usar una imagen personalizada.

```ts
ParticleSystem.create(emitter, {
	texture: { src: 'assets/scene/Images/spark.png' },
})
```

### Modo de fusión

Controla cómo se compone el color de la partícula con la scene detrás de ella.

| Valor                                  | Descripción                                                                    |
| -------------------------------------- | ------------------------------------------------------------------------------ |
| `ParticleSystemBlendMode.PSB_ALPHA`    | Transparencia estándar (predeterminada).                                       |
| `ParticleSystemBlendMode.PSB_ADD`      | Fusión aditiva — las partículas iluminan la scene. Buena para fuego y brillos. |
| `ParticleSystemBlendMode.PSB_MULTIPLY` | Multiplica el color de la partícula con la scene detrás de ella.               |

```ts
import { ParticleSystemBlendMode } from '@dcl/sdk/ecs'

ParticleSystem.create(emitter, {
	texture: { src: 'assets/scene/textures/ember.png' },
	blendMode: ParticleSystemBlendMode.PSB_ADD,
})
```

### Billboard

Cuando `billboard` es `true` (el valor predeterminado), cada partícula siempre mira hacia la cámara, independientemente de la orientación del emitter. Establécelo en `false` para partículas que deberían girar en el espacio 3D.

```ts
ParticleSystem.create(emitter, {
	billboard: false,
})
```

## Rotación

### Rotación inicial y rotación a lo largo del tiempo

`initialRotation` es la orientación de una partícula cuando aparece. `rotationOverTime` es una velocidad angular por eje aplicada cada segundo. Ambos aceptan un `Quaternion`.

```ts
import { Quaternion } from '@dcl/sdk/math'

ParticleSystem.create(emitter, {
	initialRotation: Quaternion.fromEulerDegrees(0, 0, 45),
	rotationOverTime: Quaternion.fromEulerDegrees(0, 0, 90), // gira 90°/s en Z
})
```


> [!WARNING]
> **📔 Nota**: Si `billboard` se establece en `true`, entonces las partículas solo rotarán en un eje, manteniendo siempre su orientación hacia la cámara.


### Orientar hacia la dirección de viaje

Cuando `faceTravelDirection` es `true`, cada partícula rota automáticamente para apuntar en la dirección de su movimiento, como un asteroide o una hoja que cae por el aire.

```ts
ParticleSystem.create(emitter, {
	faceTravelDirection: true,
	billboard: false,
})
```


> [!WARNING]
> **📔 Nota**: Si `faceTravelDirection` es true, el valor de `billboard` se ignora.


## Animación de sprite sheet

Puedes animar las partículas tratando una textura como una cuadrícula de fotogramas de animación. Especifica el número de columnas y filas en la hoja y la velocidad de reproducción.

```ts
ParticleSystem.create(emitter, {
	texture: { src: 'assets/scene/textures/flame-sheet.png' },
	spriteSheet: {
		tilesX: 4, // 4 columnas
		tilesY: 3, // 3 filas  (12 fotogramas en total)
		framesPerSecond: 12,
	},
})
```

## Control de reproducción

El sistema de partículas puede estar en uno de tres estados controlados por `playbackState`.

| Valor                                    | Descripción                                                         |
| ---------------------------------------- | ------------------------------------------------------------------- |
| `ParticleSystemPlaybackState.PS_PLAYING` | Emitiendo activamente (predeterminado).                             |
| `ParticleSystemPlaybackState.PS_PAUSED`  | Congela todas las partículas actuales en su lugar y deja de emitir. |
| `ParticleSystemPlaybackState.PS_STOPPED` | Deja de emitir y elimina todas las partículas existentes.           |

```ts
import { ParticleSystemPlaybackState } from '@dcl/sdk/ecs'

// Pausar cuando el jugador se aleja
ParticleSystem.getMutable(emitter).playbackState =
	ParticleSystemPlaybackState.PS_PAUSED

// Reanudar
ParticleSystem.getMutable(emitter).playbackState =
	ParticleSystemPlaybackState.PS_PLAYING
```

### Loop y prewarm

Por defecto, el system se ejecuta en loop indefinidamente. Establece `loop` en `false` para un efecto de una sola vez que se detiene automáticamente después de que todas las partículas hayan desaparecido.

Establece `prewarm` en `true` (requiere `loop: true`) para simular el system como si hubiera estado ejecutándose desde el inicio, de modo que las partículas ya llenen la scene cuando el jugador lo vea por primera vez.

```ts
ParticleSystem.create(emitter, {
	loop: false, // reproducir una vez y luego detener
	prewarm: false,
})
```

## Emisión por ráfagas

Las ráfagas te permiten emitir una gran cantidad de partículas en momentos específicos en lugar de a una tasa constante. Son útiles para explosiones, fuegos artificiales u otros eventos de una sola vez.

Un solo sistema de partículas puede pasar por un ciclo con varias ráfagas, incluso con intervalos o probabilidades variables para que resulte más natural.

```ts
ParticleSystem.create(emitter, {
	rate: 0, // desactivar la emisión continua
	bursts: {
		values: [
			{
				time: 0, // tiempo en segundos después de que comience la reproducción
				count: 200, // partículas que se emitirán por ráfaga
				cycles: 1, // cuántas veces repetir (0 = infinito)
				interval: 0.2, // segundos entre ciclos repetidos
				probability: 1, // probabilidad de 0–1 de que la ráfaga se active en cada ciclo
			},
		],
	},
})
```

Para un efecto de fuegos artificiales en loop con ráfagas escalonadas:

```ts
ParticleSystem.create(emitter, {
	loop: true,
	rate: 0,
	lifetime: 2,
	bursts: {
		values: [
			{ time: 0.0, count: 80, cycles: 0, interval: 3 },
			{ time: 0.7, count: 100, cycles: 0, interval: 3 },
			{ time: 1.4, count: 60, cycles: 0, interval: 3 },
		],
	},
})
```

## Espacio de simulación

Controla si las partículas se mueven con respecto al emitter (`PSS_LOCAL`, predeterminado) o permanecen fijas en coordenadas del mundo después de aparecer (`PSS_WORLD`).

Usa `PSS_WORLD` para un emitter en movimiento que debería dejar una estela detrás de sí (p. ej., un asteroide o un cohete).

```ts
import { PBParticleSystem_SimulationSpace } from '@dcl/sdk/ecs'

ParticleSystem.create(emitter, {
	simulationSpace: PBParticleSystem_SimulationSpace.PSS_WORLD,
})
```

## Rendimiento

El engine aplica un presupuesto de partículas por scene y ajustará automáticamente a la baja las tasas de emisión en todos los particle systems activos de la scene si el recuento total de partículas superara el límite. Planifica tus scenes en consecuencia:

* Prefiere menos systems con alto impacto visual que muchos de bajo impacto.
* Usa `maxParticles` para limitar los systems individuales.
* Usa `active` o `playbackState` para desactivar systems que estén fuera de pantalla o fuera de rango.
* Cortos `lifetime` valores mantienen el recuento de partículas activas por debajo de una emisión alta `rate` por sí sola sugeriría.

El engine limita a 1000 el número máximo de partículas que se renderizan en cualquier momento. Si estás emitiendo más partículas que eso, puede que no las estés viendo todas.

Ten en cuenta también que las partículas solo pueden ser vistas por un jugador mientras esté dentro de tu scene. Los jugadores que miren la scene desde fuera no verán ninguna partícula hasta que entren.

## Particle Lab

Entrar al mundo [ParticleLab.dcl.eth](decentraland://?realm=particlelab.dcl.eth\&dclenv=org) para experimentar con diferentes sistemas de partículas. Cuando te acerques a un sistema de partículas, la UI mostrará todos los campos disponibles; puedes ajustarlos en vivo sin necesidad de recargar la scene. Cuando estés satisfecho, haz clic en el **Copiar** botón para copiar el código de ese sistema de partículas en tu portapapeles.


> [!NOTE]
> **💡 Consejo**: Para un ejemplo funcional que utilice cada `ParticleSystem` campo, consulta la [`0,7-particle-system`](https://github.com/decentraland/sdk7-test-scenes/tree/main/scenes/0,7-particle-system) escena de prueba. Cubre las cuatro formas del emitter, los `PSB_ADD`/`PSB_ALPHA`/`PSB_MULTIPLY` modos de fusión, los `PS_PLAYING`/`PS_PAUSED`/`PS_STOPPED` estados de reproducción, sprite sheets, emisiones continuas `rate` frente a ráfagas de una sola vez y en loop `ráfagas`, y `PSS_LOCAL` frente a `PSS_WORLD` en un emitter en movimiento.



---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-es/scenes-sdk7/fundamentos-del-contenido-3d/particle-system.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
