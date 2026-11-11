> Aprende qué propiedades de mesh de sombrero son compatibles en los modelos 3D importados a Decentraland.

# Meshes

Los modelos 3D tienen una *malla* compuesta por caras triangulares *caras*. Estas caras se encuentran entre sí en *aristas* (las líneas a lo largo de las cuales se tocan) y *vértices* (los puntos donde se unen sus esquinas).


> [!NOTE]
> **💡 Consejo**: Instala el [Decentraland Tools Blender plugin](https://extensions.blender.org/add-ons/decentraland-tools/). Incluye varias funciones útiles para ayudarte a editar y exportar modelos 3D, wearables y emotes.


## Limitaciones de espacio

Todos los modelos 3D de tu escena deben caber dentro de los límites de sus parcelas. Si alguna parte de tus modelos se extiende más allá de estos límites al ejecutar un Preview, esas partes que sobresalen se recortarán y no se renderizarán.

![](/docs/_remote/9ba86deac82448c396d45bba.webp)

En el editor de Creator Hub, verás el modelo teñido de rojo y su caja delimitadora marcada en rojo.

![](/docs/_remote/ac6b1ca6eff1a48fc7dfe75d.webp)


> [!NOTE]
> **📔 Nota**: En el cliente Web anterior de Decentraland, por razones de rendimiento, si alguna parte de un modelo 3D quedaba fuera de los límites, incluso si solo era la caja delimitadora, entonces no se renderizaba el modelo completo. A partir del cliente de escritorio de Decentraland 2.0, solo se recortan las partes de un modelo que quedan fuera de los límites. Las partes del modelo que están dentro de la escena se renderizan.


## Bounding Boxes

Si tienes un modelo que tiene todos sus vértices perfectamente dentro del área de la escena, pero que tiene cajas delimitadoras grandes que están en su mayor parte vacías y se extienden más allá de los límites de la escena, el modelo completo se marcará como fuera de los límites de la escena en Creator Hub. Las partes del modelo que están dentro de tus parcelas se renderizarán correctamente y podrás publicar tu escena.

Cada malla tiene una caja delimitadora que rodea los límites de la forma. Para hacer que un modelo 3D sea más utilizable dentro de Decentraland, asegúrate de que sus cajas delimitadoras no se extiendan más allá del modelo más de lo necesario.

Para evitar este problema, puedes limpiar tus modelos 3D para restablecer las posiciones y rotaciones de las mallas, de modo que las cajas delimitadoras no se extiendan más allá de las mallas que envuelven.

![](/docs/_remote/7c3c5e5108be6fd7e142dbf6.webp)

*En Blender puedes hacerlo seleccionando los objetos, pulsando Ctrl+A y luego aplicando “All transforms”.*

Por ejemplo, ten cuidado al rotar una submalla cerca del borde de tu modelo. Como las cajas delimitadoras son cubos, aunque la malla sea redonda, las esquinas de su caja delimitadora podrían terminar sobresaliendo después de rotarla 45°.

![](/docs/_remote/15f51a636f43103faeefa105.webp)

Recomendamos hornear la rotación y la escala de cada malla en el modelo, para asegurarte de que no haya cajas delimitadoras no deseadas que se extiendan más allá del tamaño que deben tener.

## Limitaciones de triángulos

Dado que Decentraland es un mundo abierto lleno de diferentes escenas y objetos que se descargan sobre la marcha (en el mismo espacio 3D), es importante optimizar nuestras mallas para tener un buen rendimiento mientras juegas. En ese sentido, hay algunas limitaciones de la escena que siempre debemos tener en cuenta cuando se trata de mallas:

> n representa el número de parcelas que ocupa una escena.

* **Triangles:** n x 10000 Cantidad total de triángulos de todos los modelos de la escena.
* **Altura:** 330 metros, independientemente del número de parcelas.
* **Tamaño del archivo:** 15 MB por parcela - 300 MB máx. Tamaño total de los archivos subidos al servidor de contenido. Incluye modelos 3D y audio. No incluye archivos que no se suben, como paquetes de node.js.
* **Tamaño máximo de archivo: 50 MB por archivo** Ningún archivo individual de ningún tipo en la escena puede superar los 50 MB, sin importar cuántas parcelas tenga la escena.

Para más información, consulta todas las limitaciones de la escena [aquí.](/docs/creator/sdk7/optimizing/scene-limitations/)


> [!WARNING]
> **🔥Consejo de optimización🔥: Añadir conteo de polígonos** Un consejo valioso es llevar siempre un control del polycount de tus modelos. Para hacerlo en Blender, debes activar las estadísticas en el panel de superposiciones del viewport.
> 
> <img src="/docs/_remote/11ef35913355fa3c25f5a19a.webp" alt="" data-size="original">


## Mallas en escenas grandes

Al crear mallas, debemos tener en cuenta estas 2 buenas prácticas:

* **Modularidad:** Divide las mallas grandes en partes más pequeñas y modulares. Por ejemplo, en lugar de tener una sola malla para todo un edificio, el edificio podría dividirse en mallas separadas para cada pared, suelo y sección del edificio. Esto permite que el frustum culling de Unity funcione de forma más eficaz, ya que solo las partes visibles deben tenerse en cuenta para el renderizado. También mejorará la huella de memoria, porque una malla puede reutilizarse varias veces para el mismo objeto en distintas ubicaciones (para lograr esto tendrás que referenciar la misma malla en la definición de tu escena de SDK7 o usar instancias en lugar de objetos duplicados al exportar la escena desde Blender).
* **Volumen delimitador**: Asegúrate de que el volumen delimitador de cada objeto se ajuste lo más posible. El espacio sobrante en el volumen delimitador puede hacer que los objetos se rendericen cuando no son visibles.

![](/docs/_remote/85cd1390893917a4f9f84647.webp)

### ¿Qué es Frustum Culling?

Frustum Culling es una técnica de optimización que desactiva los renderers (mallas) de los objetos que están fuera del área de visión de la cámara. Mira estos 2 ejemplos:

![](/docs/_remote/aa272a10f2f7825fafd9b7e8.webp)

*Un nivel interior tipo laberinto. Esta vista normal de la escena muestra todos los Game Objects visibles.*

![](/docs/_remote/2581f915e835c33d658e7101.webp)

*El frustum culling normal solo renderiza los objetos dentro de la vista de la cámara. Esto es automático y siempre ocurre.*

## Instanciar objetos vs Duplicar objetos

En Blender, duplicar un objeto crea una copia completamente separada del objeto, mientras que instanciar un objeto crea un duplicado que comparte los mismos datos que el objeto original.

Cuando un objeto se duplica, crea un nuevo objeto con un conjunto de datos completamente independiente, incluyendo toda su geometría, materiales y animaciones. Esto significa que cualquier cambio realizado en el objeto original no se reflejará en el objeto duplicado, y viceversa.

Por otro lado, cuando un objeto se instancia, cualquier cambio realizado en el objeto original se reflejará en todas sus instancias, y viceversa. Esto se debe a que todas las instancias comparten los mismos datos subyacentes.

En términos de rendimiento, instanciar puede ser mucho más eficiente que duplicar, especialmente cuando se trabaja con escenas complejas o grandes cantidades de objetos. Esto se debe a que instanciar usa menos memoria que duplicar, ya que no crea nuevos datos para cada instancia. Además, instanciar puede permitir que Blender optimice el proceso de renderizado tratando todas las instancias como un solo objeto, en lugar de renderizar cada duplicado por separado.

En general, instanciar puede ser una herramienta muy poderosa para mejorar el rendimiento y la eficiencia del flujo de trabajo en Blender y en tu escena de Decentraland, especialmente cuando se trabaja con escenas grandes y complejas.

![](/docs/_remote/403e1a531d34d0b12ef576b8.webp)

En algunos casos, al duplicar objetos como árboles, plantas y postes de luz, instanciar puede mejorar significativamente el rendimiento en comparación con duplicarlos. Por ejemplo, esta escena tiene catorce postes de luz, y todos son exactamente iguales. Sabemos que cada poste de luz tiene dos texturas diferentes, una para el poste y otra para la luz. Así que en este caso tendremos dos llamadas de dibujo, una por textura.

![](/docs/_remote/b4811452757e72b24eb01716.webp)

En Blender, cuando estás en la etapa de clonar y posicionar elementos en el espacio, tienes tres opciones. Duplicar, Instanciar o Fusionar objetos.

Analicemos cuáles son las ventajas y desventajas de cada procedimiento.

### Duplicar objetos

| Menú:  | Object ‣ Duplicate Objects |
| ------ | -------------------------- |
| Atajo: | Shift-D                    |

Esto creará una copia visualmente idéntica del objeto seleccionado, pero se tratarán como objetos diferentes. Así que al exportarlo tendrá 2 llamadas de dibujo por objeto, 2 \* 14 = 28 llamadas de dibujo, y un uso total de disco de 320 kb.

![](/docs/_remote/4a2e11a5d69b78b82bfaeffb.webp)

### Instanciar objetos

| Panel: | Toolbar ‣ Tools ‣ Edit ‣ Duplicate Linked |
| ------ | ----------------------------------------- |
| Menú:  | Object ‣ Duplicate Linked                 |
| Atajo: | Alt-D                                     |

Esto creará un nuevo objeto con todos sus datos vinculados al objeto original. Si modificas uno de los objetos vinculados en Modo Edición, todas las copias vinculadas se modificarán. Las propiedades de transformación siguen siendo copias, no enlaces, así que aún puedes rotar, escalar y mover libremente sin afectar a las otras copias.

![](/docs/_remote/2366fb1c01bf3e7b09b2e7f4.webp)

*Al exportar estos modelos, seguiremos teniendo el mismo número de llamadas de dibujo (28). Sin embargo, observa que ahora solo tenemos una malla, lo que reduce significativamente el uso en disco (41 kb).*

![](/docs/_remote/8af76e55acae51d4209ab721.webp)

*Para mantener todo organizado, puedes simplemente instanciar colecciones en tu escena; de esta manera puedes controlar todas las instancias desde una sola carpeta y fuente.*


> [!NOTE]
> **💡 Consejo**: La misma lógica se aplica en el lado del SDK. Catorce entities que apuntan todas a su `GltfContainer` al mismo *.glb* archivo se comportan como instancias: el archivo se descarga una vez, se convierte una vez, y sus mallas y texturas se mantienen en memoria una sola vez, sin importar cuántas entities lo usen. Dividir el mismo contenido en catorce *.glb* archivos casi idénticos es el equivalente de duplicar, y te cuesta catorce descargas y catorce copias en memoria.


### Fusionar objetos

| Menú:  | Object ‣ Join |
| ------ | ------------- |
| Atajo: | Ctrl-J        |

Esto une los objetos seleccionados en un solo objeto con una sola malla. Funciona en Modo Objeto, fusiona todo en el último *active* objeto seleccionado y requiere que todos los objetos seleccionados sean del mismo tipo. Los materiales se conservan y se fusionan, así que nuestros catorce postes de luz dejan de ser catorce objetos renderizables y se convierten en uno, y el motor los dibuja con 2 llamadas de dibujo en lugar de 28: una por material, una sola vez.

Esta es la única de las tres opciones que realmente reduce las llamadas de dibujo, y vale la pena dejar claro por qué: el motor de Decentraland construye tu escena en tiempo de ejecución a partir de contenido transmitido, así que no puede agrupar o combinar objetos repetidos por ti de la misma forma en que un motor puede hacerlo con contenido horneado con antelación. Si necesitas menos llamadas de dibujo, la fusión tiene que hacerse en Blender.

Esa reducción no es gratuita. Fusionar implica renunciar a varias cosas a la vez:

* **El uso en disco vuelve a aumentar.** La malla fusionada almacena la geometría de los catorce postes, así que pierdes el ahorro que te daba la instanciación.
* **El frustum culling pasa a ser todo o nada.** Un objeto fusionado se dibuja completo siempre que cualquier parte de él esté en pantalla. Catorce postes distribuidos a lo largo de una calle casi siempre tendrán un poste visible, así que acabas dibujando los catorce todo el tiempo, lo que puede costar fácilmente más que las llamadas de dibujo que ahorraste.
* **Las partes dejan de poder dirigirse individualmente.** Ya no puedes mover, animar, hacer click o ocultar un solo poste, porque solo queda un objeto.

Usa la fusión de forma limitada: para muchos props pequeños que siempre se ven juntos en un mismo lugar, que nunca se interactúan y solo después de haber confirmado que el número de objetos es lo que realmente está afectando al rendimiento. Para todo lo demás, instanciar es la mejor opción por defecto.


> [!WARNING]
> **📔 Nota**: Exportar catorce objetos separados en un solo *.glb* archivo es **no** fusionar. El motor sigue viendo catorce objetos y sigue emitiendo 28 llamadas de dibujo. Solo unir las mallas cambia el recuento.


## Nomenclatura de mallas

Usa nombres significativos para tus mallas. El nombre debe dar contexto de dónde se usa el asset o a qué parte del objeto se relaciona.

Para nombrar los assets usa una mezcla de `PascalCase` y `snake_case`, que podemos llamar `PascalSnake_case`. Las reglas básicas son: una palabra nueva o una palabra después de un separador (`_`) comienza con **letra mayúscula.**

* Usa nombres significativos para tus mallas **Ejemplos:** 🟢 **Prefiere** nombres - `Teatro`, `Tranvía` 🔴 **Evita** nombres - `Sin título.008`, `primitivo(1)`, `Cilindro.091`, `_esfera-AB`
* Usa guion bajo `_` como separador para aportar más contexto al nombre. Pon primero las cosas más comunes y los detalles específicos al final. **Ejemplos:** 🟢 **Prefiere** nombres - `EstatuaFuente`, `ParedPrincipalTeatro`, `RuedaIzquierdaTranvía` 🔴 **Evita** nombres - `Norte_ParedPrincipal_Teatro`, `Tranvía_RuedaDelanteraIzquierda`

Si hay más de un objeto con el mismo nombre, añade un número después de la palabra: `EstatuaFuente01, EstatuaFuente02, etc.`

## Herramientas para crear modelos

Hay muchos addons y herramientas externas que facilitan el trabajo al crear assets para hacer que el pipeline sea más rápido y eficiente; algunas son gratis y otras de pago, por nombrar algunas:

### Decimate

Este es un modificador muy conocido que se puede usar para reducir la cantidad de tris de tu malla mientras se mantiene la estructura de la superficie del modelo. Aunque esta es una herramienta muy poderosa para optimizar modelos, ten en cuenta que una vez que el modelo se decima, la topología puede verse afectada, provocando una mala topología (porque rompe la geometría en piezas impredecibles). Por otra parte, la decimación convertirá los quads de tu malla en triángulos, lo que dificulta modificarla después de usarla. En ese sentido **la decimación puede ser útil para modelos estáticos, pero no es la opción preferida para modelos animados.**

![](/docs/_remote/5c1b597b93cffd4692e756c8.webp) ![](/docs/_remote/ffc83dd73f6105e78f5d4ed1.webp)

Otra forma de optimizar tus mallas usando decimación es decimar el modelo y luego hacer una retopología adecuada. De esta manera puedes asegurar un resultado más predecible y una topología limpia.

Algunos addons de retopología que pueden ser útiles son (algunos son de pago):

* **Simplygon:** <https://www.simplygon.com/>
* **Speedretopo:** <https://blendermarket.com/products/speedretopo?ref=2>
* **Retopoflow:** <https://blendermarket.com/products/retopoflow>

### Mirror Modifier

El Mirror Modifier en Blender es una herramienta que se utiliza para crear modelos simétricos reflejando la mitad de un objeto en cualquier dirección (X,Y,Z). Es una gran herramienta cuando tienes modelos simétricos porque reduce el tiempo de modelado 3D, de hacer UV Unwrapping (porque solo necesitarás desempaquetar una parte del modelo y el resto compartirá las mismas coordenadas UV) y también optimiza tus texturas evitando desperdiciar resolución de textura en partes que son esencialmente reflejadas.

![](/docs/_remote/d2acc97f8cfac981e2ef4ed3.webp)

### Batch Rename

Una herramienta muy útil para cambiar las convenciones de nombres de forma sencilla es usar el Batch Rename que Blender tiene integrado en su toolkit. Para acceder, ve a *Edit* y luego *Batch Rename*.

![](/docs/_remote/b586dae5ffc349455527b2fc.webp)

Selecciona los objetos a los que quieres cambiar el nombre y simplemente reemplaza el nombre de objeto predeterminado por el nuevo. Esta herramienta ofrece diferentes métodos y tipo de asset para renombrar, por lo que es muy flexible para cambiar nombres también de mallas, animaciones, huesos, etc.

![](/docs/_remote/ff8b93deb811edcecb68aeba.webp) ![](/docs/_remote/7934176b00dd96ce2f519b23.webp)

## Buenas prácticas para geometrías [#](/docs/creator/3d-modeling/meshes/#best-practices-for-geometries)

* Ten en cuenta cuántas caras añades a tus modelos 3D, ya que más caras hacen que su renderizado sea más exigente. Mira [**limitaciones de la escena**](/docs/creator/sdk7/optimizing/scene-limitations/) para conocer los límites impuestos por una escena.
* Asegúrate de que no haya caras ocultas que no puedan verse pero que sumen al conteo de triángulos.
* Para las formas que deban tener lados redondeados, configúralas para que sean *smooth* en lugar de añadir caras adicionales.
* Asegúrate de que las *normals* de todas las caras apunten hacia afuera en lugar de hacia adentro. Si hay caras en tu modelo que parecen no estar cuando lo renderizas, esa es probablemente la causa.
* Aplica la rotación y la escala de tus mallas, para que sus cajas delimitadoras no se extiendan innecesariamente hacia afuera.


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-es/modelado-y-animaciones-3d/meshes.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
