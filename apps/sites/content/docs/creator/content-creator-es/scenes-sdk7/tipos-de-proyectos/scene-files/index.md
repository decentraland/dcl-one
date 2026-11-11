> Archivos predeterminados creados en una escena nueva.

# Archivos de escena

Después de [crear una nueva escena](/docs/creator/content-creator-es/scenes-sdk7/primeros-pasos/sdk-101/), la carpeta de la escena tendrá una serie de archivos con contenido predeterminado.

## Archivos predeterminados en una escena

Las escenas incluyen los siguientes archivos:

* **src/index.ts**: El punto de entrada de la escena.
* **scene.json**: El manifiesto que contiene los metadatos de la escena.
* **package.json** y **package-lock.json**: Especifica las versiones de todas las dependencias de la escena.
* **tsconfig.json**: Archivo de configuración de TypeScript.
* **.dclignore**: Enumera los archivos de tu proyecto que no se subirán cuando publiques tu escena.
* **assets/scene/main.composite**: Archivo generado automáticamente que incluye todo lo que agregaste y configuraste visualmente en el Scene Editor en Creator Hub. No está pensado para ser legible por personas. Es un ejemplo de un [composite](/docs/creator/content-creator-es/scenes-sdk7/arquitectura/composites/), un archivo que describe un árbol de entities y components. Las escenas creadas mediante la CLI, en cambio, incluyen un `main.crdt` archivo en el nivel raíz.

### index.ts

Este es el punto de entrada al código personalizado de tu escena. Podrías incluir toda la lógica de tu escena en este archivo, aunque, por claridad, en la mayoría de los casos recomendamos distribuir tu código entre varios otros *.ts* archivos.

Si dependes solo del Scene Editor y [Smart Items](/docs/creator/content-creator-es/scene-editor/interactividad/smart-items/), no necesitarás modificar este archivo.

Si piensas escribir código personalizado, lo más probable es que solo necesites editar este y otros archivos .ts para crear tu escena. Contiene el código que genera los [entities, components](/docs/creator/content-creator-es/scenes-sdk7/arquitectura/entities-components/) y [systems](/docs/creator/content-creator-es/scenes-sdk7/arquitectura/systems/) de tu escena.

Al ejecutar la escena, el contenido de tus `.ts` archivos se compila en un único `.js` archivo, `bin/index.js`. Al compilar para producción, este archivo también se minimiza.


> [!WARNING]
> **📔 Nota**: Puedes usar otra herramienta o lenguaje en lugar de TypeScript, siempre que tus scripts estén contenidos dentro de un único archivo JavaScript (bin/index.js). Todas las declaraciones de tipos proporcionadas están hechas en TypeScript, y otros lenguajes y transpiladores no cuentan con soporte oficial.


### scene.json

El *scene.json* el archivo es un manifiesto con formato JSON para una escena en el mundo. Una escena puede abarcar un único parcel de LAND o varios. El *scene.json* manifiesto describe qué objetos existen en la escena, una lista de cualquier recurso necesario para renderizarla, la información de contacto del propietario del parcel y la configuración de seguridad.

La mayoría de los campos del *scene.json* archivo pueden editarse directamente en la UI del Scene Editor. Consulta [Scene Settings](/docs/creator/content-creator-es/scene-editor/primeros-pasos/scene-editor-essentials/#scene-settings).

![](/docs/_remote/0d1050cf72d4ecbad89ddda0.webp)

Para más información, consulta [metadatos de la escena](/docs/creator/content-creator-es/scenes-sdk7/tipos-de-proyectos/scene-metadata/#metadata).

Todos estos metadatos son opcionales para hacer preview de la escena localmente, pero una parte de ellos se necesita para desplegarla. Puedes cambiar esta información manualmente en cualquier momento.

### package.json

Este archivo proporciona información a NPM que le permite identificar el proyecto, así como gestionar las dependencias del proyecto. Las escenas de Decentraland requieren un paquete principal:

* **@dcl/sdk**: La dependencia fundamental del SDK de Decentraland, incluidas las definiciones y tipos para el engine, components, systems, etc.
* **@dcl/js-runtime**: Una serie de declaraciones de tipos que facilita la integración de @dcl/sdk.

Tu escena puede incluir cualquier cantidad de otros paquetes, por ejemplo para incluir [librerías](https://studios.decentraland.org/resources?sdk_version=SDK7&resource_type=Library) que pueden ayudar a facilitar la escritura de código, o habilitar funcionalidades especiales.

### package-lock.json

Este archivo enumera las versiones de todas las demás dependencias del proyecto. Estas versiones están bloqueadas, lo que significa que el compilador usará literalmente la misma versión menor que se indica aquí.

Puedes cambiar manualmente cualquier versión de paquete editando este archivo.

### tsconfig.json

Los directorios que contienen un *tsconfig.json* archivo son directorios raíz para proyectos de TypeScript. El *tsconfig.json* archivo especifica los archivos raíz y las opciones necesarias para compilar tu proyecto de TypeScript a JavaScript.

Al instalar cualquier librería adicional en tu escena, debería añadirse automáticamente una entrada a este archivo. Para instalar librerías utilitarias de Decentraland, no debería ser necesario hacer cambios manuales en este archivo.

## Ubicaciones de archivo recomendadas

Ten en cuenta que cuando despliegues tu escena en Decentraland, cualquier recurso o librería externa que se necesite para usar tu escena debe estar empaquetado dentro de la carpeta de la escena o disponible mediante un servidor remoto.

Todo lo que deba ejecutarse en el client del jugador debe estar ubicado dentro de la carpeta de la escena. No deberías hacer referencia a archivos o librerías instalados en otra parte de tu máquina local, porque no estarán disponibles para la escena desplegada.

Sugerimos usar estos nombres de carpetas de forma consistente para almacenar los distintos tipos de recursos que tu escena podría necesitar:

* Modelos 3D: `assets/scene/models`
* Videos: `assets/scene/videos`
* Archivos de sonido: `assets/scene/sounds`
* Archivos de imagen para texturas (excepto para modelos glTF): `assets/scene/materials`
* *.ts* definiciones de components `/src/components`
* *.ts* definiciones de systems `/src/systems`


> [!WARNING]
> **📔 Nota**: Archivos de soporte para modelos glTF, como sus archivos de imagen de textura o *.bin* archivos, siempre deben colocarse en la misma carpeta que el *.gltf* o *.glb* .



> [!WARNING]
> **📔 Nota**: Recomendamos usar siempre nombres en minúsculas para todas las carpetas y nombres de archivo, para evitar posibles problemas.


Al importar cualquier recurso mediante el Scene Editor, se añade automáticamente dentro de la `assets/scene` carpeta. Al usar cualquiera de los assets predeterminados en los Asset Packs del Scene Editor, sus archivos se añaden a la `assets/builder` carpeta.

## El archivo dclignore

Todas las escenas incluyen un *.dclignore* archivo, este archivo especifica qué archivos de la carpeta de la escena deben ignorarse al desplegar una escena en Decentraland.

Por ejemplo, podría interesarte conservar los archivos de Blender de los modelos 3D de tu escena dentro de la carpeta de la escena, pero quieres evitar que esos archivos se desplieguen en Decentraland. En ese caso, podrías agregar `*.blend` para *.dclignore* para ignorar todos los archivos con esa extensión.


> [!WARNING]
> **📔 Nota**: *.dclignore* también se aplica al preview local, no solo a los despliegues. Un archivo ignorado no aparece en el contenido que sirve el preview, así que tu escena tampoco puede cargarlo allí. No es un 404 que puedas detectar en los logs: el archivo simplemente nunca se solicita. Si un asset funciona en tu editor pero no en el preview, revisa *.dclignore* primero.


| Qué ignorar          | Ejemplo     | Descripción                                                                                                  |
| -------------------- | ----------- | ------------------------------------------------------------------------------------------------------------ |
| Archivos específicos | `BACKUP.ts` | Ignora un archivo específico                                                                                 |
| Carpetas             | `drafts/`   | Ignora todo el contenido de una carpeta y de sus subcarpetas                                                 |
| Extensiones          | `*.blend`   | Ignora todos los archivos con una extensión determinada                                                      |
| Secciones de nombre  | `test*`     | Ignora todos los archivos cuyos nombres coincidan con la consulta. En este caso, los que empiezan con *test* |


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-es/scenes-sdk7/tipos-de-proyectos/scene-files.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
