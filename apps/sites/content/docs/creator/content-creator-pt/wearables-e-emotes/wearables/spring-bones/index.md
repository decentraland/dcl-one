> Adicione física dinâmica a wearables com Spring Bones

# Spring Bones

### O que são Spring Bones?

Spring bones (também conhecidos como jiggle bones ou physics bones) são ossos extras adicionados a um wearable que se movem dinamicamente em resposta ao movimento do avatar e à gravidade, em vez de serem controlados por clipes de animação. Eles dão vida a elementos como cabelo, brincos, capas, cintos, rabos de cavalo e outros acessórios pendentes, fazendo-os balançar, quicar e se acomodar naturalmente conforme o avatar anda, corre ou gira.

Os Spring bones são **não** parte da armature base do avatar. Eles são ossos adicionais que você adiciona ao esqueleto base do avatar no Blender (ou em qualquer software 3D), e o comportamento de física deles é configurado no Builder após o upload.

Esta implementação segue o [padrão VRM Spring Bone](https://vrm.dev/en/vrm1/springbone/), uma convenção amplamente adotada para física de avatar em formatos como VRM e MMD.

![](/docs/_remote/34cee41d461f2df0b8a03bc9.webp)

### Como os Spring Bones Funcionam

Um **cadeia spring** é uma sequência de ossos que simula física em conjunto e deve ter pelo menos dois ossos: o spring bone real e um osso final. Um único spring bone, sozinho, não produzirá movimento algum — a simulação precisa da cabeça do spring bone para acionar a física, e a cabeça do osso final definirá o ponto final geométrico. Cadeias mais longas (3+ ossos) terão movimento mais suave e natural, ideal para cabelos longos ou capas.

1. **Spring bone pai** — O primeiro osso da cadeia. Ele possui a configuração de física (rigidez, gravidade, arrasto etc.). Identificado por conter `springbone` em qualquer parte do nome do osso (sem diferenciar maiúsculas de minúsculas).
2. **Spring bone filho** — É o filho do osso anterior na hierarquia. Ele herda os parâmetros de física do pai e forma a cadeia.
3. **Spring bone final** — O último osso da cadeia. Ele define o ponto final geométrico da cadeia, mas não é afetado pela simulação e não deforma nenhuma mesh.

A cadeia, ou hierarquia, dos spring bones deve ser **linear**, o que significa que cada osso pode ter apenas um filho. Ossos com dois ou mais filhos podem apresentar um comportamento inesperado.

![Uma cadeia spring bonita e linear.](/docs/_remote/69a822cfcb94b03d0ba7ebc5.webp)

![Ossos com 2 ou mais filhos terão um comportamento inesperado.](/docs/_remote/9ef1ec407222b18595a504e1.webp)

### A Hierarquia

Para que os spring bones funcionem bem, eles precisam estar parented a um dos ossos originais do avatar. Para cabelos e brincos, por exemplo, o osso no topo da hierarquia deve estar parented a ***Avatar\_Head***. Para cachecóis, parentear a cadeia em ***Avatar\_Neck*** é uma boa ideia. Para saias, talvez ***Avatar\_Hips*** ou ***Avatar\_LeftUpLeg/Avatar\_RightUpLeg*** possa funcionar.

É importante perceber como a estrutura dessa hierarquia funciona.

* *Hair\_springBone\_1* (spring bone parent): este é o osso no topo da cadeia, ele tem configuração de física
* *Hair\_springBone\_2* (child spring bone): este é o filho do osso anterior na hierarquia e herda a configuração.
* *Hair\_springBone\_end* (spring bone end): é o último osso da cadeia e serve apenas como ponto final.

### Convenção de Nomenclatura

Todos os spring bones **devem** conter a substring `springbone` (sem diferenciar maiúsculas de minúsculas) no nome. A substring pode aparecer em qualquer posição:

* *SpringBone\_hair\_left*
* *hair\_springbone\_l*
* *springbone\_earring.R*

Para manter seu fluxo de trabalho organizado, é sugerido usar este formato: **BodyPart/WearableName\_springBone**

Exemplos: *Hair\_springBone\_1*, *Earring\_springBone.L* etc… Sinta-se à vontade para usar o formato que melhor lhe convier, desde que siga a convenção de nomenclatura do Blender (ou do software de sua preferência) para esquerda e direita.


> [!WARNING]
> Atenção!
> 
> Os spring bones não vão funcionar sem `springbone` no nome do osso. Isso precisa ser usado, inclusive no osso final.


### Limites

Cada wearable deve permanecer dentro dos seguintes limites de spring bones:

* Máximo **6** cadeias de spring por wearable
* Máximo **12** total de spring bones (soma de todos os ossos em todas as cadeias)
* Profundidade máxima da cadeia de **6** ossos

## Criando Spring Bones no Blender

Spring bones são ossos adicionais que você cria no Blender para a armature base do avatar. Os parâmetros de física são configurados depois no Builder — agora você só precisa configurar a hierarquia dos ossos.

Para criar um novo osso, selecione a Armature do avatar e, em **Edit Mode**, certifique-se de que o cursor esteja onde você quer que o osso seja criado e pressione **Shift+A**. Outra maneira de fazer isso é duplicando um osso existente com **Shift+D**. Assim que tiver o primeiro osso da cadeia, pressione **E** para extrudi-lo e criar o restante da cadeia.

![Pressione Shift+A para criar um osso onde o cursor estiver.](/docs/_remote/ba60206570eff70b8cf1e047.webp)

É importante notar que uma cadeia spring não precisa estar conectada ao pai do esqueleto, então você pode colocá-la em qualquer lugar da mesh. No entanto, a cadeia spring **precisa** estar conectada; você não pode deslocar nenhum dos spring bones.

Depois de criar os ossos, renomeie-os seguindo a convenção de nomenclatura mencionada acima e certifique-se de parentear a cadeia ao osso correto. Linhas pontilhadas mostrarão o pai da cadeia. Se não houver nenhuma, significa que a cadeia não tem pai. Em **Edit Mode**, selecione primeiro o filho, depois selecione o pai e pressione **Ctrl+P** > **Manter Offset**.

![Parentear ossos selecionando primeiro o filho, depois o pai, e pressione Ctrl+P.](/docs/_remote/c74a20db130a6672bd12b248.webp)

Para renomear um osso, selecione-o em Edit Mode ou Pose Mode, vá para a aba Bone Properties e renomeie-o de acordo com a convenção de nomenclatura. Faça isso para todos os ossos da cadeia.

![](/docs/_remote/ce5dc94e935f8d3e30031022.webp)


> [!NOTE]
> Dica!
> 
> Para garantir que o osso esteja posicionado corretamente, selecione a mesh em **Object Mode** e, em **Edit Mode**, selecione os vértices da área onde você quer colocar o osso (pode ser um loop ou um grupo de vértices) > pressione **Shift+S** > **Cursor to Selected.**
> 
> Volte para **Object Mode**, selecione a armature > vá para **Edit Mode** > selecione o osso desejado > **Shift+S** > **Selection to cursor**.


![Use Shift+S para posicionar o osso no lugar certo na mesh.](/docs/_remote/f67fcda5bdba4aa2338aeafe.webp)

### Skinning da Mesh

Skinning é o processo de vincular a mesh à armature, para que elas se movam juntas. Para isso, definimos quanto de influência (peso) cada osso terá sobre os vértices. Quanto mais peso, mais o osso deformará a mesh. Para fazer isso, vá para **Object Mode**, selecione primeiro a mesh, pressione **Shift** e selecione a armature, pressione **Ctrl+P**. Existem duas maneiras de fazer isso: selecione **With Empty Groups** ou **With Automatic Weights**.

![](/docs/_remote/af0366e9dd4fcda637585505.webp)

#### With Automatic Weights

Como o nome diz, com este método o Blender tentará definir os pesos de skin automaticamente, criando grupos de vértices para cada osso na armature e atribuindo pesos automáticos a cada um deles. Pode funcionar em alguns casos, mas também pode exigir alguns ajustes. Você pode verificar os grupos criados clicando na **Data** aba.

Se você tiver certeza de que não precisará que certos ossos afetem a mesh, basta clicar no **ícone de cadeado** para bloquear os grupos que deseja, clique no menu suspenso e **Delete All Unlocked Groups** para removê-los do objeto. Por exemplo, se você estiver trabalhando em um cabelo, ter grupos para pés e mãos não faz sentido. Nesse caso, exclua tudo o que não for a cabeça ou os spring bones, como no exemplo abaixo. Certifique-se também de excluir o grupo do osso final da cadeia spring.

![Excluindo grupos de vértices.](/docs/_remote/7241732fd5c95ea8e45c2fc5.webp)

#### With Empty Groups

Com este método, o Blender criará todos os grupos de vértices para cada osso na armature, mas eles terão peso zero por padrão. Você terá que atribuir manualmente os pesos em Edit Mode ou pintá-los no modo Weight Paint. Isso lhe dá mais controle sobre o que é afetado por cada grupo e pode ser muito útil para superfícies rígidas ou objetos que precisam ser totalmente atribuídos a um grupo específico. De qualquer forma, depois de atribuir os pesos, será preciso testá-los em Pose Mode e então ajustá-los no Weight Paint.

![Parentear com grupos vazios.](/docs/_remote/7117bb961a31e9863ce1f2b6.webp)

#### Pintando Pesos

Para testar a mesh com skinning, selecione a armature e vá para **Pose Mode**, defina um keyframe, depois gire os ossos para criar outra pose e defina outro keyframe. Assim você pode verificar como a mesh está deformando com o movimento. Depois de definir as poses, volte para **Object Mode**, selecione a mesh e vá para **Weight Paint**.

![](/docs/_remote/b65b3d58d5501afc7e9c1613.webp)

Em **Weight Paint**, na aba Tools, você encontrará diferentes brushes para ajustar os pesos de skin. Selecione o Vertex Group que deseja editar (se estiver bloqueado, basta desbloqueá-lo para poder editar a influência) e use o brush para adicionar ou remover influência. Preto significa influência zero, enquanto vermelho significa que aquele grupo influencia a mesh completamente. Ativar o wireframe em **Overlays** facilitará ver o que você está pintando.

![](/docs/_remote/90b7445921156a708238f07c.webp)

Use os brushes de adicionar, subtrair ou suavizar para obter o resultado desejado. Teste também diferentes poses extremas para ver como a mesh está deformando e se não há nenhum vértice sem pesos. Se estiver satisfeito com o resultado, é hora de exportar!

### Exportando

Antes de exportar, certifique-se de excluir qualquer clipe de animação criado ao testar as poses. Vá para Pose Mode, selecione todos os ossos pressionando **Um** e pressione **Ctrl+R**, depois **Ctrl+S** e, por fim, **Ctrl+G** para excluir quaisquer transforms da sua armature. Depois, volte para object mode, mude o Display Mode de View Layer para Blender File, expanda Actions, clique com o botão direito e exclua o arquivo de animação.

![](/docs/_remote/d7f2ca8abd2e46afcdc74627.webp)

Se você tiver outros objetos na sua cena, como a mesh do avatar, desative a visibilidade clicando no ícone de olho no Outliner. Depois, vá para **File** > **Export** > **gltf 2.0 (.glb, .gltf)**. Nas configurações de exportação, expanda Include e em **Limit to** ative **Visible Objects**. Clique em Export e você estará pronto para fazer upload do seu arquivo para o Builder!

![](/docs/_remote/22840326793df160d616c22f.webp)

## Configurando Spring Bones no Builder

Depois de fazer upload de um wearable que contenha ossos nomeados com `springbone`-, o Builder os detecta automaticamente e mostra o painel de configuração de **Spring Bones** .

![](/docs/_remote/6198c56189e1f79689bb9bd2.webp)

Nesse painel, você pode configurar os parâmetros de física para cada osso raiz de spring. A visualização do avatar no Builder refletirá suas alterações imediatamente, permitindo ajustar o comportamento em tempo real.


> [!NOTE]
> Você não pode adicionar novos spring bones pelo Builder. Os ossos já devem existir no arquivo `.glb` enviado com a convenção de nomenclatura correta. O Builder apenas permite configurar os parâmetros de física dos spring bones detectados.


### Referência de Parâmetros

#### Força de Rigidez

|            |       |
| ---------- | ----- |
| **Faixa**  | 0 a 4 |
| **Padrão** | `2.0` |

Controla quão fortemente o osso tenta retornar à sua pose de repouso. Esta é a força de retorno — pense nela como a "rigidez" da mola.

* **0**: O osso não voltará ao repouso de forma alguma — ele apenas ficará pendurado sem força sob a gravidade.
* **Valores baixos** (por exemplo, 0–1): O osso fica solto e flácido, balançando livremente. Bom para cabelo longo e solto ou acessórios pendentes leves.
* **Valores altos** (por exemplo, 3–4): O osso fica rígido, permanecendo próximo da posição original e voltando rapidamente. Bom para cabelo curto ou acessórios rígidos.

#### Potência da Gravidade

|            |       |
| ---------- | ----- |
| **Faixa**  | 0 a 2 |
| **Padrão** | `0`   |

Controla a magnitude da força da gravidade que puxa o osso a cada frame.

* **0**: Sem efeito de gravidade — o osso é afetado apenas pela inércia do movimento e pela rigidez.
* **Valores baixos** (por exemplo, 0,3–0,8): Tração de gravidade sutil, boa para a maioria dos acessórios.
* **Valores altos** (por exemplo, 1,5–2,0): Tração de gravidade forte, fazendo o osso pender bastante.

#### Direção da Gravidade

|             |                                  |
| ----------- | -------------------------------- |
| **Formato** | vetor X, Y, Z                    |
| **Padrão**  | `X: 0, Y: -1, Z: 0` (para baixo) |

Define a direção da força da gravidade no espaço global. Por padrão, a gravidade puxa para baixo (Y = -1), simulando a gravidade natural.

| X  | Y  | Z  | Efeito                                    |
| -- | -- | -- | ----------------------------------------- |
| 0  | -1 | 0  | Para baixo (gravidade natural, padrão)    |
| 0  | 1  | 0  | Para cima (efeito flutuante/sobrenatural) |
| 1  | 0  | 0  | Para a esquerda                           |
| -1 | 0  | 0  | Para a direita                            |
| 0  | 0  | 1  | Para a frente                             |
| 0  | 0  | -1 | Para trás                                 |

Você pode combinar eixos (por exemplo, `X: 0,5, Y: -0,5, Z: 0`) para direções diagonais. Isso é útil para simular efeitos de vento ou criar uma aparência flutuante para personagens sobrenaturais.

#### Força de Arrasto

|            |       |
| ---------- | ----- |
| **Faixa**  | 0 – 1 |
| **Padrão** | `0.5` |

Controla com que rapidez o osso perde impulso e se acomoda. Pense nisso como resistência do ar ou amortecimento.

* **Valores baixos** (por exemplo, 0–0,2): O osso balança livremente por bastante tempo antes de se acomodar, como um pêndulo com pouco atrito.
* **Valores altos** (por exemplo, 0,7–1,0): O osso se acomoda quase instantaneamente após o movimento, dando uma sensação pesada ou amortecida.
* **1**: O osso mal se move — desaceleração máxima.

#### Centro (Opcional)

|             |                         |
| ----------- | ----------------------- |
| **Formato** | Nome do osso (dropdown) |
| **Padrão**  | Nenhum                  |

Um osso de referência opcional usado para calcular o movimento do spring bone em relação a um ponto no avatar, em vez de em relação ao espaço global. Isso evita que a spring chain balance excessivamente quando o avatar se movimenta (andando, correndo).

**Sem um osso centro**, os springs calculam a inércia no espaço global — cada passo que o avatar dá faz os ossos reagirem como se o mundo inteiro tivesse se movido, resultando em balanço exagerado durante a locomoção.

**Com um osso centro**, a simulação usa o espaço desse osso como quadro de referência, então apenas os *movimentos locais* (virar a cabeça, inclinar o corpo) acionam a reação dos spring.

Escolha o osso centro com base em onde o wearable está localizado no corpo:

| Localização do wearable                              | Osso centro recomendado |
| ---------------------------------------------------- | ----------------------- |
| Cabeça (cabelo, brincos, tiaras)                     | `Avatar_Head`           |
| Parte superior do corpo (capas, colares)             | `Avatar_Spine`          |
| Parte inferior do corpo (cintos, acessórios de saia) | `Avatar_Hips`           |


> [!WARNING]
> O osso centro **não deve** fazer parte de nenhuma spring chain. Ele deve ser um osso da armature base do avatar.


## Casos de Uso Comuns e Valores Recomendados

Estes são pontos de partida sugeridos. Ajuste a gosto usando a visualização em tempo real do Builder.

| Caso de uso                   | Rigidez   | Potência da Gravidade | Dir. da Gravidade | Arrasto   | Centro         | Notas                                                 |
| ----------------------------- | --------- | --------------------- | ----------------- | --------- | -------------- | ----------------------------------------------------- |
| Cabelo longo / rabo de cavalo | 1,0 – 2,0 | 0,3 – 0,8             | 0, -1, 0          | 0,3 – 0,5 | `Avatar_Head`  | Mais ossos na cadeia = movimento mais suave           |
| Cabelo curto                  | 3,0 – 4,0 | 0 – 0,3               | 0, -1, 0          | 0,4 – 0,6 | `Avatar_Head`  | Rigidez mais alta mantém o cabelo próximo da cabeça   |
| Brincos                       | 0,5 – 1,5 | 0,5 – 1,0             | 0, -1, 0          | 0,4 – 0,6 | `Avatar_Head`  | Cadeia curta (2–3 ossos), rigidez menor para pendurar |
| Capa / manto                  | 1,0 – 2,0 | 0,5 – 1,0             | 0, -1, 0          | 0,3 – 0,5 | `Avatar_Spine` | Múltiplas cadeias paralelas para dar largura          |
| Cinto / enfeite pendente      | 1,5 – 2,5 | 0,3 – 0,8             | 0, -1, 0          | 0,5 – 0,7 | `Avatar_Hips`  | Maior arrasto para itens mais pesados                 |
| Efeito flutuante / fantasma   | 1,0 – 2,0 | 0,5 – 1,0             | 0, 1, 0           | 0,3 – 0,5 | `Avatar_Spine` | Gravidade para cima para uma aparência sobrenatural   |

## Limitações

Tenha em mente as seguintes limitações ao trabalhar com spring bones:

* **Sem colliders**: Spring bones não colidem com o corpo do avatar nem com outros ossos. As cadeias podem atravessar a mesh do corpo em poses extremas. Versões futuras podem adicionar suporte a colliders.
* **Sem interações entre wearables**: As spring chains de cada wearable são independentes. Cadeias de wearables diferentes não afetam umas às outras.
* **Sem vento global**: Não existe uma força de vento no nível da scene. Você pode aproximar um efeito de vento por wearable ajustando o `gravityDir` para uma direção diagonal.
* **Desempenho em avatares remotos**: A simulação de spring bone é desativada para avatares que estão longe do jogador local para economizar desempenho. Avatares próximos exibirão a física de spring bones normalmente.
* **Compatibilidade com clientes alternativos**: Wearables com spring bones são compatíveis com versões anteriores de clientes alternativos e versões mais antigas do Explorer, mas os elementos de spring bone permanecerão estáticos. Em alguns casos, pode haver pequenos problemas visuais.

## Referência Técnica

A física de spring bone usa dois artefatos separados que funcionam em conjunto:

1. **O `.glb` file** armazena apenas os nomes dos ossos (com o `springbone` token) e sua hierarquia. Nenhum parâmetro de física é armazenado no arquivo do modelo.
2. **Os metadados do item do wearable** (`wearable.data.springBones`) armazenam todos os parâmetros de física como JSON.

Quando você configura parâmetros no Builder, eles são salvos nos metadados do item do wearable — o `.glb` file nunca é modificado pelas edições de parâmetros.

Você não precisa interagir com esse formato diretamente — o Builder cuida da leitura e da escrita dele. Esta seção é fornecida como referência.

### `.glb` Hierarquia de Nós (exemplo)

O arquivo do modelo contém apenas os nomes dos ossos e os relacionamentos pai-filho:

```json
{
  "asset": {"version": "2.0"},
  "nodes": [
    {"name": "Avatar_Head", "children": [1, 2]},
    {"name": "SpringBone_earring_r", "children": [3]},
    {"name": "SpringBone_hair_left", "children": [4]},
    {"name": "springbone_earring_r_tip"},
    {"name": "SpringBone_hair_left_tip"}
  ]
}
```

### Metadados do Item (`wearable.data.springBones`)

Os parâmetros de física são armazenados nos metadados do wearable, indexados pelo hash de conteúdo do GLB:

```json
{
  "version": 1,
  "models": {
    "bafkreialsvt77jvpy673cnugp5ggnxfaalfncufweayuk3jbxskh3pelkm": {
      "SpringBone_earring_r": {
        "stiffness": 0.5,
        "gravityPower": 1.0,
        "gravityDir": [0, -1, 0],
        "drag": 0.6,
        "isRoot": true,
        "center": "Avatar_Hips"
      },
      "SpringBone_hair_left": {
        "stiffness": 2.0,
        "gravityPower": 0.8,
        "gravityDir": [0, -1, 0],
        "drag": 0.4,
        "isRoot": true
      }
    }
  }
}
```

* `models` é indexado pelo hash de conteúdo do GLB. Wearables cujas representações masculina e feminina compartilham o mesmo `.glb` têm uma única entrada.
* Cada entrada de osso usa o nome exato do nó do `.glb` (com distinção entre maiúsculas e minúsculas).
* ossos de ponta (por exemplo, `springbone_earring_r_tip`) não precisam de uma entrada de metadata — servem apenas como extremidades geométricas.

### Parâmetros de Metadata

| Parâmetro      | Tipo    | Padrão       | Descrição                                                                            |
| -------------- | ------- | ------------ | ------------------------------------------------------------------------------------ |
| `stiffness`    | float   | `2.0`        | Força de restauração em direção à pose de repouso. Intervalo: 0–4.                   |
| `gravityPower` | float   | `0`          | Magnitude da força da gravidade. Intervalo: 0–2.                                     |
| `gravityDir`   | vec3    | `[0, -1, 0]` | Direção da gravidade no espaço do mundo.                                             |
| `drag`         | float   | `0.5`        | Fator de amortecimento / desaceleração. Intervalo: 0–1.                              |
| `isRoot`       | boolean | —            | Se este nó é a raiz de uma cadeia de molas. É necessário ser `true` para root bones. |
| `center`       | string  | —            | Opcional. Nome de um bone de referência para o cálculo de inércia relativa.          |


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-pt/wearables-e-emotes/wearables/spring-bones.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
