# Colliders

Saiba como adicionar colliders a modelos 3D importados para o Decentraland.

Para ativar colisões entre um modelo 3D e os usuários da sua cena, você precisa atribuir colliders ao modelo. Sem um collider, os jogadores conseguem atravessar os modelos como se eles não estivessem lá. Colliders também são necessários para fazer um objeto responder aos pointer events do jogador ou bloqueá-los, ou para ser intersectado por raycasts ou bloqueá-los. Por motivos de desempenho, colliders geralmente são modelados separadamente como uma geometria invisível mais simples, com menos vertices do que a geometria visível.


> [!NOTE]
> **💡 Dica**: Instale o [plugin Decentraland Tools para Blender](https://extensions.blender.org/add-ons/decentraland-tools/). Ele inclui várias funções úteis para ajudar você a editar e exportar modelos 3D, wearables e emotes.


O modelo pode usar uma geometria invisível como collider ou, no SDK7, você também pode atribuir que a geometria visível se comporte como um collider. Veja [**Colliders em modelos 3D**](/docs/creator/sdk7/3d-essentials/colliders/#colliders-on-3d-models) . Você também pode atribuir diferentes layers de colisão às diferentes geometrias para controlar separadamente a física do jogador, os pointer events e outras layers personalizadas; veja [**Colliders em modelos 3D**](/docs/creator/sdk7/3d-essentials/colliders/#collision-layers) .

Colliders não afetam como modelos e entities interagem entre si; eles sempre podem se sobrepor. Colliders afetam apenas como o modelo interage com o avatar do jogador.

Para que um objeto seja reconhecido por uma cena da Decentraland como um collider, tudo o que ele precisa é ser nomeado de uma certa forma. O nome do objeto deve incluir o sufixo “\_collider” no final.

Por exemplo, para criar um collider para um poste de luz, você pode criar um objeto de caixa simples envolvendo o seu tronco. Os jogadores na cena não verão essa caixa, mas ela bloqueará o caminho deles.

![](/docs/_remote/872cedc3afd092432696701c.webp)

Nesse caso, podemos nomear a caixa “LampPost*collider” e exportar tanto a árvore quanto a caixa como um único modelo \_\_gltf. A tag \*\_\_collider*\* avisa ao motor de mundos da Decentraland que o objeto caixa pertence à coleção de colliders, tornando a malha do collider invisível.

Sempre que um jogador visualizar o modelo do poste de luz na sua cena, ele verá o modelo complexo da sua luminária. No entanto, quando caminhar até ele, colidirá com a caixa, não com a árvore.

## Restrições Espaciais do Jogador

Ao projetar modelos destinados a serem caminhados por jogadores, você precisa ter em mente as seguintes medidas de referência:

* Os jogadores podem subir um degrau de até cerca de 40 centímetros sem pular. Um degrau de 45 centímetros ou mais exige um salto. Para escadas que sejam confortáveis, mantenha cada degrau com 25 centímetros de altura ou menos e pelo menos 30 centímetros de profundidade.
* Os jogadores podem subir inclinações de até 46 graus sem pular. Em superfícies mais íngremes, o avatar escorrega para fora. Para caminhar com facilidade, mantenha rampas com 30 graus ou menos. Um collider de rampa sobre escadas é a exceção: ele segue a inclinação das escadas, cerca de 40 graus para degraus de 25 centímetros de altura e 30 centímetros de profundidade (veja [Adicionar um collider a uma escadaria](#add-a-collider-to-a-staircase)).
* Os jogadores podem saltar até 1,8 metro de altura
* Os avatares têm cerca de 1,9 metro de altura, mas considere que eles podem estar usando chapéus ou outros wearables na cabeça.
* O collider do avatar é uma cápsula com 0,6 metro de largura (raio de 0,3 metro). Uma viga ou teto com menos de cerca de 1,85 metro impede o avatar. Deixe pelo menos 2 metros de altura livre em qualquer lugar por onde os jogadores passem. Em ambientes internos, deixe bem mais espaço para a câmera, veja [Constrói interiores maiores do que na vida real](/docs/creator/content-creator-pt/scenes-sdk7/conceber-a-experiencia/design-games/#build-interiors-larger-than-real-life).

Sempre que um jogador visualizar o modelo da árvore na sua cena, ele verá o modelo complexo da sua árvore. No entanto, quando entrar na sua árvore, ele colidirá com a caixa, não com a árvore.

### Adicionar Um Collider A uma Escadaria

Escadas são um caso de uso muito comum para objetos collider. Para que os jogadores subam escadas, deve haver um objeto correspondente \_collider sobre o qual os jogadores consigam pisar.

Recomendamos usar um objeto em rampa para os colliders da sua escada; isso proporciona uma experiência muito melhor ao subir ou descer. Quando eles subirem sua escada, isso parecerá uma subida ou descida suave, em vez de exigir que “pulem” cada degrau individual.

Usar um objeto em rampa também evita criar geometria desnecessária, poupando espaço para outros modelos mais complicados. Tenha em mente que a geometria do collider também é levada em conta ao calcular [**limitações da scene**](/docs/creator/sdk7/optimizing/scene-limitations/)

1. Crie um novo objeto com o formato de uma rampa que se pareça com o tamanho e as proporções das escadas originais.

![](/docs/_remote/8162fc04d84389328f79ddb3.webp)

2. Nomeie o objeto de rampa com algo semelhante a S*tair\_collider*. Ele deve terminar em \_*collider*.
3. Sobreponha o objeto de rampa às escadas para que ocupem o mesmo espaço.

![](/docs/_remote/fab0dd4c3439215495a72aa2.webp)

4. Exporte ambos os objetos juntos como um único *glTF* modelo.

Agora, quando os jogadores visualizarem as escadas na sua cena, eles verão o modelo mais elaborado das escadas, mas quando as subirem, colidirão com a rampa.

## Otimizando colliders

* Prefira usar **Box**, **Sphere** e **Capsule** malhas de colisão, porque isso terá melhor desempenho no carregamento e em runtime.
* Considere aproximar uma malha de colisão complexa com várias malhas de colisão Primitive (**Box**/**Sphere**/**Capsule**)

![](/docs/_remote/195d7d652ad646f33e815b49.webp)

*Exemplo: o collider de um anel pode ser composto por vários colliders Capsule ou Box*

* Coloque a malha de colisão dentro da Hierarchy do objeto ao qual ela está relacionada. Por exemplo, Spaceship\_collider pode ser um filho dentro do pai Spaceship (do ponto de vista da Hierarchy)

### Colliders Convexos vs Côncavos

Se usar um collider de mesh arbitrária for a única possibilidade, então torne a malha de colisão convexa (não côncava). Isso evitará situações físicas estranhas, como travamentos, ficar preso dentro do collider ou atravessá-lo. Note que muitas vezes é possível imitar uma mesh côncava com várias meshes convexas.

![](/docs/_remote/f40bda0817152df8a3cd5005.webp)

## Nomeação de colisão

**Use nomes significativos. O nome deve dar contexto de onde o asset é usado ou a que parte do objeto ele se relaciona.**

Para nomeação de assets, usamos uma mistura de `PascalCase` e `snake_case`, que podemos chamar de `PascalSnake_case`. As regras básicas são: uma nova palavra ou uma palavra após o separador (`_`) começa com **letra maiúscula.**

Collision Mesh é a mesh usada como referência para criar um collider no Explorer

* Ela segue os mesmos princípios das Meshes acima, mas tem `_collider` o sufixo no final **Exemplos:** 🟢 **Prefira** nomes - `Tram_Wheel_LeftFront_collider` 🔴 **Evite** nomes - `Primitive.001_Collider`, `collider (1)`, `Cylinder.007_collider`,


> [!WARNING]
> **🔥Dica de otimização🔥**
> 
> Se um collider puder ser aproximado por uma forma Box, Sphere ou Capsule, então use um sufixo adicional com uma destas 3 variações possíveis: \_box, \_sphere ou \_capsule. Usar essas 3 formas em vez de uma mesh arbitrária para detecção de colisão aumentará o desempenho da sua cena e o tempo de carregamento 💪🚀
> 
> Exemplos: 🟢 Teater\_MainWall\_North\_collider\_box, Fountain\_Statue\_collider\_sphere


## Boas práticas para colliders

* **Sempre use o menor número possível de triângulos ao criar colliders.** Evite fazer uma cópia de um objeto complexo para usar como collider. Colliders simples garantem uma boa experiência do usuário e mantêm sua cena dentro dos limites de triângulos.
* **Objetos collider não devem ter nenhum material**, pois os jogadores da sua cena nunca os verão. Colliders são invisíveis para os jogadores.
* **Todos os nomes dos objetos collider devem terminar com \_*****collider*****.** Por exemplo, T*ree\_collider*.
* **Se você usar um \_plane**\_\*\* como collider, ele bloqueará apenas em uma direção.\*\* Se você quiser que colliders bloqueiem dos dois lados, por exemplo para uma parede, você precisa criar dois planos com suas normais voltadas em direções opostas.
* Ao duplicar objetos collider, preste atenção aos nomes deles. Alguns programas acrescentam um \_*1* ao final do nome do arquivo para evitar duplicatas, por exemplo *tree\_collider\_1*. Objetos nomeados assim serão interpretados pelo Decentraland World Engine como objetos normais, não como colliders.
* Você pode evitar adicionar uma mesh de collider se adicionar um `MeshCollider` componente invisível à entity, que aproxima a forma do modelo 3D.
* Você também pode evitar adicionar uma mesh de collider se configurar o `GLTFContainer` componente para usar a geometria visível como collider. Veja [**Colliders em modelos 3D**](/docs/creator/sdk7/3d-essentials/colliders/#colliders-on-3d-models) .
* Se você estiver criando um modelo 3D com muitas meshes que precisam do seu próprio collider, há alguns addons úteis que podem ajudar. No Blender, você pode usar a opção de renomeação em lote dentro do Blender \*\*\*\*para adicionar automaticamente um `_collider` sufixo a todos os nomes das meshes em um grupo. Mais sobre essa ferramenta na seção “**Usando o Blender para renomear assets” section**


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-pt/modelacao-3d-e-animacoes/colliders.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
