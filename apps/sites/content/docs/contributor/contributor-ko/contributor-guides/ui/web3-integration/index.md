# Web3 통합

이 페이지에서는 지갑 연결, 트랜잭션, 온체인 이벤트 처리를 포함하여 블록체인 기능을 Redux 및 RTK Query와 통합하는 패턴을 다룹니다.

## 핵심 원칙: Web3 객체는 Redux 밖에 두기


> [!CAUTION]
> **다음은 Redux에 저장하지 마세요:**
> 
> * `window.ethereum`
> * Provider 인스턴스 (`ethers.Provider`, `Web3Provider`)
> * Signer 인스턴스
> * WebSocket 연결
> * 계약 인스턴스
> * `AbortController` 인스턴스
> 
> 이것들은 직렬화할 수 없으며 Redux 원칙을 위반합니다.


## 권장 아키텍처

### Web3 컨텍스트 (Redux 외부)

```tsx
// src/contexts/Web3Context.tsx
import { createContext, useContext, useEffect, useState, ReactNode } from 'react';
import { ethers } from 'ethers';

interface Web3ContextValue {
  provider: ethers.providers.Web3Provider | null;
  signer: ethers.Signer | null;
  account: string | null;
  chainId: number | null;
  connect: () => Promise<void>;
  disconnect: () => void;
}

const Web3Context = createContext<Web3ContextValue | undefined>(undefined);

export function Web3Provider({ children }: { children: ReactNode }) {
  const [provider, setProvider] = useState<ethers.providers.Web3Provider | null>(null);
  const [signer, setSigner] = useState<ethers.Signer | null>(null);
  const [account, setAccount] = useState<string | null>(null);
  const [chainId, setChainId] = useState<number | null>(null);

  const connect = async () => {
    if (!window.ethereum) {
      MetaMask가 설치되어 있지 않습니다
    }

    const web3Provider = new ethers.providers.Web3Provider(window.ethereum);
    const accounts = await web3Provider.send('eth_requestAccounts', []);
    const network = await web3Provider.getNetwork();
    const signer = web3Provider.getSigner();

    setProvider(web3Provider);
    setSigner(signer);
    setAccount(accounts[0]);
    setChainId(network.chainId);
  };

  const disconnect = () => {
    setProvider(null);
    setSigner(null);
    setAccount(null);
    setChainId(null);
  };

  // 계정 변경 감지
  useEffect(() => {
    if (!window.ethereum) return;

    const handleAccountsChanged = (accounts: string[]) => {
      if (accounts.length === 0) {
        disconnect();
      } else {
        setAccount(accounts[0]);
      }
    };

    const handleChainChanged = (chainIdHex: string) => {
      const newChainId = parseInt(chainIdHex, 16);
      setChainId(newChainId);
    };

    window.ethereum.on('accountsChanged', handleAccountsChanged);
    window.ethereum.on('chainChanged', handleChainChanged);

    return () => {
      window.ethereum?.removeListener('accountsChanged', handleAccountsChanged);
      window.ethereum?.removeListener('chainChanged', handleChainChanged);
    };
  }, []);

  return (
    <Web3Context.Provider
      value={{ provider, signer, account, chainId, connect, disconnect }}
    >
      {children}
    </Web3Context.Provider>
  );
}

export function useWeb3() {
  const context = useContext(Web3Context);
  if (!context) {
    useWeb3는 Web3Provider 내부에서 사용해야 합니다
  }
  return context;
}
```

### Web3 상태를 위한 Redux Slice

Redux에는 직렬화 가능한 Web3 상태만 저장하세요:

```tsx
// src/features/web3/web3.slice.ts
import { createSlice, PayloadAction } from '@reduxjs/toolkit';
import type { RootState } from '@/app/store';

interface Web3State {
  account: string | null;
  chainId: number | null;
  isConnected: boolean;
  pendingTxs: Record<string, PendingTransaction>;
}

interface PendingTransaction {
  hash: string;
  type: 'transfer' | 'mint' | 'approve';
  status: 'pending' | 'confirmed' | 'failed';
  timestamp: number;
}

const initialState: Web3State = {
  account: null,
  chainId: null,
  isConnected: false,
  pendingTxs: {},
};

const web3Slice = createSlice({
  name: 'web3',
  initialState,
  reducers: {
    connected(state, action: PayloadAction<{ account: string; chainId: number }>) {
      state.account = action.payload.account;
      state.chainId = action.payload.chainId;
      state.isConnected = true;
    },
    
    disconnected(state) {
      state.account = null;
      state.chainId = null;
      state.isConnected = false;
      state.pendingTxs = {};
    },
    
    chainChanged(state, action: PayloadAction<number>) {
      state.chainId = action.payload;
    },
    
    accountChanged(state, action: PayloadAction<string>) {
      state.account = action.payload;
    },
    
    txPending(state, action: PayloadAction<PendingTransaction>) {
      state.pendingTxs[action.payload.hash] = action.payload;
    },
    
    txConfirmed(state, action: PayloadAction<string>) {
      if (state.pendingTxs[action.payload]) {
        state.pendingTxs[action.payload].status = 'confirmed';
      }
    },
    
    txFailed(state, action: PayloadAction<string>) {
      if (state.pendingTxs[action.payload]) {
        state.pendingTxs[action.payload].status = 'failed';
      }
    },
    
    txRemoved(state, action: PayloadAction<string>) {
      delete state.pendingTxs[action.payload];
    },
  },
});

export const {
  connected,
  disconnected,
  chainChanged,
  accountChanged,
  txPending,
  txConfirmed,
  txFailed,
  txRemoved,
} = web3Slice.actions;

export default web3Slice.reducer;

// 셀렉터
export const selectAccount = (state: RootState) => state.web3.account;
export const selectChainId = (state: RootState) => state.web3.chainId;
export const selectIsConnected = (state: RootState) => state.web3.isConnected;
export const selectPendingTxs = (state: RootState) => 
  Object.values(state.web3.pendingTxs);
```

## 컨텍스트를 Redux와 동기화

Web3 컨텍스트를 Redux에 연결:

```tsx
// src/components/Web3Sync.tsx
import { useEffect } from 'react';
import { useWeb3 } from '@/contexts/Web3Context';
import { useAppDispatch } from '@/app/hooks';
import { connected, disconnected, chainChanged, accountChanged } from '@/features/web3/web3.slice';
import { client } from '@/services/client';

export function Web3Sync() {
  const { account, chainId, connect } = useWeb3();
  const dispatch = useAppDispatch();

  // 연결 상태 동기화
  useEffect(() => {
    if (account && chainId) {
      dispatch(connected({ account, chainId }));
    } else {
      dispatch(disconnected());
    }
  }, [account, chainId, dispatch]);

  // 계정/체인 변경 시 캐시 초기화
  useEffect(() => {
    if (account || chainId) {
      // 옵션 1: 전체 클라이언트 상태 초기화
      dispatch(client.util.resetApiState());
      
      // 옵션 2: 특정 태그 무효화
      // dispatch(client.util.invalidateTags(['User', 'Parcels', 'Credits']));
    }
  }, [account, chainId, dispatch]);

  // 이전에 연결되었다면 마운트 시 자동 연결
  useEffect(() => {
    const autoConnect = async () => {
      const wasConnected = localStorage.getItem('walletConnected');
      if (wasConnected && window.ethereum) {
        try {
          await connect();
        } catch (error) {
          자동 연결에 실패했습니다:
        }
      }
    };

    autoConnect();
  }, [connect]);

  null 반환
}
```

## 트랜잭션 라이프사이클

### 트랜잭션 전송

```tsx
// src/hooks/useTransferParcel.ts
import { useCallback } from 'react';
import { useWeb3 } from '@/contexts/Web3Context';
import { useAppDispatch } from '@/app/hooks';
import { txPending, txConfirmed, txFailed } from '@/features/web3/web3.slice';
import { client } from '@/services/client';
import { ParcelContract } from '@/contracts';

export function useTransferParcel() {
  const { signer, account } = useWeb3();
  const dispatch = useAppDispatch();

  return useCallback(async (parcelId: string, to: string) => {
    if (!signer || !account) {
      지갑이 연결되지 않았습니다
    }

    // 계약 인스턴스 가져오기
    const contract = ParcelContract.connect(signer);

    try {
      // 트랜잭션 전송
      const tx = await contract.transfer(parcelId, to);

      // 보류 목록에 추가
      dispatch(txPending({
        hash: tx.hash,
        type: 'transfer',
        status: 'pending',
        timestamp: Date.now(),
      }));

      // 낙관적으로 캐시 업데이트
      dispatch(
        client.util.updateQueryData('getParcel', { id: parcelId }, (draft) => {
          draft.owner = to;
        })
      );

      // 확인을 기다림
      const receipt = await tx.wait();

      if (receipt.status === 1) {
        // 트랜잭션 확인됨
        dispatch(txConfirmed(tx.hash));
        
        // 영향을 받는 쿼리 무효화
        dispatch(client.util.invalidateTags([
          { type: 'Parcels', id: parcelId },
          'Parcels',
        ]));
      } else {
        // 트랜잭션 실패
        dispatch(txFailed(tx.hash));
        
        // 낙관적 업데이트 롤백
        dispatch(client.util.invalidateTags([
          { type: 'Parcels', id: parcelId },
        ]));
      }

      return receipt;
    } catch (error) {
      // 트랜잭션이 거부되었거나 실패함
      if (error.hash) {
        dispatch(txFailed(error.hash));
      }
      
      // 낙관적 업데이트 롤백
      dispatch(client.util.invalidateTags([
        { type: 'Parcels', id: parcelId },
      ]));
      
      throw error;
    }
  }, [signer, account, dispatch]);
}
```

### 컴포넌트에서 사용하기

```tsx
function TransferParcelButton({ parcelId }: { parcelId: string }) {
  const transferParcel = useTransferParcel();
  const [recipient, setRecipient] = useState('');
  const [isLoading, setIsLoading] = useState(false);

  const handleTransfer = async () => {
    setIsLoading(true);
    try {
      await transferParcel(parcelId, recipient);
      전송 성공!
      setRecipient('');
    } catch (error) {
      if (error.code === 4001) {
        트랜잭션이 거부되었습니다
      } else {
        전송 실패
      }
    } finally {
      setIsLoading(false);
    }
  };

  return (
    <div>
      <input
        value={recipient}
        onChange={(e) => setRecipient(e.target.value)}
        placeholder="수신자 주소"
      />
      <button onClick={handleTransfer} disabled={isLoading || !recipient}>
        {isLoading ? '전송 중...' : '전송'}
      </button>
    </div>
  );
}
```

## 온체인 이벤트 수신

```tsx
// src/hooks/useParcelEvents.ts
import { useEffect } from 'react';
import { useWeb3 } from '@/contexts/Web3Context';
import { useAppDispatch } from '@/app/hooks';
import { client } from '@/services/client';
import { ParcelContract } from '@/contracts';

export function useParcelEvents() {
  const { provider } = useWeb3();
  const dispatch = useAppDispatch();

  useEffect(() => {
    if (!provider) return;

    const contract = ParcelContract.connect(provider);

    // Transfer 이벤트 수신
    const handleTransfer = (from: string, to: string, tokenId: string) => {
      console.log(`토큰 ID ${tokenId}의 소유권이 ${from}에서 ${to}로 이전되었습니다`);
      
      // 영향을 받는 쿼리 무효화
      dispatch(client.util.invalidateTags([
        { type: 'Parcels', id: tokenId },
        'Parcels',
      ]));
    };

    // 이벤트 구독
    contract.on('Transfer', handleTransfer);

    // 정리
    return () => {
      contract.off('Transfer', handleTransfer);
    };
  }, [provider, dispatch]);
}
```

## Web3 데이터와 RTK Query

블록체인 데이터를 사용하는 엔드포인트를 만드세요:

```tsx
// src/features/nft/nft.client.ts
import { client } from '@/services/client';
import { ethers } from 'ethers';

export const nftClient = client.injectEndpoints({
  endpoints: (build) => ({
    // 하이브리드: API에서 가져오고 온체인에서 검증
    getNFTWithOwnership: build.query<NFT, { id: string; account?: string }>({
      async queryFn({ id, account }, { getState }) {
        try {
          // API에서 메타데이터 가져오기
          const response = await fetch(`/api/nfts/${id}`);
          const nft = await response.json();

          // account가 제공되면 온체인에서 소유권 검증
          if (account && window.ethereum) {
            const provider = new ethers.providers.Web3Provider(window.ethereum);
            const contract = NFTContract.connect(provider);
            const owner = await contract.ownerOf(id);
            
            nft.isOwner = owner.toLowerCase() === account.toLowerCase();
          }

          return { data: nft };
        } catch (error) {
          return { error: error.message };
        }
      },
      providesTags: (result, error, arg) => [
        { type: 'NFTs', id: arg.id },
      ],
    }),
  }),
});
```

## 캐시 무효화 전략

### 네트워크 변경 시

```tsx
// 네트워크가 변경되면 모든 데이터 무효화
useEffect(() => {
  if (chainId) {
    dispatch(client.util.resetApiState());
  }
}, [chainId, dispatch]);
```

### 계정 변경 시

```tsx
// 사용자별 데이터 무효화
useEffect(() => {
  if (account) {
    dispatch(client.util.invalidateTags(['User', 'Credits', 'NFTs']));
  }
}, [account, dispatch]);
```

### 트랜잭션 확인 후

```tsx
// 트랜잭션 후 관련 데이터 무효화
if (receipt.status === 1) {
  dispatch(client.util.invalidateTags([
    { type: 'Parcels', id: parcelId },
    'Parcels',
    'User', // 사용자의 잔액에 영향을 줄 수 있음
  ]));
}
```

## 권장 사항

### 1. 관심사 분리

```tsx
// ✅ 좋음: Web3는 컨텍스트에, 상태는 Redux에
const { signer } = useWeb3(); // 컨텍스트에서
const account = useAppSelector(selectAccount); // Redux에서

// ❌ 나쁨: 모든 것을 Redux에
const { signer, account } = useAppSelector(selectWeb3); // 이렇게 하지 마세요
```

### 2. 트랜잭션 상태 처리

```tsx
// ✅ 좋음: 모든 상태 추적
const tx = await contract.transfer(...);
dispatch(txPending(tx.hash));
const receipt = await tx.wait();
if (receipt.status === 1) {
  dispatch(txConfirmed(tx.hash));
} else {
  dispatch(txFailed(tx.hash));
}

// ❌ 나쁨: 실행만 하고 무시
await contract.transfer(...);
```

### 3. 롤백이 있는 낙관적 업데이트

```tsx
// ✅ 좋음: 롤백이 있는 낙관적 처리
dispatch(client.util.updateQueryData(...));
try {
  await tx.wait();
  dispatch(client.util.invalidateTags(...));
} catch {
  dispatch(client.util.invalidateTags(...)); // 롤백
}

// ❌ 나쁨: 롤백 없음
dispatch(client.util.updateQueryData(...));
await tx.wait(); // 이게 실패하면?
```

### 4. 이벤트 리스너 정리

```tsx
// ✅ 좋음: 리스너 정리
useEffect(() => {
  contract.on('Transfer', handler);
  return () => contract.off('Transfer', handler);
}, [contract]);

// ❌ 나쁨: 메모리 누수
useEffect(() => {
  contract.on('Transfer', handler);
}, [contract]);
```

## 다음 단계

* 검토 [테스트 및 성능](/docs/contributor/contributor-ko/contributor-guides/ui/testing-and-performance/) 최적화를 위해
* 참고 [컴포넌트 패턴](/docs/contributor/contributor-ko/contributor-guides/ui/component-patterns/) 사용 예시용
* 이해하기 [RTK Query](/docs/contributor/contributor-ko/contributor-guides/ui/rtk-query/) 캐시 관리를 위해


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/contributor/contributor-ko/contributor-guides/ui/web3-integration.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
