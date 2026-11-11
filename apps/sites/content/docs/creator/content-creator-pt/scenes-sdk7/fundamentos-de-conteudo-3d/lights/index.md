> Saiba como usar luzes na sua scene

# Luzes

As luzes são uma parte fundamental dos gráficos 3D. Elas são usadas para iluminar a cena e criar uma sensação de profundidade e realismo, e podem ser usadas para criar diferentes humores e atmosferas.

Por padrão, a cena é iluminada por uma única luz direcional. Esta é uma luz que brilha em uma direção específica e é usada para simular o sol ou a lua. Veja [Controle do Skybox](/docs/creator/content-creator-pt/scenes-sdk7/interatividade/skybox-control/) para mais informações.

Você pode adicionar até 1 luz por parcel na sua cena.

Há dois tipos de luz suportados:

* Luz pontual: uma luz que brilha em todas as direções a partir de um ponto específico.
* Luz spot: uma luz que brilha em uma direção específica e cobre apenas uma área em forma de cone.


> [!WARNING]
> **📔 Nota**: Luzes que estão incorporadas dentro de um `.glb` ou `.gltf` arquivo são ignoradas pela Decentraland. Se o seu modelo 3D foi exportado com luzes nele, elas não iluminarão nada na cena. As únicas fontes de luz válidas são entidades que têm o `LightSource` componente. Se você quiser que o modelo de uma lâmpada brilhe, adicione um `LightSource` à mesma entidade, ou a uma entidade filha posicionada onde está a lâmpada.



> [!NOTE]
> **💡 Dica**: O catálogo de ativos do Creator Hub tem **Spotlight** e **Point Light** smart items prontos, na categoria **lights** . Cada um reúne um modelo, um `LightSource`configurado, e ações Turn On, Turn Off e Toggle, então você não precisa escrever nenhum código. Há também um **Spotlight** decorativo em **decorações**, que é apenas um modelo e não projeta luz. Escolha na categoria **lights** quando quiser iluminação real.


## Adicionando uma luz

Para adicionar uma luz à sua cena, você precisa criar uma entidade de luz e adicionar o `LightSource` componente a ela.

```ts
import { engine, LightSource } from '@dcl/sdk/ecs'

const light = engine.addEntity()

Transform.create(light, {
  position: Vector3.create(10, 3, 10),
})

LightSource.create(light, {
  type: LightSource.Type.Point({})
})
```


> [!WARNING]
> **📔 Nota**: Uma luz com o brilho padrão mal será visível com o sol do meio-dia, como no mundo real. Você pode usar o [Controle do Skybox](/docs/creator/content-creator-pt/scenes-sdk7/interatividade/skybox-control/) para forçar o skybox para o horário noturno, ou aumentar o brilho definindo a `intensidade` propriedade do `LightSource` componente para um valor maior.


## Luzes spot

As luzes spot são luzes que brilham em uma direção específica e cobrem uma área específica em forma de cone. A direção da luz é definida pelo componente Transform da entidade: a luz brilha ao longo da direção frontal da entidade (seu eixo local +Z). A abertura do cone é definida pelas propriedades `innerAngle` e `outerAngle` do `LightSource` componente.

O exemplo abaixo gira a entidade 90 graus em torno do seu eixo X, então a luz aponta diretamente para baixo. Uma rotação de -90 graus a aponta para o céu.

```ts
import { engine, LightSource } from '@dcl/sdk/ecs'

const light = engine.addEntity()

Transform.create(light, {
  position: Vector3.create(10, 3, 10),
  rotation: Quaternion.fromEulerDegrees(90, 0, 0),
})

LightSource.create(light, {
	type: LightSource.Type.Spot({
      innerAngle: 30,
      outerAngle: 60
    }),
	shadow: true
})
```

O `innerAngle` é o ângulo do cone interno, onde a luz está no brilho máximo, e o `outerAngle` é o ângulo do cone externo, onde a luz vai se apagando gradualmente em direção às bordas do cone. Você pode brincar com esses valores para criar efeitos diferentes, para luzes mais focadas ou mais difusas.

## Intensidade e cor

Todas as luzes, tanto pontuais quanto spot, têm uma cor e uma intensidade. A cor é definida pelo componente `color` propriedade do `LightSource` , e a intensidade é definida pela `intensidade` propriedade.

```ts
import { engine, LightSource } from '@dcl/sdk/ecs'

const light = engine.addEntity()

Transform.create(light, {
  position: Vector3.create(10, 3, 10),
})

LightSource.create(light, {
  type: LightSource.Type.Point({}),
  color: Color3.Red(),
  intensity: 10000,
})
```

A cor é um objeto `Color3` , se não for especificada, ela será branca. Você pode definir qualquer cor que quiser, o que pode ter um grande impacto no clima da cena.

A intensidade é expressa em candelas (lúmens/m^2 a 1 m de distância, ou lúmens divididos por 4\*pi).

A intensidade padrão é 16000; este é o brilho de uma lâmpada média no mundo real e pode ser visto até cerca de 10 metros de distância da fonte de luz. Se você precisar que a luz seja visível de mais longe, ou durante o dia, você pode aumentar a intensidade.

A distância na qual a luz é visível é a quarta raiz do valor da intensidade (`intensity^0.25`).

* Com uma intensidade de 625, a luz é visível até cerca de 5 metros de distância.
* Com uma intensidade de 10000, a luz é visível até cerca de 10 metros de distância.
* Com uma intensidade de 160000, a luz é visível até cerca de 20 metros de distância.

## Sombras

Cada luz pode ou não projetar sombras. Por padrão, elas não projetam, mas você pode ativá-las definindo o `shadow` propriedade do `LightSource` componente para `true`.

```ts
import { engine, LightSource } from '@dcl/sdk/ecs'

const light = engine.addEntity()

Transform.create(light, {
  position: Vector3.create(10, 3, 10),
  rotation: Quaternion.fromEulerDegrees(90, 0, 0),
})

LightSource.create(light, {
	type: LightSource.Type.Spot({
      innerAngle: 30,
      outerAngle: 60
    }),
	shadow: true
})
```


> [!WARNING]
> **📔 Nota**: Sombras são suportadas apenas para luzes spot. Luzes pontuais não suportam sombras. Se houver várias luzes na cena, algumas delas podem não estar projetando sombras, veja [Otimização de luzes](#light-optimization) para mais informações.


## Ligando e desligando uma luz

O componente LightSource tem uma propriedade `active` que pode ser usada para ligar e desligar uma luz. Isso é útil se você quiser desligar uma luz sem removê-la da cena, ou sem definir a `intensidade` como 0 e perder a referência de qual era a intensidade original.

```ts
import { engine, LightSource } from '@dcl/sdk/ecs'

const light = engine.addEntity()

Transform.create(light, {
  position: Vector3.create(10, 3, 10),
  rotation: Quaternion.fromEulerDegrees(90, 0, 0),
})

LightSource.create(light, {
	type: LightSource.Type.Spot({
      innerAngle: 30,
      outerAngle: 60
    }),
	shadow: true,
	active: true
})

const lightSwitch = engine.addEntity()

Transform.create(lightSwitch, {
  position: Vector3.create(8, 1, 10),
})

MeshRenderer.setBox(lightSwitch)

MeshCollider.setBox(lightSwitch)

pointerEventsSystem.onPointerDown(
	{
		entity: lightSwitch,
		opts: {
			button: InputAction.IA_POINTER,
			hoverText: 'Clique',
		},
	},
	function () {
		const lightSource = LightSource.getMutable(light)
		lightSource.active = !lightSource.active
	}
)
```

## Otimização de luzes

As fontes de luz podem ter um impacto bastante grande no desempenho da sua cena. Por isso, o engine irá otimizar automaticamente a cena desativando algumas das luzes ou suas sombras, começando pelas que estão mais distantes.

O número de luzes ativas permitidas em uma cena é limitado a uma por parcel, e além disso depende das configurações de qualidade selecionadas pelo usuário.

* Baixa qualidade: nenhuma luz na cena
* Qualidade média: máximo de 6 luzes (em uma cena com parcels suficientes)
* Alta qualidade: máximo de 10 luzes (em uma cena com parcels suficientes)
* Mobile: o app tem seus próprios presets, de nenhuma luz na cena em Very Low até 8 luzes em High. Veja [Luzes e partículas no mobile](/docs/creator/content-creator-pt/criar-para-mobile/desenvolver/optimize-performance/#lights-and-particles).

O controle deslizante das configurações de qualidade pode reduzir ainda mais o limite de um preset.

Se houver mais luzes do que o permitido, o engine desativará automaticamente as luzes com base na proximidade da fonte de luz ao player. À medida que o player se move, o engine reativará as luzes que estiverem suficientemente perto do player.


> [!WARNING]
> **📔 Nota**: No preset de qualidade Low, as luzes da cena são totalmente desligadas. Uma cena corretamente construída `LightSource` então não mostra nada, e nenhuma quantidade de intensidade, alcance ou ajuste de material muda isso. Se uma luz "não funcionar", verifique a configuração de qualidade antes de qualquer outra coisa. Considere Medium como a configuração mais baixa que sua cena suporta, em vez de exigir High.
> 
> Um material emissivo que brilha não é prova de que uma luz está funcionando. Um mesh de lâmpada brilhando e a luz que ele projeta nas superfícies próximas são duas coisas separadas que você precisa verificar.


Como alguns players não veem nenhuma luz da cena (qualidade Low no desktop, Very Low no mobile), certifique-se de que sua cena ainda funcione sem elas. Tudo o que os players precisem ver, como o caminho à frente ou um objeto com o qual possam interagir, deve se destacar por meio de materiais emissivos, texturas ou do skybox. Use `LightSource` como um elemento extra, não como a única indicação. O mesmo vale para sombras: não faça da sombra a única maneira de dizer onde um objeto está.

Em todos os casos, o engine só renderizará sombras de até 3 fontes de luz. Se houver mais luzes com sombras do que 3, o engine desativará automaticamente as sombras das luzes restantes que estiverem mais distantes.

Além do número máximo de luzes permitidas, as sombras também dependem da distância do player. As distâncias exatas variam conforme o tipo de luz e as configurações de qualidade do player, mas como regra geral:

* A menos de 10 metros de distância: as sombras são renderizadas como sombras suaves (alta qualidade)
* Entre 10 e 20 metros de distância: as sombras são renderizadas como sombras duras (baixa qualidade)
* A mais de 20 metros de distância: as sombras não são renderizadas

As próprias fontes de luz continuam iluminando a cena a distâncias muito maiores: elas só são desativadas quando o player está a mais de 160 metros de distância (10 parcels). Isso torna as luzes adequadas para configurações em grande escala, como iluminação de palco em eventos ao vivo, onde a maior parte do público está longe das fontes de luz.

Também é importante notar que as luzes só são renderizadas se o player estiver dentro da cena. Se o player estiver fora da cena, as luzes não serão renderizadas.

## Alcance da luz

O componente lightSource tem uma propriedade `range` que pode ser usada para definir a distância máxima na qual a luz é visível. Por padrão, o valor da `range` propriedade é -1, o que significa que o alcance da luz depende da intensidade da luz.

O alcance é calculado como a quarta raiz do valor da intensidade (`intensity^0.25`).

* Com uma intensidade de 16000, o alcance é de cerca de 11 metros.
* Com uma intensidade de 160000, o alcance é de cerca de 20 metros.
* Com uma intensidade de 1600000, o alcance é de cerca de 36 metros.

A configuração padrão garante que a curva de queda seja suave e pareça natural. Mas, caso você queira limitar o alcance da luz, pode definir a `range` propriedade para um número positivo.

```ts
import { engine, LightSource } from '@dcl/sdk/ecs'

const light = engine.addEntity()

Transform.create(light, {
  position: Vector3.create(10, 3, 10),
})

LightSource.create(light, {
  type: LightSource.Type.Point({}),
  intensity: 16000,
  range: 20,
})
```


> [!WARNING]
> **📔 Nota**: Definir a `range` propriedade para um número positivo cortará abruptamente a luz na distância informada. Isso é útil se você quiser criar uma luz visível apenas em uma área específica, ou para otimizar o desempenho da sua cena. Definir a `range` para um valor maior do que a luz realmente consegue alcançar com sua intensidade atual não terá efeito algum.


## Máscaras de luz

Você pode usar uma máscara de luz para produzir alguns efeitos interessantes. Em vez de iluminar toda a área, você pode aplicar uma textura para ser usada como filtro que iluminará apenas partes da área.

As máscaras são mais comumente usadas com luzes spot, mas também podem ser usadas com luzes pontuais. Abaixo está um exemplo de uma luz spot com uma máscara aplicada.

```ts
import { engine, LightSource } from '@dcl/sdk/ecs'

const light = engine.addEntity()

Transform.create(light, {
  position: Vector3.create(10, 3, 10),
})

LightSource.create(light, {
	type: LightSource.Type.Spot({
      innerAngle: 30,
      outerAngle: 60
    }),
	shadow: true,
	shadowMaskTexture: Material.Texture.Common({src: "assets/scene/images/lightmask1.png"})
})
```

Por exemplo, você pode aplicar a imagem abaixo para produzir um efeito divertido em que a luz só fica visível nas áreas onde a textura é branca. Você pode usar isso com luzes de qualquer cor e intensidade.

![](/docs/_remote/b2cd4d8e15d52341e551dffb.webp)

Com uma imagem em preto e branco, a luz ou atinge uma área ou não. Também é possível usar uma imagem com cores, e essas cores irão tingir a luz em cada área; isso pode ser usado para projetar um logotipo ou imagem colorida sobre uma superfície.


> [!WARNING]
> **📔 Nota**: A imagem usada como máscara deve ter altura e largura em pixels que sejam uma potência de 2 (por exemplo: 1024, 512, 256). Este recurso não funciona com imagens que tenham dimensões diferentes.


Ao aplicar uma máscara a uma luz pontual, a textura será envolvida como um cubo ao redor da fonte de luz. Se você quiser evitar bordas visíveis entre os lados do cubo, certifique-se de que a textura tenha continuidade nas bordas.

```ts
import { engine, LightSource } from '@dcl/sdk/ecs'

const light = engine.addEntity()

Transform.create(light, {
  position: Vector3.create(10, 3, 10),
})

LightSource.create(light, {
	type: LightSource.Type.Point({}),
	shadowMaskTexture: Material.Texture.Common({src: "assets/scene/images/point-light-mask1.png"})
})
```

Por exemplo, a imagem abaixo exibe cada uma das letras em lados diferentes do cubo (Y em cima, -Y embaixo, X à direita, -X à esquerda, Z na frente, -Z atrás).

![](/docs/_remote/384393f9ee871b7e47ed3a80.webp)


> [!NOTE]
> **💡 Dica**: Combine luzes com um [Sistema de partículas](/docs/creator/content-creator-pt/scenes-sdk7/fundamentos-de-conteudo-3d/particle-system/) para efeitos visuais mais ricos. Por exemplo, combine um sistema de partículas de fogo com blend aditivo e uma luz pontual quente para criar uma fogueira convincente.



> [!NOTE]
> **💡 Dica**: Para um exemplo funcional deste componente, veja a cena de teste [`0,4-dynamic-lights`](https://github.com/decentraland/sdk7-test-scenes/tree/main/scenes/0,4-dynamic-lights) , que cobre tanto `LightSource.Type.Point` e `LightSource.Type.Spot`, alterações em tempo de execução via `LightSource.getMutable()`, sombras e `shadowMaskTexture`.


## Luzes no escuro

As luzes são mais fáceis de apreciar quando o céu não compete com elas. Os componentes `Skybox`, `Sun` e `EnvironmentFog` podem bloquear a luz do sol, as cores do céu (e, com elas, a luz ambiente), a névoa, as nuvens e as estrelas, para que suas luzes se tornem a única fonte de iluminação. Veja [Escuridão total](/docs/creator/content-creator-pt/scenes-sdk7/interatividade/skybox-control/#complete-darkness).

No preset de qualidade Low, as luzes da cena não são renderizadas de forma alguma, então uma cena totalmente escura aparece completamente preta para esses players. Use materiais emissivos para manter visível tudo o que for essencial. O app mobile e o explorer Bevy ainda não suportam esses componentes, então os players nesses clients continuam vendo o céu normal.


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-pt/scenes-sdk7/fundamentos-de-conteudo-3d/lights.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
