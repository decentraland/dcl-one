# Criação de Components

Para criar novos components, precisamos fazer algumas coisas antes de começar a codificá-los no renderer. Há um guia passo a passo que você precisa seguir. Vou listar todas as etapas e, depois, você pode seguir cada etapa com uma explicação mais detalhada.

1. Criar a definição proto em [@dcl/protocol](https://github.com/decentraland/protocol)
2. Gerar o novo código TypeScript do proto em [js-sdk-toolchain](https://github.com/decentraland/js-sdk-toolchain)
3. Criar um teste para `js-sdk-toolchain`
4. Gerar o novo código C# do proto no projeto
5. Codificar o novo component
6. Garantir que o component siga a convenção

## Criar a definição proto em [@dcl/protocol](https://github.com/decentraland/protocol)

Para criar uma definição, você deve ir para este repo: <https://github.com/decentraland/protocol>

1. Crie a definição proto nesta pasta: <https://github.com/decentraland/protocol/tree/main/ecs/components>
2. Criar um PR com as novas alterações

> ***OBSERVAÇÃO:*** Depois de criar o PR, um GitHub Bot comentará com o link do package para testar o PR. Você pode usar esse link para testar nas etapas a seguir

Aspectos a ter em conta

* Estamos usando proto 3, então toda a definição do proto deve compilar com sua [sintaxe](https://developers.google.com/protocol-buffers/docs/proto3/)
* Temos alguns tipos comuns que não podem ser recriados
* O proto deve ter a definição básica
* Você deve adicionar o seguinte código para enumerar o component

```
import "common/id.proto";
option (ecs_component_id) = 1100;
```

Exemplo de .proto

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

## Gerar o novo código TypeScript do proto em [js-sdk-toolchain](https://github.com/decentraland/js-sdk-toolchain)

Baixe o seguinte repo: <https://github.com/decentraland/js-sdk-toolchain>

Dentro dele, navegue até `packages/dcl/ecs` (`https://github.com/decentraland/js-sdk-toolchain/tree/main/packages/%40dcl/ecs`)

E, ali, você pode executar

```
npm install @dcl/protocol@next
```

Ou o comando gerado pelo GitHub Bot no seu PR de @dcl/protocol (isso deve ser temporário para testar os PRs).

Depois, execute os seguintes comandos na raiz do `js-sdk-toolchain` projeto:

```
make install
make build
```

E envie o código gerado.

## Gerar o novo código C# do proto no projeto

Para gerar o código C#, precisamos ir para o `protocol-gen` caminho na raiz do `@decentraland/unity-renderer` repositório. E execute os seguintes comandos:

```
npm install
npm run build-components
```

Para atualizar para a versão mais recente do `@dcl/protocol` (branch main), devemos atualizar usando

```
npm install @dcl/protocol@next
npm run build-components
```

Para testar um PR, podemos usar uma URL gerada pelo GitHub Bot no `@dcl/protocol` PR: Exemplo:

```
npm install "https://sdk-team-cdn.decentraland.org/@dcl/protocol/branch//dcl-protocol-1.0.0-3143233696.commit-45f1290.tgz"
npm run build-components
```

Depois de mesclar o PR de @dcl/protocol, devemos usar o `@dcl/protocol@next` e gerar o código.

## Codificar o novo component

Agora é hora de implementar a funcionalidade do novo component.

Os components têm cinco partes essenciais.

* ComponentID O ID que o component usará. Ele deve ser único e gerado a partir da definição proto
* Model Este é o model do component. Estes são os dados que usaremos para lidar com o component. Ele foi gerado automaticamente com a geração do proto e tem o mesmo nome do arquivo proto com PB na frente. Por exemplo, se você tiver uma `BoxShape.proto` definição, a classe gerada do model será `PBBoxShape`
* Component Handler O component handler vai gerenciar toda a funcionalidade do component. Nesta classe você deve implementar o `IECSComponentHandler<ModelClass>` (ModelClass é o model. É uma classe gerada a partir do proto, o nome será PB + nome do arquivo .proto). Esta interface tem 3 métodos importantes de implementar para criar um component

```
        void OnComponentCreated(IParcelScene scene, IDCLEntity entity);
        void OnComponentRemoved(IParcelScene scene, IDCLEntity entity);
        void OnComponentModelUpdated(IParcelScene scene, IDCLEntity entity, ModelType model);
```

* Serializer Cada component é responsável por implementar sua serialização e desserialização do component. Este serializer deve ser capaz de serializar/desserializar em byte array
* Register Isto registrará o component no system, conectando-o ao system. Este register registrará o component na factory e no component writer

O design dos components é evitar herança, então incentivamos usar funções puras sempre que possível

Para criá-los, devemos seguir os próximos passos

1. Crie a pasta e a assembly do component. Temos todos os components sob a seguinte pasta `DCLPlugins/ECS7/ECSComponents`. Você precisa criar uma pasta e uma nova assembly que conterá o component
2. Na nova assembly, você deve referenciar a seguinte `DCL.ECSComponents.Data`. Isso fará referência ao novo model do component que você acabou de atualizar
3. Você deve criar o component handler com toda a lógica (Dê uma olhada em `ECSBoxShapeComponentHandler.cs` como exemplo)
4. Você deve criar a classe serializer (provavelmente você pode copiá-la de outra classe e adaptá-la ao seu)
5. Você deve criar a classe register

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

6. Adicione o novo register ao `ECS7ComponentsComposer` class com seu ID correspondente

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

E agora você tem seu component adicionado e funcionando!

## Garantir que o component siga a convenção

Há um checklist que precisamos ter em conta durante o desenvolvimento de novos components; esta parte tenta resumi-lo.

* Teste unitário Todos os components devem incluir testes unitários que cubram sua funcionalidade, dispose e serialização/desserialização, no mínimo, para garantir que o component funcione
* Leve em conta o que acontece quando o component não está dentro da scene (Confira `SceneBoundariesChecker` class para mais informações)
* Se o component renderiza algo no world, ele deve adicionar as informações renderizáveis ao data store; assim, adicionamos as informações do renderer à scene para que elas contem para o limite
* Se o component renderiza algo no world, ele deve adicionar o `MeshesInfo` ao entity
* Ele deve ser o mais performant possível. Este código será executado muitas vezes, então precisamos garantir que tudo funcione da forma mais suave possível
* Ele deve funcionar com `Hot reload` no modo Preview. Se você tiver codificado o `OnComponentRemoved` corretamente, isso funcionará imediatamente, mas o hot reload é uma forma de testar se tudo funciona bem com o dispose do component
* Se o component usar um recurso, você deve implementar o gerenciamento de recurso com um `AssetPromiseKeeper`. O component deve notificar o `AssetPromiseKeeper` quando o recurso é usado e quando ele não é mais usado


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/contributor/contributor-pt/runtime-da-scene/components/component-creation.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
