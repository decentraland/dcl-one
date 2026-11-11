> Dicas e práticas para criar cenas nas quais os jogadores vão querer ficar e voltar!

# Guia de UX e UI

É de importância vital para nós que os criadores de conteúdo da Decentraland tenham liberdade para fazer uso da sua criatividade em toda a sua extensão. Isso é essencial para que a Decentraland se torne um lugar recompensador para explorar. No entanto, também é importante ter em mente que as scenes na Decentraland serão visitadas por uma grande variedade de usuários e queremos tornar as scenes acessíveis a todos eles. Por isso, não podemos ignorar a necessidade de estabelecer critérios de design unificados, um guia que possa garantir que os jogadores tenham uma experiência uniformemente intuitiva e agradável, não importa em qual scene estejam.

Neste documento, compartilhamos os Valores de Design da Decentraland, com o objetivo de promover o que consideramos serem as melhores práticas para projetar a interface e a experiência dos jogadores. Esperamos que estes critérios possam servir como um ponto de partida para construir e/ou melhorar as scenes que compõem o Metaverse.

## Valores de design

Todas as scenes na Decentraland devem ser projetadas tendo estes valores em mente:

* **Acolhedor**: O jogador se sente calorosamente acolhido.
* **Fácil de usar**: A UI é fácil e divertida de usar – você deve evitar reinventar a roda
* **Fácil de aprender**: Os jogadores acham a UI familiar e intuitiva. Os padrões devem ser usados de forma consistente ao longo da scene.
* **Oferecer orientação**: A scene oferece uma mão amiga. Texto, movimento, som e gráficos guiarão os jogadores e lhes darão pistas na direção certa.
* **Reativo**: Ação-reação clara às entradas dos jogadores.
* **Minimalista**: Menos é mais. Os jogadores podem concentrar sua atenção no que importa.
* **Interessante**: A scene aproveita as coisas que tornam a Decentraland única e digna de visita.
* **Com propósito**: Os jogadores têm um forte motivo para voltar.
* **Agradável**: Os jogadores apreciam como as coisas parecem, soam e se sentem

## Experiência do usuário

### Por onde começar?

Qual é o objetivo da sua scene? É oferecer uma paisagem bonita para contemplar e explorar? É um museu? É um jogo para um jogador? Um competitivo? Ou segue um fluxo linear de narrativa?

É importante estar bem ciente desse objetivo em cada etapa do processo de design; ele deve guiar todas as suas decisões, todas as quais podem afetar profundamente a experiência do visitante.

Para scenes que seguem um fluxo linear, em que cada etapa depende da anterior, recomendamos delimitar a scene com paredes e estabelecer um ou vários pontos de entrada fixos, para evitar que os jogadores esbarrem nas coisas na ordem errada. A propósito, os pontos de entrada são um excelente lugar para exibir uma mensagem de boas-vindas ao jogador e explicar qual deve ser seu objetivo na scene.

<figure><img src="/docs/_remote/978e0e21e5bb4482320c0ae1.webp" alt="In-world banner" width="800"><figcaption><p>Instruções no mundo</p></figcaption></figure>

Se a experiência do jogador na sua scene não tiver a intenção de ser linear, mas você ainda precisar fornecer algumas instruções básicas para garantir que ele a aproveite plenamente, você pode mostrar instruções em um pop-up. É possível exibir um pop-up na tela do jogador assim que ele entrar, independentemente da direção de onde venha, e assim garantir que os jogadores sempre saberão o essencial.

<figure><img src="/docs/_remote/bab29f1240d71035b8426a51.webp" alt="Pop-up banner" width="800"><figcaption><p>Instruções em pop-up</p></figcaption></figure>


> [!WARNING]
> **📔 Nota**: Certifique-se de que o pop-up não seja muito intrusivo e que seja fácil fechá-lo clicando em qualquer lugar.


Uma mensagem de boas-vindas deve comunicar o seguinte ao jogador:

* O título da scene
* Dê boas-vindas ao jogador
* Objetivo final e motivação para o jogador
* Instruções/próximos passos
* *(Opcional)* Controles. É mais útil se a sua scene usar eventos globais de botão. Caso contrário, os próprios itens indicam como usá-los ao passar o cursor sobre eles.

### Motivação

Você provavelmente vai querer que seus visitantes continuem voltando para a sua scene; pense em maneiras de motivá-los a fazer isso! Pergunte-se: “O que faria um jogador querer retornar à minha scene, ou recomendá-la a um amigo?”

Por exemplo, você pode desafiá-los, dando algo a alcançar que seja difícil o suficiente para exigir várias tentativas. Você também pode organizar eventos competitivos multiplayer, que são também uma oportunidade interessante para socializar. Faça da sua scene um lugar dinâmico, onde os jogadores sintam que gostam de passar o tempo lá!

### Feedback e Som

O som desempenha um papel duplo e fundamental; não deixe de usá-lo! Por um lado, ele é vital para gerar imersão, muitas coisas não parecerão reais se não forem acompanhadas de som. A outra coisa crucial para a qual o som serve é fornecer feedback às ações do jogador. Se você depender apenas de pistas visuais, não pode ter certeza de que o jogador estará olhando na direção certa para ver os efeitos de suas ações. Ao usar som, você pode garantir que ele perceba que algo aconteceu. Se você usar pistas visuais e sonoras em combinação, ouvir o som convida o jogador a procurar a pista visual, caso não a tenha visto.

Lembre-se de que alguns jogadores podem estar jogando com o som desligado. Se você estiver usando som como forma de fornecer instruções da scene ou uma parte fundamental da mecânica do jogo, tente também fornecer algo visual para sugerir na mesma direção. Por exemplo, você pode exibir texto na parte inferior da UI para acompanhar esses sons, como legendas em um filme.

## UI

Esta seção tem como objetivo ajudá-lo a construir uma narrativa por meio do design da sua scene que direcione o foco do jogador para os elementos que mais importam. Fazer bom uso das ferramentas descritas aqui pode tornar a experiência dele muito mais imersiva e bem-sucedida.

Não podemos subestimar o quão valiosos são cor, ícones, movimento e escala para transmitir a mensagem certa: tudo na scene comunica um significado, lembre-se disso.

### Layout

Tenha cuidado ao posicionar sua UI em regiões da tela que se sobreponham aos elementos padrão de UI da Decentraland. No cliente desktop, a UI padrão da Decentraland (sidebar, minimap, chat etc.) foi projetada para ocupar apenas os 25% esquerdos da tela (destacados em vermelho na imagem). O espaço restante da tela é uma zona segura que os criadores de conteúdo podem usar livremente.

<figure><img src="/docs/_remote/1f94cb72ac68f183e0aaa73e.webp" alt="Screen regions used by UI" width="800"><figcaption></figcaption></figure>

Não há restrições para colocar elementos de UI nos 25% esquerdos da tela, mas lembre-se de que o Explorer está continuamente fazendo mudanças e melhorias no seu design. Qualquer espaço que hoje não esteja obstruído nessa região poderá ser coberto em versões futuras.


> [!NOTE]
> **📱 Área segura no mobile**: As unidades [client mobile](/docs/creator/content-creator-pt/criar-para-mobile/cliente-movel/overview/) desenha seus próprios controles sobre parte do canvas: o **lado esquerdo** (chat, search, profile, joystick, emotes), o **canto superior direito** (controles de profile e câmera), e o **canto inferior direito** (botões de ação e interação). Não meça essas regiões manualmente: renderize dentro da área que o client informa, com `screenInset: 'interactable'`. Mantenha a UI crítica no centro ou no centro superior da tela e siga a [Área segura para dispositivos móveis](/docs/creator/content-creator-pt/criar-para-mobile/desenvolver/safe-area/). Use [`isMobile()`](/docs/creator/content-creator-pt/criar-para-mobile/desenvolver/detect-platform/) para alternar layouts quando as restrições de desktop e mobile não coincidirem.



> [!WARNING]
> **📔 Nota**: elementos de UI de scenes e wearables inteligentes sempre aparecem em uma camada atrás da UI padrão da Decentraland.


#### Nunca ancore sua UI no canto superior esquerdo

O erro de layout mais comum é fixar um painel no canto superior esquerdo ou deixá-lo na origem de uma root que cobre a tela. Esse canto, e toda a borda esquerda, é onde o Explorer desenha o próprio minimapa e chat, e no mobile o joystick virtual. A UI da scene colocada ali ainda é renderizada, mas a própria UI do client é desenhada por cima dela, então acaba coberta e seus botões muitas vezes não podem ser clicados. Nada no mecanismo de layout avisa você sobre isso.

Use uma destas opções em vez disso:

* **Ancore no lado direito ou centralize o elemento.** Para uma pontuação, um cronômetro ou um pequeno painel, defina `positionType: 'absolute'` e `position: { top: 40, right: 40 }`, ou centralize-o com `justifyContent: 'center'` na raiz.
* **Deixe o Explorer dizer onde é seguro.** Defina [`screenInset: 'interactable'`](/docs/creator/content-creator-pt/scenes-sdk7/ui-2d/onscreen-ui/#screen-inset-area) no renderer. Sua UI é então colocada dentro do retângulo que o Explorer reserva para a UI da scene, que já exclui o minimapa, o chat e os controles do lado esquerdo.

```ts
ReactEcsRenderer.setUiRenderer(uiComponent, { screenInset: 'interactable' })
```

No mobile, os botões de ação no canto inferior direito são desenhados sobre essa área de propósito, e os toques ali vão para os botões do client em vez de irem para a sua scene. Mantenha os elementos interativos fora do canto inferior direito ou, para uma UI que de qualquer forma toma a tela inteira, oculte os controles de toque enquanto ela estiver aberta:

```ts
import { TouchScreenControls } from '@dcl/sdk/ecs'

// enquanto um painel em tela cheia estiver aberto, o jogador não precisa pular nem pressionar E
TouchScreenControls.hideAll()
TouchScreenControls.hideJoystick()

// restaure-os quando o painel fechar
TouchScreenControls.showDefault()
TouchScreenControls.showJoystick()
```

`showDefault()` restaura o conjunto com que uma scene começa: jump, pointer e primary. Use `showAll()` somente se a sua scene realmente quiser os oito botões na tela. Ambos exigem SDK 7.32.0 ou posterior; em SDKs mais antigos, todos os oito botões aparecem por padrão e `showAll()` é a única forma de restaurar. Essas funções auxiliares não fazem nada no desktop, então você pode chamá-las incondicionalmente. Veja [Controles na tela](/docs/creator/content-creator-pt/scenes-sdk7/interatividade/touch-screen-controls/).

Tenha sempre uma grade em mente e use-a como principal critério ao organizar espacialmente sua UI. Se estiver trabalhando em uma scene em que um HUD seja necessário, você pode começar reunindo todos os consumíveis (por exemplo, moeda, comida, vida) e, do outro lado, os itens armazenáveis, como ferramentas ou armas. Tente ser consistente com as métricas da grade e o padding. Torne claro o feedback para cada ação do jogador.

### Cor

É importante escolher uma paleta de cores, pois isso dá identidade à sua scene e também sinaliza relações e hierarquia entre os elementos.

Ao montar uma paleta de cores, comece selecionando uma cor principal e, opcionalmente, uma secundária. Depois, você deve decidir se quer que a paleta seja análoga, complementar ou triádica etc., em relação a essa cor principal.

<figure><img src="/docs/_remote/06d8337c29d507083f4727ab.webp" alt="Chromatic color palette" width="300"><figcaption><p>Paleta cromática</p></figcaption></figure>

<figure><img src="/docs/_remote/cf0b4424895884aa68f549b1.webp" alt="Strategies for combining colors properly" width="300"><figcaption><p>Estratégias para combinar cores corretamente</p></figcaption></figure>


> [!NOTE]
> **💡 Dica**: Geradores gratuitos de paletas de cores que recomendamos: [Coolors](https://coolors.co), [Adobe Color CC](https://color.adobe.com/), [Colour Lovers](http://www.colourlovers.com), [Color Hunt](http://www.colorhunt.co), [Color by Hailpixel](http://color.hailpixel.com), [Colour Code](http://colourco.de), [Sip](https://sipapp.io/), [Color Scheme Designer by Paletton](http://paletton.com), [Cohesive Colors](http://javierbyte.github.io/cohesive-colors), [Colr](http://www.colr.org)


A cor principal deve ser a mais usada em seus componentes de UI. Se a sua paleta não tiver uma cor secundária, você pode destacar elementos combinando a cor principal com preto ou branco. Ter uma cor secundária não é obrigatório, mas ajuda a enfatizar e distinguir os elementos de UI da sua scene. As cores secundárias funcionam melhor para destaques, controles de seleção (sliders e switches), links e títulos.

Lembre-se de que cada cor tem qualidades expressivas únicas; aproveite isso para comunicar mensagens por meio delas. Por exemplo, o vermelho é frequentemente associado a conotações negativas, enquanto o verde é associado a conotações positivas.

<figure><img src="/docs/_remote/f0a0dc7306efbc4e1bb5c257.webp" alt="Colored switches" width="300"><figcaption><p>Usar cor da forma errada pode ser confuso</p></figcaption></figure>

Você também pode usar cores estrategicamente para indicar mudanças de estado, por exemplo, mudando a cor de um elemento para indicar se ele está ativo ou inativo.


> [!WARNING]
> **📔 Nota**: Os estados comunicam o status dos elementos de UI. Os estados de um elemento devem manter certa continuidade, mas precisam ter affordances claras e ser facilmente distinguíveis de outros estados e do layout ao redor.


<figure><img src="/docs/_remote/006fc8ebd49f1ffb83ca7363.webp" alt="Active inactive button" width="300"><figcaption><p>Usando cor para estados de botão</p></figcaption></figure>

Atenção! Ao selecionar cores que serão mostradas em sobreposição, tome cuidado especial para garantir que elas sejam legíveis quando usadas juntas. Aqui vai uma regra fundamental: as cores de elementos que são mostrados juntas devem sempre ter bastante contraste entre si. Lembre-se de que alguns jogadores podem estar olhando para suas telas em condições de iluminação subótimas, o que dificulta a leitura.

<figure><img src="/docs/_remote/78f863c16ac2c51e8736e68a.webp" alt="Icon contrast" width="300"><figcaption><p>Ambos os ícones podem ser distinguidos, mas o primeiro tem melhor contraste. Fica mais fácil de ler e exige menos esforço para entender.</p></figcaption></figure>

### Hierarquia tipográfica

Tente definir ao menos uma escala de 3 tipos para Títulos, Subtítulos e Corpo. Tenha cuidado com o uso de cor e tamanho da fonte. Todos os tamanhos de fonte devem ser de 12 px ou maiores.

Se você for colocar texto exibido sobre imagens (ou sobre o mundo), certamente precisará experimentar sua legibilidade. Nesses casos, sugerimos adicionar uma região sólida colorida, em uma camada entre o texto e as imagens, para garantir que o texto permaneça legível.

<figure><img src="/docs/_remote/26f47a30aa943bf272e86fd8.webp" alt="Text over images" width="300"><figcaption><p>Priorize a legibilidade</p></figcaption></figure>

### Ícones

Os ícones sintetizam informação, ajudando você a identificar ações por meio de imagens. Eles são uma ferramenta incrivelmente poderosa para fornecer entrada que pode ser interpretada rapidamente, em vez de usar texto para rotular coisas, o que exige mais atenção e tempo do jogador. Também é útil mostrar ícones em combinação com texto, pois isso ajuda a desambiguar seus significados.

<figure><img src="/docs/_remote/d3655e8d247e6f40487d2d6a.webp" alt="Icon examples" width="300"><figcaption><p>Use ícones como uma linguagem universal</p></figcaption></figure>

### Movimento

Use movimento para fornecer feedback e guiar o caminho quando necessário. O movimento ajuda os jogadores a focar sua atenção e ajuda a manter a continuidade à medida que a UI muda.

<figure><img src="/docs/_remote/db15a9b89e2052e22e2de076.webp" alt="Adding glow" width="300"><figcaption><p>Use partículas, escala ou margens em fade para fazer as coisas se destacarem</p></figcaption></figure>

#### Evite efeitos de piscar

Luzes piscantes, strobes e mudanças rápidas de brilho ou cor podem desencadear convulsões em pessoas com epilepsia fotossensível. Isso inclui materiais emissivos pulsantes, luzes cintilantes, flashes de UI em tela cheia e vídeos.

* Limite qualquer efeito de piscar a **3 flashes por segundo ou menos**, seguindo a [diretriz WCAG "Three Flashes or Below Threshold"](https://www.w3.org/WAI/WCAG22/Understanding/three-flashes-or-below-threshold.html) diretriz.
* Mantenha as áreas piscantes pequenas e evite vermelho saturado e flashes grandes, de alto contraste, que ocupem a tela.
* Se a sua scene depender de efeitos intensos, como strobes de boate, relâmpagos ou flashes de dano, adicione uma opção na scene para desligá-los ou suavizá-los.

### Escrita e estrutura do conteúdo

O texto da UI pode tornar as interfaces mais utilizáveis e dar mais confiança aos jogadores em suas ações. Sempre torne o texto da UI o mais conciso possível. Os jogadores estão ali para jogar, não para ler. Qualquer texto que pareça longo demais não será lido pela maioria dos jogadores.

Receba seus jogadores com uma mensagem de boas-vindas e diga a eles o objetivo da scene. Comece esclarecendo a meta na scene e depois as ações necessárias para alcançá-la. Então você pode revelar informações progressivamente conforme elas forem necessárias; dessa forma, os jogadores não se sentirão sobrecarregados logo no início da experiência. Assim como com recursos gráficos, tente usar palavras consistentes em seus recursos de UI e na narrativa.

#### Mostre, não conte

Dois padrões fazem os jogadores se perderem antes da primeira interação:

* **O pop-up de regras**: um painel de introdução com vários parágrafos densos cobrindo cada mecânica, regra de pontuação e controle. Os jogadores o dispensam sem ler.
* **O livro de regras no mundo**: os mesmos parágrafos em uma placa ou quadro na scene. Parece design, mas o texto no mundo é pequeno demais e longo demais para ser lido, e os jogadores não vão ficar parados para apertar os olhos e lê-lo.

Mantenha um painel de introdução em uma única tela, sem rolagem: de 3 a 5 linhas curtas, idealmente uma linha para o objetivo, uma linha para os controles e uma imagem. Uma placa deve dizer uma única coisa, em 10 palavras ou menos, com fonte grande, com uma placa por ideia.

Depois, mostre o resto em vez de escrevê-lo:

* Torne o primeiro alvo inconfundível. Coloque um objeto brilhante, animado ou superdimensionado na linha de visão inicial do jogador. Um contorno pulsante ensina "clique em mim" melhor do que uma frase.
* Deixe o jogador executar uma mecânica uma vez com segurança, em uma área de prática, antes do jogo real começar.
* Use `hoverText` em entities interativas como uma instrução just-in-time. Ela aparece exatamente quando é relevante e não exige leitura prévia. Veja [Feedback de Hover](/docs/creator/content-creator-pt/scenes-sdk7/interatividade/eventos-de-botao/register-callback/#hover-feedback).
* Use um diagrama para regras espaciais ou de sequência. Uma imagem de "gemas vermelhas = 1, gemas azuis = 5" substitui um parágrafo.
* Apresente qualquer coisa mais longa uma mensagem de cada vez, por meio de um NPC ou de um diálogo que o jogador possa pular.

Se uma regra precisar de mais texto do que isso, redesenhe a mecânica ou mostre-a visualmente.

### Interações com objetos

Certifique-se de que os jogadores consigam perceber quais objetos são interativos e quais não são. Ao registrar eventos de botão em uma Entity, o SDK exibe feedback de Hover por padrão: um realce na borda da Entity e uma dica ao lado do cursor mostrando qual input usar, com texto personalizado para descrever os efeitos de interagir com ela. Recomendamos fortemente usar isso; sua scene ficará mais natural e fácil de usar. Veja [Feedback de Hover](/docs/creator/content-creator-pt/scenes-sdk7/interatividade/eventos-de-botao/register-callback/#hover-feedback).

### câmera em 3ª pessoa

Os jogadores podem alternar entre a câmera em 1ª pessoa e a câmera em 3ª pessoa a qualquer momento pressionando a tecla ‘V’. Projete sua scene para que as interações funcionem confortavelmente nos dois modos: por exemplo, alvos de clique pequenos são mais difíceis de acertar em 3ª pessoa. Sua scene pode ler o modo atual de câmera do jogador ou forçar um específico em uma área, veja [Câmera](/docs/creator/content-creator-pt/scenes-sdk7/fundamentos-de-conteudo-3d/camera/).

A câmera em 3ª pessoa também muda o tamanho que uma sala precisa ter. Interiores construídos nas dimensões do mundo real parecem apertados na tela, veja [Construa interiores maiores do que na vida real](/docs/creator/content-creator-pt/scenes-sdk7/conceber-a-experiencia/design-games/#build-interiors-larger-than-real-life).


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-pt/scenes-sdk7/conceber-a-experiencia/ux-ui-guide.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
