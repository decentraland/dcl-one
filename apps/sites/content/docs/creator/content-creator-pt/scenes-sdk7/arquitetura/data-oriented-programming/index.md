> A Programação Orientada a Dados é uma abordagem poderosa de programação que aproveita ao máximo o desempenho.

# Programação Orientada a Dados

Programação Orientada a Dados é uma abordagem poderosa de programação que resulta em grandes melhorias de desempenho. Ela se concentra em tratar *dados* como o elemento central, com todo o resto organizado em torno dele, para acessar ou modificar esses dados. Essa abordagem também é muito amigável para multiplayer, pois torna os dados que precisam ser sincronizados entre players mais fáceis e rápidos de acessar.

Programação Orientada a Dados incentiva você a pensar em tudo na sua scene como dados que precisam ser copiados e mutados ao longo dos vários systems. O principal benefício dessa abordagem é otimizar a velocidade com que os dados podem ser lidos da memória, que muitas vezes é o principal gargalo ao executar aplicações e jogos modernos.

Devido a essa melhoria notável de desempenho, grande parte da indústria de desenvolvimento de jogos vem migrando para adotar essa abordagem nos últimos anos.

O SDK da Decentraland executa scenes em JavaScript, e uma desvantagem dessa linguagem é que ela não oferece controle sobre alocação de memória. A Engine que executa a Decentraland, no entanto, usa C#, que se beneficia muito ao seguir princípios orientados a dados. O SDK e a Engine estão constantemente enviando mensagens entre si. Para tornar essa comunicação o mais eficiente possível, faz sentido manter as estruturas de dados dos dois lados o mais semelhantes possível, para evitar ter que reorganizar esses dados constantemente.

## Como funciona

Programação Orientada a Dados é diferente de Programação Orientada a Objetos, uma abordagem com a qual muitos desenvolvedores estão atualmente familiarizados. Em Programação Orientada a Objetos, o código é estruturado seguindo abstrações que tentam replicar construções do mundo real: objects. Cada um desses objects pode conter tanto dados quanto funcionalidade. Applications que usam essa abordagem costumam ser fáceis de planejar conceitualmente, mas também mais ineficientes para executar.

Na Programação Orientada a Dados, os dados não são estruturados em torno de objects; eles são estruturados para otimizar sua facilidade de acesso. As construções do mundo real que os dados representam não têm papel nos diferentes fluxos que mutam esses dados.

O modelo Entity Component System (ECS) no qual o SDK da Decentraland é construído é muito compatível com a abordagem de Programação Orientada a Dados. Cada component faz parte de uma coleção estruturada de dados. Components pertencem a uma entity por referência, mas os dados não são estruturados em torno da entity; os dados são estruturados como uma coleção de components semelhantes. Por exemplo, todos `Transform` components em uma scene são iguais. Um desses transforms pode pertencer ao modelo do seu prédio principal, outro a um vidro que é filho de uma table. Os systems na scene então processam a lista de `Transform` components um por um, sem fazer nenhuma distinção. Todos `Transform` components têm os mesmos campos e passam pelas mesmas verificações.

![](/docs/_remote/29703f638d6018eac3f30fbf.webp)

Imagine uma scene que tem uma dúzia de doors que podem estar abertas ou fechadas. Você pode representar o estado de todas essas doors como um componente simples "isOpen" que contém um valor booleano. Se "isOpen" for true, a door deve estar aberta; se for false, a door deve estar fechada. Se um player clicar em uma door, ela deve alternar de estado, e os outros players também devem vê-la alternar. Enquanto sua scene está processando uma mudança no estado de uma door e sincronizando isso com outros players, a scene não se importa realmente com o que "isOpen" representa. Todo o conjunto de components é apenas uma coleção de booleanos que precisam ser sincronizados com outros players. Um system separado na sua scene pode então cuidar de corresponder regularmente o estado de cada "isOpen" à rotação da door correspondente.

Programação Orientada a Dados não é necessariamente mais difícil, mas é uma abordagem diferente que precisa ser aprendida e adotada. Desenvolvedores que não estão acostumados com essa abordagem podem precisar de algum tempo para se familiarizar com ela; encorajamos você a explorar e brincar com exemplos para ganhar mais prática.

## Por que funciona

Para entender por que a Programação Orientada a Dados faz tanta diferença, precisamos dar uma olhada no hardware.

Quando o processador precisa buscar dados da memória, ele traz um bloco inteiro de dados para o cache, incluindo dados que por acaso estejam gravados na memória ao lado do valor que queríamos. Quanto mais seu código puder usar dados que já estão no cache, mais rápido seu código vai executar.

Imagine que a memória da máquina seja um armazém literal, onde os dados são armazenados em muitas caixas empilhadas. Sempre que precisamos de uma certa parte de dados, precisamos chamar uma empilhadeira para buscar a caixa onde esses dados estão localizados e trazê-la até o balcão para inspeção. Esse balcão só consegue comportar algumas caixas por vez, então você não pode manter muitos dados por perto.

Leva muito tempo para a empilhadeira ir até o fundo do armazém e trazer uma caixa para nós. Se os dados que você quer dessas caixas estiverem espalhados, isso significa pedir para a empilhadeira fazer muitas viagens. Na maior parte do tempo, você vai ficar de braços cruzados no balcão, esperando a próxima caixa que solicitou chegar.

Você pode evitar grande parte desse tempo desperdiçado se empilhar essas caixas de forma inteligente e organizar os dados para que as coisas de que você provavelmente precisará ao mesmo tempo fiquem em grande parte agrupadas. Se você agrupar os dados de maneira inteligente, muitas vezes vai descobrir que a próxima coisa de que precisa já está em uma das caixas do balcão. Você poderá ir direto a ela sem incomodar o operador da empilhadeira.

Por exemplo, se você tem uma scene que precisa buscar o estado de uma door para verificar se ela está aberta ou fechada, o hardware não está apenas buscando o valor do boolean específico que descreve o estado dessa door; ele está buscando um monte de outros dados que podem ou não ser relevantes.

Suponha que sua scene tenha um system que precise atualizar o estado aberta/fechada de uma dúzia de doors, uma vez por frame. Se o seu código estiver organizado seguindo uma abordagem de Programação Orientada a Objetos, não há como saber como os diferentes pedaços relevantes de informação podem estar agrupados. Talvez uma "caixa" do nosso armazém contenha o estado "isOpen" da door A e também contenha a textura da door A e o áudio que ela reproduz ao abrir. Talvez você precise fazer uma viagem para buscar uma nova "caixa" de dados para cada uma das dúzias de doors. E, claro, todo esse processo precisa acontecer de novo a cada frame. Então são 360 (30 x 12) viagens metafóricas até o fundo do armazém por segundo, mesmo quando nenhuma das doors mudou de estado.

Se o seu código seguir uma abordagem Orientada a Dados, por outro lado, é muito provável que todos esses 12 booleanos estejam na mesma caixa. Isso acontece porque todos esses booleanos fazem parte de um único array que foi gravado na memória de uma só vez. Você nunca organiza explicitamente como esses dados se encaixam na memória, então, no pior cenário, o array pode acabar dividido entre duas caixas. Mas, mesmo nesse pior cenário, 2 viagens é muito melhor do que 12.

Verificar o estado de cada door em cada frame parece muito trabalho, mas na verdade é super rápido se todos os dados já estiverem no cache de memória. Se os seus dados estiverem bem organizados, sua scene pode executar processos como esses sobre muitas entities e ainda assim rodar muito rápido.


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-pt/scenes-sdk7/arquitetura/data-oriented-programming.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
