> Execute vários projetos DCL ao mesmo tempo

# Workspaces

Execute vários projetos do Decentraland em preview agrupando-os em um workspace. Execute várias scenes adjacentes para ver como se encaixam, ou também execute várias [smart wearables](/docs/creator/content-creator-pt/scenes-sdk7/tipos-de-projetos/smart-wearables/) juntos para ver como interagem entre si e com scenes diferentes.

Executar vários projetos em um workspace oferece uma alternativa de teste muito mais completa, para garantir que conteúdos diferentes funcionem bem juntos. Um workspace é um recurso de debugging; ele não afeta a experiência na scene publicada.


> [!WARNING]
> **📔 Nota**: O **Creator Hub** não oferece suporte atualmente para lidar com Workspaces.


## Criar um workspace

1. Baixe o [Goerli Plaza](https://github.com/decentraland/sdk7-goerli-plaza) repo.
2. Crie uma pasta separada de nível superior para conter o workspace.
3. A partir do repo Goerli Plaza, copie os seguintes arquivos para o seu workspace:
   * `dcl-workspace.json`
   * `package.json`
   * `.gitignore`
4. Dentro desta pasta, adicione uma pasta na raiz para cada projeto com o qual você quer trabalhar. Você pode arrastar pastas existentes com scenes ou smart wearables. Para novas pastas, execute `npx @dcl/sdk-commands init` dentro de cada uma, para criar um projeto do Decentraland.

   > Observação: certifique-se de que os parcels de cada uma das scenes não se sobreponham.
5. Na pasta raiz do workspace, execute o seguinte para criar os arquivos necessários:

   `npm run update-parcels && npm run sync && npm run test && npm run format`

Você pode confirmar quais projetos fazem parte do workspace abrindo o `dcl-workspace.json` arquivo e verificando os caminhos listados em suas `pastas` array.

## Executar um workspace

Execute `npm run start` na pasta raiz do workspace. Isso executa todos os projetos ao mesmo tempo, visíveis em uma única janela de preview. Este preview se comporta exatamente como ao fazer preview de uma única scene.

Qualquer smart wearable no workspace pode ser experimentado procurando-o na backpack.

## Adicionar projetos

Depois que um workspace é criado, você pode adicionar projetos adicionais editando o `dcl-workspace.json` arquivo manualmente. Adicione uma entrada ao `pastas` array com o caminho relativo para a pasta que você quer adicionar. Por exemplo `my-other-example`.

Você também pode adicionar um projeto que não esteja dentro da pasta do workspace, usando o caminho absoluto.


> [!WARNING]
> **📔 Nota**: A pasta já deve conter um projeto do Decentraland iniciado com `npx @dcl/sdk-commands init`. Não pode ser uma pasta vazia.


Para adicionar ou remover projetos, edite o `dcl-workspace.json` arquivo para incluir os caminhos relativos para cada um dos projetos no workspace no `pastas` array.

```json
{
	"folders": [
		{
			"path": "example-scene"
		},
		{
			"path": "example-scene2"
		}
	]
}
```


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-pt/scenes-sdk7/tipos-de-projetos/workspaces.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
