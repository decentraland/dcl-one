# Materiales

Aprende qué propiedades de material y texturas son compatibles con los modelos 3D importados a Decentraland.

Los materiales se incrustan en un *.gltf* o *.glb* .


> [!NOTE]
> **💡 Consejo**: Instala el [Decentraland Tools Blender plugin](https://extensions.blender.org/add-ons/decentraland-tools/). Incluye varias funciones útiles para ayudarte a editar y exportar modelos 3D, wearables y emotes.


Este documento se refiere a materiales que se importan en un modelo 3D. Para materiales definidos mediante código que se apliquen a formas primitivas, consulta [**materials**](https://docs.decentraland.org/creator/development-guide/materials/) .


> [!WARNING]
> **📔 Nota**: Actualmente no puedes cambiar dinámicamente los materiales de un modelo 3D desde el código de tu escena, a menos que se trate de una forma primitiva.


## Compatibilidad de shaders

No todos los shaders pueden usarse en modelos que se importan a Decentraland. Asegúrate de usar uno de los siguientes:

* **Materiales estándar:** se admiten cualquier shader, por ejemplo diffuse, specular, transparency, etc.
* **Materiales PBR (Physically Based Rendering)**: Este shader es más flexible, ya que incluye propiedades como diffuse, roughness, metalness y emission que te permiten configurar cómo interactúa un material con la luz.

La imagen de abajo muestra dos modelos idénticos, creados con los mismos colores y texturas. El modelo de la izquierda usa todos los *PBR* materiales, algunos de ellos incluyen *metalness*, *transparency*, y *emissividad*. El modelo de la derecha usa todos los *estándar* materiales, algunos de ellos incluyen *transparency* y *emissividad*.

![](/docs/_remote/129845688fefc65d479ded1d.webp)

### Propiedades PBR que actualmente funcionan con el Decentraland Engine

> * Color base
> * Metallic
> * Roughness
> * Specular
> * Emissive
> * Intensidad de emisión
> * Alpha
> * Normal

![](/docs/_remote/9edb0c21919b4803373b3885.webp)

Para visualizar cómo se comportan estas propiedades en el mundo, puedes ir a este [mundo de prueba](decentraland://?realm=TestEnvironment.dcl.eth) para encontrar diferentes objetos y materiales, y ver cómo interactúan con las luces y el entorno del mundo.

![](/docs/_remote/1abdb0c1779701d95d2637dc.webp)

### Color base/diffuse

Define el color base de la superficie del objeto. Por sí solo, no recibe ningún efecto de la iluminación; por eso se combina con otros nodos como roughness, metallic, specular, etc.

### Metallic

Un shader metálico se refiere a un tipo de técnica de renderizado usada para simular la apariencia de superficies metálicas. Un shader metálico tiene en cuenta las propiedades físicas de los metales y cómo interactúan con la luz para producir las cualidades brillantes y reflectantes características del metal.

![](/docs/_remote/b601bec72f14836b99d6f451.webp)

### Roughness

Los materiales con roughness están relacionados con la simulación realista de cómo la luz interactúa con la superficie del material. Normalmente se usan mapas de roughness para dar a los modelos un rango de "suavidad" o "rugosidad" en sus superficies. Normalmente se usa un mapa de textura en escala de grises para proporcionar información de este tipo.

![](/docs/_remote/c4e81d0781e9fcd132dbed8f.webp)

Esta propiedad mezcla entre un modelo de material no metálico y uno metálico. Un valor de 1.0 da una reflexión especular completamente tintada con el color base, sin reflexión difusa ni transmisión. En 0.0 el material consiste en una capa base difusa o transmisiva, con una capa de reflexión especular encima.

### Specular

En un shader Physically-Based Rendering (PBR), las propiedades specular se refieren a cómo interactúa la luz con una superficie en términos de reflectividad y brillo. La reflexión especular es la reflexión de la luz sobre una superficie similar a un espejo. En PBR, esta propiedad se usa para controlar cuánta luz refleja una superficie de manera similar a un espejo. Materiales como los metales suelen tener una alta reflexión especular, creando brillos nítidos e intensos, mientras que los materiales no metálicos como el plástico tienen una reflexión especular más baja, lo que produce brillos más amplios y suaves.

![](/docs/_remote/92855ba60b2140bd9d3977ad.webp)

### Alpha

Puedes configurar un material para que sea *transparente*. Los materiales transparentes pueden verse a través de ellos en distintos grados, según su *alpha*. Para hacerlo, activa la propiedad de transparencia del material y luego establece su *alpha* en la cantidad deseada. Un alpha de 1 hará que el material sea completamente opaco; un alpha de 0 lo hará invisible.

La imagen de abajo muestra dos modelos idénticos creados con materiales estándar. El de la izquierda usa solo materiales opacos, mientras que el de la derecha usa materiales transparentes y opacos en algunas de sus partes.


> [!WARNING]
> 💡 Recuerda que usar materiales transparentes siempre es más costoso en términos de rendimiento que usar materiales diffuse. Intenta mantener los materiales transparentes al mínimo posible.


![](/docs/_remote/69e874b1001f67ad59d3ac60.webp)

Hay dos modos principales de transparencia diferentes: *Alpha Clip* y *Alpha Blend*. Las principales diferencias son:

* **Alpha Clip:** Alpha Clip renderiza valores absolutos de 0 o 1 a partir de un umbral de clip de un valor en escala de grises. El color anterior será reemplazado por el color de la superficie, pero solo si el valor alpha está por encima del valor de Clip Threshold.

![](/docs/_remote/6e885f9bbcbfca598d148a89.webp)

* **Alpha Blend:** Alpha Blend interpola los valores entre 0 y 1. Puedes usar alpha blending para superponer el color de la superficie sobre el color anterior.

![](/docs/_remote/293aba2fd75ef42086bd5c31.webp)


> [!WARNING]
> **🔥Consejo de optimización🔥** A menos que quieras específicamente poder tener un nivel intermedio de transparencia, **siempre es más eficiente para el renderizado usar \_Alpha Clip***\*\* en lugar de \*\****Alpha Blend**\_**.**



> [!WARNING]
> **⚠️ Problemas de ordenación**
> 
> Cuando usas modos de mezcla transparentes en tu juego, es fundamental considerar el orden en el que se realiza la mezcla de colores. Esto se debe a que el color de salida final puede verse afectado significativamente por el orden de la mezcla. Actualmente, el motor solo admite ordenación por objeto, lo que significa que ordena automáticamente todas las superficies transparentes según el origen del objeto. Sin embargo, la ordenación por fragmento (píxel) y la ordenación por triángulo no están admitidas actualmente.
> 
> Para evitar problemas relacionados con la ordenación, lo mejor es evitar usar objetos con alpha clip y alpha blend en el mismo mesh. Esto puede ayudar a prevenir artefactos de mezcla inesperados y garantizar que tu juego se vea lo mejor posible.


### Emissive

También puedes hacer que un material sea *emissive*. Los materiales emissive emiten su propia luz. Ten en cuenta que, al renderizarse, en realidad no iluminan los objetos cercanos de la escena; solo parecen tener un resplandor difuso a su alrededor.

La imagen de abajo muestra dos modelos idénticos creados con materiales estándar. El de la derecha tiene materiales emissive brillantes en algunas de sus superficies.

![](/docs/_remote/6a611f35617386705cb8bcce.webp) ![](/docs/_remote/4deead5b2aeaa81746397cb9.webp)

*Para hacer que un material sea emissive en Blender, solo tienes que añadir un `emission` shader al material.*

### Intensidad de emisión

Intensidad de la luz emitida. Un valor de 1.0 garantizará que el objeto de la imagen tenga exactamente el mismo color que el Emission Color, es decir, que no tenga sombreado.

![](/docs/_remote/5392ea458ab888ee365b3678.webp)

*Puedes comprobar en el* [*mundo de prueba*](decentraland://?realm=TestEnvironment.dcl.eth) *cómo se comporta la intensidad de emisión en el mundo*

### Normal

El nodo "normal" en un shader PBR es un componente fundamental usado para controlar las normales de la superficie de un material. Las normales son vectores que definen la dirección perpendicular a una superficie en un punto específico, y desempeñan un papel crucial al determinar cómo interactúa la luz con la superficie.

![](/docs/_remote/c3f4920500591fa40e453fd3.webp)

## Pintado de vértices

El pintado de vértices de modelos 3D no está admitido actualmente por el motor de Decentraland.

## Limitaciones de materiales

Ten en cuenta que los límites de materiales por parcel son:

> log2(n+1) x 20 Cantidad de materiales en la escena. Incluye materiales importados como parte de los modelos.

Es importante tener en cuenta que cada material representa un draw call por objeto, así que es crucial mantener los materiales al mínimo posible e intentar reutilizarlos tanto como sea posible mediante técnicas como Texture Atlases; esto también beneficiará a la escena al darle un estilo coherente entre los assets de tu escena.

## Nomenclatura de materiales

Para tener un pipeline artístico organizado y saludable, recomendamos nombrar correctamente tus materiales. Una forma de hacerlo es usando esta convención.

```
<Objeto>_<Clasificación>_<Subclasificación(opcional)>_<_MAT>
```

Por ejemplo, supongamos que hicimos 2 árboles diferentes, uno emissive y luminoso para primavera y otro frío y metálico para invierno. Podríamos nombrar los materiales: *"TreeSpring\_Emissive\_MAT"* y otro *"TreeWinter\_Metallic\_MAT"*

En conclusión,

* 🟢 **Prefiere** usar nombres que empiecen con el objeto y la clasificación: *"Wood\_Oak\_MAT"*, *"SciFiFence\_Metallic\_MAT"*, etc.
* 🔴 **Evita** usar nombres como *"Material009"*, *"material1"*, que hacen que la escena y los modelos sean realmente difíciles de seguir y analizar.

## Sobrescribir materiales glTF

Puedes sobrescribir los materiales de un *archivo glTF* modelo usando el [GltfNodeModifiers](/docs/creator/content-creator-es/scenes-sdk7/fundamentos-del-contenido-3d/materials/#modify-gltf-materials) componente en el código de tu escena. Consulta [Modificar materiales glTF](/docs/creator/content-creator-es/scenes-sdk7/fundamentos-del-contenido-3d/materials/#modify-gltf-materials) para más detalles.

## Buenas prácticas para materiales

* Si tu escena incluye varios modelos que usan la misma textura, referencia la textura como un archivo externo en lugar de tenerla incrustada en el modelo 3D.

Las texturas incrustadas se duplican para cada modelo y aumentan el tamaño de la escena. *.glb* Los archivos tienen sus texturas incrustadas por defecto, pero puedes usar [**glTF pipeline**](https://github.com/AnalyticalGraphicsInc/gltf-pipeline) para extraerlas fuera.

> Nota: Después de referenciar un archivo para una textura que no se incrustará, asegúrate de que ese archivo no se mueva ni se renombre; de lo contrario, se perderá la referencia al archivo. El archivo también debe estar dentro de la carpeta de la escena para que se suba junto con la escena.

* Al configurar la transparencia de un material, intenta usar siempre *Alpha clip* en lugar de *Alpha blend*, a menos que necesites específicamente tener un material parcialmente transparente (como el vidrio). Esto evitará problemas en los que el motor renderiza el modelo equivocado delante del otro.
* Como regla general, recuerda establecer siempre *backface culling* en tus materiales. Esto hará que tu escena sea más eficiente, ya que el motor renderizará solo la cara visible de tus modelos. Desactívalo solo *backface culling* en caso de que necesites que un modelo se renderice por ambos lados (por ejemplo, un grupo de hojas de un árbol hechas con planos 3D).

![](/docs/_remote/83df362d7fc9ab62e2ae130d.webp)

* Usa las [**texturas predeterminadas de Decentraland**](https://github.com/decentraland/builder-assets/tree/master/textures) , que se cargan previamente para los jugadores, haciendo que tus assets se rendericen mucho más rápido.
* Lee [**este artículo**](https://www.khronos.org/blog/art-pipeline-for-gltf) para una visión detallada de un pipeline artístico completo que usa texturas PBR en modelos glTF.
* Puedes encontrar una referencia detallada sobre cómo crear materiales compatibles con glTF con Blender en [**la documentación de Blender**](https://docs.blender.org/manual/en/latest/addons/import_export/scene_gltf2.html) .
* Encuentra texturas PBR gratuitas y de alta calidad en [**cgbookcase**](https://cgbookcase.com/) .


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-es/modelado-y-animaciones-3d/materials.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
