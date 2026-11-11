> Aprende qué assets y components son compatibles en modelos 3D externos y cómo configurarlos antes de importarlos a Decentraland.

# Modelos 3D

![](/docs/_remote/62bf81e2e6497cde955d408a.webp)

Cuando importas modelos 3D a Decentraland, es importante asegurarse de que estén en formato glTF. Estos modelos pueden incluir una variedad de funciones compatibles. Esta sección trata sobre cómo hacer que sean compatibles con Decentraland y seguir las mejores prácticas.


> [!NOTE]
> **💡 Consejo**: Instala el [Decentraland Tools Blender plugin](https://extensions.blender.org/add-ons/decentraland-tools/). Incluye varias funciones útiles para ayudarte a editar y exportar modelos 3D, wearables y emotes.


Ten en cuenta que todos los modelos, shaders y textures deben cumplir con los [**limitaciones de la escena**](/docs/creator/sdk7/optimizing/scene-limitations/).

## **Formatos de modelos 3D compatibles**

Todos los modelos 3D en Decentraland deben estar en formato glTF. [**archivo glTF**](https://www.khronos.org/gltf) (GL Transmission Format) es un proyecto abierto de Khronos que proporciona un formato común y extensible para recursos 3D que es eficiente y altamente interoperable con las tecnologías web modernas.

Los modelos glTF pueden tener una *.gltf* o un *.glb* extensión. Los archivos glTF son legibles por humanos; puedes abrir uno en un editor de texto y leerlo como un archivo JSON. Esto es útil, por ejemplo, para verificar que las animaciones estén correctamente adjuntas y comprobar sus nombres. Los archivos glb son binarios, así que no son legibles, pero son considerablemente más pequeños, lo cual es bueno para el rendimiento de la escena.

Recomendamos usar *.gltf* mientras trabajas en una escena, pero luego cambiar a *.glb* al subirla.

Los siguientes aspectos de un modelo 3D pueden estar incrustados en un *archivo glTF* archivo o referenciados externamente:

* Las textures pueden estar incrustadas o referenciadas desde un archivo de imagen externo.
* Los datos binarios sobre la geometría, las animaciones y otros aspectos relacionados con el buffer del modelo pueden estar incrustados o referenciados desde un *.bin* .


> [!WARNING]
> **📔 Nota**: Animaciones *debe* estar incrustados dentro del *archivo glTF* archivo para usar en Decentraland. Además, cada modelo solo puede tener 1 mapa UV.


## **Por qué usamos glTF**

Usamos glTF porque ofrece un potente conjunto de funciones que incluye:

* Objetos jerárquicos
* Estructura esquelética y animación
* Flujo de trabajo sólido para materials y shaders
* Información de escena, como fuentes de luz y cámaras (que pueden usarse en desarrollos futuros).

En comparación con *COLLADA*, las funciones compatibles son muy similares. Sin embargo, como glTF se centra en proporcionar un “formato de transmisión” en lugar de un formato de editor, es más interoperable con las tecnologías web.

## **Exportar y convertir glTF**

### **Exportar a glTF desde Blender**

Blender tiene un exportador nativo de glTF.

Para exportar, selecciona los modelos que quieres que estén incluidos en el archivo .glb y luego ve a File - Export - glTF 2.0

![](/docs/_remote/1f95091f3ce5ddb4219f6eea.webp) ![](/docs/_remote/3edef4656ec7f2427b2d9dbe.webp)

*En el panel lateral, marca “Selected Objects” en caso de que quieras exportar solo los objetos seleccionados en la escena. Además, en el desplegable Mesh, selecciona Apply Modifiers (si los hubiera) y luego Export.*

### **Exportar a glTF desde 3D Studio Max**

3D Studio Max no admite exportar a glTF de forma predeterminada, pero puedes instalar un plugin para habilitarlo.

1. Descarga el plugin desde [este enlace](https://github.com/BabylonJS/Exporters/tree/master/3ds%20Max).
2. Instala el plugin siguiendo [estas instrucciones](http://doc.babylonjs.com/resources/3dsmax#how-to-install-the-3ds-max-plugin).
3. Exporta archivos glTF usando el plugin siguiendo [estas instrucciones](http://doc.babylonjs.com/resources/3dsmax_to_gltf).

### **Exportar a glTF desde Maya**

Maya no admite exportar a glTF de forma predeterminada, pero puedes instalar un plugin para habilitarlo.

1. Instala el plugin siguiendo [estas instrucciones](http://doc.babylonjs.com/resources/maya).
2. Exporta archivos glTF usando el plugin siguiendo [estas instrucciones](http://doc.babylonjs.com/resources/maya_to_gltf#pbr-materials).


> [!WARNING]
> **📔 Nota**: Como alternativa, puedes probar [este otro plugin](https://github.com/WonderMediaProductions/Maya2glTF) también.


### **Exportar a glTF desde Unity**

Unity no admite exportar a glTF de forma predeterminada, pero puedes instalar un plugin para habilitarlo.

Descarga el plugin desde [este enlace](https://github.com/sketchfab/Unity-glTF-Exporter).


> [!WARNING]
> **📔 Nota**: Como alternativa, puedes probar [este otro plugin](https://assetstore.unity.com/packages/tools/utilities/collada-exporter-for-unity2017-99793) también.


### **Exportar a glTF desde SketchUp**

SketchUp no admite exportar a glTF de forma predeterminada, pero puedes instalar un plugin para habilitarlo.

Descarga el plugin desde [este enlace](https://extensions.sketchup.com/en/content/gltf-exporter).

### **Convertir FBX a glTF**

*.fbx* es un estándar muy popular para modelos 3D. No es compatible con nuestro engine, pero puedes exportar fácilmente un *.fbx* modelo a *.gltf* formato.

Recomendamos usar estas herramientas:

* [**Blender**](https://www.blender.org/download/): Una forma sencilla de convertir .fbx a glTF es importar el .fbx a una escena usando Blender y luego exportarlo desde allí.
* [**La herramienta CLI de Facebook**](https://github.com/facebookincubator/FBX2glTF): esta es la alternativa más robusta, pero requiere usar la línea de comandos.
* [**Blackthread**](https://blackthread.io/gltf-converter): Esta es la herramienta web más completa. Menos robusta que la CLI, pero mucho más fácil de usar.
* [**Modelconverter**](https://modelconverter.com/convert.html): Otra herramienta web fácil de usar.

## **Previsualizar un modelo glTF**

Una forma rápida y sencilla de previsualizar el contenido de un modelo glTF antes de importarlo a una escena es usar [**Babylon.js Sandbox**](https://sandbox.babylonjs.com/). Solo arrastra y suelta el archivo glTF (y su *.bin* archivo si corresponde) sobre el canvas para ver el modelo.

En el sandbox también puedes previsualizar las animaciones que están incrustadas en el modelo; selecciona cuáles mostrar eligiéndolas de un menú desplegable.

![](/docs/_remote/34159024016bd56402873a74.webp)

## **Analizar glTF**

Una gran herramienta para analizar tus glTFs es usar [**glTF Sample Viewer**](https://github.khronos.org/glTF-Sample-Viewer-Release/). Esta herramienta te permite inspeccionar channels, textures, la reacción a distintos entornos de luz, previsualizar animaciones y mucho más. ¡Ideal para modelos individuales!

![](/docs/_remote/173cb3c41baf0f4c374aea20.webp)

Otra herramienta útil es [**gltf.report/**](https://gltf.report/). Solo arrastra y suelta tus modelos en el sitio web para ver el contenido del paquete glTF y evaluar qué partes del modelo se pueden optimizar.

Esta herramienta puede ser especialmente útil para archivos grandes que contienen varios modelos, textures y animaciones. Por ejemplo, al organizar las textures por tamaño, puedes identificar fácilmente qué textures están usando más recursos.

![](/docs/_remote/40647d97e437f4808c189320.webp)

## **Optimizar un glTF**

La siguiente herramienta ofrece algunas optimizaciones que harán que los modelos 3D sean más ligeros y se descarguen más rápido para los jugadores de tu escena.

[glTF pipeline](https://github.com/AnalyticalGraphicsInc/gltf-pipeline)

Entre otras cosas, convierte *.gltf* formato en *.glb*, que es binario y por eso ocupa mucho menos. También coloca los archivos de textures fuera del modelo 3D, lo que te permite usar la misma texture en varios modelos.


> [!WARNING]
> **📔 Nota**: *.glb* El formato por defecto siempre tiene las textures incrustadas en el archivo. El engine no puede reconocer dos textures incrustadas como la misma; necesitan ser archivos externos que compartan el mismo hash.


## Ver también

Las siguientes páginas también cubren temas relacionados con los modelos 3D para Decentraland:

* [Meshes](/docs/creator/content-creator-es/modelado-y-animaciones-3d/meshes/)
* [Materials](/docs/creator/content-creator-es/modelado-y-animaciones-3d/materials/)
* [Textures](/docs/creator/content-creator-es/modelado-y-animaciones-3d/textures/)
* [Colliders](/docs/creator/content-creator-es/modelado-y-animaciones-3d/colliders/)
* [Animations](/docs/creator/content-creator-es/modelado-y-animaciones-3d/animations/)
* [Recursos útiles](/docs/creator/content-creator-es/scenes-sdk7/primeros-pasos/useful-resources/): add-ons de Blender, bibliotecas de assets y herramientas de IA para acelerar tu flujo de trabajo 3D


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-es/modelado-y-animaciones-3d/3d-models.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
