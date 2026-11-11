> Quantas coisas posso colocar na minha cena?

# Limitações da cena

A fim de melhorar o desempenho no metaverso, estabelecemos um conjunto de limites que toda scene deve seguir. Esses limites são por parcel. Portanto, quanto maior a scene, maiores são esses limites.

Ao trabalhar com o [Creator Hub](/docs/creator/content-creator-pt/scene-editor/primeiros-passos/editor-installation/), você pode ver estatísticas sobre os recursos usados por modelos 3D na sua scene, junto com os limites da sua scene.

![](/docs/_remote/ab87e37107598ee524831639.webp)

Você pode expandir este menu para ver os detalhes.

![](/docs/_remote/8d4619b1778683c8d72d3c61.webp)


> [!NOTE]
> **💡 Dica**: Para uma tabela de referência com todos os números específicos por quantidade de parcels, veja:
> 
> [Tabela de referência](https://docs.google.com/spreadsheets/d/1BTm0C20PqdQDAN7vOQ6FpnkVncPecJt-EwTSNHzrsmg/edit#gid=0)


## Regras de limitação da scene

Abaixo estão os números máximos de elementos que uma scene pode renderizar ao mesmo tempo:

> *n* representa o número de parcels que uma scene ocupa.

* **Triangles:** `n x 10000` Quantidade total de triangles de todos os models na scene.
* **Entities:** `n x 200` Quantidade de entities na scene.
* **Corpos:** `n x 300` Quantidade de meshes na scene.
* **Materials:** `log2(n+1) x 20` Quantidade de materials na scene. Inclui materials importados como parte dos models.
* **Textures:** `log2(n+1) x 10` Quantidade de textures na scene. Inclui textures importadas como parte dos models.
* **Height:** `330 metros` Altura máxima da scene, independentemente de quantos parcels ela ocupa.

  > Aviso: Entities posicionadas acima de 200 metros podem sofrer problemas de sync no multiplayer. Mantenha qualquer conteúdo com o qual os players interajam ou que seja sincronizado entre players abaixo dessa altura.

  > Importante: apenas as entities que estão sendo renderizadas atualmente na scene são contabilizadas nesses limites. Se a sua scene alterna entre modelos 3D, o que importa são os models renderizados em qualquer momento, não a soma total. Os avatares dos players e quaisquer itens trazidos por um player de fora da scene também não contam para calcular esses limites.
* **Tamanho total do arquivo:** Em Genesis City -`15 MB por parcel - 300 MB no máximo`. Para Worlds, veja [Tamanho do World](#world-size). Tamanho total dos arquivos enviados ao servidor de conteúdo. Inclui modelos 3D e áudio. Não inclui arquivos que não são enviados, como pacotes node.js. Você pode ver a lista completa de arquivos que estão sendo publicados e seus tamanhos antes de confirmar um deploy.
* **Quantidade de arquivos:** `200 arquivos por parcel` Quantidade total de arquivos enviados. Inclui modelos 3D e áudio. Não inclui arquivos que não são enviados, como pacotes node.js.
* **Tamanho máximo de arquivo** `50 MB por arquivo` Nenhum arquivo individual de qualquer tipo na scene pode exceder 50 MB. Scenes pequenas têm restrições ainda maiores porque o arquivo não pode ultrapassar o limite de Tamanho Total do Arquivo delas (por exemplo, uma scene de um único parcel é limitada a 15 MB no total).


> [!WARNING]
> **📔 Nota**: Os limites de tamanho de arquivo e de quantidade de arquivos são aplicados ao fazer deploy; excedê-los bloqueia o deploy. Os outros limites (triangles, entities, bodies, materials, textures, height) são limites flexíveis: eles são reportados como avisos pelo Creator Hub, mas excedê-los normalmente resulta em desempenho degradado em vez de uma falha crítica. Trate-os como recomendações fortes.



> [!NOTE]
> **💡 Dica**: Nem todos os arquivos na pasta do projeto da sua scene contam para o limite de tamanho de arquivo, apenas os que são enviados aos servidores. Todo o conteúdo da *node\_modules* pasta, que é muito grande, são dependências que não são enviadas e, portanto, não contam. O mesmo se aplica a quaisquer arquivos na `/src` pasta, já que o código-fonte não é enviado.
> 
> Você pode listar quaisquer outros arquivos ou pastas que queira excluir do envio no arquivo `.dclignore` do seu projeto. Observe que `.dclignore` também se aplica ao preview local, então não liste nada que sua scene realmente carrega. Veja [O arquivo dclignore](/docs/creator/content-creator-pt/scenes-sdk7/tipos-de-projetos/scene-files/#the-dclignore-file).


## Otimização

Veja [Otimização de performance](/docs/creator/content-creator-pt/scenes-sdk7/otimizacao/performance-optimization/) para dicas sobre como manter sua scene abaixo desses limites e fazê-la funcionar com mais fluidez para os players.


> [!NOTE]
> **📱 Mobile**: Os limites rígidos desta página se aplicam a todos os clients, mas dispositivos mobile têm menos margem do que desktop. Procure ficar confortavelmente abaixo desses limites ao mirar o [app mobile](/docs/creator/content-creator-pt/criar-para-mobile/cliente-movel/overview/), e prefira menos triangles, menos materials e textures menores.


## Limites da scene

Ao executar um preview, qualquer conteúdo localizado fora dos limites do parcel é destacado em vermelho quando renderizado. Se algum conteúdo estiver fora desses limites, essa parte do seu conteúdo não será renderizada quando os players visitarem sua scene.

Se a ponta de um objeto grande ultrapassar os limites, essa ponta será cortada do objeto.

Uma scene de um único parcel mede 16 metros x 16 metros. Se a scene tiver múltiplos parcels, as dimensões variam dependendo da disposição dos parcels.

É possível posicionar entities no subterrâneo, seja para escondê-las ou para fazer com que apenas uma parte delas apareça. Uma scene não pode ter túneis que vão abaixo da altura padrão do ground, os players não podem viajar abaixo da `y = 0` altura.

## Limitações de shader

Modelos 3D usados na Decentraland devem usar shaders e materials suportados. Veja [Materiais de modelos 3D](/docs/creator/content-creator-pt/modelacao-3d-e-animacoes/materials/) para uma lista de shaders suportados.

## Compressão de meshes


> [!WARNING]
> **📱 Meshes comprimidos com Draco no** [**client mobile**](/docs/creator/content-creator-pt/criar-para-mobile/cliente-movel/overview/) **exigem otimização de assets.** Modelos glTF/GLB que usam [Draco](https://google.github.io/draco/) compressão de meshes carregam no app mobile da Decentraland apenas depois de terem sido processados pelo pipeline de otimização de assets. Se seus models comprimidos com Draco não carregarem no mobile, verifique se eles foram otimizados — ou exporte seus models **sem** compressão Draco para evitar essa etapa completamente. Outras formas de otimização de glTF (quantização, compressão de texturas, decimação de meshes) não são afetadas.


## Iluminação

As condições de iluminação da scene não podem ser alteradas para todos os players a partir da configuração padrão, embora cada player individual seja livre para alterar suas próprias configurações de skybox na UI do Explorer.

## Restrições de tamanho de textures

Os tamanhos de textures devem usar números de largura e altura (em pixels) que correspondam aos números a seguir:

```
1, 2, 4, 8, 16, 32, 64, 128, 256, 512, 1024
```

> Esta sequência é composta por potências de dois: `f(x) = 2 ^ x` . 1024 é o número máximo que permitimos para o tamanho de uma texture; textures maiores do que isso são reduzidas pelo processo de conversão do asset bundle. Este é um requisito bastante comum entre outros engines de renderização, devido a otimizações internas dos processadores gráficos.

A largura e a altura não precisam ter o mesmo número, mas ambas precisam pertencer a essa sequência.

**O tamanho recomendado para textures é 1024x1024**, descobrimos que esse é o tamanho ideal para ser transportado por redes domésticas e para oferecer experiências razoáveis de carregamento/qualidade.

Exemplos de outros tamanhos válidos:

```
32x32
64x32
512x256
512x512
1024x1024
```


> [!WARNING]
> **📔 Nota**: Embora textures de tamanhos arbitrários às vezes funcionem, elas também costumam ser renderizadas com bugs e são mais instáveis. Recomendamos fortemente que todas as suas textures correspondam a esses tamanhos.


## Tamanho do World

Decentraland [Worlds](/docs/creator/content-creator-pt/scenes-sdk7/publicacao/publishing-options/#decentraland-worlds) têm limitações diferentes, já que são carregadas como scenes únicas.

* Worlds publicados em Decentraland NAMEs usam um orçamento de armazenamento compartilhado entre todos os Worlds pertencentes à mesma wallet. Cada NAME que você possui concede `100 MB`, cada parcel de LAND concede mais `100 MB`, e a cada 2.000 MANA mantidos concede mais `100 MB`. Você pode distribuir esse orçamento entre seus Worlds como quiser.
* Worlds publicados em domínios ENS têm um limite de `36 MB` que não pode ser expandido.

Você pode verificar quanto do seu orçamento foi usado e quanto resta na **Manage** seção do Creator Hub, ou na **Worlds** aba do [Builder](https://decentraland.org/builder/worlds). Se você exceder seu orçamento (por exemplo, após vender ou transferir assets), terá 48 horas para liberar espaço antes que seus Worlds fiquem inacessíveis.

Veja [Limite de tamanho de Worlds](/docs/creator/content-creator-pt/scenes-sdk7/tipos-de-projetos/kinds-of-project/#size-limits) para mais detalhes.

Tamanho total dos arquivos enviados ao servidor de conteúdo. Inclui modelos 3D e áudio. Não inclui arquivos que não são enviados, como pacotes node.js. Você pode ver a lista completa de arquivos que estão sendo publicados e seus tamanhos antes de confirmar um deploy.

Todos os outros limites em worlds são por parcel, incluindo triangles, materials etc. Como adicionar mais parcels a um world é grátis, você pode adicionar até 300x300 parcels (90.000 no total) à sua scene e ter os limites correspondentes a essa quantidade de parcels.


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-pt/scenes-sdk7/otimizacao/scene-limitations.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
