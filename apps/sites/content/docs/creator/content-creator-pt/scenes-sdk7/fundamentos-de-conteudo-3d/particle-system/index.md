# Sistema de Partículas

Adicione efeitos de partículas como fogo, chuva, faíscas e auras mágicas à sua scene

Sistemas de partículas permitem criar efeitos visuais dinâmicos emitindo e animando grandes quantidades de pequenos sprites. Use-os para criar fogo, fumaça, chuva, neve, faíscas, auras mágicas, explosões e muitos outros efeitos que seriam impraticáveis de obter com meshes estáticos.

## Adicionando um sistema de partículas

Para adicionar um sistema de partículas à sua scene, crie uma entity e anexe o `ParticleSystem` component a ela.

```ts
import { engine, Transform, ParticleSystem } from '@dcl/sdk/ecs'
import { Vector3 } from '@dcl/sdk/math'

const emitter = engine.addEntity()

Transform.create(emitter, {
	position: Vector3.create(8, 1, 8),
})

ParticleSystem.create(emitter, {})
```

Sem nenhuma propriedade definida, o component usa todos os padrões: um emissor de ponto lançando 10 partículas por segundo, com uma vida útil das partículas de 5 segundos.

## Formas do emissor

A forma do emissor determina onde as partículas aparecem. Quatro formas estão disponíveis por meio dos `ParticleSystem.Shape` helpers.

### Ponto

As partículas aparecem a partir de um único ponto (a posição da entity). Este é o padrão quando nenhuma forma é especificada.

```ts
ParticleSystem.create(emitter, {
	shape: ParticleSystem.Shape.Point(),
})
```

### Sphere

As partículas aparecem da superfície ou do interior de uma esfera.

```ts
ParticleSystem.create(emitter, {
	shape: ParticleSystem.Shape.Sphere({ radius: 2 }),
})
```

### Cone

As partículas aparecem na base de um cone e se movem para fora na direção do cone. `angle` é o semiângulo em graus; `radius` é o raio da base em metros. Para orientar a direção do cone, defina a rotação da `Transform`.

```ts
ParticleSystem.create(emitter, {
	shape: ParticleSystem.Shape.Cone({ angle: 25, radius: 1 }),
})
```

### Box

As partículas aparecem em qualquer lugar dentro de um volume de caixa.

```ts
ParticleSystem.create(emitter, {
	shape: ParticleSystem.Shape.Box({ size: Vector3.create(3, 1, 3) }),
})
```

## Propriedades de emissão

Controle quantas partículas são criadas e por quanto tempo elas vivem.

| Propriedade    | Padrão | Descrição                                              |
| -------------- | ------ | ------------------------------------------------------ |
| `rate`         | `10`   | Partículas emitidas por segundo (contínuo).            |
| `maxParticles` | `1000` | Limite máximo rígido de partículas ativas simultâneas. |
| `lifetime`     | `5`    | Vida útil de cada partícula em segundos.               |
| `active`       | `true` | Se o system está emitindo novas partículas ativamente. |

```ts
ParticleSystem.create(emitter, {
	rate: 50,
	maxParticles: 500,
	lifetime: 3,
})
```

## Movimento

### Gravidade

A `gravity` a propriedade é um multiplicador aplicado à gravidade da scene (-9.81 m/s²). Defina-o para `0` fazer as partículas flutuarem no lugar, um valor negativo para empurrá-las para cima, ou um valor positivo para acelerá-las para baixo.

```ts
ParticleSystem.create(emitter, {
	gravity: -0.5, // as partículas flutuam lentamente para cima
})
```

### Velocidade inicial

`initialVelocitySpeed` é um `FloatRange` que define um mínimo e um máximo para a faixa aleatória de velocidade com que cada partícula é lançada (em m/s). Ambos os valores têm como padrão `1`.

```ts
ParticleSystem.create(emitter, {
	initialVelocitySpeed: { start: 2, end: 8 },
})
```

### Força adicional

Um vetor de força constante aplicado a todas as partículas a cada frame, além da gravidade. Útil para efeitos de vento ou magnéticos.

```ts
ParticleSystem.create(emitter, {
	additionalForce: Vector3.create(0.5, 0, 0), // empurrão lateral constante
})
```

### Limitação de velocidade

Usa `limitVelocity` para limitar a velocidade máxima das partículas e, opcionalmente, amortecer sua velocidade excedente a cada frame.

```ts
ParticleSystem.create(emitter, {
	limitVelocity: {
		speed: 5, // velocidade máxima permitida em m/s
		dampen: 0.8, // fração da velocidade excedente removida por frame
	},
})
```

## Propriedades visuais

### Cor

`initialColor` define a cor da partícula ao nascer, escolhida aleatoriamente dentro da faixa fornecida. `colorOverTime` faz a cor da partícula transitar gradualmente de `start` para o `end` valor ao longo da vida útil da partícula. Ambos aceitam um `ColorRange` com um `start` e `end` valor.

```ts
import { Color4 } from '@dcl/sdk/math'

ParticleSystem.create(emitter, {
	initialColor: {
		start: Color4.create(1, 0.5, 0, 1), // laranja
		end: Color4.create(1, 1, 0, 1), // amarelo
	},
	colorOverTime: {
		start: Color4.create(0.5, 0, 0, 0.5), // vermelho escuro, semitransparente
		end: Color4.create(0, 0, 0, 0), // totalmente transparente (desvanecer)
	},
})
```

Observe que o 4º valor em um `Color4` é o *alpha*. Se você definir a cor final como uma com alpha 0, as partículas irão desaparecer gradualmente e ficar invisíveis, o que geralmente é um bom efeito.

### Tamanho

`initialSize` e `sizeOverTime` são `FloatRange` valores. `initialSize` controla a escala da partícula ao nascer, escolhida aleatoriamente dentro da faixa fornecida. `sizeOverTime` faz a escala da partícula transitar gradualmente de `start` para o `end` valor ao longo da vida útil da partícula. Um valor de `1` é igual ao tamanho original da texture.

```ts
ParticleSystem.create(emitter, {
	initialSize: { start: 0.1, end: 0.3 },
	sizeOverTime: { start: 0.5, end: 1.0 }, // cresce ao longo da vida útil
})
```


> [!WARNING]
> **📔 Nota**: O `scale` da `Transform` não afeta a escala das partículas.


### Texture

Por padrão, as partículas são renderizadas como quadrados brancos. Forneça uma texture para usar uma imagem personalizada.

```ts
ParticleSystem.create(emitter, {
	texture: { src: 'assets/scene/Images/spark.png' },
})
```

### Modo de blend

Controla como a cor da partícula é composta com a scene atrás dela.

| Valor                                  | Descrição                                                                |
| -------------------------------------- | ------------------------------------------------------------------------ |
| `ParticleSystemBlendMode.PSB_ALPHA`    | Transparência padrão (default).                                          |
| `ParticleSystemBlendMode.PSB_ADD`      | Blend aditivo — as partículas iluminam a scene. Bom para fogo e brilhos. |
| `ParticleSystemBlendMode.PSB_MULTIPLY` | Multiplica a cor da partícula pela scene atrás dela.                     |

```ts
import { ParticleSystemBlendMode } from '@dcl/sdk/ecs'

ParticleSystem.create(emitter, {
	texture: { src: 'assets/scene/textures/ember.png' },
	blendMode: ParticleSystemBlendMode.PSB_ADD,
})
```

### Billboard

é chamado, os jogadores recebem uma tela de confirmação, onde são informados para onde o link os levará e onde podem aceitar ou recusar visitar o link. `billboard` é `true` (o padrão), cada partícula sempre fica voltada para a camera, independentemente da orientação do emissor. Defina como `false` para partículas que devem girar no espaço 3D.

```ts
ParticleSystem.create(emitter, {
	billboard: false,
})
```

## Rotação

### Rotação inicial e rotação ao longo do tempo

`initialRotation` é a orientação de uma partícula quando ela aparece. `rotationOverTime` é uma velocidade angular por eixo aplicada a cada segundo. Ambos aceitam um `Quaternion`.

```ts
import { Quaternion } from '@dcl/sdk/math'

ParticleSystem.create(emitter, {
	initialRotation: Quaternion.fromEulerDegrees(0, 0, 45),
	rotationOverTime: Quaternion.fromEulerDegrees(0, 0, 90), // gira 90°/s no Z
})
```


> [!WARNING]
> **📔 Nota**: Se `billboard` está definido como `true`, então as partículas girarão apenas em um eixo, mantendo sempre sua orientação voltada para a camera.


### Virar na direção do movimento

é chamado, os jogadores recebem uma tela de confirmação, onde são informados para onde o link os levará e onde podem aceitar ou recusar visitar o link. `faceTravelDirection` é `true`, cada partícula gira automaticamente para apontar na direção do seu movimento, como um asteroide ou folha caindo pelo ar.

```ts
ParticleSystem.create(emitter, {
	faceTravelDirection: true,
	billboard: false,
})
```


> [!WARNING]
> **📔 Nota**: Se `faceTravelDirection` está true, o valor de `billboard` é ignorado.


## Animação de sprite sheet

Você pode animar partículas tratando uma texture como uma grade de quadros de animação. Especifique o número de colunas e linhas na sheet e a velocidade de playback.

```ts
ParticleSystem.create(emitter, {
	texture: { src: 'assets/scene/textures/flame-sheet.png' },
	spriteSheet: {
		tilesX: 4, // 4 colunas
		tilesY: 3, // 3 linhas  (12 quadros no total)
		framesPerSecond: 12,
	},
})
```

## Controle de reprodução

O system de partículas pode estar em um de três estados controlados por `playbackState`.

| Valor                                    | Descrição                                                     |
| ---------------------------------------- | ------------------------------------------------------------- |
| `ParticleSystemPlaybackState.PS_PLAYING` | Emitindo ativamente (default).                                |
| `ParticleSystemPlaybackState.PS_PAUSED`  | Congela todas as partículas atuais no lugar e para de emitir. |
| `ParticleSystemPlaybackState.PS_STOPPED` | Para de emitir e remove todas as partículas existentes.       |

```ts
import { ParticleSystemPlaybackState } from '@dcl/sdk/ecs'

// Pausar quando o player se afastar
ParticleSystem.getMutable(emitter).playbackState =
	ParticleSystemPlaybackState.PS_PAUSED

// Retomar
ParticleSystem.getMutable(emitter).playbackState =
	ParticleSystemPlaybackState.PS_PLAYING
```

### Loop e prewarm

Por padrão, o system faz loop indefinidamente. Defina `loop` para `false` para um efeito de uso único que para automaticamente depois que todas as partículas morrerem.

Defina `prewarm` para `true` (requer `loop: true`) para simular o system como se estivesse em execução desde o início, para que as partículas já preencham a scene quando o player a vê pela primeira vez.

```ts
ParticleSystem.create(emitter, {
	loop: false, // reproduz uma vez e depois para
	prewarm: false,
})
```

## Emissão em burst

Bursts permitem emitir um grande número de partículas em momentos específicos, em vez de em uma taxa constante. São úteis para explosões, fogos de artifício ou outros eventos de uso único.

Um único system de partículas pode passar por um ciclo com vários bursts, mesmo com intervalos ou probabilidades variáveis, para parecer mais natural.

```ts
ParticleSystem.create(emitter, {
	rate: 0, // desativa a emissão contínua
	bursts: {
		values: [
			{
				time: 0, // tempo em segundos após o início do playback
				count: 200, // partículas a emitir por burst
				cycles: 1, // quantas vezes repetir (0 = infinito)
				interval: 0.2, // segundos entre ciclos repetidos
				probability: 1, // chance de 0–1 de o burst ser disparado a cada ciclo
			},
		],
	},
})
```

Para um efeito de fogos de artifício em loop com bursts escalonados:

```ts
ParticleSystem.create(emitter, {
	loop: true,
	rate: 0,
	lifetime: 2,
	bursts: {
		values: [
			{ time: 0.0, count: 80, cycles: 0, interval: 3 },
			{ time: 0.7, count: 100, cycles: 0, interval: 3 },
			{ time: 1.4, count: 60, cycles: 0, interval: 3 },
		],
	},
})
```

## Espaço de simulação

Controla se as partículas se movem em relação ao emissor (`PSS_LOCAL`, default) ou permanecem fixas nas coordenadas do world após aparecerem (`PSS_WORLD`).

Usa `PSS_WORLD` para um emissor em movimento que deve deixar um rastro atrás de si (por exemplo, um asteroide ou um foguete).

```ts
import { PBParticleSystem_SimulationSpace } from '@dcl/sdk/ecs'

ParticleSystem.create(emitter, {
	simulationSpace: PBParticleSystem_SimulationSpace.PSS_WORLD,
})
```

## Performance

O engine impõe um orçamento de partículas por scene e reduzirá automaticamente as taxas de emissão de todos os particle systems ativos na scene se a contagem total de partículas exceder o limite. Planeje suas scenes de acordo:

* Prefira menos systems com impacto visual forte em vez de muitos de baixo impacto.
* Usa `maxParticles` para limitar systems individuais.
* Usa `active` ou `playbackState` para desativar systems que estão fora da tela ou fora de alcance.
* Valores curtos `lifetime` mantêm a contagem de partículas ativas menor do que uma emissão alta `rate` por si só sugere.

O engine limita o número máximo de partículas renderizadas em qualquer momento a 1000. Se você estiver emitindo mais partículas do que isso, talvez não esteja vendo todas elas.

Lembre-se também de que as partículas só podem ser vistas por um player enquanto ele estiver dentro da sua scene. Players que estiverem olhando a scene de fora não verão nenhuma partícula até entrarem.

## Particle Lab

Entre no world [ParticleLab.dcl.eth](decentraland://?realm=particlelab.dcl.eth\&dclenv=org) para experimentar diferentes particle systems. Quando você se aproxima de um particle system, a UI exibirá todos os campos disponíveis; você pode ajustá-los em tempo real sem precisar recarregar a scene. Quando estiver satisfeito, clique no **Copiar** botão para copiar o código desse particle system para a sua área de transferência.


> [!NOTE]
> **💡 Dica**: Para um exemplo funcional que usa cada `ParticleSystem` campo, veja o [`0,7-particle-system`](https://github.com/decentraland/sdk7-test-scenes/tree/main/scenes/0,7-particle-system) test scene. Ele abrange todas as quatro formas de emissor, os `PSB_ADD`/`PSB_ALPHA`/`PSB_MULTIPLY` blend modes, os `PS_PLAYING`/`PS_PAUSED`/`PS_STOPPED` estados de playback, sprite sheets, emissão contínua `rate` em relação a bursts de uso único e em loop `bursts`, e `PSS_LOCAL` em relação a `PSS_WORLD` em um emissor em movimento.



---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-pt/scenes-sdk7/fundamentos-de-conteudo-3d/particle-system.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
