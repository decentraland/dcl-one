> Como instalar o scene editor.

# Instalação do Creator Hub

![Creator Hub](/docs/_remote/9100f2b541affb1c6fecfe89.webp)

Transferir o Creator Hub [AQUI](https://decentraland.org/download/creator-hub).

## A atualizar o Creator Hub

A aplicação Creator Hub verifica se há atualizações sempre que a abres e atualiza-se automaticamente se houver uma nova versão disponível.

Também podes verificar manualmente se há atualizações. Para isso,

1. Clica no logótipo do Creator Hub no canto superior esquerdo do ecrã para abrir o menu principal.
2. Clique em **Verificar atualizações**.

![O menu principal do Creator Hub, aberto a partir do logótipo no canto superior esquerdo, com as opções Acerca do Creator Hub, Verificar atualizações, Definições, Reportar bug e Ajuda e suporte.](/docs/_remote/1394f3aa47147ab15685f4dd.webp)

O menu principal também é onde encontras a app **Definições**, e links para reportar um bug ou obter apoio.

## Editar código

Se também planeias ler e editar código na tua cena, também terás de instalar um dos seguintes:

* <img src="/docs/_remote/358795a1c08dfc479355a174.webp" alt="VS Code" data-size="line"> [Visual Studio Code](https://code.visualstudio.com/): Esta é a opção recomendada para developers experientes.
* <img src="/docs/_remote/9510a847d1493f0d8b4cf7e3.webp" alt="Cursor" data-size="line"> [Cursor AI](https://www.cursor.com/): Este é um editor de código poderoso integrado com IA. Permite-te escolher diferentes modelos de IA para te ajudar a escrever código; existe um nível gratuito, e os modelos mais avançados requerem um plano pago. Esta é uma boa opção para developers que são novos em Decentraland ou TypeScript, ou se quiseres poupar tempo a escrever código.

Talvez precises de selecionar o teu editor de código nas definições do Creator Hub. Para isso,

1. Clica no logótipo do Creator Hub no canto superior esquerdo do ecrã e seleciona **Definições**.
2. Abra o **EDITOR** aba.
3. Em **Editor de código preferido**, seleciona o teu editor de código. Podes encontrar o teu editor listado no menu suspenso, ou talvez precises de selecionar **Escolher do teu dispositivo...** para o encontrar.

![A janela de Preferências da aplicação no separador EDITOR, mostrando o menu suspenso Editor de código preferido definido como Visual Studio Code, as caixas de verificação das Opções de Play e a caixa de verificação Avisos da aplicação.](/docs/_remote/be0b7d493dc2e145af5207ac.webp)

### Competências de IA

Se planeias usar um agente de IA para te ajudar a escrever código, recomendamos instalar as competências do Decentraland SDK. São conjuntos de instruções prontos a usar que ensinam o teu agente de IA a trabalhar com o Decentraland SDK, para que cometa menos erros e dê melhores resultados desde o primeiro prompt. Para instalar todas, executa o seguinte comando na pasta do projeto da tua cena:

```bash
npx skills add decentraland/sdk-skills --all
```

Veja [Vibe coding](/docs/creator/content-creator-pt/scenes-sdk7/primeiros-passos/vibe-coding/) para mais detalhes sobre instalar skills específicas e como usá-las.

## Solução de problemas

Se você encontrar problemas, veja a [solução de problemas](/docs/creator/content-creator-pt/scenes-sdk7/depuracao/troubleshooting/) seção.


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-pt/scene-editor/primeiros-passos/editor-installation.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
