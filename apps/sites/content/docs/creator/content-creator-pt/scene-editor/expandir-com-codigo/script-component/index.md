> Use o Script component para dar funcionalidade ao código, sem a necessidade de se aprofundar em toda a estrutura do projeto.

# Usando o Script Component

Com o novo Script Component, é possível criar Entities que executam código personalizado dentro da própria Entity.

Os Script Components permitem a execução do comportamento personalizado de uma Entity sem a necessidade de trabalhar diretamente no `index.ts` e potencialmente outros arquivos.

## Configurando o Script Component

1. Adicione o Script Component a uma Entity clicando no `+` botão e selecionando-o. Crie um novo Script clicando em **+ Adicionar novo módulo de Script** e escolha um nome, ou usando o File Path (navegue ou arraste e solte um arquivo existente).

![](/docs/_remote/2c5b0bcd10d2297a8c766d9b.webp)

2. Clique no botão CODE no componente para abrir o editor de código padrão. Vamos verificar sua estrutura. Para mais detalhes sobre como selecionar e gerenciar seu editor padrão, vá para [Combine com código](/docs/creator/content-creator-pt/scene-editor/expandir-com-codigo/overview/).

## Entendendo a estrutura do Script

Quando o Script é aberto pela primeira vez, ele tem o seguinte código:

```ts
import { engine, Entity } from '@dcl/sdk/ecs'
import {} from '@dcl/sdk/math'

export class BuildingScript {
  /**
   * Propriedades
   * Defina campos da classe que você deseja reutilizar entre métodos.
   * Exemplo de uso: this.myVariable
   */
   // private myVariable: boolean = true

  /**
   * Construtor / Entradas
   * Os parâmetros declarados aqui aparecem na UI do Script component no Creator Hub.
   * Tipos suportados: Entity, String, Number, Boolean, ActionCallback e Slider
   * para um número editado com um slider, por exemplo public speed: Slider<0, 10, 0.5> = 1
   *
   * Observação: depois de editar este arquivo, clique no ícone de refresh na UI do Script component
   * para ver as entradas atualizadas.
   *
   * Os campos `src` e `entity` no construtor são necessários para referências internas.
   */
  constructor(
    public src: string,     // DO NOT REMOVE
    public entity: Entity,   // DO NOT REMOVE
    // Adicione suas entradas personalizadas abaixo
  ) {}

  /**
   * start()
   * Chamado uma vez quando o script é inicializado.
   */
  start() {
    // Inicialização do Script
    console.log("BuildingScript initialized for entity:", this.entity);
  }

  /**
   * update(dt)
   * Chamado a cada frame.
   * @param dt - (opcional) Tempo delta desde o último frame (em segundos)
   */
  update(dt: number) {
    // Chamado a cada frame
  }
}
```

A classe é composta por três partes principais:

* A **constructor**,
* o **start()** método
* o **update()** método.

## Construtor

O construtor contém os parâmetros que você deseja expor e modificar dinamicamente a partir da sua Scene no Creator Hub.

```ts
export class BuildingScript {
  constructor(
    public src: string,
    public entity: Entity,
    public numericVariable: number, 
  ) {}
...
}
```

Depois que o arquivo for salvo, o **botão Atualizar** no Script Component atualiza todas as alterações feitas.

<img src="/docs/_remote/f1c901d70e456b61b41d362b.webp" alt="botão Atualizar" width="360">

Depois de atualizado, o Script Component agora mostra o `numericVariable` adicionado no código.

![](/docs/_remote/7bf7d0505e8aa1c4870ff35a.webp)

## Parâmetros

Se diferentes Entities usarem o mesmo arquivo no Script component, cada uma ainda terá parâmetros independentes: se a Scene tiver dois edifícios, `building1` e `building2`, ambos com um Script Component apontando para `arquivo BuildingScript.ts` , cada edifício tem seu próprio `numericVariable` parâmetro que pode ser modificado independentemente.


> [!WARNING]
> **Observação importante**: Não modifique/exclua `public src: string` e `public entity: Entity`. Você pode adicionar novas entradas seguindo estas.


Os tipos permitidos para os parâmetros do construtor são:

* `Entity`
* `string`
* `number`
* `boolean`
* `ActionCallback`
* `Slider<Min, Max, Step>`

### Parâmetros numéricos com um slider

Escreva um parâmetro numérico como `Slider<Min, Max, Step>` para editá-lo com um slider em vez de uma caixa de número simples. O Creator Hub mostra tanto um slider quanto uma caixa de número.

```ts
constructor(
  public src: string,
  public entity: Entity,
  // Um slider de 0 a 10, movendo-se em passos de 0,5
  public speed: Slider<0, 10, 0.5> = 1,
) {}
```

* `Step` é opcional e tem como padrão `1`.
* Limites negativos são permitidos, por exemplo `Slider<-90, 90>`.
* Em tempo de execução, o valor é um simples `number`, então `this.speed` se comporta como qualquer outro parâmetro numérico.


> [!NOTE]
> **📔 Nota**: Ambos `public` e `private` parâmetros do construtor são expostos ao Creator Hub. O `private` palavra-chave apenas restringe o acesso dentro da `BuildingScript` classe. Para mais detalhes, veja a documentação oficial do TypeScript sobre\
> [Propriedades de parâmetro](https://www.typescriptlang.org/docs/handbook/2/classes.html#parameter-properties).


### Acessando parâmetros dentro do Script

Para acessar o valor de um parâmetro a partir do seu código, use a notação `this.definedParameter`. Por exemplo, `this.numericVariable` ou `this.entity`.

O template padrão do Script inclui esta linha no método start():

`console.log("BuildingScript initialized for entity:", this.entity);`.

Altere-o assim para registrar o valor de um valor que você definiu no construtor:

`console.log("BuildingScript initialized with numericVariable:`, `this.numericVariable);`

Observe que, quando você altera o valor do parâmetro na UI do Creator Hub, você também deverá ver esse valor registrado refletir isso.

### Parâmetros padrão

O construtor, por padrão, contém um `src` e um `entity` parâmetro, e eles são muito úteis para o código no seu script:

* `this.entity` sempre se refere à Entity que contém o `Script` component; use isto para acessar informações sobre a Entity ou adicionar Components a ela.
* `this.src` é o caminho onde o script é armazenado. Isso é particularmente útil ao criar Smart Items destinados a ser usados por outras pessoas. Use este campo para construir o caminho para arquivos que vêm empacotados com seu smart item, mesmo que o caminho do smart item mude ou seja renomeado.

```ts
export class BuildingScript {
  constructor(
    public src: string,
    public entity: Entity,
  ) {}

  start() {
    Material.setPbrMaterial(this.entity, {
      texture: Material.Texture.Common({
        src: this.src + '/images/myImage.png',
      })
    });
  }
}
```

O script acima obtém a Entity que possui o script e aplica uma texture a ela. Ele obtém a texture de um `.png` arquivo que vem empacotado na pasta do smart item, em uma subpasta chamada `/images`. Ao usar `this.src`, garantimos que o caminho do arquivo seja sempre conhecido, não importa se o smart item é importado para a Scene sob `/assets/custom/itemName` ou `/assets/asset-packs/itemName`

### Tooltips nos parâmetros

Adicione tooltips aos seus parâmetros de entrada, para que os usuários saibam para que esses campos são usados, ou quais valores são aceitos. Os usuários verão um ícone de tooltip ao lado de cada campo na UI do Script component e poderão ler texto personalizado ao passar o mouse sobre o ícone.

Para adicionar tooltips ao seu construtor, adicione um bloco comentado logo antes do construtor e escreva uma linha com `@param` mais o nome do campo, seguido de uma descrição, para cada tooltip.

```ts
  /**
   * @param startDate - A data de início do evento no formato YYYY-MM-DD
   * @param yOffset - Quantos metros acima do chão exibir o item
   */
  constructor(
    public src: string,
    public entity: Entity,
    public startDate?: string,
    public yOffset: number = 0.5,
  ) {
  }
```

Talvez seja necessário clicar no ícone de refresh na UI do Script component para ver as alterações nos seus tooltips.

<img src="/docs/_remote/f1c901d70e456b61b41d362b.webp" alt="botão Atualizar" width="360">

## Método start() & update()

A **start()** O método contém código que é executado apenas uma vez, quando a Entity é criada (neste caso, quando a Scene é carregada pela primeira vez).

Faça preview da Scene e verifique os logs (**Dica**: você pode usar o `` ` `` atalho): Ele exibe a nova mensagem incluindo o `numericVariable` parameter.

![](/docs/_remote/3391665f06d28b07d1c783f1.webp)

A **update()** método, por outro lado, executa seu código a cada frame do jogo (assim como os Systems fazem). Por exemplo, verificando os valores do `PlayerEntity` para disparar comportamentos no script.

O código a seguir imprime Logs a cada frame do jogo em que o `PlayerEntity` é maior do que o previamente definido `numericVariable`, que é fornecido pelo criador dinamicamente a partir da UI do Script Component.

```ts
update(dt: number) {
    if (Transform.get(engine.PlayerEntity).position.y > this.numericVariable ) {
      console.log("A altura do jogador está acima de ", this.numericVariable);
    }}
```

<img src="/docs/_remote/cff6c35c55f5fdd39baaefae.webp" alt="Update Method" data-size="line">

O primeiro log pertence ao método start(), indicando que definimos numericVariable. O segundo pertence ao método update(), quando o jogador está acima desse valor.

## Expondo Actions ao Creator Hub

É possível definir um `Action` dentro de um script do Script Component e torná-lo acessível na UI do Creator Hub. Isso possibilita disparar este `Action` com outra Entity.

```ts
  /**
   * Expor esta action para ser disparada
   * @action
   */
  exposedAction(creatorHubParameter: number) {
    console.log("Disparado a partir de outra Entity usando o parâmetro: ", this.creatorHubParameter);
  }
```

`creatorHubParameter` será exposto como um `Action` parâmetro para atribuir um valor personalizado. Depois de atualizar o Script Component, a nova action ficará disponível como uma opção no dropdown Actions.

![](/docs/_remote/eb92dee98d90a44d44de70b3.webp)

Depois de adicionar a Action, qualquer Entity no Creator Hub pode dispará-la usando `Triggers`

![](/docs/_remote/e8bcd93f2829f1275473cb37.webp)


> [!NOTE]
> **📔 Nota**: Você pode adicionar quantas Actions forem necessárias dentro do Script. Todas elas ficarão acessíveis independentemente do `Action` dropdown.


## Chamando métodos do Script de fora

Para chamar um método do Script a partir de outro Script ou de `src/index.ts`, os seguintes passos devem ser seguidos:

1. Crie um `public` método dentro da classe Script.
2. Execute `npm run build` a partir do diretório raiz da Scene.
3. No arquivo em que você quer usar o método público, adicione `import { callScriptMethod } from '~sdk/script-utils'`.
4. Chame `callScriptMethod` com os parâmetros necessários (neste caso, `someParamter`).

Aqui está um exemplo com um `public` método exposto

```ts
export class BuildingScript {
  constructor(
    public src: string,
    public entity: Entity,
    ...,
  ) {}

  public publicMethod(boolParameter: boolean, someNumberParameter: number) {
    if (boolParameter) {
      console.log("Método público chamado com o parâmetro true!: ", someNumberParameter);
    } else {
      console.log("Método público chamado com o parâmetro false!", someNumberParameter);
    }
  }
...
}
```

Para chamá-lo de `src/index.ts`, use:

```ts
import { callScriptMethod } from '~sdk/script-utils'


export function main() {
    const buildingEntity = engine.getEntityOrNullByName("building")
    if (buildingEntity) {
        const scriptMethod = callScriptMethod(
            buildingEntity,
            "assets/scene/Scripts/BuildingScript.tsx",
            "publicMethod",
            false,
            3,
        )

        scriptMethod
    }
}
```

Primeiro, a `main` função procura a `Entity` que tem o Script component. Segundo, se a `Entity` existir, `callScriptMethod` é chamada com os seguintes parâmetros:

1. `entity`: `Entity` que tem o `public` método.
2. `scriptPath`: `path` onde a `Script` classe reside.
3. `methodName`: nome do `public` método a ser chamado.
4. `...args`: Argumentos do método. Neste caso, são dois. Eles devem ser adicionados em ordem, um após o outro.

Terceiro, chamamos o `callScriptMethod`, neste caso, `scriptMethod`.

Com os valores dos parâmetros fornecidos, a saída é:

![](/docs/_remote/732f383d31c15cab43ea285e.webp)


> [!NOTE]
> **📔 Nota**: Você pode seguir a mesma lógica para chamar um `public` método do Script a partir de outro script ou arquivo. Você pode usá-lo para obter ou alterar valores de `public` variáveis na classe Script.


## Disparando Actions de outras Entities a partir de um Script

É possível usar um parâmetro do tipo `ActionCallback` no construtor da classe Script. Isso permite disparar a `Entity`'s `Action` definida pela UI do Creator Hub a partir dos métodos do Script.

Neste exemplo, `anotherEntityAction` é adicionada como uma `public` parameter.

```ts
export class BuildingScript {
  constructor(
    public src: string,
    public entity: Entity,
    public anotherEntityAction: ActionCallback,
    ...,
  ) {}
  ...
}
```

Uma `Entity` e `Action` selecionável agora estão disponíveis quando o Script Component é atualizado na UI do Creator Hub. `Sphere` é uma Entity já existente na Scene que tem uma action chamada `Escala`.

![](/docs/_remote/b12b866e818c2c2562ab037c.webp)

A action da outra Entity agora está acessível na classe Script. Ela pode ser usada de muitas maneiras diferentes. No exemplo a seguir, pressionar E disparará `this.anotherEntityAction` ao definir um `pointerEventsSystem` no `start` método.

```ts
  start() {
    pointerEventsSystem.onPointerDown(
      {
        entity: this.entity,
        opts: {
          button: InputAction.IA_PRIMARY,
          hoverText: "Pressione E para disparar uma Action de outra Entity.",
        },
      },
      () => {
        this.anotherEntityAction();
      }
    );
  }
```

### Callbacks de action opcionais

Uma `ActionCallback` parâmetro que o criador deixa sem conexão é `undefined`, e seu tipo é `ActionCallback | undefined`. Verifique antes de chamá-lo, para que um script funcione com ou sem a action conectada:

```ts
if (this.anotherEntityAction) {
  this.anotherEntityAction();
}
```


> [!NOTE]
> **📔 Nota**: Combinar exposição e disparo `Actions` é uma ferramenta muito poderosa. Você pode definir um Script Component em uma Entity, expor uma action usando um `public` método e, depois, dispará-la a partir do Script Component de outra Entity usando um `ActionCallback` parameter.


## Ver também

* [Smart items - Noções básicas](/docs/creator/content-creator-pt/scene-editor/interatividade/smart-items/)
* [Smart items - Avançado](/docs/creator/content-creator-pt/scene-editor/interatividade/smart-items-advanced/)
* [Estados e condições](/docs/creator/content-creator-pt/scene-editor/interatividade/states-and-conditions/)
* [Transformar qualquer item em smart item](/docs/creator/content-creator-pt/scene-editor/interatividade/make-any-item-smart/)
* [Início rápido do SDK](/docs/creator/content-creator-pt/scenes-sdk7/primeiros-passos/sdk-101/): siga este mini tutorial para um curso rápido.
* [Fluxo de desenvolvimento](/docs/creator/content-creator-pt/scenes-sdk7/primeiros-passos/dev-workflow/): leia isto para entender a criação de Scene do início ao fim.
* [Exemplos](/docs/creator/content-creator-pt/tutoriais-e-exemplos/examples/): mergulhe diretamente em scenes de exemplo funcionais.


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-pt/scene-editor/expandir-com-codigo/script-component.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
