> Conteúdo da scene conduzido por uma Google Sheet, atualizado sem redeploy.

# Dados de uma planilha

Conduz o conteúdo da cena a partir de uma Google Sheet. Edite a planilha para alterar o nome do artista, o stream de vídeo e o wearable em destaque sem fazer redeploy da cena.

## Informações do projeto

**Mantido por:** Decentraland

[Ver código-fonte](https://github.com/decentraland/Read_Spreadsheet_Scene)

**Licença:** [Apache-2.0](https://github.com/decentraland/Read_Spreadsheet_Scene/blob/main/LICENSE). Verifique os termos upstream antes de reutilizar; o código e os assets podem ter termos diferentes.

## Explorar o código

Estes links apontam para um snapshot específico da source para que as explicações continuem úteis à medida que o desenvolvimento upstream avança. São referências de aprendizagem, não uma avaliação de segurança.

### Lê uma Google Sheet publicada como JSON

`getSpreadsheetData` solicita a planilha através do endpoint do Google Visualization (`gviz/tq?tqx=out:json`), remove o `setResponse(...)` wrapper, e transforma cada linha em um objeto indexado pelos cabeçalhos das colunas.

**Contexto do projeto:** A planilha deve ser compartilhada para que qualquer pessoa com o link possa vê-la. `gid` seleciona a aba e `headers=1` trata a primeira linha como nomes de colunas.

**Referências de origem:**

* [`getSpreadsheetData`](https://github.com/decentraland/Read_Spreadsheet_Scene/blob/57bb4086aed3054d69a74f33f20f2d86c4eb9bb6/src/scheduler.ts#L1-L47)

### Aplicar uma linha à cena

`updateFromSpreadsheet` lê uma linha e atualiza um banner `TextShape`, substitui a `VideoPlayer` source, troca o wearable exibido no dispenser e aponta a ação de clique do dispenser para o link de compra da linha.

**Contexto do projeto:** Nomes de colunas como `NowPlaying`, `VideoURL`, `WearableURN`e `BuyURL` devem corresponder aos cabeçalhos da planilha. As Entities são localizadas pelos nomes fornecidos na cena do Creator Hub.

**Referências de origem:**

* [`updateFromSpreadsheet`](https://github.com/decentraland/Read_Spreadsheet_Scene/blob/57bb4086aed3054d69a74f33f20f2d86c4eb9bb6/src/index.ts#L10-L64)
* [`createWearable`](https://github.com/decentraland/Read_Spreadsheet_Scene/blob/57bb4086aed3054d69a74f33f20f2d86c4eb9bb6/src/index.ts#L100-L130)

### Percorre as linhas a partir de um botão do Creator Hub

`main` ouve as `Next` e `Previous` ações no `Switch Artist` entity e faz o índice da linha voltar ao início dos dados baixados.

**Contexto do projeto:** Usa `getActionEvents` de `@dcl/asset-packs`, então as ações são configuradas na entity no Creator Hub.

**Referências de origem:**

* [`main`](https://github.com/decentraland/Read_Spreadsheet_Scene/blob/57bb4086aed3054d69a74f33f20f2d86c4eb9bb6/src/index.ts#L69-L97)


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-pt/tutoriais-e-exemplos/external-services/spreadsheet-data.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
