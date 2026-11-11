> Aprende cómo usar luces en tu escena

# Luces

Las luces son una parte fundamental de los gráficos 3D. Se usan para iluminar la escena y crear una sensación de profundidad y realismo, y pueden usarse para crear distintos estados de ánimo y atmósferas.

De forma predeterminada, la escena se ilumina con una sola luz direccional. Esta es una luz que brilla en una dirección específica y se usa para simular el sol o la luna. Consulta [Control de Skybox](/docs/creator/content-creator-es/scenes-sdk7/interactividad/skybox-control/) para obtener más información.

Puedes añadir hasta 1 luz por parcela en tu escena.

Hay dos tipos de luces compatibles:

* Luz puntual: una luz que brilla en todas las direcciones desde un punto específico.
* Luz focal: una luz que brilla en una dirección específica y cubre solo un área con forma de cono.


> [!WARNING]
> **📔 Nota**: Las luces incrustadas dentro de un `.glb` o `.gltf` archivo son ignoradas por Decentraland. Si tu modelo 3D fue exportado con luces incluidas, no iluminarán nada en la escena. Las únicas fuentes de luz válidas son las entidades que tienen el componente `LightSource` Si quieres que el modelo de una lámpara brille, añade un `LightSource` a la misma entidad, o a una entidad hija posicionada donde está la bombilla.



> [!NOTE]
> **💡 Consejo**: El catálogo de assets de Creator Hub tiene **Spotlight** y **Point Light** smart items, en la **lights** categoría. Cada uno incluye un modelo, un `LightSource`configurado, y acciones Turn On, Turn Off y Toggle, así que no necesitas escribir código. También hay un **Spotlight** en **decorations**, que es solo un modelo y no emite luz. Elige de la **lights** categoría cuando quieras iluminación real.


## Añadir una luz

Para añadir una luz a tu escena, necesitas crear una entidad de luz y añadirle el componente `LightSource` LightSource.

```ts
import { engine, LightSource } from '@dcl/sdk/ecs'

const light = engine.addEntity()

Transform.create(light, {
  position: Vector3.create(10, 3, 10),
})

LightSource.create(light, {
  type: LightSource.Type.Point({})
})
```


> [!WARNING]
> **📔 Nota**: Una luz con el brillo predeterminado será apenas visible con el sol del mediodía, igual que en el mundo real. Puedes usar el [Control de Skybox](/docs/creator/content-creator-es/scenes-sdk7/interactividad/skybox-control/) para forzar que el skybox esté en horario nocturno, o aumentar el brillo estableciendo la propiedad `intensity` de la `LightSource` componente a un valor más alto.


## Luces focales

Las luces focales son luces que brillan en una dirección específica y cubren un área concreta con forma de cono. La dirección de la luz se define por el componente Transform de la entidad: la luz brilla a lo largo de la dirección frontal de la entidad (su eje local +Z). La apertura del cono se define por las propiedades `innerAngle` y `outerAngle` del `LightSource` componente.

El ejemplo siguiente rota la entidad 90 grados alrededor de su eje X, así la luz apunta directamente hacia abajo. Una rotación de -90 grados la apunta hacia el cielo.

```ts
import { engine, LightSource } from '@dcl/sdk/ecs'

const light = engine.addEntity()

Transform.create(light, {
  position: Vector3.create(10, 3, 10),
  rotation: Quaternion.fromEulerDegrees(90, 0, 0),
})

LightSource.create(light, {
	type: LightSource.Type.Spot({
      innerAngle: 30,
      outerAngle: 60
    }),
	shadow: true
})
```

La `innerAngle` es el ángulo del cono interno, donde la luz tiene el brillo máximo, y la `outerAngle` es el ángulo del cono externo, donde la luz se atenúa gradualmente hacia los bordes del cono. Puedes jugar con estos valores para crear distintos efectos, con luces más enfocadas o más difusas.

## Intensidad y color

Todas las luces, tanto puntuales como focales, tienen un color y una intensidad. El color lo define el componente `color` de la `LightSource` y la intensidad la define la propiedad `intensity` .

```ts
import { engine, LightSource } from '@dcl/sdk/ecs'

const light = engine.addEntity()

Transform.create(light, {
  position: Vector3.create(10, 3, 10),
})

LightSource.create(light, {
  type: LightSource.Type.Point({}),
  color: Color3.Red(),
  intensity: 10000,
})
```

El color es un objeto `Color3` , si no se especifica será blanco. Puedes configurarlo con el color que quieras, lo que puede tener un gran impacto en el estado de ánimo de la escena.

La intensidad se expresa en candelas (lúmenes/m^2 a 1 m de distancia, o lúmenes divididos por 4\*pi).

La intensidad predeterminada es 16000; este es el brillo de una bombilla promedio en el mundo real y puede verse hasta a unos 10 metros de distancia de la fuente de luz. Si necesitas que la luz sea visible desde más lejos, o durante el día, puedes aumentar la intensidad.

La distancia a la que la luz es visible es la raíz cuarta del valor de intensidad (`intensity^0.25`).

* Con una intensidad de 625, la luz es visible hasta a unos 5 metros de distancia.
* Con una intensidad de 10000, la luz es visible hasta a unos 10 metros de distancia.
* Con una intensidad de 160000, la luz es visible hasta a unos 20 metros de distancia.

## Sombras

Cada luz puede proyectar sombras o no. De forma predeterminada no lo hacen, pero puedes habilitarlas estableciendo el `shadow` de la `LightSource` componente en `true`.

```ts
import { engine, LightSource } from '@dcl/sdk/ecs'

const light = engine.addEntity()

Transform.create(light, {
  position: Vector3.create(10, 3, 10),
  rotation: Quaternion.fromEulerDegrees(90, 0, 0),
})

LightSource.create(light, {
	type: LightSource.Type.Spot({
      innerAngle: 30,
      outerAngle: 60
    }),
	shadow: true
})
```


> [!WARNING]
> **📔 Nota**: Solo se admiten sombras para luces focales. Las luces puntuales no admiten sombras. Si hay varias luces en la escena, es posible que algunas no estén proyectando sombras; consulta [Optimización de luces](#light-optimization) para obtener más información.


## Encender y apagar una luz

El componente LightSource tiene una propiedad `active` que puede usarse para encender y apagar una luz. Esto es útil si quieres apagar una luz sin quitarla de la escena, o sin establecer el `intensity` en 0 y perder la referencia de cuál era la intensidad original.

```ts
import { engine, LightSource } from '@dcl/sdk/ecs'

const light = engine.addEntity()

Transform.create(light, {
  position: Vector3.create(10, 3, 10),
  rotation: Quaternion.fromEulerDegrees(90, 0, 0),
})

LightSource.create(light, {
	type: LightSource.Type.Spot({
      innerAngle: 30,
      outerAngle: 60
    }),
	shadow: true,
	active: true
})

const lightSwitch = engine.addEntity()

Transform.create(lightSwitch, {
  position: Vector3.create(8, 1, 10),
})

MeshRenderer.setBox(lightSwitch)

MeshCollider.setBox(lightSwitch)

pointerEventsSystem.onPointerDown(
	{
		entity: lightSwitch,
		opts: {
			button: InputAction.IA_POINTER,
			hoverText: 'Clic',
		},
	},
	function () {
		const lightSource = LightSource.getMutable(light)
		lightSource.active = !lightSource.active
	}
)
```

## Optimización de luces

Las fuentes de luz pueden tener un impacto bastante grande en el rendimiento de tu escena. Por esta razón, el engine optimizará automáticamente la escena deshabilitando algunas de las luces o sus sombras, empezando por las que están más lejos.

El número de luces activas permitido en una escena está limitado a una por parcela, y a partir de ahí depende de la configuración de calidad seleccionada por el usuario.

* Calidad baja: sin luces de la escena en absoluto
* Calidad media: máximo 6 luces (en una escena con suficientes parcelas)
* Calidad alta: máximo 10 luces (en una escena con suficientes parcelas)
* Móvil: la app tiene sus propios preajustes, desde ninguna luz de escena en Very Low hasta 8 luces en High. Consulta [Luces y partículas en mobile](/docs/creator/content-creator-es/desarrollar-para-movil/desarrollar/optimize-performance/#lights-and-particles).

El control deslizante de configuración de calidad puede reducir aún más el límite de un preajuste.

Si hay más luces de las permitidas, el engine las desactivará automáticamente según la proximidad de la fuente de luz al jugador. A medida que el jugador se mueve, el engine volverá a activar las luces que estén lo suficientemente cerca del jugador.


> [!WARNING]
> **📔 Nota**: En el preajuste de calidad baja, las luces de la escena se desactivan por completo. Una escena correctamente construida `LightSource` no muestra nada, y ninguna cantidad de intensidad, rango o ajuste del material cambia eso. Si una luz "no funciona", comprueba antes que nada la configuración de calidad. Trata Media como el ajuste más bajo que tu escena admite, en lugar de requerir Alta.
> 
> Un material emisivo que brilla no prueba que una luz esté funcionando. Un modelo de lámpara que brilla y la luz que proyecta sobre las superficies cercanas son dos cosas distintas que hay que comprobar.


Como algunos jugadores no ven luces de escena en absoluto (calidad baja en escritorio, Very Low en móvil), asegúrate de que tu escena siga funcionando sin ellas. Todo lo que los jugadores necesiten ver, como el camino a seguir o un objeto con el que puedan interactuar, debería destacar mediante materiales emisivos, texturas o el skybox. Usa `LightSource` como un extra, no como la única pista. Lo mismo ocurre con las sombras: no hagas de una sombra la única forma de saber dónde está un objeto.

En todos los casos, el engine solo renderizará sombras para hasta 3 fuentes de luz. Si hay más luces con sombras que 3, el engine desactivará automáticamente las sombras de las luces restantes que estén más lejos.

Además del número máximo de luces permitidas, las sombras también dependen de la distancia al jugador. Las distancias exactas varían según el tipo de luz y la configuración de calidad del jugador, pero como regla general:

* A menos de 10 metros de distancia: las sombras se renderizan como sombras suaves (alta calidad)
* Entre 10 y 20 metros de distancia: las sombras se renderizan como sombras duras (baja calidad)
* A más de 20 metros de distancia: las sombras no se renderizan

Las propias fuentes de luz siguen iluminando la escena a distancias mucho mayores: solo se desactivan cuando el jugador está a más de 160 metros de distancia (10 parcelas). Esto hace que las luces sean adecuadas para configuraciones a gran escala, como la iluminación de escenario en eventos en vivo, donde la mayor parte del público está lejos de las fuentes de luz.

También es importante tener en cuenta que las luces solo se renderizan si el jugador está dentro de la escena. Si el jugador está fuera de la escena, las luces no se renderizarán.

## Rango de la luz

El componente lightSource tiene una propiedad `range` que puede usarse para establecer la distancia máxima a la que la luz es visible. De forma predeterminada, el valor de la propiedad `range` es -1, lo que significa que el rango de la luz depende de su intensidad.

El rango se calcula como la raíz cuarta del valor de intensidad (`intensity^0.25`).

* Con una intensidad de 16000, el rango es de unos 11 metros.
* Con una intensidad de 160000, el rango es de unos 20 metros.
* Con una intensidad de 1600000, el rango es de unos 36 metros.

La configuración predeterminada garantiza que la curva de caída sea suave y tenga un aspecto natural. Pero, en caso de que quieras limitar el rango de la luz, puedes establecer la `range` propiedad a un número positivo.

```ts
import { engine, LightSource } from '@dcl/sdk/ecs'

const light = engine.addEntity()

Transform.create(light, {
  position: Vector3.create(10, 3, 10),
})

LightSource.create(light, {
  type: LightSource.Type.Point({}),
  intensity: 16000,
  range: 20,
})
```


> [!WARNING]
> **📔 Nota**: Establecer la `range` propiedad a un número positivo cortará bruscamente la luz a la distancia proporcionada. Esto es útil si quieres crear una luz que solo sea visible en un área específica, o para optimizar el rendimiento de tu escena. Establecer la `range` en un valor mayor que el que la luz puede alcanzar realmente con su intensidad actual no tendrá ningún efecto.


## Máscaras de luz

Puedes usar una máscara de luz para producir algunos efectos interesantes. En lugar de iluminar toda el área, puedes aplicar una textura para usarla como filtro que solo iluminará partes del área.

Las máscaras se usan más comúnmente con luces focales, pero también pueden usarse con luces puntuales. A continuación hay un ejemplo de una luz focal con una máscara aplicada.

```ts
import { engine, LightSource } from '@dcl/sdk/ecs'

const light = engine.addEntity()

Transform.create(light, {
  position: Vector3.create(10, 3, 10),
})

LightSource.create(light, {
	type: LightSource.Type.Spot({
      innerAngle: 30,
      outerAngle: 60
    }),
	shadow: true,
	shadowMaskTexture: Material.Texture.Common({src: "assets/scene/images/lightmask1.png"})
})
```

Por ejemplo, puedes aplicar la imagen de abajo para producir un efecto divertido en el que la luz solo es visible en las áreas donde la textura es blanca. Puedes usar esto con luces de cualquier color e intensidad.

![](/docs/_remote/b2cd4d8e15d52341e551dffb.webp)

Con una imagen en blanco y negro, la luz solo incide en un área o no lo hace. También es posible pasar una imagen con colores, y estos colores teñirán tu luz en cada área; esto puede usarse para proyectar un logotipo o imagen de color sobre una superficie.


> [!WARNING]
> **📔 Nota**: La imagen usada como máscara debe tener una altura y un ancho en píxeles que sean una potencia de 2 (por ejemplo: 1024, 512, 256). Esta función no funciona para imágenes que tienen dimensiones diferentes.


Al aplicar una máscara a una luz puntual, la textura se envolverá como un cubo alrededor de la fuente de luz. Si quieres evitar bordes visibles entre los lados del cubo, asegúrate de que la textura tenga continuidad en los bordes.

```ts
import { engine, LightSource } from '@dcl/sdk/ecs'

const light = engine.addEntity()

Transform.create(light, {
  position: Vector3.create(10, 3, 10),
})

LightSource.create(light, {
	type: LightSource.Type.Point({}),
	shadowMaskTexture: Material.Texture.Common({src: "assets/scene/images/point-light-mask1.png"})
})
```

Por ejemplo, la imagen de abajo muestra cada una de las letras en distintos lados del cubo (Y arriba, -Y abajo, X a la derecha, -X a la izquierda, Z al frente, -Z atrás).

![](/docs/_remote/384393f9ee871b7e47ed3a80.webp)


> [!NOTE]
> **💡 Consejo**: Combina las luces con un [Particle System](/docs/creator/content-creator-es/scenes-sdk7/fundamentos-del-contenido-3d/particle-system/) para lograr efectos visuales más ricos. Por ejemplo, combina un sistema de partículas de fuego con mezcla aditiva con una luz puntual cálida para crear una hoguera convincente.



> [!NOTE]
> **💡 Consejo**: Para ver un ejemplo funcional de este componente, consulta la escena de prueba [`0,4-dynamic-lights`](https://github.com/decentraland/sdk7-test-scenes/tree/main/scenes/0,4-dynamic-lights) , que cubre tanto `LightSource.Type.Point` y `LightSource.Type.Spot`, cambios en tiempo de ejecución mediante `LightSource.getMutable()`, sombras y `shadowMaskTexture`.


## Luces en la oscuridad

Las luces se aprecian mejor cuando el cielo no compite con ellas. Los componentes `Skybox`, `Sun` y `EnvironmentFog` pueden oscurecer la luz del sol, los colores del cielo (y con ellos la luz ambiental), la niebla, las nubes y las estrellas, de modo que tus luces se conviertan en la única fuente de iluminación. Consulta [Oscuridad completa](/docs/creator/content-creator-es/scenes-sdk7/interactividad/skybox-control/#complete-darkness).

En el preajuste de calidad baja, las luces de la escena no se renderizan en absoluto, así que una escena completamente oscura aparece totalmente negra para esos jugadores. Usa materiales emisivos para mantener visible cualquier cosa esencial. La app móvil y el explorer Bevy aún no admiten estos componentes, así que los jugadores en esos clientes seguirán viendo el cielo normal.


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-es/scenes-sdk7/fundamentos-del-contenido-3d/lights.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
