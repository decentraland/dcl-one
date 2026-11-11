> Sincronize o estado da scene entre jogadores.

# Multiplayer sem Servidor

Decentraland executa scenes localmente na instância do Explorer de um jogador. Por padrão, os jogadores conseguem ver uns aos outros e interagir diretamente, mas cada jogador interage com o ambiente de forma independente. As alterações no ambiente não são compartilhadas entre os jogadores por padrão.

Ver o mesmo conteúdo no mesmo estado é extremamente importante para que os jogadores interajam de maneiras mais significativas.

Há três maneiras de sincronizar o estado da cena, para que todos os jogadores vejam o mesmo:

* **Marcar uma entidade como sincronizada**: A opção mais fácil. Veja [Marcar uma entidade como sincronizada](#mark-an-entity-as-synced)
* **Enviar mensagens explícitas do MessageBus**: Envie e escute manualmente mensagens específicas. Veja [Enviar mensagens explícitas do MessageBus](#send-explicit-messagebus-messages)
* **Usar um Servidor Multiplayer**: Veja [Servidor Multiplayer](/docs/creator/content-creator-pt/scenes-sdk7/rede/authoritative-servers/). O servidor valida todas as alterações de estado e é a única fonte da verdade. Exige mais configuração, mas é fortemente recomendado quando os jogadores têm incentivos para explorar sua cena.

As duas primeiras opções são abordadas neste documento. Elas são mais simples, pois não exigem servidor. A desvantagem é que você depende mais da velocidade de conexão do jogador, e o estado da cena não é persistido quando todos os jogadores saem da cena.

## Marcar uma entidade como sincronizada

No [Creator Hub](/docs/creator/content-creator-pt/scene-editor/primeiros-passos/about-editor/), marque uma entidade como sincronizada adicionando um **componente Multiplayer** a ela. Ele inclui uma caixa de seleção para cada um dos outros componentes na entidade, permitindo que você selecione quais devem ser atualizados.

![](/docs/_remote/710c6ed40b2f7a539e355f1b.webp)

Para marcar uma entidade como sincronizada via código, use a `syncEntity` função:

```ts
import { syncEntity } from "@dcl/sdk/network";

export function main() {
  const doorEntity = engine.addEntity();

  syncEntity(doorEntity, [Transform.componentId, Animator.componentId], 1);
}
```


> [!WARNING]
> **📔 Nota**: Sempre chame `syncEntity()` dentro da `main()` função, ou em uma função que é executada depois de `main()` (como um callback ou um system). Chamá-la no nível superior de um arquivo gera um erro, porque nesse ponto o perfil do jogador ainda não foi inicializado.



> [!WARNING]
> **📔 Nota**: No multiplayer sem servidor, todo cliente chama `syncEntity` por conta própria. Se você atualizar para um [Servidor Multiplayer](/docs/creator/content-creator-pt/scenes-sdk7/rede/authoritative-servers/), o padrão muda: apenas o servidor deve chamar `syncEntity`, protegido por `isServer()`. Os clientes devem evitar declarar a sincronização de entidades compartilhadas, a menos que tenham sido criadas pelo próprio cliente.


A `syncEntity` A função recebe as seguintes entradas:

* **entityId**: Uma referência para a entidade a ser sincronizada
* **componentIds**: Uma lista dos componentes que precisam ser sincronizados dessa entidade. Este é um array que pode conter quantos componentes forem necessários. Todos os valores devem ser `componentId` propriedades.
* **entityEnumId**: (opcional) Um id único que é usado de forma consistente por todos os jogadores, veja [Sobre o enum id](#about-the-enum-id).

Nem todas as entidades ou componentes precisam ser sincronizados. Elementos estáticos como uma árvore que permanece no mesmo lugar não exigem sincronização. Nas entidades que você sincroniza, apenas os componentes que mudam com o tempo devem ser sincronizados. Por exemplo, se um cubo muda de cor quando clicado, você deve sincronizar apenas o componente Material, não o MeshRenderer nem o Transform, já que eles nunca mudarão.


> [!NOTE]
> **💡 Dica**: Se os dados que você deseja compartilhar não existirem como um componente, defina um [componente personalizado](/docs/creator/content-creator-pt/scenes-sdk7/arquitetura/custom-components/) que contenha esses dados.


### Sobre o enum id

A **entityEnumId** de uma entidade deve ser único. Não está relacionado ao entityId local atribuído por `engine.addEntity()`, que é gerado automaticamente e pode variar entre jogadores executando a mesma cena. O entityEnumId de uma entidade deve ser definido explicitamente no código e ser único.

Definir explicitamente esse ID é importante para evitar inconsistências se uma condição de corrida fizer com que uma parte da cena carregue antes de outra. Talvez, para o jogador A, a porta na cena seja a entidade *512*, mas para o jogador B essa mesma porta seja a entidade *513*. Nesse caso, se o jogador A abrir a porta, o jogador B verá em vez disso todo o prédio se mover.


> [!NOTE]
> **💡 Dica**: Crie um enum na sua cena para manter referências claras a cada id sincronizável na sua cena.
> 
> ```ts
> import { syncEntity } from "@dcl/sdk/network";
> 
> enum EntityEnumId {
>   DOOR = 1,
>   DRAW_BRIDGE = 2,
>   ELEVATOR = 3,
> }
> 
> export function main() {
>   syncEntity(
>     doorEntity,
>     [Transform.componentId, Animator.componentId],
>     EntityEnumId.DOOR
>   );
> }
> ```
> 
> Aqui o enum EntityEnumId é usado para marcar entidades com um identificador único, garantindo que cada cliente reconheça a entidade modificada, independentemente da ordem de criação.



> [!WARNING]
> **📔 Nota**: Evite usar números maiores que **8001** se sua cena também incluir Smart Items. Itens criados pelo [Creator Hub](/docs/creator/content-creator-pt/scene-editor/primeiros-passos/about-editor/) com um componente Multiplayer usarão IDs atribuídos automaticamente a partir de 8001. Qualquer ID menor que 8001 é seguro para atribuir às suas entidades sincronizadas.


**Entidades criadas por um jogador**

Se uma entidade for criada como resultado da interação de um jogador e essa entidade precisar ser sincronizada com outros jogadores, ela não precisa de um entityEnumId. Você pode usar `syncEntity()` passando apenas a entidade e a lista de componentes. Um valor único para entityEnumId é atribuído automaticamente nos bastidores.

Todas as entidades instanciadas na inicialização da cena precisam ter um ID atribuído manualmente. Isso garante que todos os jogadores usem o mesmo ID em cada uma. Quando um único jogador é responsável por instanciar uma entidade, IDs explícitos não são necessários. Os outros jogadores recebem atualizações sobre essa nova entidade com um ID já atribuído, então não há risco de incompatibilidade de IDs.

Por exemplo, em uma cena de batalha de bolas de neve, toda vez que um jogador lança uma bola de neve, ele está instanciando uma nova entidade que é sincronizada com outros jogadores. A bola de neve não precisa de um entityEnumId único.

```ts
import { syncEntity } from "@dcl/sdk/network";

function onThrow() {
  const ball = engine.addEntity();
  Transform.create(ball, {});
  GltfContainer.create(ball, { src: "assets/snowBall.glb" });
  syncEntity(ball, [Transform.componentId, GltfContainer.componentId]);
}
```

### Entidades com parent <a href="#parented-entities" id="parented-entities"></a>

O parent de uma entidade normalmente é definido via `parent` property no `Transform` componente. No entanto, essa propriedade aponta para o id local da entidade do parent, que pode variar, veja [Sobre o enum id](#about-the-enum-id). Para parentear entidades que precisam ser sincronizadas, ou que têm children que precisam ser sincronizados, use a `função parentEntity()` em vez do `Transform`.

```ts
import { syncEntity, parentEntity } from "@dcl/sdk/network";

export function main() {
  const parent = engine.addEntity();
  Transform.create(parent, { position: somePosition });
  syncEntity(parent, []);

  const child: Entity = engine.addEntity();
  syncEntity(child, [Transform.componentId]);

  parentEntity(child, parent);
}
```

Observe que tanto o parent quanto o child são sincronizados com `syncEntity`, então todos os jogadores têm um entendimento comum de quais IDs são usados por ambas as entidades. Isso é necessário mesmo que os componentes do parent talvez nunca precisem mudar. Neste exemplo, `syncEntity` inclui um array vazio de componentes para evitar a sincronização de quaisquer componentes desnecessários.


> [!WARNING]
> **📔 Nota**: Se uma entidade tiver parent definido tanto pelo `função parentEntity()` como também pelo `parent` property no `Transform` componente, a propriedade no `Transform` componente é ignorada.


Quando as entidades são parentadas via `função parentEntity()` função, você também pode usar as seguintes funções auxiliares:

* **removeParent()**: Desfaz os efeitos de `função parentEntity()`. Requer que você passe apenas a entidade child. O novo parent da entidade passa a ser a entidade raiz da cena. A entidade parent original não é removida da cena.
* **getParent()**: Retorna a entidade parent da entidade que você passou.
* **getChildren()**: Retorna a lista de children da entidade que você passou, como um iterável.
* **getFirstChild()**: Retorna o primeiro child da lista para a entidade que você passou.

```ts
import {
  syncEntity,
  parentEntity,
  getParent,
  getFirstChild,
  getChildren,
  removeParent,
} from "@dcl/sdk/network";

export function main() {
  const parent = engine.addEntity();
  Transform.create(parent, { position: somePosition });
  syncEntity(parent, []);

  const child: Entity = engine.addEntity();
  syncEntity(child, [Transform.componentId]);

  // define parent as parent
  parentEntity(child, parent);

  // getParent
  const getParentResult = getParent(child);
  // returns parent

  // getFirstChild
  const getFirstChildResult = getFirstChild(parent);
  // returns child

  // getChildren
  const getChildrenResult = Array.from(getChildren(parent));
  // returns [child]

  // removes parent from child
  removeParent(child);
}
```

## Verifique o estado de sincronização

Quando um jogador acaba de carregar em uma cena, ele talvez ainda não esteja sincronizado com outros jogadores ao seu redor. Se o jogador começar a alterar o estado do jogo antes de estar sincronizado, isso pode causar problemas no seu jogo. Recomendamos sempre verificar se um jogador está sincronizado antes de permitir que ele edite qualquer coisa na cena.

Se um jogador sair das parcelas da cena, ele também ficará fora de sincronia com a cena enquanto estiver do lado de fora. Portanto, também é importante que os systems da cena lidem com esse cenário, já que a cena continua sendo executada enquanto o jogador estiver por perto. Quando o jogador voltar, ele será atualizado automaticamente com quaisquer mudanças no estado da cena.

Você pode verificar se o estado da cena está atualmente sincronizado para um jogador por meio da `isStateSyncronized()` função. Essa função retorna um booleano, que é true se o jogador já estiver sincronizado com a cena.

```ts
import { isStateSyncronized } from "@dcl/sdk/network";

const isConnected = isStateSyncronized();
```

Você poderia, por exemplo, incluir essa verificação em um system e bloquear qualquer interação se essa função retornar false.

```ts
import { isStateSyncronized } from "@dcl/sdk/network";

engine.addSystem(() => {
  if (isStateSyncronized() && !button.enabled) {
    console.log("Ativar Iniciar Jogo");
    button.enable();
  }

  if (!isStateSyncronized() && button.enabled) {
    console.log(`Desativar Iniciar Jogo.`);
    button.disable();
  }
});
```

## Enviar mensagens explícitas do MessageBus


> [!WARNING]
> **📔 Nota**: O `MessageBus` API está marcada como obsoleta no SDK e pode ser removida em uma versão futura. Para a maioria dos casos de uso, prefira [marcar entidades como sincronizadas](#mark-an-entity-as-synced).


**Iniciar um barramento de mensagens**

Crie um objeto de barramento de mensagens para lidar com os métodos necessários para enviar e receber mensagens entre jogadores.

```ts
import { MessageBus } from "@dcl/sdk/message-bus";

const sceneMessageBus = new MessageBus();
```

**Enviar mensagens**

Use o `.emit` comando do barramento de mensagens para enviar uma mensagem a todos os outros jogadores na cena.

```ts
import { MessageBus } from "@dcl/sdk/message-bus";

const sceneMessageBus = new MessageBus();

const myEntity = engine.addEntity();
MeshRenderer.setBox(myEntity);
MeshCollider.setBox(myEntity);

pointerEventsSystem.onPointerDown(
  {
    entity: myEntity,
    opts: { button: InputAction.IA_PRIMARY, hoverText: "Clique" },
  },
  function () {
    sceneMessageBus.emit("box1Clicked", {});
  }
);
```

Cada mensagem pode conter um payload como segundo argumento. O payload é do tipo `Object`, e pode conter quaisquer dados relevantes que você queira enviar.

```ts
import { MessageBus } from "@dcl/sdk/message-bus";

const sceneMessageBus = new MessageBus();

sceneMessageBus.emit("spawn", { position: { x: 10, y: 2, z: 10 } });
```


> [!NOTE]
> **💡 Dica**: Se você precisar que uma única mensagem inclua dados de mais de uma variável, crie um tipo personalizado para guardar todos esses dados em um único objeto.


**Receber mensagens**

Para lidar com mensagens de todos os outros jogadores nessa cena, use `.on`. Ao usar essa função, você fornece uma string de mensagem e define uma função para executar. Cada vez que uma mensagem com uma string correspondente chegar, a função fornecida é executada uma vez.

```ts
import { MessageBus } from "@dcl/sdk/message-bus";

const sceneMessageBus = new MessageBus();

type NewBoxPosition = {
  position: { x: number; y: number; z: number };
};

sceneMessageBus.on("spawn", (info: NewBoxPosition) => {
  const myEntity = engine.addEntity();
  Transform.create(myEntity, {
    position: { x: info.position.x, y: info.position.y, z: info.position.z },
  });
  MeshRenderer.setBox(myEntity);
  MeshCollider.setBox(myEntity);
});
```


> [!WARNING]
> **📔 Nota**: Mensagens enviadas por um jogador também são recebidas por esse mesmo jogador. O `.on` método não consegue distinguir entre uma mensagem emitida por esse mesmo jogador e uma mensagem emitida por outros jogadores.


**Exemplo completo de MessageBus**

Este exemplo usa um barramento de mensagens para enviar uma nova mensagem cada vez que o cubo principal é clicado, gerando um novo cubo em uma posição aleatória. A mensagem inclui a posição do novo cubo, para que todos os jogadores vejam esses novos cubos nas mesmas posições.

```ts
import { MessageBus } from "@dcl/sdk/message-bus";

/// --- Criar barramento de mensagens ---
const sceneMessageBus = new MessageBus();

// Fábrica de cubos
function createCube(x: number, y: number, z: number): Entity {
  const meshEntity = engine.addEntity();
  Transform.create(meshEntity, { position: { x, y, z } });
  MeshRenderer.setBox(meshEntity);
  MeshCollider.setBox(meshEntity);

  // Quando um cubo é clicado, envie uma mensagem para gerar outro
  pointerEventsSystem.onPointerDown(
    {
      entity: meshEntity,
      opts: { button: InputAction.IA_PRIMARY, hoverText: "Pressione E para gerar" },
    },
    function () {
      sceneMessageBus.emit("spawn", {
        position: {
          x: 1 + Math.random() * 8,
          y: Math.random() * 8,
          z: 1 + Math.random() * 8,
        },
      });
    }
  );

  return meshEntity;
}

// Inicialização
createCube(8, 1, 8);

// definir o tipo de dados
type NewBoxPosition = {
  position: { x: number; y: number; z: number };
};

// ao receber a mensagem spawn, crie um novo cubo
sceneMessageBus.on("spawn", (info: NewBoxPosition) => {
  createCube(info.position.x, info.position.y, info.position.z);
});
```

## Teste uma cena multiplayer localmente

Se você iniciar uma pré-visualização de cena e abri-la em duas (ou mais) janelas diferentes do Explorer, cada janela aberta será interpretada como um jogador separado, e um servidor de comunicação simulado manterá esses jogadores sincronizados.

Interaja com a cena em uma janela e depois mude para a outra para ver que os efeitos dessa interação também ficam visíveis lá.

Usando o Creator Hub, ative **Pré-visualização multi-instância** no **Desktop Client** submenu de **Play Options**, depois clique no **Play** botão uma segunda vez para abrir uma segunda janela do Decentraland Explorer. Você deve se conectar em ambas as janelas com endereços diferentes. As mesmas sessões permanecerão abertas enquanto a cena recarrega.

![](/docs/_remote/ca7e6550cdb1bb947ce74b28.webp)

Como alternativa, você pode abrir uma segunda janela do Decentraland Explorer escrevendo o seguinte em uma URL do navegador:

> `decentraland://realm=http://127.0.0.1:8000&local-scene=true&debug=true&multi-instance=true`

## Cenas para um único jogador

Se sua cena for implantada em um [Decentraland World](/docs/creator/content-creator-pt/scenes-sdk7/publicacao/publishing-options/#decentraland-worlds), você pode transformá-la em uma cena para um único jogador. Os jogadores não se verão, não poderão conversar nem ver os efeitos das ações uns dos outros.

Para fazer isso, configure o `scene.json` arquivo da cena para definir o **fixedAdapter** para `offline:offline`. A cena não terá nenhum Serviço de Comunicação e cada usuário que entrar nesse World estará sempre sozinho.

**Exemplo:**

```json
{
  "worldConfiguration": {
    "name": "my-name.dcl.eth",
    "fixedAdapter": "offline:offline"
  }
}
```


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-pt/scenes-sdk7/rede/serverless-multiplayer.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
