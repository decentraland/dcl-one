> Altere a hora do skybox, substitua o céu pela sua própria textura ou altere a cor do céu, das nuvens e das estrelas e controle o sol e o nevoeiro

# Controle do Skybox

Você pode alterar como um jogador vê o skybox sempre que estiver na sua scene; isso também afeta a matiz e a direção da iluminação global.

O céu em Decentraland segue um ciclo padrão de dia/noite que leva 2 horas para ser concluído, então há 12 ciclos completos a cada dia real. Se a scene não estiver impondo nenhuma hora do dia fixa, os jogadores também podem mudar para uma hora do dia específica alterando um controle deslizante na UI.

Sempre que os jogadores entram em uma scene com uma hora do dia diferente, ou a scene altera a hora do dia dinamicamente, o skybox faz uma transição suave ao longo de alguns segundos para esse novo valor.

## Hora do dia fixa

Você pode definir uma hora do dia fixa para a sua scene. Todos os jogadores verão a scene com essa hora do dia, e o skybox não seguirá o ciclo de dia/noite.

No Creator Hub, abra as configurações da scene e clique na **Configurações** aba para encontrar a **Skybox** seção. Desmarque a **Auto** opção e defina a hora do dia desejada.

![](/docs/_remote/6242ab0073574f8cc2c9e7d9.webp)

Você também pode definir a hora do dia do skybox no código da sua scene. Para fazer isso, adicione a seguinte seção ao seu `scene.json` no nível raiz:

```json
 "skyboxConfig": {
    "fixedTime": 36000
  }
```

O número se refere à quantidade de segundos desde o início do dia, variando de 0 (que se refere a *00:00*) até 86400 (que se refere a *24:00*). Qualquer número maior que 86400 também é interpretado como meia-noite.

Aqui estão mais alguns exemplos de valores válidos:

* 0 segundos => *00:00*
* 21600 segundos => *06:00*
* 43200 segundos => *12:00*
* 64800 segundos => *18:00*
* 86400 segundos => *24:00*

## Lendo a hora do dia

Você pode ler a hora do dia do código da sua scene usando a `getWorldTime()` função.

```ts
import { getWorldTime } from '~system/Runtime'

executeTask(async () => {
  let time = await getWorldTime({})
  console.log(time.seconds)
})
```

A função retorna um número entre 0 e 86400, onde 0 é meia-noite e 86400 é 24:00. Esse valor é atualizado se a scene altera a hora do dia dinamicamente ou se o jogador altera a hora do dia na UI. Caso contrário, ele retorna o valor relativo ao ciclo padrão de dia/noite.

## Alterando a hora do dia dinamicamente

Você pode alterar a hora do dia dinamicamente usando o `SkyboxTime` componente. Esse componente só pode ser adicionado ao RootEntity da scene `engine.RootEntity`.

```ts
import { SkyboxTime } from '@dcl/sdk/ecs'

function main() {
  SkyboxTime.create(engine.RootEntity, { fixedTime: 36000 })
}
```

O `fixedTime` propriedade é um número entre 0 e 86400, onde 0 é meia-noite e 86400 é 24:00. Qualquer número maior que 86400 também é interpretado como meia-noite.

Sempre que esse componente é adicionado, removido, ou a `fixedTime` propriedade é alterada, a hora do dia do skybox faz uma transição suave ao longo de alguns segundos para esse novo valor. O mesmo acontece quando o jogador sai ou entra na scene. Enquanto a hora do dia do skybox estiver fixa, o skybox não seguirá mais o progresso do seu ciclo de dia/noite e os jogadores não poderão alterar a hora do dia pela UI.

Por padrão, a transição sempre acontece na direção para frente, mas você pode alterar isso definindo o `transitionMode` property para `TransitionMode.TM_FORWARD` ou `TransitionMode.TM_BACKWARD`.

```ts
import { SkyboxTime, TransitionMode } from '@dcl/sdk/ecs'

function main() {
  SkyboxTime.create(engine.RootEntity, { fixedTime: 36000, transitionMode: TransitionMode.TM_BACKWARD })
}
```

### Escolha a transição mais curta

Nada nunca salta bruscamente: toda mudança percorre todos os tempos intermediários. `TM_FORWARD` sempre avança, dando a volta pela meia-noite, então ir de 10:00 para 08:40 percorre quase um dia inteiro de sky em vez de voltar 80 minutos.

Descubra qual direção é mais curta e passe o modo correspondente:

```ts
import { engine, SkyboxTime, TransitionMode } from '@dcl/sdk/ecs'

const DAY = 86400

// Mude para uma nova hora do dia pelo caminho mais curto
function setTimeOfDay(fixedTime: number) {
  const current = SkyboxTime.getOrNull(engine.RootEntity)?.fixedTime
  // sem um SkyboxTime definido ainda, a scene não sabe a hora atual, então apenas avance
  let transitionMode = TransitionMode.TM_FORWARD
  if (current !== undefined) {
    const forward = (fixedTime - current + DAY) % DAY
    // mais de meio dia à frente significa que voltar é o caminho mais curto
    if (forward > DAY / 2) transitionMode = TransitionMode.TM_BACKWARD
  }
  SkyboxTime.createOrReplace(engine.RootEntity, { fixedTime, transitionMode })
}
```

Para um salto grande que você não quer que o jogador veja, esconda-o atrás de uma UI em tela cheia: faça fade in, mude a hora, espere e então faça fade out.

## Céu, sol e fog personalizados

Três componentes permitem que sua scene assuma o controle do ambiente enquanto o jogador estiver dentro dela:

* `Skybox`: substitui o céu pela sua própria texture, ou recolore o céu procedural, suas nuvens e suas estrelas.
* `Sun`: recolore, reduz a intensidade, oculta ou fixa a direção da luz do sol (e da lua).
* `EnvironmentFog`: recolore a neblina de distância e altera sua densidade.

Assim como `SkyboxTime`, os três só podem ser adicionados ao RootEntity da scene, `engine.RootEntity`. Em qualquer outra entity, eles não têm efeito. Cada campo é opcional: tudo o que você omitir mantém seu comportamento padrão de hora do dia.


> [!WARNING]
> **📔 Nota**: As unidades `Skybox`, `Sun` e `EnvironmentFog` Os components ainda não são suportados no app mobile nem no Bevy Explorer. Os players nesses clients veem o céu, o sol e a neblina normais, então certifique-se de que sua scene ainda funcione sem essas substituições.


```ts
import { engine, Skybox, Sun, EnvironmentFog } from '@dcl/sdk/ecs'
import { Color4, Vector3 } from '@dcl/sdk/math'

function main() {
  Skybox.create(engine.RootEntity, {
    skyboxTexture: 'images/sky.png'
  })
  Sun.create(engine.RootEntity, { intensity: 0.5, direction: Vector3.create(-1, -1, 0) })
  EnvironmentFog.create(engine.RootEntity, {
    color: { keys: [{ time: 0, color: Color4.create(0.6, 0.2, 0.1, 1) }] },
    density: 0.01
  })
}
```

### Escopo e redefinição

O `Skybox`, `Sun` e `EnvironmentFog` As substituições se aplicam apenas enquanto o jogador estiver dentro da sua scene. Quando o jogador sai da scene, quando um componente é removido ou quando um campo não está definido, o céu e a iluminação voltam aos padrões, e ao entrar novamente na scene eles são aplicados outra vez. As alterações se aplicam imediatamente, sem transição.


> [!WARNING]
> **📔 Nota**: Enquanto ativas, essas substituições são globais: os parcels vizinhos vistos de dentro da sua scene também são renderizados com o seu céu, sol e neblina.


### Textura de céu personalizada

`skyboxTexture` substitui o céu visível por uma imagem dos assets da scene. Defina-a como o caminho do arquivo de imagem, da mesma forma que o `src` de uma Texture. A imagem deve ser um panorama equiretangular (uma imagem latitude-longitude 2:1): seu centro horizontal aponta para o eixo Z positivo (norte) da scene, sua linha superior é o zênite e sua linha inferior é o nadir. Cube cross, 3x2 e layouts de cubemap com seis faces não são suportados.

```ts
import { engine, Skybox } from '@dcl/sdk/ecs'

function main() {
  Skybox.create(engine.RootEntity, {
    skyboxTexture: 'images/sky.png'
  })
}
```

A imagem se repete horizontalmente e é filtrada bilinearmente. Se a imagem falhar ao carregar, o céu padrão permanece no lugar.

Mantenha o tamanho da imagem razoável: todo o panorama é baixado e mantido na memória enquanto sua scene é carregada. 2048x1024 pixels é suficiente para a maioria dos céus; não passe de 4096x2048. Use um formato compactado como `.jpg` para céus fotográficos.

A texture só muda a aparência do céu: a luz do sol continua seguindo a hora do dia, a luz ambiente continua seguindo `skyColors` (para que você possa combiná-la com sua imagem), e os `Sun` e `EnvironmentFog` components ainda se aplicam por cima dela. Na predefinição de alta qualidade, os reflexos em materiais brilhantes também seguem sua texture de céu.

### Cores do céu, nuvens e estrelas

Se você mantiver o céu procedural, o `Skybox` componente permite recolorir e ajustar sua camada de nuvens e campo de estrelas.

```ts
import { engine, Skybox, ColorGradient } from '@dcl/sdk/ecs'
import { Color4 } from '@dcl/sdk/math'

// Um gradient com uma única key é uma cor constante
function constant(color: Color4): ColorGradient {
  return { keys: [{ time: 0, color }] }
}

function main() {
  Skybox.create(engine.RootEntity, {
    skyColors: {
      zenith: constant(Color4.create(0.55, 0.25, 0.12, 1)),
      horizon: constant(Color4.create(0.95, 0.55, 0.3, 1)),
      nadir: constant(Color4.create(0.35, 0.15, 0.08, 1))
    },
    clouds: { opacity: 0.3, speed: 0.01, color: constant(Color4.create(0.9, 0.6, 0.4, 1)) },
    stars: { brightness: 4.62 }
  })
}
```

* `skyColors`: a cor do céu no seu `zenith` (diretamente acima), no `horizon`, e no seu `nadir` (o céu abaixo do horizonte, visível onde nenhum terreno o cobre, por exemplo em grande altitude ou em um World com o [landscape terrain](/docs/creator/content-creator-pt/scenes-sdk7/tipos-de-projetos/scene-metadata/#landscape-terrain) desativado). Essas cores também definem a luz ambiente da scene: a cor do zênite ilumina os objetos de cima, a cor do horizonte pelas laterais e a cor do nadir por baixo, para que objetos e avatares combinem com o céu ao redor. `skyColors.rim` é um brilho estreito desenhado bem na linha do horizonte, como um brilho de pôr do sol; quando você o deixa sem definir, ele segue sua `horizon` cor, se você definir uma (caso contrário, ele mantém seu valor padrão), então você só precisa dele como um detalhe (por exemplo, uma borda laranja de amanhecer em um céu escuro).
* `clouds.opacity`: de 0 (sem nuvens) até 1, o padrão. `clouds.speed`: quão rápido a camada de nuvens gira ao redor do céu, em radianos por segundo: 0.01 por padrão (uma volta a cada \~10 minutos), 0 para nuvens estáticas; valores negativos contam como 0. `clouds.color`: o tom da camada de nuvens; sem ele, as nuvens mantêm suas cores padrão da hora do dia, mesmo em um céu recolorido.
* `stars.brightness`: um multiplicador da textura das estrelas, 4.62 por padrão (valores acima de 1 são HDR, 0 oculta as estrelas, valores negativos contam como 0). As estrelas só são visíveis durante a parte noturna do dia.

Enquanto um `skyboxTexture` está definido, a texture substitui o céu procedural: `clouds` e `stars` não têm efeito, e `skyColors` não coloram mais o céu, mas ainda definem a luz ambiente.

### Sun

O `Sun` componente altera a luz direcional do sol e da lua.

```ts
import { engine, Sun } from '@dcl/sdk/ecs'
import { Color4, Vector3 } from '@dcl/sdk/math'

function main() {
  Sun.create(engine.RootEntity, {
    color: { keys: [{ time: 0, color: Color4.create(1, 0.65, 0.4, 1) }] },
    intensity: 0.5,
    direction: Vector3.create(-1, -1, 0)
  })
}
```

* `color`: a cor da luz direcional. Ela também tinge o disco do sol.
* `visíveis`: `true` por padrão. Defina-a como `false` para ocultar os discos do sol e da lua e o reflexo da lente do sol, por exemplo quando eles apareceriam através de uma `skyboxTexture`. A luz que o sol projeta não é afetada.
* `intensity`: um multiplicador da intensidade da luz da hora do dia, 1 por padrão. 0 desliga a luz solar; valores negativos contam como 0.
* `direction`: a direção na qual a luz do sol viaja, em coordenadas da scene: `(0, -1, 0)` coloca o sol diretamente acima, e `(-1, -1, 0)` coloca-o no lado positivo X (leste) do céu, 45 graus acima do horizonte. Enquanto estiver definido, o sol permanece fixo em qualquer hora do dia, mas sua cor e intensidade ainda seguem a hora do dia. Deixe-o sem definir, ou defina-o como `(0, 0, 0)`, para manter o caminho regular do sol pelo céu.

### Fog

O `EnvironmentFog` componente altera a neblina de distância.

```ts
import { engine, EnvironmentFog } from '@dcl/sdk/ecs'
import { Color4 } from '@dcl/sdk/math'

function main() {
  EnvironmentFog.create(engine.RootEntity, {
    color: { keys: [{ time: 0, color: Color4.create(0.85, 0.5, 0.3, 1) }] },
    density: 0.01
  })
}
```

* `color`: a cor da neblina.
* `density`: quão rapidamente as coisas desaparecem na neblina, como uma densidade exponencial por metro: a neblina a uma dada distância é `1 - e^(-density * distance)`, então `1 / density` é aproximadamente a distância em que dois terços da view estão cobertos pela neblina. O padrão é 0.0005 (cerca de 2 km); 0.02 cobre cerca de dois terços da view a 50 m e quase toda ela a 150 m, e 0 torna a neblina invisível. Valores negativos contam como 0.

Se a neblina é renderizada ou não é uma configuração de qualidade escolhida pelo jogador; uma scene não pode ativá-la nem desativá-la.

### Gradientes de cor ao longo do dia

Toda cor nesses components é um `ColorGradient`: uma lista de `keys`, cada uma com um `time` e uma `Color4`. A `time` é a hora do dia normalizada, de 0 (*00:00*) a 1 (*24:00*), então 0.5 é meio-dia. É o `SkyboxTime` clock dividido por 86400: `fixedTime: 21600` (6 AM) é `time: 0.25`. A cor é interpolada entre keys vizinhas; antes da primeira key e depois da última, usa-se a cor dessa key. Tempos fora de 0 a 1 são limitados a esse intervalo, e se duas keys compartilham o mesmo `time`, a última na lista vence. Um gradient com uma única key é simplesmente uma cor constante.

```ts
import { engine, Skybox } from '@dcl/sdk/ecs'
import { Color4 } from '@dcl/sdk/math'

function main() {
  Skybox.createOrReplace(engine.RootEntity, {
    skyColors: {
      horizon: {
        keys: [
          { time: 0, color: Color4.create(0.05, 0.05, 0.2, 1) }, // meia-noite
          { time: 0.25, color: Color4.create(0.95, 0.5, 0.4, 1) }, // amanhecer
          { time: 0.5, color: Color4.create(0.7, 0.9, 1, 1) }, // meio-dia
          { time: 0.75, color: Color4.create(0.95, 0.4, 0.2, 1) }, // anoitecer
          { time: 1, color: Color4.create(0.05, 0.05, 0.2, 1) } // meia-noite novamente, então não há salto
        ]
      }
    }
  })
}
```

Os gradients não dão a volta pela meia-noite, então repita a mesma cor em `time: 0` e `time: 1` se o dia deve fazer loop sem emenda. Os valores de cor não são limitados a 1: valores acima de 1 produzem um sol ou céu mais brilhante, HDR. O canal alfa é ignorado. Use até 8 keys por gradient: gradients mais longos são reamostrados para 8 keys igualmente espaçadas, o que suaviza qualquer detalhe entre elas. Combine gradients com `SkyboxTime` para fixar o dia em um ponto específico do seu gradient.

### Escuridão completa

Para fazer com que as luzes colocadas na sua scene sejam a única fonte de iluminação, apague tudo o que o ambiente contribui: a luz solar e seu disco, as cores do céu (e com elas a luz ambiente), a neblina, as nuvens e as estrelas.

```ts
import { engine, Skybox, Sun, EnvironmentFog } from '@dcl/sdk/ecs'
import { Color4 } from '@dcl/sdk/math'

function main() {
  const black = { keys: [{ time: 0, color: Color4.Black() }] }

  Skybox.createOrReplace(engine.RootEntity, {
    skyColors: { zenith: black, horizon: black, nadir: black },
    clouds: { opacity: 0 },
    stars: { brightness: 0 }
  })
  Sun.createOrReplace(engine.RootEntity, { intensity: 0, visible: false })
  EnvironmentFog.createOrReplace(engine.RootEntity, { color: black })
}
```


> [!WARNING]
> **📔 Nota**: No preset de baixa qualidade, as luzes da scene são desligadas, então uma scene totalmente escura é renderizada completamente preta para esses jogadores. Use materiais emissivos para manter visível tudo o que for essencial.


Consulte [Luzes](/docs/creator/content-creator-pt/scenes-sdk7/fundamentos-de-conteudo-3d/lights/) para adicionar luzes pontuais e spot à sua scene.


> [!NOTE]
> **💡 Dica**: Para ver exemplos práticos de controle de skybox, consulte as [`2,1-skybox-sdk-scene-a`](https://github.com/decentraland/sdk7-test-scenes/tree/main/scenes/2,1-skybox-sdk-scene-a) e [`3,1-skybox-sdk-scene-b`](https://github.com/decentraland/sdk7-test-scenes/tree/main/scenes/3,1-skybox-sdk-scene-b) scenes de teste, que controlam `SkyboxTime` e `TransitionMode` ao vivo a partir de um painel da UI, e [`2,0-skybox-scene-json`](https://github.com/decentraland/sdk7-test-scenes/tree/main/scenes/2,0-skybox-scene-json), que lê o `fixedTime` definido em `scene.json` de volta em runtime por meio de `getSceneInformation()`. A [`2,2-skybox-control`](https://github.com/decentraland/sdk7-test-scenes/tree/main/scenes/2,2-skybox-control) scene explora os `Skybox`, `Sun` e `EnvironmentFog` components: texturas de céu, predefinições de ambiente, ocultar o sol e um modo de escuridão completa.



---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-pt/scenes-sdk7/interatividade/skybox-control.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
