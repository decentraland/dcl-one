> Crie suas próprias bibliotecas do Decentraland para compartilhar com outras pessoas

# Criar uma Biblioteca

As libraries são uma ótima forma de partilhar soluções para problemas comuns. Desafios complexos podem ser abordados uma vez, as soluções encapsuladas numa library, e sempre que surgirem só precisas de escrever uma linha de código. Ao partilhar libraries com a comunidade, podemos fazer com que a produtividade de todos os creators cresça exponencialmente.

Atualmente, estas libraries em [página de exemplos](https://studios.decentraland.org/resources?sdk_version=SDK7&resource_type=Library) estão disponíveis para todos usarem. Incentivamos-te também a criar e partilhar as tuas próprias.

Seguindo os passos detalhados aqui, podes evitar a maior parte da complexidade que vem com a criação de uma library compatível com scenes do Decentraland e fácil de partilhar com outros.

## Criar uma library

> **📔 Nota**: Certifica-te de que estás a usar a versão 20 do Node ou mais recente antes de construíres a tua library.

Para criar a tua própria library e partilhá-la via NPM, faz o seguinte:

1. Cria uma nova pasta vazia e abre aí uma nova janela do VS Studio Code.
2. Seleciona o separador Decentraland na barra lateral esquerda do Visual Studio
3. Clique em **Criar projeto**, depois seleciona **Library**

   Isto criará todos os ficheiros e dependências predefinidos para uma library do Decentraland.
4. Define um `nome da scene` em `package.json`. Este é o nome que será usado ao publicar no NPM; certifica-te de que não existe já outro projeto com esse nome no npm. Preenche também a descrição e quaisquer tags para ajudar outros a encontrar a tua library.
5. Cria um novo *public* repositório GitHub para o teu projeto.

   O projeto está configurado para usar GitHub Actions para publicar uma nova versão do pacote em cada push para `main`.

   No `package.json` ficheiro, certifica-te de que em `repositório` aponta o `url` campo para o URL do teu repo. Dessa forma, é fácil de encontrar para os utilizadores que navegam [npmjs.com](https://www.npmjs.com).
6. Obter um token NPM:
   1. Cria uma conta ou inicia sessão em <https://www.npmjs.com/>.
   2. Vai para **Account > Access Tokens > Generate new Token** Cria um novo token, atribui-lhe **Publicar** permissões.

      A mensagem de sucesso inclui uma string para o token recém-gerado. Copia essa string e guarda-a num local seguro.
7. No teu repositório github, vai para **Settings > Secrets > Actions**, depois clica em **New Repository Secret**.

   Dá um nome ao teu secret **NPM\_TOKEN**, e cola a string do token NPM como valor.
8. Faz push de uma alteração (qualquer alteração) para a branch principal do teu repositório GitHub e o pacote será publicado.

   É isso, agora o pacote pode ser instalado por qualquer pessoa usando `npm i <package-name>`! Agora já podes encontrar a tua library se a pesquisares pelo nome em [npmjs.com](https://www.npmjs.com).
9. Completa a `/src` pasta do teu projeto com toda a funcionalidade que queres expor e faz push das alterações para o GitHub.

## Desenvolver

Adiciona quaisquer funções, components, etc. que queiras expor ao `index.ts` ficheiro da library, assim será fácil importá-los.

Por exemplo, se adicionares um `RotatorComponent` componente para `index.ts`, podes então importar este component para uma scene escrevendo o seguinte.:

`import { RotatorComponent } from 'my-library'`

Testa a tua library usando-a numa nova scene do Decentraland enquanto a desenvolves. Instala o teu pacote npm numa scene executando `npm i <package-name>`. Depois cria uma scene que sirva para experimentar a funcionalidade da library. Tenta cobrir diferentes casos de teste com a tua scene, para garantir que a tua library funciona como esperado em todos os casos.

### Iterações rápidas

Se precisares de fazer continuamente pequenos ajustes à tua library e testá-los, pode tornar-se exaustivo ter de fazer commit das alterações e depois esperar que a publicação no npm termine antes de poderes experimentar a nova versão numa scene. Felizmente, há uma alternativa muito mais rápida para executar testes com a tua library.

Numa **test scene project**:

1. Adiciona este script à `scripts` lista em package.json: `"link-sdk": "cd node_modules/@dcl/sdk && npm link && cd ../js-runtime && npm link"`
2. Execute `npm install`
3. Execute `npm install YOUR_LIBRARY_PATH`, por exemplo, `npm install /User/projects/dcl-sdk7-library-test`
4. Execute `npm run link-sdk`

No **projeto da library**:

1. Execute `npm install`
2. Execute `npm run build`
3. Executa: `npm link @dcl/sdk @dcl/js-runtime`

> **❗Aviso**: A ordem destes passos é importante. Pode não funcionar noutra ordem.

Isto manterá a tua scene sincronizada com a versão da library que está diretamente no teu disco local. Para quaisquer alterações à library que queiras testar, basta executar `npm run build` na pasta da library; não é preciso publicar as alterações no GitHub ou no NPM.

> **💡 Dica**: Para verificar se a ligação foi bem-sucedida, executa `npm ls --link`. Deverás ver o nome da library a apontar para a pasta nos teus ficheiros locais.

Se fizeres alterações à library, tens de executar `npm run build` para as atualizar. Para evitar ter de o fazer sempre:

1. Adiciona o script "start": "tsc -p tsconfig.json --watch" à library
2. Execute `npm start` na library

Quando terminares os testes, lembra-te de desassociar a library.

1. Na pasta da scene executa `npm install <library name>`
2. Depois, na library executa `rm -rf node_modules && npm install`

## Versionamento

As versões da tua library são publicadas automaticamente em `npm` com um `@latest` e um `@next` flag.

A `@next` flag aponta sempre para o último commit na `main` branch. Esta versão pode ser instável, pois as últimas alterações podem não ter sido testadas. Os utilizadores da tua library podem instalá-la (por sua conta e risco) fazendo `npm i <library name>@next`.

A `@latest` flag aponta para a última versão estável da library. É esta a versão que os utilizadores da tua library devem instalar normalmente. É a versão que o npm obtém quando fazem `npm i <library name>`.

Para fazer com que o `@latest` flag aponte para os teus commits mais recentes, terás de fazer uma **release** da tua library no GitHub.

1. Abre a página GitHub do teu projeto. Abre o **Releases** link, na margem direita da página.
2. Clique em **Draft new release**.
3. On **Chose tag** escreve um nome para a tua nova versão, por exemplo "1.1.0". Escreve também um nome em **Release Title**. Muitas vezes, é o mesmo nome, "1.1.0".

   > Importante: o tag tem de ser um número, sem letras ou símbolos adicionais. O número tem de ser superior às publicações anteriores.
4. Descreve a tua release, para que os utilizadores saibam o que há de novo.

   > Dica: clica no botão **Auto generate release notes** para listar todos os commits desde a última release.
5. Clica em **Publish Release**. Esta ação desencadeia uma nova publicação automática para o npm com o `@latest` flag. Agora, os utilizadores da tua library irão descarregar esta versão.

## Notas sobre usabilidade

Faz o teu melhor para tornar a tua library fácil de usar por outros creators. As nossas suposições ajudam sempre a moldar as tools que criamos. Muitas vezes, estas suposições parecem óbvias para nós, mas outros, com outro contexto, podem ter suposições completamente diferentes.

* Escolhe os nomes com cuidado para tudo na tua library, incluindo functions, components e parâmetros. É melhor ter um nome longo e autoexplicativo do que um curto que seja um mistério completo.
* Pensa de forma abrangente sobre os diferentes possíveis casos de uso da tua library. Talvez alguém use a tua library num smart wearable, talvez alguém precise de ligar ou desligar o teu system em certos momentos, talvez alguém precise de adicionar várias instâncias do teu system na mesma scene. Nem toda a gente estará a enfrentar os mesmos desafios que tu.
* Dito isto, não precisas de suportar todos os possíveis casos de uso. Na verdade, há sempre compromissos a fazer entre flexibilidade e simplicidade de uso; criar boas tools é, acima de tudo, encontrar o equilíbrio certo. Sê estratégico quanto ao limite do que decidiste não suportar. No entanto, é importante documentar claramente as limitações que escolheste.
* Isto leva-nos à documentação. Documenta a tua library de forma clara, com exemplos, descrições do que cada parâmetro faz, comentários sobre que cenários não funcionariam e que suposições estás a fazer sobre o contexto onde ela está a ser usada. O README.md da library é muitas vezes o melhor local para expor este conteúdo.
* Outra ótima ferramenta é adicionar comentários de metadata diretamente no teu código. Estes comentários são apresentados pelo IDE (como o VS Studio Code) à medida que os utilizadores da tua library escrevem. Podes descrever para que serve cada function/component, descrever o que cada parâmetro espera receber e o que uma function devolve. Graças a isso, os utilizadores da tua library não precisam de alternar entre o código e a documentação; está tudo num só lugar! Por exemplo, olha para esta function da `@dcl/ecs-scene-utils` library:

  ```ts
  /**
   * Limita um valor para que não ultrapasse um valor mínimo ou máximo.
   *
   * @param value - número de entrada
   * @param min - valor mínimo de saída.
   * @param max - valor máximo de saída.
   * @returns O valor resultante mapeado entre o min e o max
   * @public
   */
  export function clamp(value: number, min: number, max: number) {
  	let result = value

  	if (value > max) {
  		result = max
  	} else if (value < min) {
  		result = min
  	}
  	return result
  }
  ```

  Os utilizadores desta library veem estas dicas apresentadas à medida que escrevem `clamp()` função:
* Declara sempre os types. Não deixes as pessoas a adivinhar que estrutura de objeto terão de consumir.
* Mantém a tua library leve e focada numa única funcionalidade. Quando uma scene do Decentraland é compilada, todo o código de todas as suas libraries é empacotado com ela, incluindo código que nunca é chamado pela scene. Por este motivo, é melhor evitar criar libraries grandes e pesadas. É preferível ter libraries leves que permitam aos creators usar apenas o que realmente precisam.
* Fica atento ao repo da tua library; as pessoas podem fazer pull requests ou reportar issues.
* Inclui informação de licenciamento no teu repo, assim os outros saberão se a tua library pode ser usada livremente. A library predefinida inclui uma licença Apache 2 aberta; sente-te à vontade para a alterar, se quiseres.


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-pt/scenes-sdk7/bibliotecas/create-libraries.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
