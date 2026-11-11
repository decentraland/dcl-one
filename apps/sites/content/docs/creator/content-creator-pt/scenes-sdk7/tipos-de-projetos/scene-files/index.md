> Arquivos padrão criados em uma nova scene.

# Arquivos da Scene

Depois de [criar uma nova scene](/docs/creator/content-creator-pt/scenes-sdk7/primeiros-passos/sdk-101/), a pasta da scene terá uma série de arquivos com conteúdo padrão.

## Arquivos padrão em uma scene

As scenes incluem os seguintes arquivos:

* **src/index.ts**: O ponto de entrada da scene.
* **scene.json**: O manifesto que contém metadados da scene.
* **package.json** e **package-lock.json**: Especifica as versões de todas as dependências da scene.
* **tsconfig.json**: Arquivo de configuração do TypeScript.
* **.dclignore**: Lista os arquivos do seu projeto que não serão enviados quando você publicar sua scene.
* **assets/scene/main.composite**: Arquivo gerado automaticamente que inclui tudo o que você adicionou e configurou visualmente no Scene Editor no Creator Hub. Não foi feito para ser legível por humanos. É um exemplo de um [composite](/docs/creator/content-creator-pt/scenes-sdk7/arquitetura/composites/), um arquivo que descreve uma árvore de entities e components. Scenes criadas via CLI, em vez disso, incluem um `main.crdt` arquivo na raiz.

### index.ts

Este é o ponto de entrada para o código personalizado da sua scene. Você poderia encaixar toda a lógica da sua scene neste arquivo, embora, para maior clareza, na maioria dos casos recomendemos distribuir seu código por vários outros *.ts* arquivos.

Se você depender apenas do Scene Editor e dos [Smart Items](/docs/creator/content-creator-pt/scene-editor/interatividade/smart-items/), você não precisará modificar este arquivo.

Se você pretende escrever código personalizado, provavelmente só precisará editar este e outros arquivos .ts para criar sua scene. Ele contém o código que gera os [entities, components](/docs/creator/content-creator-pt/scenes-sdk7/arquitetura/entities-components/) e [systems](/docs/creator/content-creator-pt/scenes-sdk7/arquitetura/systems/) da sua scene.

Ao executar a scene, o conteúdo dos seus `.ts` arquivos é compilado em um único `.js` arquivo, `bin/index.js`. Ao compilar para produção, este arquivo também é minificado.


> [!WARNING]
> **📔 Nota**: Você pode usar outra ferramenta ou linguagem em vez de TypeScript, desde que seus scripts estejam contidos em um único arquivo JavaScript (bin/index.js). Todas as declarações de tipo fornecidas são feitas em TypeScript, e outras linguagens e transpilers não são oficialmente suportados.


### scene.json

O *scene.json* arquivo é um manifesto formatado em JSON para uma scene no mundo. Uma scene pode abranger um único ou vários parcels de LAND. O *scene.json* manifesto descreve quais objetos existem na scene, uma lista de quaisquer assets necessários para renderizá-la, informações de contato do proprietário do parcel e configurações de segurança.

A maioria dos campos no *scene.json* arquivo pode ser editada diretamente na UI do Scene Editor. Veja [Scene Settings](/docs/creator/content-creator-pt/scene-editor/primeiros-passos/scene-editor-essentials/#scene-settings).

![](/docs/_remote/0d1050cf72d4ecbad89ddda0.webp)

Para mais informações, veja [metadados da scene](/docs/creator/content-creator-pt/scenes-sdk7/tipos-de-projetos/scene-metadata/#metadata).

Todos esses metadados são opcionais para visualizar a scene localmente, mas parte deles é necessária para fazer deploy. Você pode alterar essas informações manualmente a qualquer momento.

### package.json

Este arquivo fornece informações ao NPM que permitem identificar o projeto, bem como lidar com as dependências do projeto. As scenes do Decentraland exigem um pacote principal:

* **@dcl/sdk**: A dependência fundamental do Decentraland SDK, incluindo definições e tipos para o engine, components, systems etc.
* **@dcl/js-runtime**: Uma série de declarações de tipo que torna a integração de @dcl/sdk mais fluida.

Sua scene pode incluir qualquer número de outros pacotes, por exemplo para incluir [libraries](https://studios.decentraland.org/resources?sdk_version=SDK7&resource_type=Library) que podem ajudar a facilitar a escrita de código ou habilitar funcionalidades especiais.

### package-lock.json

Este arquivo lista as versões de todas as outras dependências do projeto. Essas versões ficam travadas, o que significa que o compilador usará literalmente a mesma versão minor listada aqui.

Você pode alterar manualmente a versão de qualquer pacote editando este arquivo.

### tsconfig.json

Diretórios que contêm um *tsconfig.json* arquivo são diretórios raiz para projetos TypeScript. O *tsconfig.json* arquivo especifica os arquivos raiz e as opções necessárias para compilar seu projeto de TypeScript para JavaScript.

Ao instalar quaisquer bibliotecas adicionais na sua scene, uma entrada deve ser adicionada automaticamente a este arquivo. Para instalar bibliotecas utilitárias do Decentraland, não deveria ser necessário fazer nenhuma alteração manual neste arquivo.

## Locais de arquivo recomendados

Lembre-se de que, quando você fizer deploy da sua scene no Decentraland, quaisquer assets ou bibliotecas externas necessárias para usar sua scene devem estar empacotados dentro da pasta da scene ou disponíveis por meio de um servidor remoto.

Tudo o que se destina a rodar no client do jogador deve estar localizado dentro da pasta da scene. Você não deve referenciar arquivos ou bibliotecas instalados em outro lugar na sua máquina local, porque eles não estarão disponíveis para a scene implantada.

Sugerimos usar estes nomes de pasta de forma consistente para armazenar os diferentes tipos de assets que sua scene pode precisar:

* Modelos 3D: `assets/scene/models`
* Vídeos: `assets/scene/videos`
* Arquivos de som: `assets/scene/sounds`
* Arquivos de imagem para textures (exceto para modelos glTF): `assets/scene/materials`
* *.ts* definições para components `/src/components`
* *.ts* definições para systems `/src/systems`


> [!WARNING]
> **📔 Nota**: Arquivos de suporte para modelos glTF, como seus arquivos de imagem de texture ou *.bin* arquivos, devem sempre ser colocados na mesma pasta que o *.gltf* ou *.glb* File.



> [!WARNING]
> **📔 Nota**: Recomendamos usar sempre nomes em minúsculas para todas as pastas e nomes de arquivo, para evitar possíveis problemas.


Ao importar quaisquer assets via o Scene Editor, eles são adicionados automaticamente dentro da `assets/scene` pasta. Ao usar quaisquer assets padrão nos Asset Packs do Scene Editor, seus arquivos são adicionados à `assets/builder` pasta.

## O arquivo dclignore

Todas as scenes incluem um *.dclignore* arquivo, este arquivo especifica quais arquivos na pasta da scene devem ser ignorados ao fazer deploy de uma scene no Decentraland.

Por exemplo, você pode querer manter os arquivos do Blender para os modelos 3D da sua scene dentro da pasta da scene, mas quer impedir que esses arquivos sejam enviados para o Decentraland. Nesse caso, você poderia adicionar `*.blend` a *.dclignore* para ignorar todos os arquivos com essa extensão.


> [!WARNING]
> **📔 Nota**: *.dclignore* também se aplica ao preview local, não apenas aos deploys. Um arquivo ignorado não é listado no conteúdo que o preview serve, então sua scene também não consegue carregá-lo lá. Não é um 404 que você possa ver nos logs: o arquivo simplesmente nunca é solicitado. Se um asset funciona no seu editor, mas não no preview, verifique *.dclignore* primeiro.


| O que ignorar        | Exemplo     | Descrição                                                                                          |
| -------------------- | ----------- | -------------------------------------------------------------------------------------------------- |
| Arquivos específicos | `BACKUP.ts` | Ignora um arquivo específico                                                                       |
| Pastas               | `drafts/`   | Ignora todo o conteúdo de uma pasta e de suas subpastas                                            |
| Extensões            | `*.blend`   | Ignora todos os arquivos com uma determinada extensão                                              |
| Seções de nome       | `test*`     | Ignora todos os arquivos com nomes que correspondem à consulta. Neste caso, que começam com *test* |


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-pt/scenes-sdk7/tipos-de-projetos/scene-files.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
