> Migre uma scene legada do SDK6 para o SDK7 usando um assistente de IA.

# Migrar Scenes Legadas do SDK6

Se você tem uma cena antiga do SDK6 que gostaria de atualizar, pode migrá-la automaticamente para o SDK7 com a ajuda de um assistente de IA. O [Decentraland SDK Skills](/docs/creator/content-creator-pt/scenes-sdk7/primeiros-passos/vibe-coding/#install-skills-for-any-ai-agent) incluem um fluxo de trabalho de migração dedicado que a IA segue passo a passo, para que você não precise traduzir o código manualmente.

Este guia orienta você por todo o processo.

## 1. Crie uma nova cena vazia do SDK7

Comece com um projeto SDK7 novo e vazio. A IA moverá seu código e assets legados para essa estrutura limpa.

**Opção A — Creator Hub**

1. Abra o Creator Hub.
2. Selecione as **Cenas** aba e clique **Nova cena**.
3. Escolha o **Empty Scene** modelo.

**Opção B — Linha de comando**

Execute o seguinte numa pasta vazia:

```bash
npx @dcl/sdk-commands init
```

Isso preenche a pasta com os arquivos padrão de um projeto SDK7.

## 2. Instale as SDK Skills

As SDK Skills ensinam seu agente de IA a trabalhar com o Decentraland SDK7, incluindo como realizar uma migração adequada do SDK6 → SDK7.

Você pode instalá-las de duas maneiras.

**Opção A — Peça à IA para instalá-las**

Abra seu assistente de IA na pasta da cena e diga simplesmente:

> Instale as skills do Decentraland SDK.

A IA conhece o comando e as instalará para você.

**Opção B — Instalar pela linha de comando**

```bash
npx skills add decentraland/sdk-skills --all
```

A `--all` A flag instala todas as skills disponíveis. Se você a omitir, um seletor interativo será aberto para você escolher quais skills instalar.

Para mais detalhes sobre quais skills estão disponíveis e como elas funcionam, consulte [Vibe Coding com IA](/docs/creator/content-creator-pt/scenes-sdk7/primeiros-passos/vibe-coding/#install-skills-for-any-ai-agent).

## 3. Peça à IA para migrar a cena

Coloque os arquivos do seu projeto legado SDK6 em algum lugar que a IA possa lê-los — por exemplo, em uma pasta separada na sua máquina ou em um repositório público no GitHub.

Depois, peça à IA:

> Migre esta cena SDK6 para SDK7.

As skills incluem o roteiro completo de migração, então a IA irá:

* Identificar o ponto de entrada e os components usados na cena SDK6.
* Traduzir entities, components e systems para os seus equivalentes em SDK7.
* Mover modelos 3D, áudio e outros assets para o novo projeto.
* Atualize `scene.json` e a configuração do projeto.
* Substituir APIs obsoletas pelos seus equivalentes atuais no SDK7.

## 4. Teste e itere

Assim que a passagem de migração terminar, visualize a cena e teste-a do início ao fim.

* No Creator Hub, clique **Play**.
* Na linha de comando, execute `npm run start`.

Enquanto testa, procure:

* Problemas visuais — modelos ausentes, posições ou escalas incorretas, materiais quebrados.
* Problemas de interação — cliques que não disparam, triggers que não ativam, diálogos que não aparecem.
* Problemas de multiplayer ou networking, se a sua cena os usava.
* Erros de console na janela de preview.

Quando encontrar um bug, descreva-o à IA em linguagem simples:

> O botão vermelho na mesa não abre mais a porta. Antes ele reproduzia um som e girava a porta em 90 graus.

A IA pode usar as skills adjacentes (interatividade, animações, áudio etc.) para corrigir os problemas um a um. Itere até que a cena se comporte como a original.


> [!NOTE]
> **💡 Dica**— Corrija um bug de cada vez e faça um preview após cada alteração. É muito mais fácil detectar regressões quando as mudanças são pequenas.


## Próximos passos

* [Vibe Coding com IA](/docs/creator/content-creator-pt/scenes-sdk7/primeiros-passos/vibe-coding/) — Referência completa para trabalhar com assistentes de IA e SDK Skills.
* [Início rápido do SDK](/docs/creator/content-creator-pt/scenes-sdk7/primeiros-passos/sdk-101/) — Aprenda os fundamentos do SDK7 para que você possa revisar e refinar o código migrado.


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-pt/scenes-sdk7/outros/migrate-legacy-sdk6-scenes.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
