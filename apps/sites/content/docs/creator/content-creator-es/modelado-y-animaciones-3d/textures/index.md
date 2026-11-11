> Aprende cómo funcionan las texturas en Decentraland

# Texturas

Las texturas son una parte clave del pipeline de arte 3D para conseguir el aspecto y la sensación que quieres para tu escena. En esta sección encontrarás todo lo que necesitas saber para crear tus texturas, sus limitaciones, los nodos en Blender y cómo optimizarlas para que rindan al máximo.


> [!NOTE]
> **💡 Consejo**: Instala el [Decentraland Tools Blender plugin](https://extensions.blender.org/add-ons/decentraland-tools/). Incluye varias funciones útiles para ayudarte a editar y exportar modelos 3D, wearables y emotes.


## Limitaciones

### Restricciones de tamaño de textura

Actualmente, el Decentraland Explorer comprime las texturas a un máximo de 1024 px con fines de optimización mediante un Asset Bundle Converter después de que la escena se sube a los content servers. ¡Asegúrate de tener en cuenta estas limitaciones al crear tus assets!

Otro punto importante a tener en cuenta es que las texturas siempre deben ser potencias de dos; cualquier textura que no siga esta especificación puede provocar problemas al renderizar la escena.

Los tamaños de Texture deben usar números de ancho y alto (en píxeles) que coincidan con los siguientes números:

```
1, 2, 4, 8, 16, 32, 64, 128, 256, 512, 1024
```

> Esta secuencia está compuesta por potencias de dos: `f(x) = 2 ^ x` . **1024 es el número máximo que permitimos para el tamaño de una textura.** Este es un requisito bastante común entre otros motores de renderizado; existe debido a optimizaciones internas de las GPU.

El ancho y el alto no necesitan tener el mismo número, pero ambos deben pertenecer a esta secuencia.

**El tamaño recomendado para las Textures es 1024x1024**, hemos descubierto que este es el tamaño óptimo para transportarse a través de redes domésticas y ofrecer experiencias razonables de carga/calidad.

Ejemplos de otros tamaños válidos:

```
32x32px
64x32px
512x256px
512x512px
1024x1024px
```

Por otra parte, hay un límite de texturas por parcel:

```
log2(n+1) x 10 Cantidad de texturas por parcela. Incluye las texturas importadas como parte de los modelos.
```

## UVMapping

UVmapping es el proceso de desplegar las caras de tu modelo 3D en una coordenada 2D que se usará más adelante para añadir los diferentes maps a tus assets. Es una parte clave del proceso de creación. Hacer un unwrap correcto de tus modelos es un factor clave para exprimir la resolución de tus modelos y también organizará tus maps para que sean flexibles para su modificación.

Para saber más sobre UV Unwrapping puedes ver este increíble video hecho por la Blender Foundation:

[![Video Preview](/docs/_remote/b25b76a5b0e4c1d0cec817e8.webp)](https://youtu.be/Y7M-B6xnaEM?si=qMnWTMsXxC-vxZAH)

También hay otro gran tutorial de nivel intermedio para desempaquetar UVs hecho por \[Blender Guru] (<https://www.youtube.com/@blenderguru>) que explica cómo desempaquetar un modelo más complejo:

[![Video Preview](/docs/_remote/2b79a21fed4d0ba2abc23bfa.webp)](https://youtu.be/scPSP_U858k?si=Uw0xHbv9jtqVstDS)

## Mapas

En la [sección de materiales](/docs/creator/3d-modeling/materials/) explicamos cómo funciona Decentraland con los shaders PBR. En esta sección te mostraremos cómo cada mapa de textura modifica el shader y el aspecto y la sensación del objeto 3D que luego se exportará al mundo.

## Mapas difusos

Este es el color base de la superficie del objeto. Tener una paleta de colores equilibrada entre tus modelos es clave para lograr un aspecto y una sensación cohesivos en tu experiencia.

![](/docs/_remote/3a9f1ec299edbbb69e0531e0.webp)

Aquí tienes un generador de paletas gratuito muy útil por si lo necesitas.

* **Coolors:** <https://coolors.co/>

O algo de inspiración de paletas proporcionada por películas increíbles:

* **Movies in Color:** <https://moviesincolor.com/>

## Mapas metálicos

En un mapa metálico, el mapa en escala de grises representa el grado de metalicidad que posee un objeto, siendo blanco metálico total y negro no metálico. En el siguiente ejemplo vemos cómo la luz afecta al modelo y cómo interactúa con el entorno.

![](/docs/_remote/d7d680ed47a91de7cef0901a.webp)

*En la imagen podemos ver cómo las partes oscuras de la textura afectan al modelo. Las franjas oscuras son opacas, mientras que la franja blanca se comporta según las propiedades del shader metálico y el gris siendo una mezcla entre ambas.*

## Mapas de rugosidad

En un mapa de rugosidad, las zonas más oscuras corresponden a superficies más lisas, mientras que las zonas más brillantes corresponden a superficies más ásperas. Esta representación en escala de grises se usa en los motores de renderizado para determinar cómo debe dispersarse o reflejarse la luz en distintos puntos de la superficie.

![](/docs/_remote/ab1077956ce52917b01f28a4.webp) ![](/docs/_remote/cf4b607b56f34ffc9787473a.webp)

## Mapas transparentes

### Alpha Clip

Alpha Clip en Blender, cuando se usa con una textura, implica usar el canal alfa de la textura para determinar qué partes del material deben ser visibles. Los píxeles con valores alfa por encima de un umbral especificado se muestran, mientras que los que están por debajo del umbral se descartan, creando un efecto de recorte basado en la información de transparencia de la textura.

En el siguiente ejemplo usamos un material con Alpha Clip, utilizando 2 texturas, el color difuso y la textura alfa en blanco y negro, conectando el color al canal alfa.

![](/docs/_remote/a6eb0c3607b3ac754dd99f4a.webp) ![](/docs/_remote/7efa7539bc36fae5c645b2f7.webp)

![](/docs/_remote/85b8e0c6b3f5f217698536fc.webp)

*Como resultado podemos ver que las áreas pintadas en negro se descartan mientras que el área blanca se está renderizando*

### Alpha Blend

*Alpha Blend* te permite elegir valores intermedios por región.

Alpha Blend en Blender, cuando se usa con una textura, implica mezclar las áreas transparentes y opacas de la textura según su canal alfa. Esto permite que la textura combine suavemente tanto las partes visibles como las transparentes, interactuando de forma natural con el fondo u otros objetos de la escena.

![](/docs/_remote/46cdac0bd41b0f225bd498e4.webp)

*Mientras que Alpha Clip renderiza valores de 0 o 1 (en un umbral específico), Alpha Blend interpola los valores entre 0 y 1. En el ejemplo anterior, el material de Alpha Blend muestra la transición completa del degradado, mientras que Alpha Clip excluye parte del conjunto de texturas establecido por el umbral de recorte*


> [!WARNING]
> **🔥Consejo de optimización🔥**
> 
> * Ten en cuenta que las texturas transparentes (RGB+A) siempre son más costosas en términos de rendimiento que usar un valor en escala de grises para las transparencias.
> * La transparencia siempre es una operación costosa al renderizar la escena. Procura mantener las transparencias al mínimo y usar Alpha Blend solo cuando sea necesario; de lo contrario, se prefiere Alpha Clip en lugar de Alpha Blend.


## Mapas emisivos

Un mapa emisivo es un tipo de mapa de textura usado para controlar la autoiluminación o la luz emitida por una superficie en una escena 3D. Es un componente del shader que determina cuánta luz, y de qué color, emite una parte concreta de un modelo 3D, independientemente de las fuentes de luz externas. Los mapas emisivos se usan comúnmente para simular materiales u objetos que parecen emitir su propia luz.

![](/docs/_remote/e4c3fc1cd832dcc33edb3f1c.webp)

*En este ejemplo podemos ver el uso de un mapa emisivo combinado con emissive strength en un entorno que usa glow postprocessing para probar aproximadamente cómo se comporta el emissive en el mundo.*

## Mapas normales

Un normal map es un tipo de textura usado en gráficos 3D para simular detalles finos de la superficie y crear la ilusión de geometría compleja sin alterar realmente la geometría subyacente de un modelo. Se usa comúnmente para mejorar el realismo de modelos low-poly añadiendo la apariencia de bultos, hendiduras y otras irregularidades de la superficie. También te permite mantener los objetos más ligeros, ya que muchos detalles pueden proporcionarse en la capa del normal map en lugar de mediante geometría compleja.

![](/docs/_remote/76188ce5daf39c2ac1f11691.webp) ![](/docs/_remote/c3f4920500591fa40e453fd3.webp)

Para añadir un normal map a tu material usando el *Shader Editor*, necesitarás conectar el ***Normal Map*** nodo entre la textura y el *Principled BSDF* shader.

![](/docs/_remote/813d6e575903a5c3a19e8e2e.webp)


> [!WARNING]
> **⚠️Importante⚠️:** Nunca uses una textura como albedo y normal al mismo tiempo porque puede crear problemas al renderizar la escena.


## Optimización de texturas

El proceso de optimización de texturas aporta muchos beneficios al renderizar la escena en el Explorer, y también es una buena forma de mantener el estilo de tu escena coherente y más flexible durante el proceso de diseño. Algunos de estos beneficios son:

* Tener texturas optimizadas en tamaño y compresión hará que la escena funcione mucho más suave y rápido, facilitando su descarga y su renderizado (especialmente para jugadores con una conexión a internet lenta).
* Reduce la cantidad de memoria y potencia de procesamiento necesaria para renderizar tu experiencia, lo que se traduce en una mejor experiencia de usuario para tus jugadores.
* Ahorra almacenamiento en los servidores de contenido de la comunidad.
* Usar **Texture Atlases** y/o **Trim Sheets** te dará más flexibilidad para iterar la creación de tu escena y coherencia de estilo entre los objetos. Usando estas técnicas puedes intercambiar texturas, ajustar colores o patrones fácilmente en lugar de hacerlo individualmente para cada modelo.
* Compartir texturas entre modelos permite tener menos texturas por escena, reduciendo drásticamente los draw calls en el juego. Si estás trabajando con GLBs (con texturas embebidas) encontrarás en las siguientes guías un extrator para extraer las texturas, redirigiendo los modelos para que usen la misma textura.

## Texturas compartidas entre modelos glTF

Una práctica inteligente y común para optimizar tu escena es compartir texturas y materiales entre modelos de toda la escena. Hacerlo reducirá drásticamente los draw calls y tu escena de Decentraland funcionará mucho más fluida.

La siguiente herramienta basada en [**glTF pipeline**](https://github.com/AnalyticalGraphicsInc/gltf-pipeline)**,** ofrece algunas optimizaciones que harán que los modelos 3D sean más ligeros y se descarguen más rápido para los jugadores de tu escena.

**Mac:** [MAC GLB Extractor](https://github.com/decentraland/docs/blob/main/creator/images/3d-models-and-animations/glb-extractor/texture_extractor.sh)

**PC:** [PC GLB Extractor](https://github.com/decentraland/docs/blob/main/creator/images/3d-models-and-animations/glb-extractor/glb_texture_extract.bat)

Convierte el formato .gltf a .glb, que es binario y por tanto ocupa mucho menos. También coloca los archivos de textura fuera del modelo 3D, lo que te permite usar la misma textura en varios modelos.

> 📔 Nota: el formato .glb por defecto siempre tiene las texturas incrustadas en el archivo. El engine no puede reconocer dos texturas incrustadas como si fueran la misma; necesitan ser archivos externos que compartan el mismo hash.

### Cómo usar GLB Texture Extractor

En esta escena de ejemplo, tenemos una escena sencilla de ciencia ficción en Blender.

![](/docs/_remote/09c63fb13acc8a4914118992.webp)

Esta escena contiene el entorno base para los modelos estáticos, pero también hay otros dos assets, un droid y una nave espacial que queremos exportar por separado para poder moverlos después mediante código, de modo que puedan interactuar con los jugadores. En este caso usamos 4 texturas (una para el suelo, un atlas de color para la mayoría de los assets, una emisiva y una textura sci-fi de UI para los paneles)

![](/docs/_remote/1167ef79495fce28d3f04994.webp) ![](/docs/_remote/a8815581bf426ea4c90234ea.webp)

Una vez que exportamos todos estos assets a la carpeta de modelos tenemos 3 modelos: la escena estática del entorno, la nave espacial y el droid.

![](/docs/_remote/ba80d47093d413ede575a657.webp)

Pero tenemos un problema: los archivos .glb tienen las texturas incrustadas, así que si estás exportando distintos assets que reutilizan las texturas, estas se duplicarán cada vez que haya un asset nuevo en la carpeta. Para evitar tener texturas duplicadas podemos usar esta útil herramienta.


> [!WARNING]
> ⚠️ NOTA IMPORTANTE: Antes de usar la herramienta haz una COPIA DE SEGURIDAD de tus modelos, ¡por si algo sale mal!


**En Mac**

Una vez que hayas exportado todos los assets a tu carpeta de modelos, puedes arrastrar el archivo del script hasta ella.

![](/docs/_remote/e18bbaa902da38ea04efdabb.webp)

1 - Abre la carpeta en la terminal arrastrando la carpeta hasta ella.

![](/docs/_remote/b110a995c9095984ab0e64a6.webp)

2 - Arrastra texture-extractor.sh a la terminal.

![](/docs/_remote/21793b724c96391277db0f59.webp)

3 - Ejecuta el comando. Puede tardar unos segundos en procesar todos los assets. Después de eso, verás algo como esto:

![](/docs/_remote/98233ac951185a1a694e7d05.webp)

4 - Ve a tu carpeta de modelos y habrá una nueva carpeta llamada "out" en la que verás tus nuevos assets con las texturas extraídas.

![](/docs/_remote/fb16a7c2d30d3624207c138e.webp)

5 - ¡Reemplaza todos los assets por los nuevos! Además, una vez que termines, borra "texture-extractor.sh" y la carpeta vacía "out".

![](/docs/_remote/98e2e796967f27259ce9c137.webp)

**En Windows**

Una vez que hayas exportado todos los assets a tu carpeta de modelos, puedes arrastrar el archivo del script dentro de ella.

![](/docs/_remote/7a5b21bf0d0ceb24563c52c3.webp)

1 - Haz doble clic en glb\_texture\_extract.bat para extraer los archivos. Si Windows te advierte sobre una aplicación no reconocida, ve a **Más información** y luego **Ejecutar de todos modos**.

![](/docs/_remote/c0f9074745dc366687e508e5.webp) ![](/docs/_remote/e286200a0e0ba4a6327ffc69.webp)

2 - El script generará una carpeta llamada out, allí encontrarás todos los nuevos archivos .glb con sus texturas extraídas.

![](/docs/_remote/c4d9063db0fa909ac05097d1.webp)

5 - Reemplaza todos los assets por los nuevos. Una vez terminado, elimina el script "texture-extractor.bat" y la carpeta vacía "out".

Si sigues todos los pasos, tu escena será mucho más rápida ahora y los assets compartirán la misma textura. Cuando trabajamos con varios assets y escenas grandes, la mejora es bastante notable. ¡Con esta herramienta puedes ahorrar bastantes megabytes de información!


> [!WARNING]
> ⚠️ Después de completar este paso, es crucial comprobar que cada textura sigue las pautas. Si las texturas no siguen las pautas o son demasiado pesadas, optimízalas para que sean más ligeras antes de desplegar. El tamaño de textura recomendado para escenas con buen rendimiento es 1024x1024. Comprueba también la resolución; las imágenes con más de 72 DPI afectarían al rendimiento y no harán que la imagen se vea mejor.


## Texture Atlas / Trim Sheets

Un texture atlas es un único archivo de imagen que contiene datos de varias imágenes más pequeñas empaquetadas juntas. En lugar de tener una textura para cada mesh, varios meshes comparten una textura más grande.

Puedes crear un texture atlas antes de hacer el asset, lo que significa que el asset se desempaqueta en UV según el texture atlas. Esto requiere cierta planificación previa al crear la textura.

Alternativamente, puedes crear el texture atlas después de terminar el asset fusionando texturas en un software de pintura. Sin embargo, esto también significa que las islas UV deben reorganizarse según la textura.

A continuación hay una imagen que muestra varios objetos 3D que usan un mismo conjunto de texturas:

![](/docs/_remote/ed4a55e913cda2f92bcb60db.webp) ![](/docs/_remote/d4e59128a0be5c8f7414ad23.webp)

Otra forma de hacer Atlases es usar Trim Sheets, una técnica común en modelado 3D y desarrollo de juegos que los grandes estudios y la industria del juego utilizan para garantizar la coherencia visual y un pipeline artístico eficiente. Trim Sheets es el uso de una sola imagen o textura que contiene múltiples pequeños detalles o elementos que pueden aplicarse a diferentes partes de un modelo. Esta técnica es muy útil al crear escenas grandes y debe considerarse en la primera etapa del pipeline artístico.

![](/docs/_remote/6e219acc9852ed30d1a9b768.webp)

*Un ejemplo en collage de un Trim Sheet que utiliza una textura difusa, normal y emisiva.*

Hay un gran [tutorial](https://www.artstation.com/blogs/jennifermcgarry/yd4Q/jenns-guide-to-trim-sheets) por [Jennifer McGarry](https://www.artstation.com/jennifermcgarry/blog) que explica el uso de Trim Sheets usando Blender!

## Nomenclatura de texturas

**Es crucial nombrar correctamente nuestras texturas.** Tener una nomenclatura correcta para la textura permitirá:

* Hacer que tu pipeline artístico sea más eficiente, flexible, organizado y fácil de localizar y modificar si es necesario.
* Evitar solapar texturas con el mismo nombre al usar el extractor de texturas u otras herramientas.
* Evitar problemas de solapamiento de texturas al usar Asset Bundles.
* Una forma más eficiente de abordar los puntos problemáticos al analizar la escena.

**Cómo nombrar correctamente tus Textures:**

* **El nombre del asset** debe representar claramente de qué es la textura.
* El nombre de las texturas debe comenzar con el prefijo `T_`.
* El nombre de la textura debe terminar con el sufijo que define el tipo de textura:
  * `_D` - **Mapa difuso/de color**
  * `_A` - **Textura alfa**
  * `_MT` - **Metallic**
  * `_R` - **Roughness**
  * `_N` **- Normal Map**
  * `_E` - **Emisión**

Por ejemplo: si se trata de un mapa difuso de textura de ladrillo para una pared, el nombre `T_BrickWall_D` podría ser apropiado. Si se trata de un Normal Map del mismo asset, el nombre podría ser `T_BrickWall_N`

**Ejemplos:**

* 🟢 **Prefiere** empezar el nombre de la textura con - `T_Parquet_D`, `T_Floor_R`, `T_Pipes_MT`,
* 🔴 **Evita** empezar el nombre de la textura con - `Image_`, `sprite_`,`Sin título`

## Herramientas opcionales

Hay muchos addons y herramientas externas que facilitan el trabajo al crear assets para hacer que el pipeline sea más rápido y eficiente; algunas son gratis y otras de pago, por nombrar algunas:

### UVTools

* **UV Packer (Gratis)**:<https://www.uv-packer.com/blender/>
* **Zen UV:** <https://blendermarket.com/products/zen-uv>
* **Uvpackmaster 3:** <https://blendermarket.com/products/uvpackmaster>

### Compresores de imágenes

Hay varios compresores de imágenes en línea que puedes usar para hacer que tus texturas sean más ligeras. Por nombrar algunos:

* **CompressPNG:** <https://compresspng.com/>
* **TinyPNG:** <https://tinypng.com/>
* **FreeConvert:** <https://www.freeconvert.com/>


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-es/modelado-y-animaciones-3d/textures.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
