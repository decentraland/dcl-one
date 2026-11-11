> Migre sua scene do Web Editor para o Creator Hub.

# Migrar para o Creator Hub

Se você tiver uma scene criada com outras ferramentas além do Creator Hub, você pode migrá-la facilmente para o Creator Hub.

O Creator Hub é a ferramenta recomendada para criar scenes no Decentraland. Ele tem uma interface muito mais refinada do que o Web Editor e permite combinar a facilidade da interface de arrastar e soltar com a possibilidade de personalizar ainda mais com código. Ele também permite executar a preview da sua scene usando o cliente desktop mais recente.

## Migrar do Web Editor

Para editar o código em uma scene criada com o Web Editor, você deve exportar a scene para sua máquina e abri-la com o Creator Hub.


> [!WARNING]
> **📔 Nota**: Se você não tiver o Creator Hub instalado, siga os passos na página a seguir antes de começar.
> 
> [Instalar Creator Hub](/docs/creator/content-creator-pt/scene-editor/primeiros-passos/editor-installation/)


1. Clique nos **Ícone de download** no menu superior do Web Editor enquanto edita a scene.

![](/docs/_remote/013ac41c9f6c08ef06e89d47.webp)

2. Isso fará o download de um *.zip* file, extraia-o.
3. Abra o **Creator Hub**, vá para o **Cenas** .
4. Clique nos **Import Scene** button e selecione o caminho para a folder do seu projeto exportado.

![](/docs/_remote/7cb4f59604e7cd5cbb0a15bd.webp)

Quando terminar, você pode continuar trabalhando no seu projeto dentro do Creator Hub, com uma interface visual que se parece bastante com o Web Editor, mas muito mais refinada.

Você também pode editar os files sob a `/src` folder para adicionar comportamento com código à sua scene. Veja [Combine com código](/docs/creator/content-creator-pt/scene-editor/expandir-com-codigo/overview/) sobre como editar o código da sua cena.

## Migrar um projeto apenas de code

Você pode importar qualquer projeto apenas de code para o Creator Hub. Para isso,

1. Abra o Creator Hub, vá para a **Cenas** .
2. Clique nos **Import Scene** button e selecione o caminho para a folder do seu projeto exportado.

![](/docs/_remote/7cb4f59604e7cd5cbb0a15bd.webp)

Quando terminar, você pode começar a trabalhar no seu projeto dentro do Creator Hub; isso não impede que você continue usando seu editor de código favorito para editar o código da sua scene, ou usar a linha de comando para executar ou fazer deploy da sua scene.

Após importar seu projeto, qualquer conteúdo criado via código não ficará visível ou editável no canvas do Creator Hub, o que pode dificultar posicionar e alinhar novos itens. Inicialmente, você verá sua scene como uma grade vazia.

![](/docs/_remote/3b0a67be118d41cad070cf4b.webp)

Em vez de adicionar manualmente seu conteúdo ao canvas do zero, você pode executar um comando para adicioná-lo automaticamente para você. Para isso, certifique-se de ter a versão mais recente do SDK instalada e execute o seguinte comando no seu terminal:

```
npx sdk-commands code-to-composite
```


> [!CAUTION]
> **❗Aviso**: Certifique-se de ter um backup do seu projeto antes de executar este comando.
> 
> Este comando substituirá o `main.composite` file com o novo snapshot. Ele também comentará todo o código nos `.ts` files in the `src` folder. Você precisará descomentar o código para fazê-lo funcionar novamente.


Este comando executa sua scene e tira um snapshot do conteúdo criado via código no primeiro frame. Esse snapshot é salvo no `main.composite` file, que o Creator Hub usa para exibir o conteúdo da sua scene. O código na sua scene é comentado, para evitar duplicatas de todas as entities.

Observe que este comando captura apenas entities e os components que podem ser representados na UI do Creator Hub. Ele não replica components personalizados, nem reproduz código que executa lógica, nem elementos de UI criados via código. Para adicionar novamente qualquer comportamento que foi comentado, você precisará editar o código no `.ts` files in the `src` folder e descomentar as linhas de que você precisa.

Você também pode querer reescrever parte do código para que, em vez de criar novas entities, ele referencie entities existentes pelo nome ou por tags para lhes dar comportamento. Veja [Combine com código](/docs/creator/content-creator-pt/scene-editor/expandir-com-codigo/overview/) para ver como buscar essas entities a partir do seu código.


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-pt/scene-editor/primeiros-passos/migrate-from-web.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
