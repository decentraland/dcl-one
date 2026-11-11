> Use a CLI do Decentraland para executar e publicar scenes. O Creator Hub é a ferramenta recomendada para a maioria dos criadores; a CLI é adequada para fluxos de trabalho avançados e automatizados.

# Usando a CLI


> [!NOTE]
> **💡 Dica**: Se você estiver trabalhando com um assistente de codificação de IA (como Claude Code, Cursor ou Copilot), instale os Skills oficiais do Decentraland SDK antes que ele escreva qualquer código, para que ele conheça padrões verificados do SDK7:
> 
> ```bash
> npx skills add decentraland/sdk-skills
> ```
> 
> Veja [Vibe Coding com IA](/docs/creator/content-creator-pt/scenes-sdk7/primeiros-passos/vibe-coding/) para o fluxo de trabalho completo assistido por IA.


Para criar cenas no Decentraland, você pode usar:

* O [Creator Hub](/docs/creator/content-creator-pt/scene-editor/primeiros-passos/editor-installation/)
* A Interface de Linha de Comando (CLI)

Ambas as ferramentas permitem compilar e visualizar sua cena em um ambiente de desenvolvimento "off-chain". Depois de testar sua cena localmente, você pode enviar seu conteúdo para o servidor de conteúdo, vinculando-o ao seu LAND ou WORLD.

Embora o Scene Editor no Creator Hub seja mais fácil de usar, a CLI oferece mais flexibilidade e pode ser facilmente usada em processos automatizados.


> [!WARNING]
> **📔 Nota**: O Scene Editor executa as mesmas operações de linha de comando por trás dos panos.



> [!NOTE]
> **💡 Dica**: Veja [Guia de instalação](/docs/creator/content-creator-pt/scene-editor/primeiros-passos/editor-installation/) para instruções sobre como instalar o Creator Hub.


## Antes de começar

Para lidar com a cena pela linha de comando, instale as seguintes dependências antes de executar comandos da CLI com a cena:

* [Node.js](https://nodejs.org) (versão 20 ou posterior)

## Inicie um novo projeto

Execute `npx @dcl/sdk-commands init` em uma pasta vazia para preenchê-la com os arquivos padrão de uma [cena](/docs/creator/content-creator-pt/scenes-sdk7/tipos-de-projetos/scene-metadata/) projeto. Você também pode criar uma nova cena com o **New Scene** botão no Creator Hub, que produz a mesma estrutura de projeto.

Para começar a partir de outro tipo de projeto, use a `--project` flag. Por exemplo, para criar um projeto de [wearable inteligente](/docs/creator/content-creator-pt/scenes-sdk7/tipos-de-projetos/smart-wearables/) projeto:

```bash
npx @dcl/sdk-commands init --project smart-wearable
```

As opções disponíveis para `--project` são `scene-template` (o padrão), `px-template`, `smart-wearable`, e `library`.

## Atualize a versão do SDK de uma cena

Execute o seguinte comando na pasta da cena:

```bash
npm i @dcl/sdk@latest
```

Você pode confirmar que funcionou verificando o arquivo `package.json` da cena e procurando a versão `@dcl/sdk` ali.

## Executar uma prévia

Execute `npm run start` na raiz de um projeto de cena, workspace ou smart wearable para abrir uma prévia no cliente Decentraland Desktop.

```bash
npm run start
```

Para visualizar sua cena no [aplicativo móvel do Decentraland](/docs/creator/content-creator-pt/criar-para-mobile/cliente-movel/overview/) em vez disso, execute `npm run start -- --mobile` (alias `-- -m`). A CLI imprime um código QR que abre a cena em um telefone conectado à mesma rede Wi-Fi que seu computador. Veja [Prévia no mobile](/docs/creator/content-creator-pt/criar-para-mobile/desenvolver/preview-on-mobile/) para o guia completo.

```bash
npm run start -- --mobile
```

Para fazer a prévia rodar de forma mais suave e renderizar modelos 3D exatamente como ficarão depois de publicados, adicione a flag `--asset-bundles` . Veja [Prévia com assets otimizados](/docs/creator/content-creator-pt/scenes-sdk7/primeiros-passos/preview-scene/#preview-with-optimized-assets).

```bash
npm run start -- --asset-bundles
```

Veja [pré-visualizar cenas](/docs/creator/content-creator-pt/scenes-sdk7/primeiros-passos/preview-scene/) para detalhes e opções especiais ao executar uma prévia.

## Compilar

Execute `npm run build` para compilar seu projeto. As cenas do Decentraland são escritas em TypeScript, mas são compiladas para JavaScript minificado quando publicadas. Veja [codificação de cenas](https://github.com/decentraland/docs/blob/main/creator/sdk7/getting-started/coding-scenes.md) para mais detalhes.

O comando build é opcional, pois ele também é executado em segundo plano antes do deploy (embora você possa adicionar uma flag para ignorá-lo).

O comando build executa verificações de tipo mais rigorosas do que aquelas executadas com `npm run start`, e executá-lo pode às vezes ser útil para depurar uma cena.

## Implantar uma cena

Execute `npm run deploy` para publicar sua cena no Decentraland. Este comando abre uma janela do navegador onde você pode assinar com sua Wallet para autorizar o deploy.

Veja [publicação](/docs/creator/content-creator-pt/scenes-sdk7/publicacao/publishing/) para detalhes e opções especiais ao publicar uma cena.

## Solução de problemas

Se você encontrar problemas, veja a [solução de problemas](/docs/creator/content-creator-pt/scenes-sdk7/depuracao/troubleshooting/) seção.


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-pt/scenes-sdk7/primeiros-passos/using-the-cli.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
