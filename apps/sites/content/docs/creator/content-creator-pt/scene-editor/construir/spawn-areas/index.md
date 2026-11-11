> Usando Áreas de Spawn para definir visualmente a posição de spawn e a posição da Spawn Camera.

# Áreas de Spawn

O criador pode definir o **Posição de Spawn** e **Alvo da Câmera de Spawn** no Creator Hub editando uma **Área de Spawn** Entity.

Isto dá ao criador uma forma visual de definir a Área de Spawn e a direção para a qual o avatar estará voltado quando for spawnado, como acontece com outros objetos na scene.

As Spawn Areas definem onde os jogadores aparecem quando acessam sua scene diretamente, seja digitando as coordenadas ou teleportando. Sua scene pode ter objetos que bloqueiam os jogadores de se moverem se eles acontecerem de spawnar exatamente sobre eles, como árvores ou escadas, ou sua scene pode ter um terreno elevado. Seria uma má experiência para os jogadores se eles spawnassem sobre algo que não os deixasse se mover. Por isso, você tem a opção de definir uma ou várias posições de spawn em locais ad hoc.


> [!WARNING]
> **📔 Nota**: Todos os spawn points devem estar dentro dos parcels que compõem a scene. Você não pode fazer spawn de um jogador fora do espaço desses parcels.


<img src="/docs/_remote/04f3a7c93b45dca9a38de100.webp" alt="" width="200">

A Spawn Area é composta por duas Entities diferentes que trabalham juntas:

* **Spawn** Entity: pode ser encontrada dentro da **Player** Entity. Define o Spawn Point real do Player.

<img src="/docs/_remote/da6fb5ebe275854d0b596645.webp" alt="" width="200">

* **Camera Target** Entity: pode ser encontrada dentro da **Área de Spawn** Entity. Mova-a para a direção desejada na qual o Player será spawnado.

<img src="/docs/_remote/13b486809fcb8431fd6ecb7c.webp" alt="" width="200">

## Alterando Spawn Areas

**Spawn Areas** e seus **Camera Target** podem ser movidos diretamente do Scene Editor como qualquer outro objeto na scene, ou modificar seus valores manualmente (como com Transforms em outras Entities).

### Parâmetros

Quando uma Spawn Area é selecionada, o criador consegue ver e definir os seguintes valores na seção Components:

<img src="/docs/_remote/81d5fae5f7131c97a7c0709a.webp" alt="" width="600">

* **Posição**: Posição de Spawn do Avatar. São coordenadas dentro da scene, relativas à origem da scene, semelhantes ao que você usaria em um componente Transform.
* **Alvo da Câmera de Spawn**: A direção para a qual a camera (e o avatar) estarão voltados quando spawnados na scene. Use isto para ter mais controle sobre a primeira impressão do jogador quando ele entrar na sua scene.
* **Área Aleatorizada**: Adiciona aleatoriedade à Spawn Area em metros, permitindo ativá-la e desativá-la com o **Não aleatorizar** toggle. Quando ativada, uma área é renderizada no Scene Editor, mostrando a área na qual o Player pode ser spawnado aleatoriamente. O deslocamento aleatório é aplicado nos eixos X e Z e evita que todos os jogadores apareçam sobrepostos quando spawnam, o que fica especialmente ruim em scenes lotadas.

<img src="/docs/_remote/87b06caf636a947bd765acb5.webp" alt="" width="600">

## Múltiplas Spawn Areas

<img src="/docs/_remote/80f2cf42b9eb7797e76dccd8.webp" alt="" width="600">

O criador pode ter várias **Spawn Areas** definidas. Para criar uma nova Spawn Area, siga estes passos:

1. Selecione qualquer Spawn Area, **spawn1** sendo a padrão.
2. Todas as Spawn Areas aparecem no painel da direita. Você pode adicionar novas clicando em **+ Adicionar Nova Spawn Area**&#x20;

   <img src="/docs/_remote/2d3066e023e44c9ba5f38990.webp" alt="ícone" width="200">

   .

<img src="/docs/_remote/24547b03ea234f33b90a044e.webp" alt="" width="600">

3. Ative o **Spawn Principal**&#x20;

   <img src="/docs/_remote/745070068b65895eedbc404a.webp" alt="ícone" width="100">

   &#x20;da Spawn Area desejada. Se houver muitas Spawn Areas ativadas como **Spawn Principal**, o jogador aparecerá aleatoriamente em uma delas.
4. Pré-visualize a scene.


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-pt/scene-editor/construir/spawn-areas.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
