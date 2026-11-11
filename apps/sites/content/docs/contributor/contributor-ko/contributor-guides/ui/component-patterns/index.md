# 컴포넌트 패턴

이 페이지에서는 훅, 최적화 패턴, 모범 사례를 포함하여 React 컴포넌트에서 Redux와 RTK Query를 효과적으로 사용하는 방법을 다룹니다.

## 기본 쿼리 사용

RTK Query 엔드포인트에서 생성된 훅을 사용하세요:

```tsx
import { useGetParcelByCoordsQuery } from '@/features/land/land.client';

function ParcelCard({ x, y }: { x: number; y: number }) {
  const { data, isLoading, isError, error } = useGetParcelByCoordsQuery({ x, y });

  if (isLoading) return <div>로딩 중...</div>;
  if (isError) return <div>오류: {error.toString()}</div>;
  if (!data) return null;

  return (
    <div>
      <h2>{data.name || `필지 ${x},${y}`}</h2>
      <p>소유자: {data.owner}</p>
    </div>
  );
}
```

## 리렌더링 최적화하기 `selectFromResult`

결과를 필요한 필드만 남기도록 좁히세요:

```tsx
function ParcelOwner({ x, y }: { x: number; y: number }) {
  // ✅ 좋음: owner 필드 변경 사항에만 구독
  const { owner, isFetching } = useGetParcelByCoordsQuery(
    { x, y },
    {
      selectFromResult: ({ data, isFetching }) => ({
        owner: data?.owner,
        isFetching,
      }),
    }
  );

  if (isFetching) return <span>로딩 중...</span>;
  return <span>소유자: {owner}</span>;
}

// ❌ 나쁨: 데이터가 변경될 때마다 리렌더링됨
function ParcelOwnerBad({ x, y }: { x: number; y: number }) {
  const { data } = useGetParcelByCoordsQuery({ x, y });
  return <span>소유자: {data?.owner}</span>;
}
```

## 조건부 쿼리와 `건너뛰기`

인수가 준비되지 않았을 때는 쿼리를 건너뛰세요:

```tsx
function UserProfile() {
  const { data: session } = useGetSessionQuery();
  const userId = session?.userId;

  // userId가 생길 때까지 프로필 쿼리를 건너뜀
  const { data: profile } = useGetProfileQuery(
    { id: userId! },
    {
      skip: !userId, // userId가 정의되지 않았으면 가져오지 않음
    }
  );

  if (!userId) return <div>로그인해 주세요</div>;
  if (!profile) return <div>프로필을 불러오는 중...</div>;

  return <div>{profile.name}</div>;
}
```

## 실시간 업데이트를 위한 폴링

```tsx
function LiveBalance({ address }: { address: string }) {
  const { data } = useGetBalanceQuery(
    { address },
    {
      pollingInterval: 10000, // 10초마다 폴링
      skipPollingIfUnfocused: true, // 탭이 포커스되지 않으면 일시 중지
    }
  );

  return <div>잔액: {data?.amount ?? 0}</div>;
}
```

## 지연 쿼리

컴포넌트 마운트 시가 아니라 수동으로 쿼리를 트리거하세요:

```tsx
function SearchParcels() {
  const [trigger, result] = useLazyGetParcelsByOwnerQuery();
  const [owner, setOwner] = useState('');

  const handleSearch = () => {
    if (owner) {
      trigger({ owner });
    }
  };

  return (
    <div>
      <input
        value={owner}
        onChange={(e) => setOwner(e.target.value)}
        placeholder="소유자 주소 입력"
      />
      <button onClick={handleSearch}>검색</button>
      
      {result.isLoading && <div>검색 중...</div>}
      {result.data && (
        <ul>
          {result.data.map((parcel) => (
            <li key={parcel.id}>{parcel.name}</li>
          ))}
        </ul>
      )}
    </div>
  );
}
```

## 뮤테이션 사용

### 기본 뮤테이션

```tsx
function GrantCreditsButton({ address }: { address: string }) {
  const [grantCredits, { isLoading, isError, error }] = useGrantCreditsMutation();

  const handleGrant = async () => {
    try {
      await grantCredits({ address, amount: 100 }).unwrap();
      toast.success('크레딧이 성공적으로 부여되었습니다!');
    } catch (err) {
      toast.error('크레딧 부여에 실패했습니다');
      console.error('부여 실패:', err);
    }
  };

  return (
    <button onClick={handleGrant} disabled={isLoading}>
      {isLoading ? '부여 중...' : '100 크레딧 부여'}
    </button>
  );
}
```

### 피드백이 있는 뮤테이션

```tsx
import { isFetchBaseQueryError } from '@/services/client';

function UpdateParcelName({ parcelId }: { parcelId: string }) {
  const [name, setName] = useState('');
  const [updateName, { isLoading, isSuccess, isError, error }] =
    useUpdateParcelNameMutation();

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    
    try {
      const result = await updateName({ id: parcelId, name }).unwrap();
      toast.success(`필지 이름이 "${result.name}"으로 변경되었습니다`);
      setName(''); // 폼 지우기
    } catch (err) {
      if (isFetchBaseQueryError(err)) {
        toast.error(`오류: ${JSON.stringify(err.data)}`);
      } else {
        toast.error('예상치 못한 오류가 발생했습니다');
      }
    }
  };

  return (
    <form onSubmit={handleSubmit}>
      <input
        type="text"
        value={name}
        onChange={(e) => setName(e.target.value)}
        placeholder="새 이름"
        disabled={isLoading}
      />
      <button type="submit" disabled={isLoading || !name}>
        {isLoading ? '업데이트 중...' : '이름 업데이트'}
      </button>
      
      {isSuccess && <p className="success">이름이 업데이트되었습니다!</p>}
      {isError && <p className="error">업데이트에 실패했습니다</p>}
    </form>
  );
}
```

## 슬라이스 상태 사용

다음을 사용하여 슬라이스 상태에 접근 `useAppSelector`:

```tsx
import { useAppSelector, useAppDispatch } from '@/app/hooks';
import { selectViewMode, viewModeChanged } from '@/features/ui/ui.slice';

function ViewModeToggle() {
  const dispatch = useAppDispatch();
  const viewMode = useAppSelector(selectViewMode);

  const toggleMode = () => {
    dispatch(viewModeChanged(viewMode === 'grid' ? 'list' : 'grid'));
  };

  return (
    <button onClick={toggleMode}>
      보기: {viewMode === 'grid' ? '⊞ 그리드' : '☰ 목록'}
    </button>
  );
}
```

## 여러 쿼리 결합

```tsx
function ParcelDetails({ id }: { id: string }) {
  const { data: parcel, isLoading: parcelLoading } = useGetParcelQuery({ id });
  const { data: owner, isLoading: ownerLoading } = useGetProfileQuery(
    { id: parcel?.owner! },
    { skip: !parcel?.owner }
  );

  const isLoading = parcelLoading || ownerLoading;

  if (isLoading) return <div>로딩 중...</div>;
  if (!parcel) return <div>필지를 찾을 수 없습니다</div>;

  return (
    <div>
      <h1>{parcel.name}</h1>
      <p>좌표: {parcel.x}, {parcel.y}</p>
      {owner && <p>소유자: {owner.name}</p>}
    </div>
  );
}
```

## 셀렉터를 사용한 파생 상태

```tsx
import { useAppSelector } from '@/app/hooks';
import { selectFilteredParcels, selectActiveFiltersCount } from '@/features/land/land.selectors';

function ParcelList() {
  const parcels = useAppSelector(selectFilteredParcels);
  const filterCount = useAppSelector(selectActiveFiltersCount);

  return (
    <div>
      <h2>
        필지 ({parcels.length})
        {filterCount > 0 && ` - 활성 필터 ${filterCount}개` }
      </h2>
      <ul>
        {parcels.map((parcel) => (
          <li key={parcel.id}>{parcel.name}</li>
        ))}
      </ul>
    </div>
  );
}
```

## 액션 디스패치

```tsx
import { useAppDispatch } from '@/app/hooks';
import { modalOpened, modalClosed } from '@/features/ui/ui.slice';

function TransferButton({ parcelId }: { parcelId: string }) {
  const dispatch = useAppDispatch();

  const handleClick = () => {
    // 모달을 열기 위한 액션 디스패치
    dispatch(modalOpened());
  };

  return <button onClick={handleClick}>필지 이전</button>;
}
```

## 엔티티 어댑터 셀렉터

```tsx
import { useAppSelector } from '@/app/hooks';
import { creditsSelectors, selectTotalCredits } from '@/features/credits/credits.slice';

function CreditsSummary() {
  // 모든 거래 가져오기
  const allTxs = useAppSelector(creditsSelectors.selectAll);
  
  // 특정 거래 가져오기
  const txId = 'tx-123';
  const tx = useAppSelector((state) => 
    creditsSelectors.selectById(state, txId)
  );
  
  // 총 개수 가져오기
  const count = useAppSelector(creditsSelectors.selectTotal);
  
  // 파생 값 가져오기
  const total = useAppSelector(selectTotalCredits);

  return (
    <div>
      <p>총 크레딧: {total}</p>
      <p>거래 수: {count}</p>
      <ul>
        {allTxs.map((tx) => (
          <li key={tx.id}>
            {tx.type === 'grant' ? '+' : '-'}
            {tx.amount} - {tx.description}
          </li>
        ))}
      </ul>
    </div>
  );
}
```

## 데이터 사전 가져오기

더 빠른 UX를 위해 이동 전에 데이터를 사전 가져오세요:

```tsx
import { useAppDispatch } from '@/app/hooks';
import { client } from '@/services/client';
import { Link } from 'react-router-dom';

function ParcelListItem({ parcel }: { parcel: Parcel }) {
  const dispatch = useAppDispatch();

  const handleMouseEnter = () => {
    // 호버 시 필지 세부 정보를 사전 가져오기
    dispatch(
      client.util.prefetch('getParcel', { id: parcel.id }, { force: false })
    );
  };

  return (
    <Link
      to={`/parcels/${parcel.id}`}

      onMouseEnter={handleMouseEnter}
    >
      {parcel.name}
    </Link>
  );
}
```

## 수동 캐시 관리

```tsx
import { useAppDispatch } from '@/app/hooks';
import { client } from '@/services/client';

function RefreshButton() {
  const dispatch = useAppDispatch();

  const handleRefresh = () => {
    // 모든 Parcels 쿼리 무효화
    dispatch(client.util.invalidateTags(['Parcels']));
    
    // 또는 클라이언트 상태 전체 초기화
    // dispatch(client.util.resetApiState());
  };

  return <button onClick={handleRefresh}>데이터 새로 고침</button>;
}
```

## 쿼리 상태 처리

```tsx
function ComprehensiveExample({ id }: { id: string }) {
  const {
    data,
    isLoading,      // 초기 로드
    isFetching,     // 모든 가져오기(재요청 포함)
    isSuccess,      // 쿼리 성공
    isError,        // 쿼리 실패
    error,          // 오류 객체
    refetch,        // 수동 재요청 함수
  } = useGetParcelQuery({ id });

  // 다양한 상태
  if (isLoading) {
    return <Spinner />;
  }

  if (isError) {
    return (
      <div>
        <p>오류: {error.toString()}</p>
        <button onClick={() => refetch()}>다시 시도</button>
      </div>
    );
  }

  if (!data) {
    return <div>데이터 없음</div>;
  }

  return (
    <div>
      {isFetching && <div className="refetch-indicator">업데이트 중...</div>}
      <h1>{data.name}</h1>
      <button onClick={() => refetch()}>새로 고침</button>
    </div>
  );
}
```

## 폼 통합

```tsx
import { useState } from 'react';
import { useAppDispatch } from '@/app/hooks';
import { filtersUpdated } from '@/features/land/land.slice';

function FilterForm() {
  const dispatch = useAppDispatch();
  const [owner, setOwner] = useState('');
  const [minPrice, setMinPrice] = useState('');

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    
    dispatch(filtersUpdated({
      owner: owner || undefined,
      minPrice: minPrice ? parseInt(minPrice, 10) : undefined,
    }));
  };

  return (
    <form onSubmit={handleSubmit}>
      <input
        type="text"
        value={owner}
        onChange={(e) => setOwner(e.target.value)}
        placeholder="소유자 주소"
      />
      <input
        type="number"
        value={minPrice}
        onChange={(e) => setMinPrice(e.target.value)}
        placeholder="최소 가격"
      />
      <button type="submit">필터 적용</button>
    </form>
  );
}
```

## 권장 사항

### 1. 타입이 지정된 훅 사용

```tsx
// ✅ 좋음: 타입이 지정된 훅 사용
import { useAppSelector, useAppDispatch } from '@/app/hooks';

const value = useAppSelector(selectValue);
const dispatch = useAppDispatch();

// ❌ 나쁨: 타입이 지정되지 않은 훅 사용
import { useSelector, useDispatch } from 'react-redux';

const value = useSelector((state: RootState) => state.value);
const dispatch = useDispatch();
```

### 2. 모든 쿼리 상태 처리

```tsx
// ✅ 좋음: 포괄적인 상태 처리
const { data, isLoading, isError, error } = useQuery(args);
if (isLoading) return <Loading />;
if (isError) return <Error error={error} />;
if (!data) return null;
return <Content data={data} />;

// ❌ 나쁨: 오류 처리가 없음
const { data } = useQuery(args);
return <Content data={data} />; // 충돌할 수 있음
```

### 3. 사용 `.unwrap()` 뮤테이션에

```tsx
// ✅ 좋음: 명시적 오류 처리
try {
  const result = await mutation(args).unwrap();
  toast.success('성공!');
} catch (error) {
  toast.error('실패!');
}

// ❌ 나쁨: 오류 처리 없음
mutation(args);
```

### 4. 결과를 좁히기 위해 `selectFromResult`

```tsx
// ✅ 좋음: 필요한 필드에만 구독
const { name } = useQuery(args, {
  selectFromResult: ({ data }) => ({ name: data?.name }),
});

// ❌ 나쁨: 전체 객체에 구독
const { data } = useQuery(args);
const name = data?.name;
```

### 5. 전체 슬라이스 선택 피하기

```tsx
// ✅ 좋음: 특정 값 선택
const viewMode = useAppSelector(selectViewMode);
const filters = useAppSelector(selectFilters);

// ❌ 나쁨: 전체 슬라이스 선택
const ui = useAppSelector((state) => state.ui);
const viewMode = ui.viewMode;
```

## 다음 단계

* 학습해 보세요 [Web3 통합](/docs/contributor/contributor-ko/contributor-guides/ui/web3-integration/) 블록체인 패턴을 위해
* 검토 [테스트 및 성능](/docs/contributor/contributor-ko/contributor-guides/ui/testing-and-performance/) 최적화를 위해
* 참고 [RTK Query](/docs/contributor/contributor-ko/contributor-guides/ui/rtk-query/) 고급 엔드포인트 구성을 위해


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/contributor/contributor-ko/contributor-guides/ui/component-patterns.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
