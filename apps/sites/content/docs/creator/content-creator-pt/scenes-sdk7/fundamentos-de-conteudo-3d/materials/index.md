# Materiais

Saiba como adicionar materiais e texturas a entities com formas primitivas.

## Materiais

Materiais podem ser aplicados a entities que usam formas primitivas (cubo, esfera, plano, etc.) adicionando um `Material` componente. Esse componente tem vários campos que permitem configurar as propriedades do material, adicionar uma textura, etc.

*glTF* models incluem seus próprios materiais, que são importados implicitamente para uma scene junto com o modelo. Para modificar ou substituir esses materiais, use o `GltfNodeModifiers` componente. Veja [Modificar materiais glTF](#modify-gltf-materials) para mais detalhes.

Ao importar um modelo 3D com seus próprios materiais, tenha em mente que nem todos os shaders são suportados pelo engine do Decentraland. Apenas materiais padrão e materiais PBR (physically based rendering) são suportados. Veja [considerações sobre modelos 3D externos](https://github.com/decentraland/docs/blob/main/creator/3d-modeling/materials.md) para mais detalhes.

Há diferentes tipos de materiais suportados:

* PBR (Physically Based Rendering): O tipo de material mais comum no Decentraland. Ele suporta cores sólidas ou texturas, e diferentes propriedades como metallic, emissive, transparência, etc. Leia mais sobre [PBR](https://en.wikipedia.org/wiki/Physically_based_rendering).
* Materiais básicos: Eles não respondem a luzes e sombras, o que os torna ideais para exibir imagens de billboard.

## Use o Scene Editor no Creator Hub

A maneira mais fácil de dar um Material a uma entity é usar o Scene Editor. Você pode adicionar um **Material** componente à sua entity e então configurar todos os campos disponíveis na UI do Scene Editor. Veja [Adicionar Componentes](/docs/creator/content-creator-pt/scene-editor/construir/components/#add-components).

## Adicionar um material

O exemplo a seguir cria um material PBR e define alguns de seus campos para dar a ele uma cor vermelha e propriedades metálicas. Esse material é adicionado a uma entity que também tem uma forma de caixa, então ele colorirá a caixa com esse material.

```ts
//Criar entidade e atribuir forma
const meshEntity = engine.addEntity()
Transform.create(meshEntity, {
	position: Vector3.create(4, 1, 4),
})
MeshRenderer.setBox(meshEntity)

//Criar material e configurar seus campos
Material.setPbrMaterial(meshEntity, {
	albedoColor: Color4.Red(),
	metallic: 0.8,
	roughness: 0.1,
})
```

Para alterar o material de uma entity que já tem um `Material` componente, execute `Material.setPbrMaterial()` ou qualquer uma das outras funções auxiliares e isso substituirá o material original. Não há necessidade de remover o original `Material` nem de usar a sintaxe avançada.

```ts
//Criar entidade e atribuir forma
const meshEntity = engine.addEntity()
Transform.create(meshEntity, {
	position: Vector3.create(4, 1, 4),
})
MeshRenderer.setBox(meshEntity)

//Criar material e configurar seus campos
Material.setPbrMaterial(meshEntity, {
	albedoColor: Color4.Red(),
})

//Substituir por um novo componente de material
Material.setPbrMaterial(meshEntity, {
	albedoColor: Color4.Blue(),
})
```


> [!WARNING]
> **📔 Nota**: As unidades `Material` componente deve ser importado via
> 
> > `import { Material } from "@dcl/sdk/ecs"`
> 
> Consulte [Importações](/docs/creator/content-creator-pt/scenes-sdk7/primeiros-passos/coding-scenes/#imports) para saber como lidar com isso facilmente.


## Cores do Material

Dê a um material uma cor sólida. Em um Material PBR, você define o `albedoColor` campo. As cores albedo respondem à luz e podem incluir sombras nelas.

Os valores de cor são do tipo `Color4`, composta por *r*, *g* e *b* valores (vermelho, verde e azul). Cada um deles recebe valores entre 0 e 1. Ao definir valores diferentes para eles, você pode compor qualquer cor visível. Para preto, defina os três como 0. Para branco, defina todos como 1.


> [!WARNING]
> **📔 Nota**: Se você definir qualquer cor em `albedoColor` para um valor maior que *1*, *emissivo*, com mais intensidade quanto maior o valor. Então, por exemplo, `{r: 15, g: 0, b: 0}` produz um brilho vermelho muito intenso.


Consulte [tipos de cor](/docs/creator/content-creator-pt/scenes-sdk7/fundamentos-de-conteudo-3d/color-types/) para mais detalhes sobre como definir cores.

Você também pode editar os seguintes campos em um Material PBR para ajustar melhor como sua cor é percebida:

* *emissiveColor*: A cor emitida pelo material.
* *reflectivityColor*: também conhecido como *Specular Color* em outras nomenclaturas.

Para criar um material de cor sólida que não seja afetado pela luz e pelas sombras do ambiente, crie um material básico em vez de um material PBR.

```ts
Material.setBasicMaterial(myEntity, {
	diffuseColor: Color4.Black(),
})
```

## Usando texturas

Defina um arquivo de imagem como uma textura em um material definindo o `texture` parâmetro.

```ts
//Criar entidade e atribuir forma
const meshEntity = engine.addEntity()
Transform.create(meshEntity, {
	position: Vector3.create(4, 1, 4),
})
MeshRenderer.setBox(meshEntity)

//Criar material e configurar seus campos
Material.setPbrMaterial(meshEntity, {
	texture: Material.Texture.Common({
		src: 'assets/materials/wood.png',
	}),
})
```

No exemplo acima, a imagem do material está localizada em uma `assets/materials` pasta, que está localizada no nível raiz da pasta do projeto da scene.


> [!NOTE]
> **💡 Dica**: Recomendamos manter seus arquivos de imagem de textura em algum lugar na pasta `/assets` dentro da sua scene.


Ao criar uma textura, você também pode passar parâmetros adicionais:

* `filterMode`: Determina como os pixels da textura são esticados ou comprimidos quando renderizados. Isso recebe um valor do `TextureFilterMode` enum. Veja [Escala da textura](#texture-scaling).
* `wrapMode`: Determina como uma textura é aplicada em mosaico a um objeto. Isso recebe um valor do `TextureWrapMode` enum. Veja [Repetição da textura](#texture-wrapping).

```ts
Material.setPbrMaterial(myEntity, {
	texture: Material.Texture.Common({
		src: 'assets/materials/wood.png',
		filterMode: TextureFilterMode.TFM_BILINEAR,
		wrapMode: TextureWrapMode.TWM_CLAMP,
	}),
})
```

Para criar uma textura que não seja afetada pela luz e pelas sombras do ambiente, crie um material básico em vez de um material PBR.

```ts
Material.setBasicMaterial(myEntity, {
	texture: Material.Texture.Common({
		src: 'assets/materials/wood.png',
	}),
})
```

### Texturas de uma URL externa

Você pode apontar a textura do seu material para uma URL externa em vez de um caminho interno no projeto da scene.

```ts
Material.setBasicMaterial(myEntity, {
	texture: Material.Texture.Common({
		src: 'https://wearable-api.decentraland.org/v2/collections/community_contest/wearables/cw_tuxedo_tshirt_upper_body/thumbnail',
	}),
})
```

A URL deve começar com `https`, `http` URLs não são suportadas.

### Repetição da textura

Você pode definir como uma textura se alinha com uma superfície. Por padrão, a textura é esticada para ocupar a superfície uma vez, mas você pode escalá-la e deslocá-la.

Os seguintes campos estão disponíveis em todas as texturas:

* `offset`: Desloca a textura para alterar seu alinhamento. O valor é um Vector2, em que ambos os eixos vão de 0 a 1, sendo 1 a largura ou altura total da textura.
* `tiling`: Escala a textura. O valor padrão é o Vector 2 `[1, 1]`, que faz a imagem se repetir uma vez cobrindo toda a superfície.
* `TextureWrapMode`: Determina o que acontece se o mosaico da imagem não cobrir toda a superfície. Essa propriedade recebe seus valores do `TextureWrapMode` enum, que permite os seguintes valores:
  * `TextureWrapMode.TWM_CLAMP`: A textura é exibida apenas uma vez no tamanho especificado. O restante da superfície da malha permanece transparente. O valor de `tiling` é ignorado.
  * `TextureWrapMode.TWM_REPEAT`: A textura é repetida tantas vezes quanto couber na malha, usando o tamanho especificado.
  * `TextureWrapMode.TWM_MIRROR`: Assim como no wrap, a textura é repetida tantas vezes quanto couber, mas a orientação dessas repetições é espelhada.

```ts
Material.setPbrMaterial(myEntity, {
	texture: Material.Texture.Common({
		src: 'assets/materials/wood.png',
		wrapMode: TextureWrapMode.TWM_REPEAT,
		offset: Vector2.create(0, 0.2),
		tiling: Vector2.create(1, 1),
	}),
})
```


> [!WARNING]
> **📔 Nota**: As unidades `offset` e `tiling` essas propriedades são suportadas apenas no cliente desktop DCL 2.0.


Use esse recurso para cobrir uma grande superfície com um padrão em mosaico. Por exemplo, repita a imagem a seguir:

![](/docs/_remote/5a88e3a6f74d3fb726a05771.webp)

```ts
Material.setPbrMaterial(myEntity, {
	texture: Material.Texture.Common({
		src: 'assets/materials/wood.png',
		wrapMode: TextureWrapMode.TWM_REPEAT,
		tiling: Vector2.create(8, 8),
	}),
})
```

![](/docs/_remote/5b75756c2234feb52d56048a.webp)

No exemplo abaixo, a textura usa um *espelhado* modo de repetição, e cada repetição da textura ocupa apenas 1/4 da superfície. Isso significa que veremos 4 cópias da imagem, espelhadas entre si em ambos os eixos.

```ts
Material.setPbrMaterial(myEntity, {
	texture: Material.Texture.Common({
		src: 'materials/atlas.png',
		wrapMode: TextureWrapMode.TWM_MIRROR,
		tiling: Vector2.create(0.25, 0.25),
	}),
})
```

### Tweens de textura

Faça uma textura deslizar suavemente usando um componente `Tween` configurado com o modo `TextureMove` . O tween altera gradualmente o valor das `offset` ou dos `tiling` propriedades de uma textura ao longo de um período de tempo, de forma suave e otimizada.


> [!WARNING]
> **📔 Nota**: Os Texture Tweens são um recurso que só é suportado no cliente desktop DCL 2.0.


Use a `Tween` componente com a função `setTextureMove` para mover a textura entre duas posições.

```ts
Tween.setTextureMove(myEntity, Vector2.create(0, 0), Vector2.create(1, 0), 2000)
```

O tween de textura recebe as seguintes informações:

* `entity`: A entity cuja textura será movida
* `start`: Um Vector2 para a posição inicial
* `end`: Um Vector2 para a posição final
* `duration`: Quantos milissegundos leva para mover entre as duas posições

Este outro parâmetro opcional também está disponível:

* `movementType`: define se o movimento será no `offset` ou dos `tiling` campo. Por padrão, ele usa `offset`.
* `easingFunction`: A curva da taxa de mudança ao longo do tempo; o valor padrão é `EasingFunction.EF_LINEAR`. Outros valores fazem a mudança acelerar e/ou desacelerar em taxas diferentes.

```ts
const myEntity = engine.addEntity()

MeshRenderer.setPlane(myEntity)

Transform.create(myEntity, {
	position: Vector3.create(4, 1, 4),
})

Material.setPbrMaterial(myEntity, {
	texture: Material.Texture.Common({
		src: 'materials/water.png',
		wrapMode: TextureWrapMode.TWM_REPEAT,
	}),
})

Tween.setTextureMove(myEntity, Vector2.create(0, 0), Vector2.create(0, 1), 1000)
```

O exemplo acima executa um tween que dura 1 segundo e move a textura apenas uma vez. Para obter um movimento contínuo, por exemplo para simular a queda de uma cascata, você precisa usar `setTextureMoveContinuous`.

```ts
const myEntity = engine.addEntity()

MeshRenderer.setPlane(myEntity)

Transform.create(myEntity, {
	position: Vector3.create(4, 1, 4),
})

Material.setPbrMaterial(myEntity, {
	texture: Material.Texture.Common({
		src: 'materials/water.png',
		wrapMode: TextureWrapMode.TWM_REPEAT,
	}),
})

Tween.setTextureMoveContinuous(myEntity, Vector2.create(0, 1), 1)
```

O exemplo acima usa `setTextureMoveContinuous`, com uma direção de `(0, 1)`, e uma velocidade de 1 unidade por segundo.

O tween contínuo de textura recebe as seguintes informações:

* `entity`: A entity cuja textura será movida
* `direction`: Um Vector2 para o movimento
* `speed`: Quantas unidades por segundo a entity se moverá

Estes outros parâmetros opcionais também estão disponíveis:

* `movementType`: define se o movimento será no campo offset ou tiling. Por padrão, usa offset.
* `duration`: Quantos milissegundos sustentar o movimento. Após esse tempo, o movimento será interrompido.

**Sequências complexas de tween**

Você também pode fazer os movimentos da textura seguirem uma sequência complexa com quantos passos você quiser. Use o `sequence` campo para listar quantos tweens você quiser; eles serão executados sequencialmente depois do primeiro tween descrito no `Tween` componente.

```ts
//(...)
Tween.setTextureMove(myEntity, Vector2.create(0, 0), Vector2.create(0, 1), 1000)

TweenSequence.create(myEntity, {
	sequence: [
		{
			mode: Tween.Mode.TextureMove({
				start: Vector2.create(0, 1),
				end: Vector2.create(1, 1),
			}),
			duration: 1000,
			easingFunction: EasingFunction.EF_LINEAR,
		},
		{
			mode: Tween.Mode.TextureMove({
				start: Vector2.create(1, 1),
				end: Vector2.create(1, 0),
			}),
			duration: 1000,
			easingFunction: EasingFunction.EF_LINEAR,
		},
		{
			mode: Tween.Mode.TextureMove({
				start: Vector2.create(1, 0),
				end: Vector2.create(0, 0),
			}),
			duration: 1000,
			easingFunction: EasingFunction.EF_LINEAR,
		},
	],
	loop: TweenLoop.TL_RESTART,
})
```

Observe que, ao definir um tween dentro de um TweenSequence, você precisa usar o formato mais detalhado de `Tween.Mode.TextureMove` para definir o tween.

### Texturas em várias camadas

Você pode usar vários arquivos de imagem como camadas para compor texturas mais realistas, por exemplo incluindo uma `bumpTexture` e uma `emissiveTexture`.

```ts
Material.setPbrMaterial(myEntity, {
	texture: Material.Texture.Common({
		src: 'materials/wood.png',
	}),
	bumpTexture: Material.Texture.Common({
		src: 'materials/woodBump.png',
	}),
	emissiveTexture: Material.Texture.Common({
		src: 'materials/glow.png',
	}),
})
```

O `bumpTexture` pode simular relevos e rugas em uma superfície, modificando como os normais da superfície se comportam em cada pixel.

![](/docs/_remote/b02dcfc858055419931f396c.webp)

O `emissiveTexture` pode acentuar o brilho em certas partes de um material, para obter efeitos muito interessantes.

**Definir UVs**

Outra alternativa para alterar a escala ou o alinhamento de uma textura é configurar *uv* propriedades no [componente MeshRenderer](/docs/creator/content-creator-pt/scenes-sdk7/fundamentos-de-conteudo-3d/shape-components/).

Você define *u* e *v* coordenadas na imagem 2D da textura para corresponder aos vértices da forma. Quanto mais vértices a entity tiver, mais *uv* coordenadas precisam ser definidas na textura; um plano, por exemplo, precisa ter 8 *uv* pontos definidos, 4 para cada uma de suas duas faces.

```ts
const meshEntity = engine.addEntity()
Transform.create(meshEntity, {
	position: Vector3.create(4, 1, 4),
})
MeshRenderer.setPlane(
	meshEntity,
	[
		0, 0.75,

		0.25, 0.75,

		0.25, 1,

		0, 1,

		0, 0.75,

		0.25, 0.75,

		0.25, 1,

		0, 1,
	]
)

Material.setPbrMaterial(myEntity, {
	texture: Material.Texture.Common({
		src: 'materials/wood.png',
		wrapMode: TextureWrapMode.TWM_REPEAT,
	}),
})
```

O exemplo a seguir inclui uma função que simplifica a definição de uvs. A `setUVs` função definida aqui recebe um número de linhas e colunas como parâmetros e define os uvs de modo que a imagem da textura seja repetida um número específico de vezes.

```ts
const meshEntity = engine.addEntity()
Transform.create(meshEntity, {
	position: Vector3.create(4, 1, 4),
})
MeshRenderer.setBox(meshEntity, setUVs(3, 3))

Material.setPbrMaterial(myEntity, {
	texture: Material.Texture.Common({
		src: 'materials/atlas.png',
		wrapMode: TextureWrapMode.TWM_REPEAT,
	}),
})

function setUVs(rows: number, cols: number) {
	return [
		// Lado norte de um plano não rotacionado
		0, //canto inferior esquerdo
		0,

		cols, //canto inferior direito
		0,

		cols, //canto superior direito
		rows,

		0, //canto superior esquerdo
		rows,

		// Lado sul de um plano não rotacionado
		cols, // canto inferior direito
		0,

		0, // canto inferior esquerdo
		0,

		0, // canto superior esquerdo
		rows,

		cols, // canto superior direito
		rows,
	]
}
```

Para definir os UVs de uma `caixa` forma de malha, a mesma estrutura se aplica. Cada uma das 6 faces do cubo recebe 4 pares de coordenadas, um para cada canto. Todos esses 48 valores são listados como um único array.


> [!WARNING]
> **📔 Nota**: As propriedades UV estão atualmente disponíveis apenas em `plano` e em `caixa` formas. Além disso, *uv* valores afetam igualmente todas as camadas da textura, já que são definidos na *forma*.


### Escala da textura

Quando as texturas são esticadas ou reduzidas para um tamanho diferente do da imagem original da textura, isso às vezes pode criar artefatos. Em um ambiente 3D, os efeitos de perspectiva causam isso naturalmente. Existem vários [algoritmos de filtragem de textura](https://en.wikipedia.org/wiki/Texture_filtering) que existem para compensar isso de diferentes maneiras.

O `Material` o objeto usa o *bilinear* algoritmo por padrão, mas permite configurar para usar os algoritmos *vizinho mais próximo* ou *trilinear* em vez disso, definindo a `filterMode` propriedade da textura. Isso recebe um valor do `TextureFilterMode` enum:

* `TextureFilterMode.TFM_POINT`: Usa um algoritmo de "vizinho mais próximo". Essa configuração é ideal para gráficos em estilo pixel art, pois os contornos permanecerão nitidamente marcados à medida que a textura for vista maior na tela, em vez de ficar borrada.
* `TextureFilterMode.TFM_BILINEAR`: Usa um algoritmo bilinear para estimar a cor de cada pixel.
* `TextureFilterMode.TFM_TRILINEAR`: Usa um algoritmo trilinear para estimar a cor de cada pixel.

```ts
Material.setPbrMaterial(myEntity, {
	texture: Material.Texture.Common({
		src: 'materials/atlas.png',
		filterMode: TextureFilterMode.TFM_BILINEAR,
	}),
})
```

## Materiais sem iluminação

Na maioria das vezes você vai querer que os materiais na sua scene sejam afetados pelas condições de iluminação, incluindo sombras e a coloração pelas mudanças de tonalidade de diferentes horários do dia. Mas, em outros casos, você pode querer mostrar as cores em seu estado puro. Isso é útil ao reproduzir vídeos, ou também para marcadores abstratos que precisam se destacar, destinados a sinalizar dicas para o player.

Para criar um material sem iluminação, use `Material.setBasicMaterial`. Materiais básicos não têm todas as mesmas propriedades que os materiais PBR; eles só têm as essenciais:

* `diffuseColor`: Color4 para a cor
* `texture`: Texture
* `alphaTexture`: Textura separada para a camada de transparência
* `alphaTest`: Limiar para obter transparência com base na cor da textura
* `castShadows`: Se false, nenhuma sombra é projetada sobre outras entities na scene.

```ts
Material.setBasicMaterial(screen, {
	diffuseColor: Color4.Red(),
})
```

## Retratos de Avatar

Para exibir uma imagem em miniatura de qualquer player, use `Material.Texture.Avatar` ao definir a textura do seu material, passando o endereço de um player existente. Isso cria uma textura a partir de uma imagem 256x256 do player, mostrando cabeça e ombros. O player é exibido usando o conjunto de wearables que o servidor atual registrou por último.

```ts
Material.setPbrMaterial(myEntity, {
	texture: Material.Texture.Avatar({
		userId: '0x517....',
	}),
})
```

![](/docs/_remote/3907d796a3f7f47add13d85a.webp)

Você pode obter o retrato de qualquer player do Decentraland, mesmo que ele não esteja conectado no momento e mesmo que não tenha um nome do Decentraland reivindicado.

As seguintes propriedades são suportadas dentro do objeto que você passa como argumento:

* `userId`: ID do usuário cujo perfil você deseja exibir
* `filterMode`: Determina como os pixels da textura são esticados ou comprimidos quando renderizados. Isso recebe um valor do `TextureFilterMode` enum. Veja [Escala da textura](#texture-scaling).
* `wrapMode`: Determina como uma textura é aplicada em mosaico a um objeto. Isso recebe um valor do `TextureWrapMode` enum. Veja [Repetição da textura](#texture-wrapping).

## Materiais transparentes

Para tornar transparente um material com uma cor sólida, basta definir a cor como um `Color4`, e definir o 4º valor para algo entre *0* e *1*. Quanto mais próximo de *1*, mais opaco ele ficará.

```typescript
let transparentRed = Color4.create(1, 0, 0, 0.5)

Material.setPbrMaterial(meshEntity, {
	albedoColor: transparentRed,
})
```

Se um material usar uma textura .png que inclua transparência, ele será opaco por padrão, mas você pode ativar sua transparência definindo o `transparencyMode` a `MaterialTransparencyMode.MTM_ALPHA_BLEND`.

![](/docs/_remote/98a85f0b352d710c0c48036c.webp)

```typescript
Material.setPbrMaterial(floor, {
	texture: Material.Texture.Common({
		src: 'assets/scene/transparent-image.png',
	}),
	transparencyMode: MaterialTransparencyMode.MTM_ALPHA_BLEND,
})
```

O `transparencyMode` pode ter os seguintes valores:

* `MaterialTransparencyMode.MTM_OPAQUE`: Sem transparência alguma
* `MaterialTransparencyMode.MTM_ALPHA_TEST`: Cada pixel é totalmente opaco ou totalmente transparente, com base num limiar.
* `MaterialTransparencyMode.MTM_ALPHA_BLEND`: Valores intermédios são possíveis com base no valor de cada pixel.
* `MaterialTransparencyMode.MTM_ALPHA_TEST_AND_ALPHA_BLEND`: Usa uma combinação de ambos os métodos.
* `MaterialTransparencyMode.MTM_AUTO`: Determina o método com base na textura fornecida.


> [!WARNING]
> **📔 Nota**: Define sempre `transparencyMode` tu mesmo quando queres que o próprio canal alpha de uma texture apareça. No client de desktop, o modo predefinido `MTM_AUTO` ignora o alpha da texture e só faz blend quando o `albedoColor` alpha é inferior a 1, por isso uma `.png` sai totalmente opaca. Com `MTM_ALPHA_BLEND` ou `MTM_ALPHA_TEST`, a transparência é o alpha da texture multiplicado pelo `albedoColor` alpha.


Se definires o `transparencyMode` a `MaterialTransparencyMode.MTM_ALPHA_TEST`, podes afinar o limiar usado para determinar se cada pixel é transparente ou não. Define a `alphaTest` propriedade entre *0* e *1*. Por predefinição, o seu valor é *0.5*.

```ts
// Usando alpha test
Material.setPbrMaterial(meshEntity1, {
	texture: Material.Texture.Common({
		src: 'images/myTexture.png',
	}),
	transparencyMode: MaterialTransparencyMode.MTM_ALPHA_TEST,
	alphaTest: 1,
})
```

Ao usar um [material unlit](#unlit-materials), podes adicionar um `alphaTexture` para tornar apenas certas regiões do material transparentes, com base numa texture.


> [!WARNING]
> **📔 Nota**: Pinta a máscara de branco onde o material deve aparecer e de preto onde deve desaparecer. O client lê o brilho dos canais de cor da máscara, não o seu canal alpha, por isso qualquer canal no valor máximo conta como opaco: uma máscara vermelha pura também é opaca. Não reutilizes a tua texture de cor como a própria máscara, ou cada pixel escuro nela torna-se transparente e apaga os teus contornos e sombreado.


![](/docs/_remote/9b242e1d8862ee66017fbceb.webp)

```ts
// Corta um ecrã até um círculo com uma imagem de máscara
Material.setBasicMaterial(meshEntity1, {
	texture: Material.Texture.Common({
		src: 'images/myTexture.png',
	}),
	alphaTexture: Material.Texture.Common({
		src: 'assets/scene/circle_mask.png',
		wrapMode: TextureWrapMode.TWM_MIRROR,
	}),
})
```

Mais duas coisas a saber sobre `alphaTexture` no client de desktop:

* **Funciona apenas em materiais unlit.** `Material.setPbrMaterial` aceita um `alphaTexture` mas nunca o usa. Para um material PBR, coloca a transparência no próprio canal alpha da texture de cor e define `transparencyMode` em vez disso.
* **Um material unlit sem um `alphaTexture` é totalmente opaco**e `alphaTest` não tem qualquer efeito sobre ele. Um `texture` mais `alphaTest` sozinho desenha a cor dos pixels transparentes, que normalmente é preta.

Isto pode ser usado de formas muito interessantes em conjunto com vídeos. Vê [vídeo em reprodução](/docs/creator/content-creator-pt/scenes-sdk7/media/video-playing/).

## Vídeo em reprodução

Para fazer streaming de vídeo a partir de um URL para um material, ou reproduzir um vídeo a partir de um ficheiro guardado na scene, vê [vídeo em reprodução](/docs/creator/content-creator-pt/scenes-sdk7/media/video-playing/).

O vídeo é usado como texture num material; podes definir qualquer uma das outras propriedades dos materials para alterar o aspeto do ecrã de vídeo.

## Sintaxe avançada

A sintaxe completa para criar um `Materiais` component, sem quaisquer helpers para o simplificar, é assim:

```ts
Material.create(myEntity, {
	material: {
		$case: 'pbr',
		pbr: {
			texture: {
				tex: {
					$case: 'texture',
					texture: {
						src: 'images/scene-thumbnail.png',
					},
				},
			},
		},
	},
})

Material.create(myEntity, {
	material: {
		$case: 'pbr',
		pbr: {
			texture: {
				tex: {
					$case: 'avatarTexture',
					avatarTexture: {
						userId: '0x517....',
					},
				},
			},
		},
	},
})
```

É assim que o protocolo base interpreta os components Materials. As helper functions abstraem isto e expõem uma sintaxe mais amigável, mas por trás dos panos produzem esta sintaxe.

O `$case` fields permitem especificar um dos tipos permitidos. Cada tipo suporta um conjunto diferente de parâmetros. Há dois níveis de `$case` aqui:

* O `material` field suporta os valores `pbr` e `unlit`, para definir o tipo de material.
* O `tex` field dentro de uma texture suporta os seguintes valores, para definir o tipo de texture:
  * `texture`
  * `avatarTexture`
  * `videoTexture`

Dependendo do valor de `$case`, é válido definir o objeto para o tipo correspondente, passando quaisquer propriedades relevantes.

Para adicionar um `Material` component a uma entity que potencialmente já tem uma instância deste component, usa `Material.createOrReplace()`. As helper functions como `Material.setPbrMaterial()` tratam de sobrescrever instâncias existentes do component, mas executar `Material.create()` numa entity que já tem este component devolve um erro.

## Modificar materiais glTF

Use a `GltfNodeModifiers` component para modificar os materials de um *glTF* model. Este component permite substituir os materials de um *glTF* model pelos teus próprios materials. Podes usar qualquer uma das propriedades do `Material` component, incluindo texture, video texture, materiais unlit, etc.

Há duas formas de usar o `GltfNodeModifiers` component:

* Modificar o material de todo o model deixando o `path` property como uma string vazia.
* Modificar o material de um node específico no model (ou de vários nodes) definindo a `path` property para o path para o node.

### Modificar o material de todo o model

O exemplo seguinte mostra como modificar o material de um *glTF* model. Neste caso, o material de todo o model é modificado para vermelho.

```ts
import { GltfNodeModifiers, GltfContainer, Transform } from '@dcl/sdk/ecs'

const myEntity = engine.addEntity()

GltfContainer.create(myEntity, {
	src: 'models/myModel.glb',
})

Transform.create(myEntity, {
	position: Vector3.create(4, 0, 4),
})

GltfNodeModifiers.create(myEntity, {
	modifiers: [
		{
			path: '',
			material: {
				material: {
					$case: 'pbr',
					pbr: {
						albedoColor: Color4.Red(),
					},
				},
			},
		},
	],
})
```

O `GltfNodeModifiers` component tem as seguintes propriedades:

* `modifiers`: Um array de modifiers. Cada modifier tem as seguintes propriedades:
  * `path`: O path para o node no model a modificar.
  * `material`: O material a usar.

O `path` property é uma string que representa o path para o node no *glTF* model a modificar. Se quiseres modificar o material de todo o model, podes usar uma string vazia. Se quiseres modificar o material de um node específico, podes usar o path para o node. O path tem de apontar para um node mesh, não para um vertex node.


> [!NOTE]
> **💡 Dica**: Podes usar a [app Babylon Sandbox](https://sandbox.babylonjs.com/) para inspecionar o *glTF* model e encontrar o path para o node que queres modificar.
> 
> Em alguns models, no entanto, o Babylon sandbox pode listar paths que pertencem a vertices em vez de meshes, o que não funcionará. Se tentares usar um path inválido, a console da scene mostrará uma mensagem de erro que inclui a lista completa de paths válidos nesse model.


O `material` property é um objeto que representa o material a usar. Tem de ser escrito usando a [sintaxe avançada](#advanced-syntax) para materials, como mostrado no exemplo acima. Helper functions como `Material.setPbrMaterial()` não podem ser usadas aqui.

### Modificar o material de um node específico no model

O exemplo seguinte mostra como modificar o material de um node específico no *glTF* model. Neste caso, o material da cabeça é modificado para usar uma texture alternativa.

```ts
import { GltfNodeModifiers, GltfContainer, Transform } from '@dcl/sdk/ecs'

const myEntity = engine.addEntity()

GltfContainer.create(myEntity, {
	src: 'models/myModel.glb',
})

Transform.create(myEntity, {
	position: Vector3.create(4, 0, 4),
})

GltfNodeModifiers.create(myEntity, {
	modifiers: [
		{
			path: 'M_Head_BaseMesh',
			material: {
				material: {
					$case: 'pbr',
					pbr: {
						texture: Material.Texture.Common({
							src: 'assets/scene/images/blinking-head.png',
						}),
					},
				},
			},
		},
	],
})
```

Um `GltfNodeModifiers` pode conter vários modifiers, cada um modificando um node diferente no model. O exemplo seguinte mostra como modificar o material da cabeça e do corpo de um *glTF* model.

```ts
import { GltfNodeModifiers, GltfContainer, Transform } from '@dcl/sdk/ecs'

const myEntity = engine.addEntity()

GltfContainer.create(myEntity, {
	src: 'models/myModel.glb',
})

Transform.create(myEntity, {
	position: Vector3.create(4, 0, 4),
})

GltfNodeModifiers.create(myEntity, {
	modifiers: [
		{
			path: 'M_Head_BaseMesh',
			material: {
				material: {
					$case: 'pbr',
					pbr: {
						albedoColor: Color4.Red(),
					},
				},
			},
		},
		{
			path: 'M_Body_BaseMesh',
			material: {
				material: {
					$case: 'pbr',
					pbr: {
						albedoColor: Color4.Blue(),
					},
				},
			},
		},
	],
})
```

### Modificar campos de um material existente

O component Material fornece uma interface simplificada para aceder e modificar as propriedades do component Material. Elimina a necessidade de navegar por estruturas union profundamente aninhadas (`PBR` vs `Unlit`, `texture` vs `avatarTexture` vs `videoTexture`), tornando a manipulação de materials mais intuitiva e menos propensa a erros.

Os métodos seguintes permitem a manipulação fácil do `Material` component:

* `Material.getFlat(entity: Entity): ReadonlyFlatMaterial`: Devolve um **apenas leitura** `FlatMaterial` objeto accessor que fornece acesso direto de leitura às propriedades do material. Devolverá um erro se a entity não tiver um `Material` component. Para evitar isto, os métodos seguintes podem ser usados como alternativa.

```ts
const src = Material.getFlat(entity).texture.src
```

* `Material.getFlatOrNull(entity: Entity): ReadonlyFlatMaterial | null`: Devolve um **apenas leitura ou null**, dependendo de a entity ter ou não um `Material` componente.

```ts
const src = Material.getFlatOrNull(entity)?.texture.src
```

* `Material.getFlatMutable(entity: Entity): FlatMaterial`: Devolve um **leitura e escrita** `FlatMaterial`objeto que permite modificar as suas propriedades. Lançará um erro se a entity chamada não tiver o `Material` componente.

```ts
const src = Material.getFlatMutable(entity).texture.src
```

* `Material.getFlatMutableOrNull(entity: Entity): FlatMaterial | null`: Devolve um **leitura e escrita ou null** `FlatMaterial` objeto que permite modificar as suas propriedades. Caso a entity não tenha um `component Material`, devolverá `null`.

```ts
const flatMaterial = Material.getFlatMutableOrNull(entity)
if (flatMaterial) {
	flatMaterial.texture.src = myNewTextureFile
}
```

### Remover sombras de um model glTF

Para remover sombras de um *glTF* model, podes definir a `castShadows` property para `false` no submenu `GltfNodeModifiers` objeto. Isto mantém o material original do model, mas impede-o de projetar sombras. Isto é útil para models que não devem projetar sombras, como feixes de luz.

```ts
import { GltfNodeModifiers } from '@dcl/sdk/ecs'

GltfNodeModifiers.create(myEntity, {
	modifiers: [
		{
			path: '',
			castShadows: false,
		},
	],
})
```


> [!NOTE]
> **💡 Dica**: Textures usadas em materials também podem ser aplicadas a efeitos de partículas. Vê [Particle System](/docs/creator/content-creator-pt/scenes-sdk7/fundamentos-de-conteudo-3d/particle-system/) para detalhes sobre usar textures personalizadas e sprite sheets com partículas.



> [!NOTE]
> **💡 Dica**: Para exemplos práticos de materials, vê a [`52,-52-testing-gallery`](https://github.com/decentraland/sdk7-test-scenes/tree/main/scenes/52,-52-testing-gallery) scene de teste, que varre combinações de PBR metallic/roughness/emissive/normal-map lado a lado, [`74,-8-gltfnodemodifier`](https://github.com/decentraland/sdk7-test-scenes/tree/main/scenes/74,-8-gltfnodemodifier), que substitui materials PBR e unlit em nodes GLTF individuais, e [`0,3-texture-movement`](https://github.com/decentraland/sdk7-test-scenes/tree/main/scenes/0,3-texture-movement), que anima o deslocamento e o tiling da texture.



---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-pt/scenes-sdk7/fundamentos-de-conteudo-3d/materials.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
