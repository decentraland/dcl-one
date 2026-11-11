# Creación de componentes

Para crear nuevos componentes, necesitamos hacer un par de cosas antes de empezar a programarlos en el renderer. Hay una guía paso a paso que debes seguir. Voy a enumerar todos los pasos, y luego puedes seguir cada paso con una explicación más larga.

1. Crea la definición del proto en [@dcl/protocol](https://github.com/decentraland/protocol)
2. Genera el nuevo código TypeScript del proto en el [js-sdk-toolchain](https://github.com/decentraland/js-sdk-toolchain)
3. Crea una prueba para `js-sdk-toolchain`
4. Genera el nuevo código C# del proto en el proyecto
5. Programa el nuevo componente
6. Asegúrate de que el componente siga la convención

## Crea la definición del proto en [@dcl/protocol](https://github.com/decentraland/protocol)

Para crear una definición, debes ir a este repo: <https://github.com/decentraland/protocol>

1. Crea la definición del proto en esta carpeta: <https://github.com/decentraland/protocol/tree/main/ecs/components>
2. Crea un PR con los nuevos cambios

> ***NOTA:*** Después de crear el PR, un GitHub Bot comentará con el enlace del paquete para probar el PR. Puedes usar ese enlace para probar en los siguientes pasos

Cosas a tener en cuenta

* Estamos usando proto 3, así que toda la definición del proto debe compilar con su [syntax](https://developers.google.com/protocol-buffers/docs/proto3/)
* Tenemos algunos tipos comunes que no se pueden recrear
* El proto debe tener la definición básica
* Debes agregar el siguiente código para enumerar el componente

```
import "common/id.proto";
option (ecs_component_id) = 1100;
```

Ejemplo de .proto

```
syntax = "proto3";

import "common/id.proto";
option (ecs_component_id) = 1020;

message PBAudioSource {
  optional bool playing = 1;
  optional float volume = 2; // default=1.0f
  optional bool loop = 3;
  optional float pitch = 4; // default=1.0f
  string audio_clip_url = 5;
}
```

## Genera el nuevo código TypeScript del proto en el [js-sdk-toolchain](https://github.com/decentraland/js-sdk-toolchain)

Descarga el siguiente repo: <https://github.com/decentraland/js-sdk-toolchain>

Dentro de él, navega hasta `packages/dcl/ecs` (`https://github.com/decentraland/js-sdk-toolchain/tree/main/packages/%40dcl/ecs`)

Y allí, puedes ejecutar

```
npm install @dcl/protocol@next
```

O el comando generado por el GitHub Bot en tu PR de @dcl/protocol (esto debe ser temporal para probar el PR).

Luego ejecuta los siguientes comandos en la raíz del `js-sdk-toolchain` proyecto:

```
make install
make build
```

Y sube el código generado.

## Genera el nuevo código C# del proto en el proyecto

Para generar el código C#, necesitamos ir a la ruta `protocol-gen` en la raíz del `@decentraland/unity-renderer` repositorio. Y ejecutar los siguientes comandos:

```
npm install
npm run build-components
```

Para actualizar a la última versión de `@dcl/protocol` (rama main), debemos actualizar usando

```
npm install @dcl/protocol@next
npm run build-components
```

Para probar un PR, podemos usar una URL generada por el GitHub Bot en el `@dcl/protocol` PR: Ejemplo:

```
npm install "https://sdk-team-cdn.decentraland.org/@dcl/protocol/branch//dcl-protocol-1.0.0-3143233696.commit-45f1290.tgz"
npm run build-components
```

Después de fusionar el PR de @dcl/protocol, debemos usar `@dcl/protocol@next` y generar el código.

## Programa el nuevo componente

Ahora es el momento de implementar la funcionalidad del nuevo componente.

Los componentes tienen cinco partes esenciales.

* ComponentID: el ID que usará el componente. Debe ser único y generarse a partir de la definición del proto
* Model Este es el modelo del componente. Estos son los datos que usaremos para manejar el componente. Se ha generado automáticamente con la generación del proto y tiene el mismo nombre del archivo proto con un PB delante. Por ejemplo, si tienes una `BoxShape.proto` definición, la clase generada del modelo será `PBBoxShape`
* Component Handler El component handler gestionará toda la funcionalidad del componente. En esta clase debes implementar `IECSComponentHandler<ModelClass>` (ModelClass es el modelo. Es una clase generada a partir del proto; el nombre será PB + el nombre del archivo .proto). Esta interfaz tiene 3 métodos que son importantes de implementar para crear un componente

```
        void OnComponentCreated(IParcelScene scene, IDCLEntity entity);
        void OnComponentRemoved(IParcelScene scene, IDCLEntity entity);
        void OnComponentModelUpdated(IParcelScene scene, IDCLEntity entity, ModelType model);
```

* Serializer Cada componente es responsable de implementar su serialización y deserialización. Este serializer debe poder serializar/deserializar a un byte array
* Register Esto registrará el componente en el sistema, conectándolo con el sistema. Este register registrará el componente en la factory y en el component writer

El diseño de los componentes busca evitar la herencia, así que recomendamos usar funciones puras tanto como sea posible

Para crearlos, debemos seguir los siguientes pasos

1. Crea la carpeta y el assembly del componente. Tenemos todos los componentes bajo la siguiente carpeta `DCLPlugins/ECS7/ECSComponents`. Necesitas crear una carpeta y un nuevo assembly que contendrá el componente
2. En el nuevo assembly, debes hacer referencia al siguiente `DCL.ECSComponents.Data`. Esto hará referencia al nuevo modelo del componente que acabas de actualizar
3. Debes crear el component handler con toda la lógica (mira `ECSBoxShapeComponentHandler.cs` como ejemplo)
4. Debes crear la clase serializer (probablemente puedas copiarla de otra clase y adaptarla a la tuya)
5. Debes crear la clase register

```sh
   public static class AudioSourceSerializer
    {
        public static byte[] Serialize(PBAudioSource model)
        {
            int size = model.CalculateSize();
            byte[] buffer = new byte[size];
            CodedOutputStream output = new CodedOutputStream(buffer);
            model.WriteTo(output);
            return buffer;
        }

        public static PBAudioSource Deserialize(object data)
        {
            return PBAudioSource.Parser.ParseFrom((byte[])data);
        }
    }
```

6. Agrega el nuevo register al `ECS7ComponentsComposer` class con su ID correspondiente

```sh
   public class ECS7ComponentsComposer : IDisposable
    {
        private readonly TransformRegister transformRegister;
        private readonly SphereShapeRegister sphereShapeRegister;
        private readonly BoxShapeRegister boxShapeRegister;
        private readonly PlaneShapeRegister planeShapeRegister;
        private readonly CylinderShapeRegister cylinderShapeRegister;
        private readonly AudioStreamRegister audioStreamRegister;
        private readonly AudioSourceRegister audioSourceRegister;

        public ECS7ComponentsComposer(ECSComponentsFactory componentsFactory, IECSComponentWriter componentsWriter)
        {
            transformRegister = new TransformRegister(ComponentID.TRANSFORM, componentsFactory, componentsWriter);
            sphereShapeRegister = new SphereShapeRegister(ComponentID.SPHERE_SHAPE, componentsFactory, componentsWriter);
            boxShapeRegister = new BoxShapeRegister(ComponentID.BOX_SHAPE, componentsFactory, componentsWriter);
            planeShapeRegister = new PlaneShapeRegister(ComponentID.PLANE_SHAPE, componentsFactory, componentsWriter);
            cylinderShapeRegister = new CylinderShapeRegister(ComponentID.CYLINDER_SHAPE, componentsFactory, componentsWriter);
            audioStreamRegister = new AudioStreamRegister(ComponentID.AUDIO_STREAM, componentsFactory, componentsWriter);
            audioSourceRegister = new AudioSourceRegister(ComponentID.AUDIO_SOURCE, componentsFactory, componentsWriter);
        }

        public void Dispose()
        {
            transformRegister.Dispose();
            sphereShapeRegister.Dispose();
            boxShapeRegister.Dispose();
            planeShapeRegister.Dispose();
            cylinderShapeRegister.Dispose();
            audioStreamRegister.Dispose();
            audioSourceRegister.Dispose();
        }
    }
```

¡Y ahora tienes tu componente agregado y funcionando!

## Asegúrate de que el componente siga la convención

Hay una lista de verificación que debemos tener en cuenta al desarrollar nuevos componentes; esta parte intenta resumirla.

* Pruebas unitarias: todos los componentes deben incluir pruebas unitarias que cubran su funcionalidad, dispose y la serialización/deserialización, al menos para asegurar que el componente funcionará
* Ten en cuenta qué sucede cuando el componente no está dentro de la escena (consulta `SceneBoundariesChecker` class para más información)
* Si el componente renderiza algo en el mundo, debe agregar la información renderizable al data store; de esta manera añadimos la información del renderer a la escena para que cuente dentro del límite
* Si el componente renderiza algo en el mundo, debe agregar `MeshesInfo` a la entity
* Debe ser lo más eficiente posible. Este código se ejecutará muchas veces, así que necesitamos asegurarnos de que todo funcione de la forma más fluida posible
* Debe funcionar con `Hot reload` en el modo preview. Si has programado `OnComponentRemoved` correctamente, esto funcionará de fábrica, pero el hot reload es una forma de comprobar que todo funciona bien con el dispose del componente
* Si el componente usa un recurso, debes implementar la gestión de recursos con un `AssetPromiseKeeper`. El componente debe notificar al `AssetPromiseKeeper` cuando el recurso se usa y cuando ya no se usa


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/contributor/contributor-es/tiempo-de-ejecucion-de-la-escena/componentes/component-creation.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
