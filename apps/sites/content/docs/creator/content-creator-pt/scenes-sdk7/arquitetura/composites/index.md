> Spawn de uma árvore de entities e components em runtime a partir de um arquivo composto

# Compostos

Um *composite* é um arquivo que descreve uma árvore de [entities e components](/docs/creator/content-creator-pt/scenes-sdk7/arquitetura/entities-components/). Em vez de escrever código para criar cada entity uma por uma, você pode armazenar essa estrutura em um arquivo composite e gerar tudo com uma única chamada.

Talvez você já trabalhe com composites sem perceber:

* Toda scene tem um `main.composite` arquivo. Ele contém tudo o que você adicionou e configurou visualmente no [Scene Editor](/docs/creator/content-creator-pt/scene-editor/primeiros-passos/about-editor/). Este arquivo é carregado automaticamente quando sua scene começa.
* [Itens Personalizados](/docs/creator/content-creator-pt/scene-editor/interatividade/custom-items/) são salvos como `composite.json` files. Cada Custom Item é um composite que você pode reutilizar entre scenes.

Esta página cobre como gerar um composite a partir de código em tempo de execução, por exemplo, para gerar um Custom Item dinamicamente como parte da lógica da sua scene.

## Gerar um composite

Gerar um composite envolve dois passos: **carregar** o arquivo composite, depois **gerar** ele. O carregamento é assíncrono e a geração é síncrona, então o padrão é sempre carregar e depois gerar:

```ts
import { engine, Composite, getCompositeProvider } from "@dcl/sdk/ecs";

export async function spawnBarrel() {
  const src = "barrel.composite";
  const provider = getCompositeProvider();
  if (!provider || !provider.loadComposite) return;

  // 1. Carregue o composite do seu arquivo
  const resource = await provider.loadComposite(src);

  // 2. Gere-o: cria todas as suas entities e components
  const barrel = Composite.instance(engine, resource, provider);
  return barrel;
}
```

`provider.loadComposite(src)` lê o arquivo composite e o carrega na memória. Em uma scene normal construída com `@dcl/sdk`, um composite provider já está configurado para você, e você o acessa com a `getCompositeProvider()` função de `@dcl/sdk/ecs`.

`Composite.instance()` então cria todas as entities descritas no composite, com todos os seus components, e retorna a **entidade raiz** da árvore gerada. Use esta entity retornada para ler ou alterar components mais tarde, por exemplo, para reposicionar ou remover o item gerado.


> [!NOTE]
> **💡 Dica**: Para fazer a mesma coisa sem escrever código, use o **Spawn Entity** ação no Scene Editor. Veja [Sobre a geração de entities](/docs/creator/content-creator-pt/scene-editor/interatividade/smart-items-advanced/#about-spawning-entities).



> [!WARNING]
> **📔 Nota**: Você deve carregar um composite antes de gerá-lo. `Composite.instance()` é síncrono e só pode gerar um composite que já esteja na memória. Apenas `main.composite` vem junto com sua scene e fica disponível desde o início; qualquer outro arquivo composite precisa ser carregado com `loadComposite()` primeiro. Para verificar de forma síncrona se um composite já está disponível, use `provider.getCompositeOrNull(src)`.
> 
> `loadComposite()` é idempotente: ele usa cada composite como chave pelo seu `src` string, então chamá-lo novamente com o mesmo path não recarrega o arquivo, ele retorna o composite já carregado. Você pode chamá-lo com segurança antes de cada geração sem se preocupar em carregar o mesmo arquivo duas vezes.


### Gerar em uma entity existente

Por padrão `Composite.instance()` cria uma nova entity para conter a árvore gerada. Passe uma `rootEntity` nas opções para gerar em uma entity que você já tem:

```ts
// Gere o composite diretamente na raiz da scene, sem entity contêiner
const barrel = Composite.instance(engine, resource, provider, {
  rootEntity: engine.RootEntity,
})
```

## Posicionar um composite gerado

Para colocar o composite gerado em uma posição, rotação ou escala específica, defina um `Transform` component na entity raiz retornada por `Composite.instance()`.

```ts
import { engine, Composite, getCompositeProvider, Transform } from "@dcl/sdk/ecs";
import { Vector3 } from "@dcl/sdk/math";

export async function spawnBarrel() {
  const src = "barrel.composite";
  const provider = getCompositeProvider();
  if (!provider || !provider.loadComposite) return;

  const resource = await provider.loadComposite(src);

  // Gere o composite em uma posição específica
  const barrel = Composite.instance(engine, resource, provider);
  Transform.createOrReplace(barrel, {
    position: Vector3.create(8, 0, 8),
  });
}
```


> [!WARNING]
> **📔 Nota**: `Transform.createOrReplace()` substitui o componente Transform existente da entity raiz. Apenas a entity raiz é afetada; as entities filhas mantêm suas posições relativas à raiz.


## Limitação atual: composites aninhados

A geração funciona para composites autocontidos e Custom Items. Um composite pode referenciar outro composite a partir de uma de suas entities, mas `loadComposite()` não **não** faz recursão nessas referências aninhadas: ele carrega apenas o arquivo que você passar.

Ao gerar, `Composite.instance()` faz recursão em referências aninhadas que já estão na memória. Se um composite aninhado não estiver carregado (qualquer coisa diferente de `main.composite`, o único composite incluído com sua scene), esse ramo registra um aviso e é ignorado. Para gerar um composite com referências aninhadas, chame `loadComposite()` para cada arquivo referenciado primeiro — ou, de forma mais simples, mantenha os composites que você gera autocontidos.

## Páginas relacionadas

* [Entities & Components](/docs/creator/content-creator-pt/scenes-sdk7/arquitetura/entities-components/) — os blocos de construção que um composite descreve.
* [Itens Personalizados](/docs/creator/content-creator-pt/scene-editor/interatividade/custom-items/) — itens reutilizáveis armazenados como composites.
* [Smart Items - Advanced](/docs/creator/content-creator-pt/scene-editor/interatividade/smart-items-advanced/#about-spawning-entities) — gere composites sem código, usando a ação Spawn Entity.
* [Scene Files](/docs/creator/content-creator-pt/scenes-sdk7/tipos-de-projetos/scene-files/) — onde `main.composite` fica no seu projeto de scene.


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-pt/scenes-sdk7/arquitetura/composites.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
