# Estilização e temas

Esta página aborda padrões abrangentes de estilização para UIs web de Decentraland usando styled-components com a solução de estilização do Material UI.


> [!NOTE]
> Todos os exemplos de código neste documento são ilustrativos. Eles não representam componentes de produção, mas demonstram padrões e normas.


## Princípios Fundamentais

1. **Somente sintaxe de objeto** - Use notação de objeto para um forte suporte a TypeScript
2. **Tema em primeiro lugar** - Todos os valores vêm do tema UI2
3. **Sem estilos inline** - Use componentes estilizados para toda a estilização
4. **Tudo tipado** - Aproveite TypeScript para props e tema
5. **Responsivo por padrão** - Use os breakpoints do tema

***

## Padrão de Sintaxe de Objeto

Componentes UI2 **DEVEM** usar a sintaxe de objeto. Isso garante forte suporte a TypeScript, validação de csstype e integração direta com o tema.


> [!WARNING]
> A sintaxe de template literal **não é permitida**. Use sempre a notação de objeto.


### Exemplo Básico

```tsx
// ✅ Bom: Sintaxe de objeto
const Button = styled('button')({
  color: 'turquoise',
  padding: '8px 16px',
});

// ❌ Ruim: Sintaxe de template literal
const Button = styled.button`
  color: turquoise;
  padding: 8px 16px;
`;
```

### Com Tema e Props

```tsx
interface ButtonProps {
  primary?: boolean;
  size?: 'small' | 'medium' | 'large';
}

// ✅ Bom: Sintaxe de objeto com tema e props
const Button = styled('button')<ButtonProps>(({ theme, primary, size = 'medium' }) => ({
  color: primary ? theme.palette.primary.main : theme.palette.text.primary,
  backgroundColor: primary ? theme.palette.primary.main : 'transparent',
  borderRadius: theme.shape.borderRadius,
  padding: {
    small: theme.spacing(0.5, 1),
    medium: theme.spacing(1, 2),
    large: theme.spacing(1.5, 3),
  }[size],
}));
```


> [!CAUTION]
> **Regra**: Os valores DEVEM sempre vir do tema UI2. Códigos hexadecimais arbitrários ou valores em pixels não são permitidos.


***

## Sintaxe de Elemento

Ao estilizar elementos HTML nativos, use sempre a forma de chamada de função `styled('tag')`.

### Sintaxe Correta

```tsx
// ✅ Bom: Forma de chamada de função
const Container = styled('div')({
  display: 'flex',
  flexDirection: 'column',
});

const Action = styled('button')(({ theme }) => ({
  color: theme.palette.primary.main,
  padding: theme.spacing(1, 2),
}));

const Label = styled('label')(({ theme }) => ({
  color: theme.palette.text.secondary,
  fontSize: theme.typography.caption.fontSize,
}));
```

### Sintaxe Incorreta

```tsx
// ❌ Ruim: Forma de propriedade (sintaxe legada)
const Container = styled.div`
  display: flex;
  flex-direction: column;
`;

const Action = styled.button`
  color: ${props => props.theme.palette.primary.main};
`;
```

***

## Sem Estilos Inline

Estilos inline (`style={...}`) **NÃO DEVEM** ser usados em componentes UI2.

### Por que não usar estilos inline?

* Ignoram a tipagem do tema
* Mais difíceis de manter
* Impedem a reutilização
* Não podem ser otimizados
* Sem validação de TypeScript

### A Forma Certa

```tsx
// ❌ Ruim: Estilos inline
<Card 
  key={id} 
  style={{ backgroundColor: color } as React.CSSProperties}
>
  <CardHeader>
    <Title style={{ fontSize: 20 }}>{title}</Title>
  </CardHeader>
</Card>

// ✅ Bom: Componentes estilizados com props
interface CardProps {
  backgroundColor: string;
}

interface TitleProps {
  size: number;
}

const StyledCard = styled('div')<CardProps>(({ backgroundColor, theme }) => ({
  backgroundColor,
  borderRadius: theme.shape.borderRadius,
  padding: theme.spacing(2),
}));

const Title = styled('h2')<TitleProps>(({ size, theme }) => ({
  fontSize: theme.typography.pxToRem(size),
  color: theme.palette.text.primary,
}));

<StyledCard key={id} backgroundColor={color}>
  <CardHeader>
    <Title size={20}>{title}</Title>
  </CardHeader>
</StyledCard>
```

***

## Breakpoints

Use `theme.breakpoints` em vez de valores em pixels fixos.

### Helpers de Breakpoint

| Auxiliar              | Uso                                     | Descrição                |
| --------------------- | --------------------------------------- | ------------------------ |
| `up(key)`             | `theme.breakpoints.up('md')`            | Largura mínima e acima   |
| `down(key)`           | `theme.breakpoints.down('md')`          | Largura máxima e abaixo  |
| `between(start, end)` | `theme.breakpoints.between('sm', 'lg')` | Entre dois breakpoints   |
| `only(key)`           | `theme.breakpoints.only('md')`          | Somente neste breakpoint |

### Exemplos

```tsx
// ❌ Ruim: Breakpoints fixos
const Panel = styled('div')`
  @media (max-width: 768px) {
    width: 100%;
  }
`;

// ✅ Bom: Breakpoints do tema
interface PanelProps {
  expanded: boolean;
}

const Panel = styled('div')<PanelProps>(({ theme, expanded }) => ({
  width: expanded ? '400px' : '0',
  transition: theme.transitions.create('width'),
  
  [theme.breakpoints.down('sm')]: {
    width: expanded ? '100%' : '0',
  },
}));
```

### Múltiplos Breakpoints

```tsx
const Layout = styled('div')(({ theme }) => ({
  display: 'grid',
  gridTemplateColumns: '1fr 320px',
  gap: theme.spacing(2),
  
  // Mobile: coluna única
  [theme.breakpoints.down('md')]: {
    gridTemplateColumns: '1fr',
  },
  
  // Desktop grande: barra lateral mais larga
  [theme.breakpoints.up('xl')]: {
    gridTemplateColumns: '1fr 400px',
  },
  
  // Faixa de tablet: espaçamento diferente
  [theme.breakpoints.between('sm', 'lg')]: {
    gap: theme.spacing(3),
  },
  
  // Somente tablet
  [theme.breakpoints.only('md')]: {
    padding: theme.spacing(2),
  },
}));
```

***

## Escala de Espaçamento

Use `theme.spacing` exclusivamente para margens, paddings e gaps.

### Convenção de Espaçamento

* `theme.spacing(n)` em que `n` é um número
* A unidade base é normalmente 8px
* `spacing(1)` = 8px, `spacing(2)` = 16px, etc.
* Decimais permitidos: `spacing(1.5)` = 12px

```tsx
// ✅ Bom: Espaçamento do tema
const Box = styled('div')(({ theme }) => ({
  padding: theme.spacing(2),           // 16px
  margin: theme.spacing(1, 0),         // 8px vertical, 0 horizontal
  gap: theme.spacing(1.5),             // 12px
  paddingInline: theme.spacing(3),     // 24px à esquerda/direita
}));

// ❌ Ruim: Valores brutos em pixels
const Box = styled('div')({
  padding: '16px',
  margin: '8px 0',
  gap: '12px',
});
```

### Padrões Comuns de Espaçamento

```tsx
const Card = styled('div')(({ theme }) => ({
  // Espaçamento uniforme em todos os lados
  padding: theme.spacing(3),
  
  // Vertical/horizontal diferentes
  padding: theme.spacing(2, 3),  // 16px vertical, 24px horizontal
  
  // Todos os lados diferentes
  padding: theme.spacing(1, 2, 3, 2),  // top, right, bottom, left
  
  // Propriedades lógicas (preferidas para suporte a RTL)
  paddingBlock: theme.spacing(2),      // top e bottom
  paddingInline: theme.spacing(3),     // esquerda e direita
  marginBlockStart: theme.spacing(1),  // margin-top
}));
```

***

## Z-Index e Empilhamento

Siga a escala de z-index do tema. Nunca use valores de z-index arbitrários.

### Valores de Z-Index do Tema

```tsx
theme.zIndex.mobileStepper  // 1000
theme.zIndex.fab            // 1050
theme.zIndex.speedDial      // 1050
theme.zIndex.appBar         // 1100
theme.zIndex.drawer         // 1200
theme.zIndex.modal          // 1300
theme.zIndex.snackbar       // 1400
theme.zIndex.tooltip        // 1500
```

### Uso Correto

```tsx
// ✅ Bom: z-index do tema
const StickyBar = styled('div')(({ theme }) => ({
  position: 'sticky',
  top: 0,
  zIndex: theme.zIndex.appBar,
  backgroundColor: theme.palette.background.paper,
}));

const Overlay = styled('div')(({ theme }) => ({
  position: 'fixed',
  inset: 0,
  zIndex: theme.zIndex.modal,
  backgroundColor: 'rgba(0, 0, 0, 0.5)',
}));

// ❌ Ruim: z-index arbitrário
const StickyBar = styled('div')({
  position: 'sticky',
  top: 0,
  zIndex: 999,  // Não faça isso
});
```

### Melhores Práticas de Contexto de Empilhamento

* Esteja atento ao criar novos contextos de empilhamento
* Evite desnecessário `position: relative` nos pais
* Documente por que um z-index é necessário
* Se você precisar de uma nova camada, adicione-a ao tema primeiro

***

## Tokens de Cor

Consuma sempre as cores de `theme.palette` e `dclColors`. Sem valores hexadecimais ad hoc.

### Estrutura da Paleta

```tsx
// Cores de texto
theme.palette.text.primary
theme.palette.text.secondary
theme.palette.text.disabled

// Cores de fundo
theme.palette.background.default
theme.palette.background.paper

// Primária/Secundária/Erro
theme.palette.primary.main
theme.palette.primary.light
theme.palette.primary.dark
theme.palette.primary.contrastText

// Cores de ação
theme.palette.action.active
theme.palette.action.hover
theme.palette.action.selected
theme.palette.action.disabled
theme.palette.action.disabledBackground

// Divisores
theme.palette.divider
```

### Cores de Decentraland

```tsx
import { dclColors } from 'decentraland-ui2';

// Cores de raridade
dclColors.rarity.unique
dclColors.rarity.mythic
dclColors.rarity.legendary
dclColors.rarity.epic
dclColors.rarity.rare
dclColors.rarity.uncommon
dclColors.rarity.common
```

### Exemplos

```tsx
// ✅ Bom: Cores do tema
const Chip = styled('span')(({ theme }) => ({
  color: theme.palette.text.secondary,
  backgroundColor: theme.palette.background.paper,
  borderColor: theme.palette.divider,
  
  '&:hover': {
    backgroundColor: theme.palette.action.hover,
  },
}));

const RarityBadge = styled('span')<{ rarity: string }>(({ rarity }) => ({
  backgroundColor: dclColors.rarity[rarity],
  color: '#FFFFFF', // Cor de contraste - aceitável se documentada
  padding: '4px 8px',
  borderRadius: '4px',
}));

// ❌ Ruim: Valores hexadecimais arbitrários
const Chip = styled('span')({
  color: '#666666',
  backgroundColor: '#FFFFFF',
  borderColor: '#E0E0E0',
});
```

***

## Estados Interativos

Todos os controles interativos DEVEM mostrar estados visíveis para hover, focus, active e disabled.

### Componente Interativo Completo

```tsx
const InteractiveButton = styled('button')(({ theme }) => ({
  // Estado base
  padding: theme.spacing(1, 2),
  backgroundColor: theme.palette.primary.main,
  color: theme.palette.primary.contrastText,
  border: 'none',
  borderRadius: theme.shape.borderRadius,
  cursor: 'pointer',
  outline: 'none',
  transition: theme.transitions.create([
    'background-color',
    'transform',
    'box-shadow',
  ]),
  
  // Estado de hover (mouse)
  '&:hover': {
    backgroundColor: theme.palette.primary.dark,
  },
  
  // Estado de focus (navegação por teclado)
  '&:focus-visible': {
    outline: `2px solid ${theme.palette.primary.main}`,
    outlineOffset: 2,
    boxShadow: theme.shadows[2],
  },
  
  // Estado active/pressionado
  '&:active': {
    backgroundColor: theme.palette.primary.dark,
    transform: 'scale(0.98)',
  },
  
  // Estado disabled
  '&:disabled': {
    backgroundColor: theme.palette.action.disabledBackground,
    color: theme.palette.action.disabled,
    cursor: 'not-allowed',
    transform: 'none',
  },
}));
```

### Padrão Focus-Visible

Use sempre `:focus-visible` em vez de `:focus` para evitar mostrar anéis de foco em cliques do mouse:

```tsx
// ✅ Bom: Mostra o anel de foco apenas para o teclado
'&:focus-visible': {
  outline: `2px solid ${theme.palette.primary.main}`,
  outlineOffset: 2,
}

// ❌ Ruim: Mostra o anel de foco em cada clique
'&:focus': {
  outline: `2px solid ${theme.palette.primary.main}`,
}
```

***

## Tipografia

Use variantes de tipografia do tema em vez de propriedades de fonte personalizadas.

```tsx
// ✅ Bom: Variantes de tipografia
const Heading = styled('h1')(({ theme }) => ({
  ...theme.typography.h1,
  marginBottom: theme.spacing(2),
}));

const Body = styled('p')(({ theme}) => ({
  ...theme.typography.body1,
  color: theme.palette.text.secondary,
}));

// Com o componente Typography do MUI (preferível)
<Typography variant="h1">Título</Typography>
<Typography variant="body1">Texto do corpo</Typography>

// ❌ Ruim: Tipografia personalizada
const Heading = styled('h1')({
  fontSize: '32px',
  fontWeight: 700,
  lineHeight: 1.2,
});
```

***

## Otimização de Desempenho

### Não crie Styled Components durante o render

```tsx
// ❌ Ruim: Cria um novo componente em cada renderização
function Component({ color }) {
  const Box = styled('div')({
    backgroundColor: color,
  });
  return <Box />;
}

// ✅ Bom: Crie uma vez, passe props
const Box = styled('div')<{ color: string }>(({ color }) => ({
  backgroundColor: color,
}));

function Component({ color }) {
  return <Box color={color} />;
}
```

### Memoize props derivadas

```tsx
// ❌ Ruim: Cria um novo objeto em cada renderização
<StyledComponent style={{ color: isDark ? 'white' : 'black' }} />

// ✅ Bom: Passe props primitivas
const StyledComponent = styled('div')<{ isDark: boolean }>(({ isDark, theme }) => ({
  color: isDark ? theme.palette.common.white : theme.palette.common.black,
}));

<StyledComponent isDark={isDark} />
```

***

## Convenções de Nomenclatura

### Nomes de Componentes

* **PascalCase** para componentes
* Nomes descritivos

```tsx
// ✅ Bons nomes
const UserCard = styled('div')({...});
const PrimaryButton = styled('button')({...});
const NavigationList = styled('ul')({...});

// ❌ Maus nomes
const card = styled('div')({...});
const btn = styled('button')({...});
const list1 = styled('ul')({...});
```

### Estrutura de Arquivos

```
Component.tsx        # Componente principal
Component.styles.ts  # Styled components
Component.test.tsx   # Testes
Component.stories.tsx # Storybook
```

***

## Próximos Passos

* Revisar [Componentes Personalizados](/docs/contributor/contributor-pt/guias-para-colaboradores/padroes-de-web-ui/custom-components/) para diretrizes de criação de componentes
* Veja [Guia de Migração](/docs/contributor/contributor-pt/guias-para-colaboradores/padroes-de-web-ui/migration/) para migrações de UI1 para UI2
* Verifique [Visão geral do processo](/docs/contributor/contributor-pt/guias-para-colaboradores/padroes-de-web-ui/process-overview/) para o fluxo de trabalho completo


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/contributor/contributor-pt/guias-para-colaboradores/padroes-de-web-ui/styling-and-theming.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
