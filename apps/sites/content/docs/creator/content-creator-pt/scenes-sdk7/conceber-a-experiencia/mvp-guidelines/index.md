> Diretrizes recomendadas para produzir sua primeira scene ou experiência MVP usando o SDK

# Diretrizes de MVP

O objetivo deste documento é ajudá-lo a orientar o processo de construção das primeiras iterações de scenes em Decentraland. Chamaremos isso de um Produto Mínimo Viável (MVP).

**Ao criar o Produto Mínimo Viável (MVP) da sua scene, você precisa pensar em duas áreas de foco:**

1. A experiência básica do usuário e a funcionalidade do seu projeto.
2. A criação de um "pipeline" básico, ou fluxo de trabalho da equipe e sistema de gerenciamento de conteúdo para construir sua experiência e melhorá-la iterativamente.

Um MVP não deve tentar demonstrar todos os resultados possíveis de todas as experiências possíveis. Em vez disso, um MVP deve ser a melhor primeira impressão da sua experiência que você possa criar usando o SDK da Decentraland.

É importante considerar suas próprias limitações, como você planeja fornecer conteúdo aos seus usuários e as expectativas dos seus usuários. Abordar seu MVP dessa forma requer três perspectivas diferentes:

1. Como desenvolvedor ou produtor, como entrego uma experiência ao meu usuário/jogador?
2. Como usuário ou jogador, o que espero desta experiência?
3. Como colaborador ou stakeholder, como contribuo para o pipeline ou para a experiência?

É importante distinguir essa abordagem do desenvolvimento ágil tradicional, porque você pode ter que usar métodos não ideais para alcançar suas metas de design.

Você terá que examinar seus próprios objetivos no contexto das expectativas dos seus usuários para decidir se um determinado release está mais focado no jogador, no pipeline e nos colaboradores de conteúdo, ou um pouco de ambos.

Ao planejar cada release, é fundamental que você defina conscientemente e deliberadamente suas prioridades de acordo com cada uma dessas três perspectivas.

Você pode esperar que seu backlog de desenvolvimento siga duas frentes:

* O backlog das experiências de usuário que você deseja criar.
* O desenvolvimento das ferramentas e interfaces necessárias para construir seu pipeline de entrega. (Ou para otimizar seu pipeline existente tanto para colaboradores quanto para sua equipe de desenvolvimento.)

Essas duas frentes também seguirão duas abordagens diferentes de teste:

* Testar suas experiências de usuário se assemelha mais ao teste tradicional de interface do usuário e não requer os mesmos recursos de scripting.
* Testar suas ferramentas e interfaces de pipeline exigirá mais recursos técnicos.

Quanto mais cedo você conseguir colocar uma proposta de valor diante do seu usuário ou jogador, mais cedo poderá obter feedback para confirmar ou rejeitar essa proposta. Confirmar valor rapidamente é fundamental. Muitos desenvolvedores experientes compartilham histórias de como tinham certeza absoluta de que uma nova mechanic seria incrível até usá-la e ela parecer estranha e cheia de falhas, os jogadores não reagirem a ela de forma alguma, ou ela não resolver uma necessidade/desejo do consumidor. Você quer falhar rapidamente com o mínimo de esforço possível, para poder aprender com seu fracasso e planejar a próxima iteração.

Como falhar rapidamente? Você faz o mínimo necessário para fazer seu jogador interagir com seu produto.

## Fatores para Produtos Mínimos Viáveis

Aqui está a lista de fatores a considerar para o seu MVP básico. É aceitável declarar que você usará algo como um placeholder e depois o substituirá à medida que desenvolver uma substituição mais sólida.

1. Criação de Arte
   * Primeiro, comece com imagens estáticas básicas
   * Seu primeiro teste deve ser de estilo: o estilo que você escolheu agrada aos seus usuários?
   * Isso pode ser o início de um guia de estilo para fornecer a um artista terceirizado
2. Criação de scene
   * Desenvolva uma noção básica do seu espaço
   * O jogador deve sentir que está em um espaço novo e único
   * Delimite seu espaço em relação aos espaços vizinhos
   * As fronteiras são evidentes e óbvias – ainda que apenas por uma linha desenhada
   * Cubra toda a área com conteúdo/arte estática
3. Arte renderizada na scene
   * Usar billboards está ok, assim como outra sinalização (isso pode ser simplesmente billboards reais ou sprites mais sofisticados voltados para a câmera)
   * Estabeleça o tom e a estética do seu espaço (ou seja, estilo, claro, escuro)
   * Anote seu processo: como a arte foi criada e implantada na scene?
   * Como você quer organizar seus arquivos de arte para implantação repetida?
4. Experiência do jogador
   * Os jogadores conseguem visitar seu espaço/scene
   * Os jogadores conseguem distinguir seu espaço dos espaços vizinhos
5. Objetivos do pipeline
   * Implantar uma scene estática de exemplo: sem interação com o jogador
   * Implantar uma scene animada: elementos como fontes de água ou bandeiras tremulando repetem suas animações
   * Implantar uma scene interativa: incluindo engajamento do jogador
   * Demonstrar o pipeline de implantação reimplantando conteúdo: da criação da arte à scene, incluindo scripting + QA]
   * Expor lacunas do pipeline: identificar os pontos desconhecidos em áreas específicas de implantação de conteúdo

## Níveis de protótipos

Falhar rapidamente permite que você desenvolva sua experiência criando protótipos sucessivos, com cada iteração construindo sobre a anterior.

**Comece com um protótipo para um único jogador. Depois, você pode planejar o scripting de interações multiplayer. Por fim, você pode abordar seu persistent core loop que demonstra camadas transacionais.**

**O que é um persistent core loop?**

No design de jogos, um persistent core loop é o “game loop” fundamental que impulsiona as ações do jogador e a resposta do jogo a essas ações. Esses loops persistentes se estendem a qualquer forma de experiência virtual (como as fornecidas por Districts).

**O que são camadas transacionais?**

As camadas transacionais são as interfaces entre systems, como uma atualização na blockchain ou outra aplicação que tenha sido integrada à sua experiência para manter um registro persistente das ações do jogador. Criar e manter esse registro persistente é o que constrói uma experiência mais pessoal.

Recomendamos criar seu MVP como uma experiência para um único jogador.

Por exemplo, você poderia projetar uma scene com as seguintes experiências sucessivas:

* Um único jogador pode entrar no mundo.
* O jogador pode interagir com uma ou duas entities simples dentro da scene.
* Outros jogadores podem entrar e interagir com o mundo e com o outro jogador.
* Por fim, você pode adicionar a capacidade de lembrar que cada jogador entrou na scene e de registrar os eventos e atividades dos jogadores.

## Como compartilhar seu MVP

Recomendamos testar as alterações com usuários de teste antes de enviar uma nova versão da sua scene para Decentraland. Você pode publicar sua scene para um [Decentraland World](/docs/creator/content-creator-pt/scenes-sdk7/publicacao/publishing-options/#decentraland-worlds) para compartilhá-la por meio de um link sem precisar de nenhum LAND e sem afetar o conteúdo em produção.

## Considerações adicionais

Quando os casos de uso básicos estiverem cobertos, você pode começar a sofisticar sua estratégia de gerenciamento de releases focando em mechanics. **Mecânicas** são um termo amplo que abrange todas as ações que um jogador pode realizar e as respostas que o sistema fornecerá com base nessas ações do jogador.

**Interoperabilidade de dispositivos** é algo importante de se ter em mente. Os usuários da sua scene podem estar acessando sua scene usando um desktop, um dispositivo móvel ou um headset de VR. Os usuários devem conseguir interagir com sua scene razoavelmente bem usando qualquer um deles. Para quem estiver usando um headset de VR, tente evitar movimentos que causem tontura e possam provocar enjoo de movimento.

**Áudio** é outro aspecto crítico da atmosfera de uma scene. Sons de fundo como vento, grilos, conversas distantes, talvez até música, podem ser uma forma muito poderosa de aumentar a imersão e dar contexto. Você também pode alterar a forma como os níveis de volume se relacionam com a distância da fonte sonora para dar mais ou menos ênfase à localização de um som.

Leia [restrições de design para jogos](/docs/creator/content-creator-pt/scenes-sdk7/conceber-a-experiencia/design-games/) para uma análise detalhada de várias outras considerações.

Considere o MVP como um de muitos protótipos que você pode usar para estabelecer sua cadência de releases depois de estabelecer seu pipeline. O foco de cada release pode variar, ou pode ser um híbrido de cada aspecto da experiência. No entanto, você deve buscar entregar experiências progressivamente mais complexas, com cada iteração construindo sobre a anterior.

1. **MVP**: Um único jogador
2. **Release 2**: Adicione suporte a multiplayer e/ou interação
3. **Release 3**: Introduza sua primeira mecânica
4. **Release 4**: Adicione suporte a áudio
5. **Release 5**: Finalize seu pipeline de arte

Por exemplo, digamos que estamos construindo um MVP para um jogo de golfe de Frisbee. O MVP incluirá algumas imagens estáticas do percurso. O jogador talvez até consiga arremessar um disco, de uma forma muito rudimentar, em estilo de blocos. Isso nos permite ajustar nossas mecânicas básicas de arremesso. O próximo release pode incluir um protótipo de suporte a multiplayer, para que possamos demonstrar e testar dois usuários conectados e jogando em nosso LAND ao mesmo tempo.

Lembre-se: embora o objetivo final seja um mundo 3D verdadeiramente imersivo, é aí que seu MVP começará. Fazer com que um jogador entre no seu mundo o mais rápido possível deve ser seu primeiro objetivo. Levar semanas, e não meses, para testar seus releases é fundamental para aprender e iterar sem desperdiçar esforço.

Recomendamos fortemente que você mantenha em mente a primeira impressão que sua experiência apresenta. Uma experiência vazia deixará os jogadores decepcionados. Por outro lado, uma scene com algum conteúdo inicial e experiências básicas mostra aos jogadores o potencial do que está por vir e os incentiva a interagir com sua comunidade e retornar para os próximos releases.

## Fatores de persistência a considerar

Em última análise, você quer alcançar um nível de persistência em que possa demonstrar que as camadas transacionais da sua arquitetura estão operacionais. O aspecto transacional não se limita às ações dos jogadores, mas também às reações do sistema aos jogadores.

1. **Informações da conta**: nome de login, fuso horário, localização para sua experiência/jogo específica
2. **Estatísticas da leaderboard**: resultados anteriores de partidas, classificações globais/regionais, competições
3. **Validação de identidade**: endereço da wallet Ethereum, ou qualquer outro gerenciamento de identidade no backend
4. **Atualizações da blockchain**: conforme necessário, com base na sua experiência/jogo, para atualizar o ledger da blockchain para transparência transacional
5. **Persistência em runtime**: dados temporários para persistência em uma plataforma potencialmente distribuída (ou seja, saúde apenas para a experiência de jogo individual)


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-pt/scenes-sdk7/conceber-a-experiencia/mvp-guidelines.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
