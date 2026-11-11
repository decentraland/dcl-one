# 스타일링 및 테마

이 페이지는 styled-components와 Material UI의 스타일링 솔루션을 사용한 Decentraland 웹 UI를 위한 포괄적인 스타일링 표준을 다룹니다.


> [!NOTE]
> 이 문서의 모든 코드 예시는 설명용입니다. 프로덕션 컴포넌트를 나타내는 것이 아니라 패턴과 표준을 보여줍니다.


## 핵심 원칙

1. **객체 구문만 사용** - 강력한 TypeScript 지원을 위해 객체 표기법을 사용
2. **테마 우선** - 모든 값은 UI2 테마에서 가져옴
3. **인라인 스타일 금지** - 모든 스타일링에는 styled 컴포넌트를 사용
4. **모든 것에 타입 지정** - props와 테마에 TypeScript 활용
5. **기본적으로 반응형** - 테마 브레이크포인트 사용

***

## 객체 구문 표준

UI2 컴포넌트 **반드시** 객체 구문을 사용해야 합니다. 이는 강력한 TypeScript 지원, csstype 검증, 그리고 테마와의 직접 통합을 보장합니다.


> [!WARNING]
> 템플릿 리터럴 구문은 **허용되지 않음**. 항상 객체 표기법을 사용하세요.


### 기본 예제

```tsx
// ✅ 좋음: 객체 구문
const Button = styled('button')({
  color: 'turquoise',
  padding: '8px 16px',
});

// ❌ 나쁨: 템플릿 리터럴 구문
const Button = styled.button`
  color: turquoise;
  padding: 8px 16px;
`;
```

### 테마 및 props 포함

```tsx
interface ButtonProps {
  primary?: boolean;
  size?: 'small' | 'medium' | 'large';
}

// ✅ 좋음: 테마와 props를 사용한 객체 구문
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
> **규칙**: 값은 항상 UI2 테마에서 와야 합니다. 임의의 hex 코드나 픽셀 값은 허용되지 않습니다.


***

## 요소 구문

네이티브 HTML 요소를 스타일링할 때는 항상 함수 호출 형식을 사용하세요 `styled('tag')`.

### 올바른 구문

```tsx
// ✅ 좋음: 함수 호출 형식
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

### 잘못된 구문

```tsx
// ❌ 나쁨: 속성 형식(레거시 구문)
const Container = styled.div`
  display: flex;
  flex-direction: column;
`;

const Action = styled.button`
  color: ${props => props.theme.palette.primary.main};
`;
```

***

## 인라인 스타일 금지

인라인 스타일(`style={...}`) **MUST NOT** UI2 컴포넌트에서 사용되어서는 안 됩니다.

### 왜 인라인 스타일이 아닌가요?

* 테마 타입 지정을 우회함
* 유지보수가 더 어려움
* 재사용을 방해함
* 최적화할 수 없음
* TypeScript 검증 없음

### 올바른 방법

```tsx
// ❌ 나쁨: 인라인 스타일
<Card 
  key={id} 
  style={{ backgroundColor: color } as React.CSSProperties}
>
  <CardHeader>
    <Title style={{ fontSize: 20 }}>{title}</Title>
  </CardHeader>
</Card>

// ✅ 좋음: props를 사용하는 styled 컴포넌트
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

## 브레이크포인트

다음 `theme.breakpoints` 하드코딩된 픽셀 값 대신 헬퍼를 사용하세요.

### 브레이크포인트 헬퍼

| 헬퍼                    | 사용법                                     | 설명           |
| --------------------- | --------------------------------------- | ------------ |
| `up(key)`             | `theme.breakpoints.up('md')`            | 최소 너비 이상     |
| `down(key)`           | `theme.breakpoints.down('md')`          | 최대 너비 이하     |
| `between(start, end)` | `theme.breakpoints.between('sm', 'lg')` | 두 브레이크포인트 사이 |
| `only(key)`           | `theme.breakpoints.only('md')`          | 이 브레이크포인트에서만 |

### 예시

```tsx
// ❌ 나쁨: 하드코딩된 브레이크포인트
const Panel = styled('div')`
  @media (max-width: 768px) {
    width: 100%;
  }
`;

// ✅ 좋음: 테마 브레이크포인트
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

### 다중 브레이크포인트

```tsx
const Layout = styled('div')(({ theme }) => ({
  display: 'grid',
  gridTemplateColumns: '1fr 320px',
  gap: theme.spacing(2),
  
  // 모바일: 단일 열
  [theme.breakpoints.down('md')]: {
    gridTemplateColumns: '1fr',
  },
  
  // 대형 데스크톱: 더 넓은 사이드바
  [theme.breakpoints.up('xl')]: {
    gridTemplateColumns: '1fr 400px',
  },
  
  // 태블릿 범위: 다른 간격
  [theme.breakpoints.between('sm', 'lg')]: {
    gap: theme.spacing(3),
  },
  
  // 태블릿에서만
  [theme.breakpoints.only('md')]: {
    padding: theme.spacing(2),
  },
}));
```

***

## 간격 스케일

다음 `theme.spacing` 마진, 패딩, gap에만 사용합니다.

### 간격 규칙

* `theme.spacing(n)` 여기서 `n` 은 숫자입니다
* 기본 단위는 보통 8px입니다
* `spacing(1)` = 8px, `spacing(2)` = 16px 등.
* 소수점 허용: `spacing(1.5)` = 12px

```tsx
// ✅ 좋음: 테마 간격
const Box = styled('div')(({ theme }) => ({
  padding: theme.spacing(2),           // 16px
  margin: theme.spacing(1, 0),         // 세로 8px, 가로 0
  gap: theme.spacing(1.5),             // 12px
  paddingInline: theme.spacing(3),     // 좌우 24px
}));

// ❌ 나쁨: 원시 픽셀 값
const Box = styled('div')({
  padding: '16px',
  margin: '8px 0',
  gap: '12px',
});
```

### 일반적인 간격 패턴

```tsx
const Card = styled('div')(({ theme }) => ({
  // 모든 방향에 동일한 간격
  padding: theme.spacing(3),
  
  // 세로/가로 간격이 다름
  padding: theme.spacing(2, 3),  // 세로 16px, 가로 24px
  
  // 모든 면이 다름
  padding: theme.spacing(1, 2, 3, 2),  // top, right, bottom, left
  
  // 논리적 속성(RTL 지원에 권장됨)
  paddingBlock: theme.spacing(2),      // 위와 아래
  paddingInline: theme.spacing(3),     // 좌우
  marginBlockStart: theme.spacing(1),  // margin-top
}));
```

***

## z-index와 쌓임

테마의 z-index 스케일을 따르세요. 임의의 z-index 값을 사용하지 마세요.

### 테마 z-index 값

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

### 올바른 사용법

```tsx
// ✅ 좋음: 테마 z-index
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

// ❌ 나쁨: 임의의 z-index
const StickyBar = styled('div')({
  position: 'sticky',
  top: 0,
  zIndex: 999,  // 이렇게 하지 마세요
});
```

### 쌓임 맥락 모범 사례

* 새로운 쌓임 맥락이 생성되는지 인지하세요
* 불필요한 `position: relative` 를 부모에 사용하는 것을 피하세요
* z-index가 필요한 이유를 문서화하세요
* 새 레이어가 필요하면 먼저 테마에 추가하세요

***

## 색상 토큰

항상 다음에서 색상을 가져오세요 `theme.palette` 및 `dclColors`. 임시 hex 값은 사용하지 마세요.

### 팔레트 구조

```tsx
// 텍스트 색상
theme.palette.text.primary
theme.palette.text.secondary
theme.palette.text.disabled

// 배경 색상
theme.palette.background.default
theme.palette.background.paper

// 기본/보조/오류
theme.palette.primary.main
theme.palette.primary.light
theme.palette.primary.dark
theme.palette.primary.contrastText

// 동작 색상
theme.palette.action.active
theme.palette.action.hover
theme.palette.action.selected
theme.palette.action.disabled
theme.palette.action.disabledBackground

// 구분선
theme.palette.divider
```

### Decentraland 색상

```tsx
import { dclColors } from 'decentraland-ui2';

// 희귀도 색상
dclColors.rarity.unique
dclColors.rarity.mythic
dclColors.rarity.legendary
dclColors.rarity.epic
dclColors.rarity.rare
dclColors.rarity.uncommon
dclColors.rarity.common
```

### 예시

```tsx
// ✅ 좋음: 테마 색상
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
  color: '#FFFFFF', // 대비 색상 - 문서화되어 있다면 허용됨
  padding: '4px 8px',
  borderRadius: '4px',
}));

// ❌ 나쁨: 임의의 hex 값
const Chip = styled('span')({
  color: '#666666',
  backgroundColor: '#FFFFFF',
  borderColor: '#E0E0E0',
});
```

***

## 상호작용 상태

모든 상호작용 컨트롤은 hover, focus, active, disabled에 대한 보이는 상태를 반드시 보여야 합니다.

### 완전한 상호작용 컴포넌트

```tsx
const InteractiveButton = styled('button')(({ theme }) => ({
  // 기본 상태
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
  
  // hover 상태(마우스)
  '&:hover': {
    backgroundColor: theme.palette.primary.dark,
  },
  
  // focus 상태(키보드 탐색)
  '&:focus-visible': {
    outline: `2px solid ${theme.palette.primary.main}`,
    outlineOffset: 2,
    boxShadow: theme.shadows[2],
  },
  
  // active/눌림 상태
  '&:active': {
    backgroundColor: theme.palette.primary.dark,
    transform: 'scale(0.98)',
  },
  
  // 비활성 상태
  '&:disabled': {
    backgroundColor: theme.palette.action.disabledBackground,
    color: theme.palette.action.disabled,
    cursor: 'not-allowed',
    transform: 'none',
  },
}));
```

### 포커스 표시 패턴

항상 사용 `:focus-visible` 대신 `:focus` 마우스 클릭 시 포커스 링이 표시되지 않도록:

```tsx
// ✅ 좋음: 키보드에만 포커스 링 표시
'&:focus-visible': {
  outline: `2px solid ${theme.palette.primary.main}`,
  outlineOffset: 2,
}

// ❌ 나쁨: 모든 클릭에 포커스 링 표시
'&:focus': {
  outline: `2px solid ${theme.palette.primary.main}`,
}
```

***

## 타이포그래피

사용자 정의 글꼴 속성 대신 테마 타이포그래피 변형을 사용하세요.

```tsx
// ✅ 좋음: 타이포그래피 변형
const Heading = styled('h1')(({ theme }) => ({
  ...theme.typography.h1,
  marginBottom: theme.spacing(2),
}));

const Body = styled('p')(({ theme}) => ({
  ...theme.typography.body1,
  color: theme.palette.text.secondary,
}));

// MUI Typography 컴포넌트 사용 시(권장)
<Typography variant="h1">제목</Typography>
<Typography variant="body1">본문 텍스트</Typography>

// ❌ 나쁨: 사용자 정의 타이포그래피
const Heading = styled('h1')({
  fontSize: '32px',
  fontWeight: 700,
  lineHeight: 1.2,
});
```

***

## 성능 최적화

### 렌더링 중에 Styled Component를 만들지 마세요

```tsx
// ❌ 나쁨: 렌더링할 때마다 새 컴포넌트 생성
function Component({ color }) {
  const Box = styled('div')({
    backgroundColor: color,
  });
  return <Box />;
}

// ✅ 좋음: 한 번 만들고 props 전달
const Box = styled('div')<{ color: string }>(({ color }) => ({
  backgroundColor: color,
}));

function Component({ color }) {
  return <Box color={color} />;
}
```

### 파생 props 메모이제이션

```tsx
// ❌ 나쁨: 렌더링할 때마다 새 객체 생성
<StyledComponent style={{ color: isDark ? 'white' : 'black' }} />

// ✅ 좋음: 기본형 props 전달
const StyledComponent = styled('div')<{ isDark: boolean }>(({ isDark, theme }) => ({
  color: isDark ? theme.palette.common.white : theme.palette.common.black,
}));

<StyledComponent isDark={isDark} />
```

***

## 명명 규칙

### 컴포넌트 이름

* **파스칼 케이스** 컴포넌트에
* 설명적인 이름

```tsx
// ✅ 좋은 이름
const UserCard = styled('div')({...});
const PrimaryButton = styled('button')({...});
const NavigationList = styled('ul')({...});

// ❌ 나쁜 이름
const card = styled('div')({...});
const btn = styled('button')({...});
const list1 = styled('ul')({...});
```

### 파일 구조

```
Component.tsx        # मुख्य 컴포넌트
Component.styles.ts  # Styled Components
Component.test.tsx   # 테스트
Component.stories.tsx # Storybook
```

***

## 다음 단계

* 검토 [사용자 지정 컴포넌트](/docs/contributor/contributor-ko/contributor-guides/ui-1/custom-components/) 컴포넌트 생성 가이드라인용
* 참고 [마이그레이션 가이드](/docs/contributor/contributor-ko/contributor-guides/ui-1/migration/) UI1에서 UI2로의 마이그레이션용
* 확인 [프로세스 개요](/docs/contributor/contributor-ko/contributor-guides/ui-1/process-overview/) 전체 워크플로용


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/contributor/contributor-ko/contributor-guides/ui-1/styling-and-theming.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
