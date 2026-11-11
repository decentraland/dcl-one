# 사용자 정의 컴포넌트

우리는 서로 다른 프로세스와 기대를 가진 두 가지 유형의 커스텀 컴포넌트를 구분합니다.

## 컴포넌트 유형

### A) 프로젝트별 커스텀 컴포넌트

특정 프로젝트나 화면을 위해 만들어진 컴포넌트로, 다른 프로젝트 전반에서 재사용하도록 의도되지 않습니다.

**예시:**

* 하나의 `박스` 한 프로젝트 내에서 사용되는 특수 레이아웃 포함
* 하나의 `Card` 한 프로젝트의 화면을 위한 맞춤형 레이아웃 변형
* 프로젝트별 데이터 시각화
* 일회성 레이아웃 컴포넌트

**언제 사용하나요:**

* 한 프로젝트에만 고유한 문제를 해결하는 컴포넌트
* 다른 프로젝트에서 필요할 가능성이 낮음
* 일반화하기에는 너무 구체적임

### B) UI2 후보 컴포넌트

여러 프로젝트와 제품 전반에서 재사용하도록 의도된 컴포넌트입니다.

**예시:**

* `Navbar` - 사이트 전체 내비게이션
* `UserMenu` - 사용자 계정 메뉴
* 표준화됨 `Modal` 대화상자
* UI1에서 마이그레이션 중인 컴포넌트

**언제 사용하나요:**

* 컴포넌트가 여러 프로젝트에서 사용될 예정임
* 일반적인 Decentraland 패턴을 나타냄
* UI1 컴포넌트를 대체하거나 확장함

***

## 프로젝트별 컴포넌트

### 요구사항

#### MUI를 기반으로 사용

**반드시** 가능하면 기존 MUI 컴포넌트를 확장하세요:

```tsx
// ✅ 좋음: MUI Card를 확장
import { Card as MuiCard } from '@mui/material';
import { styled } from '@mui/material/styles';

const ProjectCard = styled(MuiCard)(({ theme }) => ({
  padding: theme.spacing(3),
  display: 'flex',
  flexDirection: 'column',
  gap: theme.spacing(2),
}));

// ❌ 나쁨: 처음부터 새로 구현
const ProjectCard = styled('div')(({ theme }) => ({
  padding: theme.spacing(3),
  borderRadius: '4px',
  boxShadow: '0 2px 4px rgba(0,0,0,0.1)',
  // Card 기능을 중복 구현
}));
```

**MUI가 이미 처리하는 패턴을 포크하거나 중복하지 마세요:**

* 다음 `Card` 그림자 효과가 있는 커스텀 박스를 만드는 대신
* 다음 `Button` 스타일링된 앵커를 만드는 대신
* 다음 `TextField` 커스텀 입력을 만드는 대신
* 확장 `Dialog` 커스텀 모달을 만드는 대신

#### 테마 값만 사용

**반드시** UI2 테마의 값만 사용하세요:

```tsx
// ✅ 좋음: 모든 값이 테마에서 옴
const StyledBox = styled('div')(({ theme }) => ({
  color: theme.palette.text.primary,
  backgroundColor: theme.palette.background.paper,
  padding: theme.spacing(2),
  borderRadius: theme.shape.borderRadius,
  border: `1px solid ${theme.palette.divider}`,
}));

// ❌ 나쁨: 임의의 값
const StyledBox = styled('div')({
  color: '#333333',
  backgroundColor: '#FFFFFF',
  padding: '16px',
  borderRadius: '8px',
  border: '1px solid #E0E0E0',
});
```

**임의의 값은 허용되지 않습니다:**

* 색상: 사용 `theme.palette` 또는 `dclColors`
* 간격: 사용 `theme.spacing(n)`
* 테두리 반경: 사용 `theme.shape.borderRadius`
* 타이포그래피: 사용 `theme.typography` 변형
* 브레이크포인트: 사용 `theme.breakpoints` 도우미

#### 상태 및 접근성

**반드시** 모든 상호작용 상태를 정의하고 구현하세요:

```tsx
const ActionButton = styled('button')(({ theme }) => ({
  // 기본/비활성 상태
  padding: theme.spacing(1, 2),
  backgroundColor: theme.palette.primary.main,
  color: theme.palette.primary.contrastText,
  border: 'none',
  borderRadius: theme.shape.borderRadius,
  cursor: 'pointer',
  transition: theme.transitions.create(['background-color', 'transform']),
  
  // 호버 상태
  '&:hover': {
    backgroundColor: theme.palette.primary.dark,
  },
  
  // focus 상태(키보드 탐색)
  '&:focus-visible': {
    outline: `2px solid ${theme.palette.primary.main}`,
    outlineOffset: 2,
  },
  
  // 활성/눌림 상태
  '&:active': {
    transform: 'scale(0.98)',
  },
  
  // 비활성 상태
  '&:disabled': {
    backgroundColor: theme.palette.action.disabledBackground,
    color: theme.palette.action.disabled,
    cursor: 'not-allowed',
  },
}));
```

**반드시** 기본 접근성을 구현하세요:\*\*

* **키보드 탐색** - 키보드로 포커스 가능하고 조작 가능
* **포커스 표시** - 보이는 포커스 상태
* **ARIA 레이블** - 텍스트가 보이지 않는 경우
* **시맨틱 HTML** - 적절한 요소 사용
* **색 대비** - WCAG AA 기준 충족

### 예시: 프로젝트별 컴포넌트

```tsx
// src/components/LandCard/LandCard.tsx
import { Card, CardContent, CardActions, Typography, Button } from '@mui/material';
import { styled } from '@mui/material/styles';
import type { Parcel } from '@/types';

interface LandCardProps {
  parcel: Parcel;
  onTransfer: (id: string) => void;
  onView: (id: string) => void;
}

const StyledCard = styled(Card)(({ theme }) => ({
  display: 'flex',
  flexDirection: 'column',
  height: '100%',
  transition: theme.transitions.create('transform'),
  
  '&:hover': {
    transform: 'translateY(-4px)',
  },
}));

const CoordinatesText = styled(Typography)(({ theme }) => ({
  color: theme.palette.text.secondary,
  fontFamily: theme.typography.fontFamilyMono,
}));

export function LandCard({ parcel, onTransfer, onView }: LandCardProps) {
  return (
    <StyledCard>
      <CardContent>
        <Typography variant="h6" gutterBottom>
          {parcel.name || `Parcel ${parcel.x},${parcel.y}`}
        </Typography>
        <CoordinatesText variant="body2">
          ({parcel.x}, {parcel.y})
        </CoordinatesText>
        <Typography variant="body2" color="text.secondary">
          소유자: {parcel.owner}
        </Typography>
      </CardContent>
      <CardActions>
        <Button size="small" onClick={() => onView(parcel.id)}>
          보기
        </Button>
        <Button size="small" onClick={() => onTransfer(parcel.id)}>
          이전
        </Button>
      </CardActions>
    </StyledCard>
  );
}
```

***

## UI2 후보 컴포넌트

프로젝트 전반에서 공유될 컴포넌트는 더 높은 기준과 더 철저한 문서화가 필요합니다.

### 요구사항

#### 테마 정렬

**반드시** UI2 테마 값만 전적으로 사용하세요:

```tsx
// ✅ 좋음: 완전한 테마 통합
const NavbarContainer = styled('nav')(({ theme }) => ({
  backgroundColor: theme.palette.background.paper,
  borderBottom: `1px solid ${theme.palette.divider}`,
  padding: theme.spacing(0, 2),
  height: 64,
  display: 'flex',
  alignItems: 'center',
  gap: theme.spacing(2),
  
  [theme.breakpoints.down('md')]: {
    padding: theme.spacing(0, 1),
  },
}));
```

#### Storybook 커버리지

**반드시** 포괄적인 Storybook 스토리를 추가하세요:

**필수 커버리지:**

1. **모든 prop과 변형**
   * 모든 prop 조합
   * 모든 크기 변형
   * 모든 색상 변형
2. **모든 상태**
   * 비활성/기본
   * 로딩
   * 오류
   * 비활성화됨
   * 호버(의사 상태 애드온 사용)
   * 포커스(의사 상태 애드온 사용)
3. **상호작용**
   * 클릭 핸들러
   * 폼 제출
   * 키보드 탐색
4. **색 구성표**
   * 라이트 모드
   * 다크 모드
5. **반응형 동작**
   * 핵심 브레이크포인트(xs, md, lg)
   * 각 브레이크포인트에서의 동작을 문서화

**예시 Storybook 파일:**

```tsx
// Navbar.stories.tsx
import type { Meta, StoryObj } from '@storybook/react';
import { Navbar } from './Navbar';

const meta: Meta<typeof Navbar> = {
  title: 'Components/Navbar',
  component: Navbar,
  parameters: {
    layout: 'fullscreen',
  },
  argTypes: {
    variant: {
      control: 'select',
      options: ['default', 'compact'],
    },
    showUserMenu: {
      control: 'boolean',
    },
  },
};

export default meta;
type Story = StoryObj<typeof Navbar>;

export const Default: Story = {
  args: {
    variant: 'default',
    showUserMenu: true,
  },
};

export const Compact: Story = {
  args: {
    variant: 'compact',
    showUserMenu: true,
  },
};

export const WithoutUserMenu: Story = {
  args: {
    variant: 'default',
    showUserMenu: false,
  },
};

export const Loading: Story = {
  args: {
    variant: 'default',
    showUserMenu: true,
    isLoading: true,
  },
};

// 다양한 뷰포트 테스트
export const Mobile: Story = {
  args: {
    variant: 'compact',
    showUserMenu: true,
  },
  parameters: {
    viewport: {
      defaultViewport: 'mobile1',
    },
  },
};

export const Tablet: Story = {
  args: {
    variant: 'default',
    showUserMenu: true,
  },
  parameters: {
    viewport: {
      defaultViewport: 'tablet',
    },
  },
};

// 색 구성표 테스트
export const DarkMode: Story = {
  args: {
    variant: 'default',
    showUserMenu: true,
  },
  parameters: {
    backgrounds: {
      default: 'dark',
    },
  },
};
```

### 컴포넌트 구조

UI2 후보 컴포넌트는 다음 구조를 따라야 합니다:

```
src/components/Navbar/
├── Navbar.tsx           # 메인 컴포넌트
├── Navbar.styles.ts     # 스타일드 컴포넌트
├── Navbar.stories.tsx   # Storybook 스토리
├── Navbar.test.tsx      # 단위 테스트
├── types.ts             # TypeScript 타입
├── index.ts             # 공개 내보내기
└── README.md            # 컴포넌트 문서
```

### 문서화 요구사항

**반드시** 컴포넌트 README에 포함할 내용:

1. **목적** - 이게 어떤 문제를 해결하나요?
2. **사용법** - 컴포넌트 사용 방법
3. **Props** - 타입과 설명이 포함된 모든 prop
4. **예시** - 일반적인 사용 사례
5. **접근성** - 키보드 지원, ARIA 레이블
6. **테마 적용** - 사용하는 테마 값
7. **마이그레이션 노트** - UI1 컴포넌트를 대체하는 경우

**예시 README:**

````markdown
# Navbar

사용자 메뉴와 반응형 동작을 갖춘 사이트 전체 내비게이션 컴포넌트입니다.

## 사용법

\```tsx
import { Navbar } from 'decentraland-ui2';

function App() {
  return (
    <Navbar
      variant="default"
      showUserMenu={true}
      onLogoClick={() => navigate('/')}
      onLoginClick={handleLogin}
    />
  );
}
\```

## Props

| Prop         | Type                   | Default   | Description                   |
| ------------ | ---------------------- | --------- | ----------------------------- |
| variant      | 'default' \| 'compact' | 'default' | 내비게이션 변형            |
| showUserMenu | boolean                | true      | 로그인했을 때 사용자 메뉴 표시 |
| onLogoClick  | () => void             | -         | 로고 클릭 핸들러            |
| onLoginClick | () => void             | -         | 로그인 버튼 클릭 핸들러    |

## 접근성

* 키보드 탐색: 메뉴 항목을 Tab으로 이동
* ARIA: 올바른 landmark와 레이블
* 스크린 리더: 메뉴 상태를 알림

## 테마 적용

다음 테마 값을 사용합니다:

* `theme.palette.background.paper`
* `theme.palette.divider`
* `theme.spacing`
* `theme.breakpoints`

````

### 테스트 요구사항

**반드시** 다음에 대한 테스트 포함:

* prop 렌더링
* 사용자 상호작용
* 접근성 기능
* 반응형 동작
* 오류 상태

```tsx
// Navbar.test.tsx
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { Navbar } from './Navbar';

describe('Navbar', () => {
  it('should render logo', () => {
    render(<Navbar />);
    expect(screen.getByRole('banner')).toBeInTheDocument();
  });

  it('should call onLogoClick when logo is clicked', async () => {
    const onLogoClick = jest.fn();
    render(<Navbar onLogoClick={onLogoClick} />);
    
    await userEvent.click(screen.getByRole('link', { name: /decentraland/i }));
    expect(onLogoClick).toHaveBeenCalled();
  });

  it('should be keyboard navigable', async () => {
    render(<Navbar />);
    const firstLink = screen.getAllByRole('link')[0];
    
    firstLink.focus();
    expect(firstLink).toHaveFocus();
  });
});
```

***

## 의사결정 매트릭스

어떤 유형의 컴포넌트를 만들지 결정하는 데 사용하세요:

| 질문                   | 프로젝트별   | UI2 후보     |
| -------------------- | ------- | ---------- |
| 다른 프로젝트에서 이것을 사용할까요? | 아니요     | 예          |
| UI1에 대응되는 것이 있나요?    | 해당 없음   | 아마도        |
| Storybook 문서가 필요한가요? | 아니요     | **예**      |
| 포괄적인 테스트가 필요한가요?     | 기본      | **광범위**    |
| 디자인 검토가 필요한가요?       | 프로젝트 수준 | **UI2 수준** |
| 프로젝트별 패턴을 사용할 수 있나요? | 예       | **아니요**    |
| 모든 테마에서 동작해야 하나요?    | 아니요     | **예**      |

***

## 승인 절차

### 프로젝트별 컴포넌트

1. 프로젝트 유지관리자의 코드 리뷰
2. 테마 준수 여부 확인
3. 프로젝트 컨텍스트에서 테스트
4. 승인되면 병합

### UI2 후보 컴포넌트

1. 디자인 검토 및 승인
2. 기술 설계 검토
3. 구현
4. Storybook 스토리
5. 포괄적인 테스트
6. 접근성 검토
7. 코드 리뷰
8. UI2 저장소로 PR
9. 버전 지정 및 배포
10. 종속 프로젝트 업데이트

***

## 권장 사항

### 커스터마이징보다 조합 우선

```tsx
// ✅ 좋음: MUI 컴포넌트 조합
function FeatureCard({ title, children }) {
  return (
    <Card>
      <CardContent>
        <Typography variant="h6">{title}</Typography>
        {children}
      </CardContent>
    </Card>
  );
}

// ❌ 나쁨: Card 기능을 다시 구현
function FeatureCard({ title, children }) {
  return (
    <div className="custom-card">
      <div className="custom-card-content">
        <h3>{title}</h3>
        {children}
      </div>
    </div>
  );
}
```

### 점진적 향상

간단하게 시작하고 필요에 따라 기능을 추가하세요:

1. 핵심 기능이 포함된 기본 버전
2. 반응형 동작 추가
3. 접근성 기능 추가
4. 고급 상호작용 추가
5. 성능 최적화

### 문서화를 먼저

코드를 작성하기 전에:

1. 컴포넌트 README 작성
2. props 인터페이스 정의
3. 필요한 상태 나열
4. Storybook 스토리 계획
5. 그런 다음 구현

***

## 다음 단계

* 검토 [스타일링 및 테마](/docs/contributor/contributor-ko/contributor-guides/ui-1/styling-and-theming/) 구현 세부 사항을 위해
* 참고 [마이그레이션 가이드](/docs/contributor/contributor-ko/contributor-guides/ui-1/migration/) UI1에서 UI2로의 마이그레이션용
* 확인 [프로세스 개요](/docs/contributor/contributor-ko/contributor-guides/ui-1/process-overview/) 전체 워크플로를 위해


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/contributor/contributor-ko/contributor-guides/ui-1/custom-components.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
