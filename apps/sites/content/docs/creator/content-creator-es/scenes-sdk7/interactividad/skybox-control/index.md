> Cambia la hora del skybox, reemplaza el cielo con tu propia textura o recolorea el cielo, las nubes y las estrellas y controla el sol y la niebla

# Control del skybox

Puedes cambiar cómo ve un jugador el skybox cuando está en tu escena; esto también afecta al tono y la dirección de la iluminación global.

El cielo en Decentraland sigue un ciclo día/noche predeterminado que tarda 2 horas en completarse, así que hay 12 ciclos completos por cada día real. Si la escena no impone una hora fija del día, los jugadores también pueden cambiar a una hora concreta del día ajustando un control deslizante en su UI.

Cada vez que los jugadores entran en una escena con una hora del día distinta, o la escena cambia dinámicamente la hora del día, el skybox transiciona suavemente durante unos segundos hasta este nuevo valor.

## Hora fija del día

Puedes establecer una hora fija del día para tu escena. Todos los jugadores verán la escena con esta hora del día, y el skybox no seguirá el ciclo día/noche.

En Creator Hub, abre la configuración de la escena y haz clic en la **Settings** pestaña para encontrar la **Skybox** sección. Desmarca la **Auto** opción y establece la hora del día que quieras.

![](/docs/_remote/6242ab0073574f8cc2c9e7d9.webp)

También puedes establecer la hora del día del skybox en el código de tu escena. Para ello, añade la siguiente sección a tu `scene.json` en el nivel raíz:

```json
 "skyboxConfig": {
    "fixedTime": 36000
  }
```

El número se refiere a la cantidad de segundos desde el inicio del día, y va de 0 (que se refiere a *00:00*) hasta 86400 (que se refiere a *24:00*). Cualquier número mayor que 86400 también se interpreta como medianoche.

Aquí tienes algunos ejemplos más de valores válidos:

* 0 segundos => *00:00*
* 21600 segundos => *06:00*
* 43200 segundos => *12:00*
* 64800 segundos => *18:00*
* 86400 segundos => *24:00*

## Lectura de la hora del día

Puedes leer la hora del día desde el código de tu escena usando la `getWorldTime()` función.

```ts
import { getWorldTime } from '~system/Runtime'

executeTask(async () => {
  let time = await getWorldTime({})
  console.log(time.seconds)
})
```

La función devuelve un número entre 0 y 86400, donde 0 es medianoche y 86400 es 24:00. Este valor se actualiza si la escena cambia dinámicamente la hora del día o si el jugador cambia la hora del día en la UI. De lo contrario, devuelve el valor relativo al ciclo día/noche predeterminado.

## Cambio dinámico de la hora del día

Puedes cambiar la hora del día dinámicamente usando el `SkyboxTime` componente. Este componente solo puede añadirse a la entidad raíz de la escena `engine.RootEntity`.

```ts
import { SkyboxTime } from '@dcl/sdk/ecs'

function main() {
  SkyboxTime.create(engine.RootEntity, { fixedTime: 36000 })
}
```

El `fixedTime` la propiedad es un número entre 0 y 86400, donde 0 es medianoche y 86400 es 24:00. Cualquier número mayor que 86400 también se interpreta como medianoche.

Cada vez que este componente se añade, se elimina o la `fixedTime` propiedad cambia, la hora del día del skybox transiciona suavemente durante unos segundos hasta este nuevo valor. Lo mismo ocurre cuando el jugador sale o entra en la escena. Mientras la hora del día del skybox esté fija, el skybox dejará de seguir el progreso de su ciclo día/noche y los jugadores no podrán cambiar la hora del día mediante la UI.

De forma predeterminada, la transición siempre ocurre en dirección hacia adelante, pero puedes cambiar esto estableciendo la `transitionMode` propiedad en `TransitionMode.TM_FORWARD` o `TransitionMode.TM_BACKWARD`.

```ts
import { SkyboxTime, TransitionMode } from '@dcl/sdk/ecs'

function main() {
  SkyboxTime.create(engine.RootEntity, { fixedTime: 36000, transitionMode: TransitionMode.TM_BACKWARD })
}
```

### Elige la transición más corta

Nada cambia de golpe: cada cambio recorre todos los tiempos intermedios. `TM_FORWARD` siempre avanza hacia adelante, dando la vuelta al pasar medianoche, así que ir de 10:00 a 08:40 recorre casi un día completo de cielo en lugar de retroceder 80 minutos.

Calcula cuál es el camino más corto y pasa el modo correspondiente:

```ts
import { engine, SkyboxTime, TransitionMode } from '@dcl/sdk/ecs'

const DAY = 86400

// Mover a una nueva hora del día por el camino más corto
function setTimeOfDay(fixedTime: number) {
  const current = SkyboxTime.getOrNull(engine.RootEntity)?.fixedTime
  // sin SkyboxTime configurado todavía, la escena no conoce la hora actual, así que simplemente avanza
  let transitionMode = TransitionMode.TM_FORWARD
  if (current !== undefined) {
    const forward = (fixedTime - current + DAY) % DAY
    // avanzar más de medio día significa que hacia atrás es el camino más corto
    if (forward > DAY / 2) transitionMode = TransitionMode.TM_BACKWARD
  }
  SkyboxTime.createOrReplace(engine.RootEntity, { fixedTime, transitionMode })
}
```

Para un salto grande que no quieres que el jugador vea, escóndelo detrás de una UI a pantalla completa: haz un fade in, cambia la hora, espera y luego haz un fade out.

## Cielo, sol y niebla personalizados

Tres componentes permiten que tu escena tome el control del entorno mientras el jugador está dentro de ella:

* `Skybox`: reemplaza el cielo con tu propia textura, o recolorea el cielo procedural, sus nubes y sus estrellas.
* `Sun`: recolorea, atenúa, oculta o fija la dirección de la luz del sol (y la luna).
* `EnvironmentFog`: recolorea la niebla de distancia y cambia su densidad.

Al igual que `SkyboxTime`, los tres solo pueden añadirse a la entidad raíz de la escena, `engine.RootEntity`. En cualquier otra entidad no tienen efecto. Todos los campos son opcionales: cualquier cosa que omitas conserva su comportamiento predeterminado según la hora del día.


> [!WARNING]
> **📔 Nota**: Los `Skybox`, `Sun` y `EnvironmentFog` Estos componentes aún no son compatibles con la app móvil ni con el Bevy explorer. Los jugadores en esos clientes ven el cielo, sol y niebla normales, así que asegúrate de que tu escena siga funcionando sin estos overrides.


```ts
import { engine, Skybox, Sun, EnvironmentFog } from '@dcl/sdk/ecs'
import { Color4, Vector3 } from '@dcl/sdk/math'

function main() {
  Skybox.create(engine.RootEntity, {
    skyboxTexture: 'images/sky.png'
  })
  Sun.create(engine.RootEntity, { intensity: 0.5, direction: Vector3.create(-1, -1, 0) })
  EnvironmentFog.create(engine.RootEntity, {
    color: { keys: [{ time: 0, color: Color4.create(0.6, 0.2, 0.1, 1) }] },
    density: 0.01
  })
}
```

### Ámbito y restablecimiento

El `Skybox`, `Sun` y `EnvironmentFog` Los overrides solo se aplican mientras el jugador está dentro de tu escena. Cuando el jugador sale de la escena, cuando se elimina un componente o cuando un campo se desconfigura, el cielo y la iluminación vuelven a sus valores predeterminados, y al volver a entrar en la escena se aplican de nuevo. Los cambios se aplican al instante, sin transición.


> [!WARNING]
> **📔 Nota**: Mientras están activos, estos overrides son globales: las parcelas vecinas vistas desde dentro de tu escena también se renderizan con tu cielo, sol y niebla.


### Textura de cielo personalizada

`skyboxTexture` reemplaza el cielo visible con una imagen de los assets de la escena. Asígnale la ruta del archivo de imagen, del mismo modo que el `src` de una textura. La imagen debe ser un panorama equirectangular (una imagen de latitud-longitud 2:1): su centro horizontal mira hacia el eje Z positivo (norte) de la escena, su fila superior es el zenith y su fila inferior es el nadir. Los formatos cube cross, 3x2 y cubemap de seis caras no son compatibles.

```ts
import { engine, Skybox } from '@dcl/sdk/ecs'

function main() {
  Skybox.create(engine.RootEntity, {
    skyboxTexture: 'images/sky.png'
  })
}
```

La imagen se envuelve horizontalmente y se filtra de forma bilineal. Si la imagen no se carga, el cielo predeterminado permanece.

Mantén un tamaño razonable para la imagen: el panorama completo se descarga y se mantiene en memoria mientras tu escena se carga. 2048x1024 píxeles bastan para la mayoría de los cielos; no superes 4096x2048. Usa un formato comprimido como `.jpg` para cielos fotográficos.

La textura solo cambia el aspecto del cielo: la luz del sol sigue la hora del día, la luz ambiental sigue `skyColors` (para que puedas adaptarla a tu imagen), y los `Sun` y `EnvironmentFog` componentes siguen aplicándose encima de ella. En el preajuste de alta calidad, los reflejos en materiales brillantes también siguen tu textura de cielo.

### Colores del cielo, nubes y estrellas

Si mantienes el cielo procedural, el `Skybox` componente te permite recolorearlo y ajustar su capa de nubes y su campo de estrellas.

```ts
import { engine, Skybox, ColorGradient } from '@dcl/sdk/ecs'
import { Color4 } from '@dcl/sdk/math'

// Un degradado con una sola key es un color constante
function constant(color: Color4): ColorGradient {
  return { keys: [{ time: 0, color }] }
}

function main() {
  Skybox.create(engine.RootEntity, {
    skyColors: {
      zenith: constant(Color4.create(0.55, 0.25, 0.12, 1)),
      horizon: constant(Color4.create(0.95, 0.55, 0.3, 1)),
      nadir: constant(Color4.create(0.35, 0.15, 0.08, 1))
    },
    clouds: { opacity: 0.3, speed: 0.01, color: constant(Color4.create(0.9, 0.6, 0.4, 1)) },
    stars: { brightness: 4.62 }
  })
}
```

* `skyColors`: el color del cielo en su `zenith` (justo arriba), en el `horizon`, y en su `nadir` (el cielo por debajo del horizonte, visible donde ningún terreno lo cubre, por ejemplo a gran altitud o en un World con el [landscape terrain](/docs/creator/content-creator-es/scenes-sdk7/tipos-de-proyectos/scene-metadata/#landscape-terrain) desactivado). Estos colores también controlan la luz ambiental de la escena: el color del zenith ilumina los objetos desde arriba, el color del horizon desde los lados y el color del nadir desde abajo, de modo que los objetos y avatares encajen con el cielo que los rodea. `skyColors.rim` es un resplandor estrecho dibujado justo sobre la línea del horizonte, como un resplandor de atardecer; cuando lo dejas sin configurar sigue tu `horizon` color si configuras uno (de lo contrario conserva el predeterminado), así que solo lo necesitas como acento (por ejemplo, un borde anaranjado de amanecer en un cielo oscuro).
* `clouds.opacity`: de 0 (sin nubes) a 1, el valor predeterminado. `clouds.speed`: qué tan rápido gira la capa de nubes alrededor del cielo, en radianes por segundo: 0.01 por defecto (una vuelta cada \~10 minutos), 0 para nubes estáticas; los valores negativos cuentan como 0. `clouds.color`: el tinte de la capa de nubes; sin él, las nubes conservan sus colores predeterminados según la hora del día incluso en un cielo recoloreado.
* `stars.brightness`: un multiplicador de la textura de estrellas, 4.62 por defecto (los valores mayores que 1 son HDR, 0 oculta las estrellas, los valores negativos cuentan como 0). Las estrellas solo son visibles durante la parte nocturna del día.

Mientras `skyboxTexture` está configurada, la textura reemplaza al cielo procedural: `nubes` y `estrellas` no tienen efecto, y `skyColors` ya no colorean el cielo, pero siguen controlando la luz ambiental.

### Sun

El `Sun` El componente cambia la luz direccional del sol y la luna.

```ts
import { engine, Sun } from '@dcl/sdk/ecs'
import { Color4, Vector3 } from '@dcl/sdk/math'

function main() {
  Sun.create(engine.RootEntity, {
    color: { keys: [{ time: 0, color: Color4.create(1, 0.65, 0.4, 1) }] },
    intensity: 0.5,
    direction: Vector3.create(-1, -1, 0)
  })
}
```

* `color`: el color de la luz direccional. También tiñe el disco del sol.
* `visibles`: `true` de forma predeterminada. Configúralo en `false` para ocultar los discos del sol y la luna, y el destello de lente del sol, por ejemplo cuando se verían a través de un `skyboxTexture`. La luz que emite el sol no se ve afectada.
* `intensity`: un multiplicador de la intensidad de la luz según la hora del día, 1 por defecto. 0 apaga la luz del sol; los valores negativos cuentan como 0.
* `dirección`: la dirección en la que viaja la luz solar, en coordenadas de la escena: `(0, -1, 0)` coloca el sol justo encima, y `(-1, -1, 0)` lo coloca en el lado positivo de X (este) del cielo, 45 grados por encima del horizonte. Mientras esté configurado, el sol permanece fijo en cada hora del día, pero su color e intensidad siguen dependiendo de la hora del día. Déjalo sin configurar, o configúralo en `(0, 0, 0)`, para mantener el recorrido normal del sol por el cielo.

### Niebla

El `EnvironmentFog` El componente cambia la niebla de distancia.

```ts
import { engine, EnvironmentFog } from '@dcl/sdk/ecs'
import { Color4 } from '@dcl/sdk/math'

function main() {
  EnvironmentFog.create(engine.RootEntity, {
    color: { keys: [{ time: 0, color: Color4.create(0.85, 0.5, 0.3, 1) }] },
    density: 0.01
  })
}
```

* `color`: el color de la niebla.
* `density`: qué tan rápido las cosas se desvanecen en la niebla, como una densidad exponencial por metro: la niebla a una distancia dada es `1 - e^(-density * distance)`, así que `1 / density` es aproximadamente la distancia a la que dos tercios de la vista quedan cubiertos por la niebla. El valor predeterminado es 0.0005 (unos 2 km); 0.02 cubre aproximadamente dos tercios de la vista a 50 m y casi toda la vista a 150 m, y 0 hace que la niebla sea invisible. Los valores negativos cuentan como 0.

Que la niebla se renderice o no es un ajuste de calidad elegido por el jugador; una escena no puede activarlo ni desactivarlo.

### Degradados de color a lo largo del día

Cada color de estos componentes es un `ColorGradient`: una lista de `keys`, cada una con un `time` y una `Color4`. El `time` es la hora del día normalizada, de 0 (*00:00*) a 1 (*24:00*), así que 0.5 es mediodía. Es la `SkyboxTime` hora del reloj dividida por 86400: `fixedTime: 21600` (6 a. m.) es `time: 0.25`. El color se interpola entre las keys vecinas; antes de la primera key y después de la última, se usa el color de esa key. Los tiempos fuera de 0 a 1 se ajustan a ese rango, y si dos keys comparten el mismo `time`, la que aparece más tarde en la lista prevalece. Un degradado con una sola key es simplemente un color constante.

```ts
import { engine, Skybox } from '@dcl/sdk/ecs'
import { Color4 } from '@dcl/sdk/math'

function main() {
  Skybox.createOrReplace(engine.RootEntity, {
    skyColors: {
      horizon: {
        keys: [
          { time: 0, color: Color4.create(0.05, 0.05, 0.2, 1) }, // medianoche
          { time: 0.25, color: Color4.create(0.95, 0.5, 0.4, 1) }, // amanecer
          { time: 0.5, color: Color4.create(0.7, 0.9, 1, 1) }, // mediodía
          { time: 0.75, color: Color4.create(0.95, 0.4, 0.2, 1) }, // anochecer
          { time: 1, color: Color4.create(0.05, 0.05, 0.2, 1) } // medianoche de nuevo, así que no hay salto
        ]
      }
    }
  })
}
```

Los degradados no se envuelven alrededor de medianoche, así que repite el mismo color en `time: 0` y `time: 1` si el día debe repetirse sin interrupciones. Los valores de color no están limitados a 1: los valores por encima de 1 producen un sol o cielo más brillante, HDR. Se ignora el canal alfa. Usa hasta 8 keys por degradado: los degradados más largos se re-muestrean a 8 keys espaciadas uniformemente, lo que suaviza cualquier detalle entre ellas. Combina degradados con `SkyboxTime` para fijar el día en un punto específico de tu degradado.

### Oscuridad total

Para que las luces colocadas en tu escena sean la única fuente de iluminación, oscurece por completo todo lo que aporta el entorno: la luz del sol y su disco, los colores del cielo (y con ellos la luz ambiental), la niebla, las nubes y las estrellas.

```ts
import { engine, Skybox, Sun, EnvironmentFog } from '@dcl/sdk/ecs'
import { Color4 } from '@dcl/sdk/math'

function main() {
  const black = { keys: [{ time: 0, color: Color4.Black() }] }

  Skybox.createOrReplace(engine.RootEntity, {
    skyColors: { zenith: black, horizon: black, nadir: black },
    clouds: { opacity: 0 },
    stars: { brightness: 0 }
  })
  Sun.createOrReplace(engine.RootEntity, { intensity: 0, visible: false })
  EnvironmentFog.createOrReplace(engine.RootEntity, { color: black })
}
```


> [!WARNING]
> **📔 Nota**: En el preajuste de baja calidad, las luces de la escena se desactivan, así que una escena completamente oscura se renderiza totalmente negra para esos jugadores. Usa materiales emisivos para mantener visible cualquier cosa esencial.


Consulta [Luces](/docs/creator/content-creator-es/scenes-sdk7/fundamentos-del-contenido-3d/lights/) para añadir luces puntuales y focos a tu escena.


> [!NOTE]
> **💡 Consejo**: Para ver ejemplos funcionales de control del skybox, consulta las [`2,1-skybox-sdk-scene-a`](https://github.com/decentraland/sdk7-test-scenes/tree/main/scenes/2,1-skybox-sdk-scene-a) y [`3,1-skybox-sdk-scene-b`](https://github.com/decentraland/sdk7-test-scenes/tree/main/scenes/3,1-skybox-sdk-scene-b) escenas de prueba, que controlan `SkyboxTime` y `TransitionMode` en vivo desde un panel de UI, y [`2,0-skybox-scene-json`](https://github.com/decentraland/sdk7-test-scenes/tree/main/scenes/2,0-skybox-scene-json), que lee el `fixedTime` configurado en `scene.json` de vuelta en tiempo de ejecución mediante `getSceneInformation()`. El [`2,2-skybox-control`](https://github.com/decentraland/sdk7-test-scenes/tree/main/scenes/2,2-skybox-control) la escena ejercita los `Skybox`, `Sun` y `EnvironmentFog` componentes: texturas de cielo, preajustes de entorno, ocultar el sol y un modo de oscuridad total.



---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-es/scenes-sdk7/interactividad/skybox-control.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
