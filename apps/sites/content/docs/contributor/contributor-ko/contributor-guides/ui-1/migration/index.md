# 마이그레이션 가이드

이 가이드는 UI1 (`decentraland-ui`)에서 UI2 (`decentraland-ui2`).

## 마이그레이션 시기

마이그레이션은 다음 세 가지 유효한 이유로 시작할 수 있습니다:

### 1. 기술적 개선

다음과 같은 이유로 컴포넌트를 마이그레이션하려는 경우:

* 더 나은 테마 지원
* 개선된 TypeScript 타입
* 다른 UI2 컴포넌트와의 일관성
* 성능 최적화
* 접근성 개선

### 2. 컴포넌트 업데이트

* UI1 컴포넌트를 업데이트해야 함
* 아직 UI2에 대응되는 것이 없음
* **요구사항**: 먼저 UI2에 만들고, 그다음 사용하세요

### 3. 프로젝트 요구사항

* 이 컴포넌트는 새 프로젝트 또는 기존 프로젝트에서 사용될 예정임
* 제대로 마이그레이션할 수 있는 시간이 있음
* 프로젝트 자원상 철저한 마이그레이션이 가능함


> [!WARNING]
> **하지 마세요** 컴포넌트를 "그냥" 마이그레이션하지 마세요. 각 마이그레이션에는 명확한 비즈니스 또는 기술적 근거가 있어야 합니다.


***

## 마이그레이션 과정

### 1단계: 계획

마이그레이션을 시작하기 전에:

1. **의존성 파악**
   * 어떤 다른 컴포넌트를 사용하나요?
   * 현재 어떤 프로젝트에서 사용하나요?
   * 예정된 파괴적 변경이 있나요?
2. **현재 사용 현황 검토**
   * 몇 개의 프로젝트에서 이 컴포넌트를 사용하나요?
   * 가장 자주 사용되는 props는 무엇인가요?
   * 알려진 문제가 있나요?
3. **범위 정의**
   * 이것이 1:1 마이그레이션인가요?
   * 예정된 개선 사항이 있나요?
   * 일정은 어떻게 되나요?

### 2단계: UI2 컴포넌트 만들기

다음을 따르세요 [사용자 지정 컴포넌트](/docs/contributor/contributor-ko/contributor-guides/ui-1/custom-components/) UI2 후보 컴포넌트 가이드.

**요구사항:**

* styled-components에 객체 문법 사용
* 테마 값만 사용하기(임의의 값 사용 금지)
* 포괄적인 Storybook 스토리 추가
* 완전한 테스트 작성
* 모든 props와 동작 문서화

**예시 구조:**

```
ui2/src/components/Button/
├── Button.tsx
├── Button.styles.ts
├── Button.stories.tsx
├── Button.test.tsx
├── types.ts
├── index.ts
└── README.md
```

### 3단계: 호환성 유지

UI2 컴포넌트는 **반드시** UI1 버전과 동일한 props와 동작을 제공해야 합니다.

#### 동일한 Props

```tsx
// UI1 Button
interface ButtonProps {
  primary?: boolean;
  size?: 'small' | 'medium' | 'large';
  onClick?: () => void;
  disabled?: boolean;
  children: React.ReactNode;
}

// UI2 Button - 동일한 props를 반드시 지원해야 함
interface ButtonProps {
  primary?: boolean;
  size?: 'small' | 'medium' | 'large';
  onClick?: () => void;
  disabled?: boolean;
  children: React.ReactNode;
  // 새 선택적 props를 추가할 수 있음
  variant?: 'text' | 'outlined' | 'contained';
}
```

#### 하위 호환 변경

props를 변경하거나 추가해야 한다면:

1. **먼저**, UI1에 새 props를 선택적으로 추가하세요
2. **그다음**, UI1 사용자를 새 props를 사용하도록 마이그레이션하세요
3. **마지막으로**, 새 API로 UI2 컴포넌트를 만드세요

```tsx
// 1단계: UI1에 선택적 prop 추가
interface ButtonProps {
  primary?: boolean;
  // 새 선택적 prop
  variant?: 'primary' | 'secondary';
}

// 2단계: UI1 구현 업데이트
export function Button({ primary, variant = primary ? 'primary' : 'secondary' }: ButtonProps) {
  // 내부에서는 primary 대신 variant 사용
}

// 3단계: 새 API로 UI2 생성
interface ButtonProps {
  // 이제 variant가 기본 prop입니다
  variant?: 'primary' | 'secondary';
  // 호환성을 위해 primary 유지, deprecated로 표시
  /** @deprecated 대신 variant를 사용하세요 */
  primary?: boolean;
}
```

### 4단계: UI1 컴포넌트 폐기

UI1 컴포넌트에 폐기 안내를 추가하세요:

````tsx
/**
 * @deprecated 이 컴포넌트는 UI2로 마이그레이션되었습니다.
 * 대신 'decentraland-ui2'에서 가져오세요:
 * 
 * ```tsx
 * import { Button } from 'decentraland-ui2';
 * ```
 * 
 * 마이그레이션 가이드 보기: https://docs.decentraland.org/contributor-guides/web-ui-standards/migration
 */
export function Button(props: ButtonProps) {
  // ... 기존 구현
}
````

### 5단계: 점진적 도입

즉시 마이그레이션을 강제하지 마세요. 점진적 도입을 허용하세요:

1. **배포** UI2 컴포넌트
2. **문서화** 마이그레이션 경로
3. **업데이트** 새 프로젝트가 UI2를 사용하도록
4. **마이그레이션** 기존 프로젝트를 기회가 있을 때마다
5. **계획** 최종 UI1 제거(사전 안내 포함)

***

## 마이그레이션 예시

### 예시 1: 간단한 컴포넌트

기본적인 `Card` 컴포넌트 마이그레이션:

#### UI1 버전

```tsx
// decentraland-ui/src/components/Card/Card.tsx
import React from 'react';
import './Card.css';

export interface CardProps {
  className?: string;
  children: React.ReactNode;
}

export function Card({ className, children }: CardProps) {
  return (
    <div className={`dcl-card ${className || ''}`}>
      {children}
    </div>
  );
}
```

#### UI2 버전

```tsx
// decentraland-ui2/src/components/Card/Card.tsx
import { styled } from '@mui/material/styles';

export interface CardProps {
  className?: string;
  children: React.ReactNode;
}

const StyledCard = styled('div')(({ theme }) => ({
  backgroundColor: theme.palette.background.paper,
  borderRadius: theme.shape.borderRadius,
  padding: theme.spacing(2),
  boxShadow: theme.shadows[1],
  
  [theme.breakpoints.down('sm')]: {
    padding: theme.spacing(1),
  },
}));

export function Card({ className, children }: CardProps) {
  return (
    <StyledCard className={className}>
      {children}
    </StyledCard>
  );
}
```

### 예시 2: 변형이 있는 컴포넌트

다음과 같은 `Button` 을 변형과 함께 마이그레이션:

#### UI1 버전

```tsx
// UI1
import './Button.css';

interface ButtonProps {
  primary?: boolean;
  secondary?: boolean;
  size?: 'small' | 'medium' | 'large';
}

export function Button({ primary, secondary, size = 'medium', ...props }: ButtonProps) {
  const classes = [
    'dcl-button',
    primary && 'primary',
    secondary && 'secondary',
    `size-${size}`,
  ] .filter(Boolean).join(' ');
  
  return <button className={classes} {...props} />;
}
```

#### UI2 버전

```tsx
// UI2
import { styled } from '@mui/material/styles';

interface ButtonProps {
  /** @deprecated 대신 variant="contained"를 사용하세요 */
  primary?: boolean;
  /** @deprecated 대신 variant="outlined"를 사용하세요 */
  secondary?: boolean;
  variant?: 'text' | 'outlined' | 'contained';
  size?: 'small' | 'medium' | 'large';
}

const StyledButton = styled('button')<ButtonProps>(({ theme, variant = 'contained', size = 'medium' }) => {
  const sizes = {
    small: theme.spacing(0.5, 1),
    medium: theme.spacing(1, 2),
    large: theme.spacing(1.5, 3),
  };
  
  const variants = {
    text: {
      backgroundColor: 'transparent',
      color: theme.palette.primary.main,
    },
    outlined: {
      backgroundColor: 'transparent',
      color: theme.palette.primary.main,
      border: `1px solid ${theme.palette.primary.main}`,
    },
    contained: {
      backgroundColor: theme.palette.primary.main,
      color: theme.palette.primary.contrastText,
    },
  };
  
  return {
    padding: sizes[size],
    borderRadius: theme.shape.borderRadius,
    border: 'none',
    cursor: 'pointer',
    ...variants[variant],
    
    '&:hover': {
      opacity: 0.9,
    },
    
    '&:disabled': {
      opacity: 0.5,
      cursor: 'not-allowed',
    },
  };
});

export function Button({ 
  primary, 
  secondary, 
  variant, 
  ...props 
}: ButtonProps) {
  // deprecated props 처리
  const actualVariant = variant || 
    (primary ? 'contained' : secondary ? 'outlined' : 'text');
  
  return <StyledButton variant={actualVariant} {...props} />;
}
```

***

## 마이그레이션 체크리스트

각 컴포넌트 마이그레이션에 이 체크리스트를 사용하세요:

### 계획 단계

* [ ] 컴포넌트를 사용하는 모든 프로젝트 식별
* [ ] 현재 props와 동작 문서화
* [ ] 마이그레이션 범위와 일정 정의
* [ ] 이해관계자 승인 받기

### 구현 단계

* [ ] 표준에 따라 UI2 컴포넌트 생성
* [ ] props 호환성 유지
* [ ] 스타일링에 객체 문법 사용
* [ ] 테마 값만 사용
* [ ] 모든 상태(비활성, 호버, 포커스, 비활성화, 오류) 구현
* [ ] 포괄적인 Storybook 스토리 추가
* [ ] 단위 테스트 작성
* [ ] 접근성 기능 문서화

### 폐기 단계

* [ ] UI1 컴포넌트에 폐기 안내 추가
* [ ] UI1 문서 업데이트
* [ ] 사용자를 위한 마이그레이션 가이드 생성
* [ ] UI2 컴포넌트 배포

### 도입 단계

* [ ] 새 프로젝트가 UI2를 사용하도록 업데이트
* [ ] 기존 프로젝트용 마이그레이션 PR 생성
* [ ] 문제 모니터링
* [ ] 피드백 수집
* [ ] UI1 제거 일정 계획

***

## 일반적인 마이그레이션 패턴

### CSS에서 Styled Components로

```tsx
// UI1: CSS 파일
.dcl-card {
  background: #fff;
  padding: 16px;
  border-radius: 8px;
}

// UI2: 스타일드 컴포넌트
const Card = styled('div')(({ theme }) => ({
  backgroundColor: theme.palette.background.paper,
  padding: theme.spacing(2),
  borderRadius: theme.shape.borderRadius,
}));
```

### 클래스 이름에서 props로

```tsx
// UI1: 클래스 기반 변형
<Button className={primary ? 'primary' : 'secondary'} />

// UI2: prop 기반 변형
<Button variant={primary ? 'contained' : 'outlined'} />
```

### 고정값에서 테마로

```tsx
// UI1: 고정값
const styles = {
  color: '#333',
  fontSize: '14px',
  padding: '8px 16px',
};

// UI2: 테마 값
const Component = styled('div')(({ theme }) => ({
  color: theme.palette.text.primary,
  fontSize: theme.typography.body2.fontSize,
  padding: theme.spacing(1, 2),
}));
```

***

## 파괴적 변경

때로는 파괴적 변경이 필요합니다. 신중하게 처리하세요:

### 파괴적 변경이 허용되는 경우

* 보안 수정
* 치명적 버그
* 대규모 버전 업데이트
* 폐기된 기능 제거(사전 안내 포함)

### 파괴적 변경을 처리하는 방법

1. **미리 공지하기** - 변경 사항을 충분히 미리 전달하세요
2. **마이그레이션 경로 제공** - 업데이트 방법을 문서화하세요
3. **버전 올리기** - 시맨틱 버저닝을 따르세요
4. **폐기 기간** - 마이그레이션할 시간을 주세요
5. **Codemod** - 가능하다면 자동화된 마이그레이션 도구를 제공하세요

### 예시: 폐기된 props 제거

```tsx
// 버전 1.0: 새 API 도입, 기존 API는 폐기
interface ButtonProps {
  /** @deprecated 대신 variant="contained"를 사용하세요 */
  primary?: boolean;
  variant?: 'text' | 'outlined' | 'contained';
}

// 버전 1.5: 제거 예정 경고
interface ButtonProps {
  /** @deprecated 2.0에서 제거됩니다. 대신 variant를 사용하세요 */
  primary?: boolean;
  variant?: 'text' | 'outlined' | 'contained';
}

// 버전 2.0: 폐기된 prop 제거
interface ButtonProps {
  variant?: 'text' | 'outlined' | 'contained';
}
```

***

## 마이그레이션 테스트

마이그레이션된 컴포넌트가 올바르게 동작하는지 확인하세요:

### 시각적 회귀 테스트

UI1과 UI2 컴포넌트를 시각적으로 비교하세요:

```tsx
// 비교를 위한 Storybook 스토리
export const ComparisonStory: Story = {
  render: () => (
    <div style={{ display: 'flex', gap: '2rem' }}>
      <div>
        <h3>UI1</h3>
        <UI1Button primary>클릭하세요</UI1Button>
      </div>
      <div>
        <h3>UI2</h3>
        <UI2Button variant="contained">클릭하세요</UI2Button>
      </div>
    </div>
  ),
};
```

### 동작 테스트

props가 동일하게 동작하는지 확인하세요:

```tsx
describe('Button migration', () => {
  it('primary prop(폐기됨)이 variant="contained"와 동일하게 처리되어야 합니다', () => {
    const { container: ui1 } = render(<UI1Button primary>Test</UI1Button>);
    const { container: ui2 } = render(<UI2Button primary>Test</UI2Button>);
    
    // 렌더링 결과 비교
    expect(ui1.textContent).toBe(ui2.textContent);
  });
});
```

***

## 문서 업데이트

마이그레이션 후 문서를 업데이트하세요:

### UI1 컴포넌트 문서 업데이트

```markdown
# Button (UI1 - Deprecated)

> ⚠️ **이 컴포넌트는 UI2로 마이그레이션되었습니다.**
> 새 버전은 [UI2 Button 문서](../ui2/button)를 참조하세요.

이 컴포넌트는 deprecated이며 향후 버전에서 제거될 예정입니다.
UI2로 마이그레이션하세요.

## 마이그레이션 가이드

자세한 내용은 [마이그레이션 가이드](./migration)를 참조하세요.
```

### UI2 컴포넌트 문서 생성

```markdown
# Button (UI2)

완전한 테마 지원을 갖춘 현대적인 버튼 컴포넌트입니다.

## UI1에서 마이그레이션

UI1에서 마이그레이션하는 경우:

- `primary` prop → `variant="contained"`
- `secondary` prop → `variant="outlined"`
- CSS 클래스 → styled-components

자세한 내용은 전체 [마이그레이션 가이드](./migration)를 참조하세요.
```

***

## 다음 단계

* 검토 [사용자 지정 컴포넌트](/docs/contributor/contributor-ko/contributor-guides/ui-1/custom-components/) UI2 컴포넌트 생성용
* 참고 [스타일링 및 테마](/docs/contributor/contributor-ko/contributor-guides/ui-1/styling-and-theming/) 스타일링 표준용
* 확인 [프로세스 개요](/docs/contributor/contributor-ko/contributor-guides/ui-1/process-overview/) 전체 워크플로용


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/contributor/contributor-ko/contributor-guides/ui-1/migration.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
