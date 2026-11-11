# Web3 集成

本页涵盖将区块链功能与 Redux 和 RTK Query 集成的模式，包括处理钱包连接、交易和链上事件。

## 核心原则：将 Web3 对象放在 Redux 之外


> [!CAUTION]
> **不要在 Redux 中存储这些：**
> 
> * `window.ethereum`
> * Provider 实例（`ethers.Provider`, `Web3Provider`)
> * Signer 实例
> * WebSocket 连接
> * 合约实例
> * `AbortController` 实例
> 
> 这些是不可序列化的，违反了 Redux 原则。


## 推荐架构

### Web3 上下文（Redux 之外）

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
      throw new Error('未安装 MetaMask');
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

  // 监听账户变化
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
    throw new Error('useWeb3 必须在 Web3Provider 内使用');
  }
  return context;
}
```

### Web3 状态的 Redux 切片

仅将可序列化的 Web3 状态存入 Redux：

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

// 选择器
export const selectAccount = (state: RootState) => state.web3.account;
export const selectChainId = (state: RootState) => state.web3.chainId;
export const selectIsConnected = (state: RootState) => state.web3.isConnected;
export const selectPendingTxs = (state: RootState) => 
  Object.values(state.web3.pendingTxs);
```

## 与 Redux 同步上下文

将 Web3 上下文连接到 Redux：

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

  // 同步连接状态
  useEffect(() => {
    if (account && chainId) {
      dispatch(connected({ account, chainId }));
    } else {
      dispatch(disconnected());
    }
  }, [account, chainId, dispatch]);

  // 在账户/链变化时重置缓存
  useEffect(() => {
    if (account || chainId) {
      // 选项 1：重置整个客户端状态
      dispatch(client.util.resetApiState());
      
      // 选项 2：使特定标签失效
      // dispatch(client.util.invalidateTags(['User', 'Parcels', 'Credits']));
    }
  }, [account, chainId, dispatch]);

  // 如果之前已连接，则在挂载时自动连接
  useEffect(() => {
    const autoConnect = async () => {
      const wasConnected = localStorage.getItem('walletConnected');
      if (wasConnected && window.ethereum) {
        try {
          await connect();
        } catch (error) {
          console.error('自动连接失败：', error);
        }
      }
    };

    autoConnect();
  }, [connect]);

  return null;
}
```

## 交易生命周期

### 发送交易

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
      throw new Error('钱包未连接');
    }

    // 获取合约实例
    const contract = ParcelContract.connect(signer);

    try {
      // 发送交易
      const tx = await contract.transfer(parcelId, to);

      // 加入待处理
      dispatch(txPending({
        hash: tx.hash,
        type: 'transfer',
        status: 'pending',
        timestamp: Date.now(),
      }));

      // 乐观更新缓存
      dispatch(
        client.util.updateQueryData('getParcel', { id: parcelId }, (draft) => {
          draft.owner = to;
        })
      );

      // 等待确认
      const receipt = await tx.wait();

      if (receipt.status === 1) {
        // 交易已确认
        dispatch(txConfirmed(tx.hash));
        
        // 使受影响的查询失效
        dispatch(client.util.invalidateTags([
          { type: 'Parcels', id: parcelId },
          'Parcels',
        ]));
      } else {
        // 交易失败
        dispatch(txFailed(tx.hash));
        
        // 回滚乐观更新
        dispatch(client.util.invalidateTags([
          { type: 'Parcels', id: parcelId },
        ]));
      }

      return receipt;
    } catch (error) {
      // 交易被拒绝或失败
      if (error.hash) {
        dispatch(txFailed(error.hash));
      }
      
      // 回滚乐观更新
      dispatch(client.util.invalidateTags([
        { type: 'Parcels', id: parcelId },
      ]));
      
      throw error;
    }
  }, [signer, account, dispatch]);
}
```

### 在组件中使用

```tsx
function TransferParcelButton({ parcelId }: { parcelId: string }) {
  const transferParcel = useTransferParcel();
  const [recipient, setRecipient] = useState('');
  const [isLoading, setIsLoading] = useState(false);

  const handleTransfer = async () => {
    setIsLoading(true);
    try {
      await transferParcel(parcelId, recipient);
      toast.success('转账成功！');
      setRecipient('');
    } catch (error) {
      if (error.code === 4001) {
        toast.error('交易被拒绝');
      } else {
        toast.error('转账失败');
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
        placeholder="收件人地址"
      />
      <button onClick={handleTransfer} disabled={isLoading || !recipient}>
        {isLoading ? '转账中...' : '转账'}
      </button>
    </div>
  );
}
```

## 监听链上事件

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

    // 监听 Transfer 事件
    const handleTransfer = (from: string, to: string, tokenId: string) => {
      console.log(`地块 ${tokenId} 已从 ${from} 转移到 ${to}`);
      
      // 使受影响的查询失效
      dispatch(client.util.invalidateTags([
        { type: 'Parcels', id: tokenId },
        'Parcels',
      ]));
    };

    // 订阅事件
    contract.on('Transfer', handleTransfer);

    // 清理
    return () => {
      contract.off('Transfer', handleTransfer);
    };
  }, [provider, dispatch]);
}
```

## 结合 Web3 数据使用 RTK Query

创建使用区块链数据的端点：

```tsx
// src/features/nft/nft.client.ts
import { client } from '@/services/client';
import { ethers } from 'ethers';

export const nftClient = client.injectEndpoints({
  endpoints: (build) => ({
    // 混合方式：从 API 获取并在链上验证
    getNFTWithOwnership: build.query<NFT, { id: string; account?: string }>({
      async queryFn({ id, account }, { getState }) {
        try {
          // 从 API 获取元数据
          const response = await fetch(`/api/nfts/${id}`);
          const nft = await response.json();

          // 如果提供了 account，则在链上验证所有权
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

## 缓存失效策略

### 在网络变化时

```tsx
// 在网络变化时使所有数据失效
useEffect(() => {
  if (chainId) {
    dispatch(client.util.resetApiState());
  }
}, [chainId, dispatch]);
```

### 在账户变化时

```tsx
// 使用户相关数据失效
useEffect(() => {
  if (account) {
    dispatch(client.util.invalidateTags(['User', 'Credits', 'NFTs']));
  }
}, [account, dispatch]);
```

### 交易确认后

```tsx
// 在交易后使相关数据失效
if (receipt.status === 1) {
  dispatch(client.util.invalidateTags([
    { type: 'Parcels', id: parcelId },
    'Parcels',
    'User', // 可能会影响用户余额
  ]));
}
```

## 最佳实践

### 1. 分离关注点

```tsx
// ✅ 好的：Web3 放在上下文中，状态放在 Redux 中
const { signer } = useWeb3(); // 来自上下文
const account = useAppSelector(selectAccount); // 来自 Redux

// ❌ 不好的：把所有东西都放在 Redux 中
const { signer, account } = useAppSelector(selectWeb3); // 不要这样做
```

### 2. 处理交易状态

```tsx
// ✅ 好的：跟踪所有状态
const tx = await contract.transfer(...);
dispatch(txPending(tx.hash));
const receipt = await tx.wait();
if (receipt.status === 1) {
  dispatch(txConfirmed(tx.hash));
} else {
  dispatch(txFailed(tx.hash));
}

// ❌ 不好的：发出后就不管了
await contract.transfer(...);
```

### 3. 带回滚的乐观更新

```tsx
// ✅ 好的：带回滚的乐观更新
dispatch(client.util.updateQueryData(...));
try {
  await tx.wait();
  dispatch(client.util.invalidateTags(...));
} catch {
  dispatch(client.util.invalidateTags(...)); // 回滚
}

// ❌ 不好的：没有回滚
dispatch(client.util.updateQueryData(...));
await tx.wait(); // 如果这一步失败会怎样？
```

### 4. 事件监听器清理

```tsx
// ✅ 好的：清理监听器
useEffect(() => {
  contract.on('Transfer', handler);
  return () => contract.off('Transfer', handler);
}, [contract]);

// ❌ 不好的：内存泄漏
useEffect(() => {
  contract.on('Transfer', handler);
}, [contract]);
```

## 下一步

* 回顾 [测试与性能](/docs/contributor/contributor-zh/gong-xian-zhe-zhi-nan/ui-biao-zhun/testing-and-performance/) 用于优化
* 查看 [组件模式](/docs/contributor/contributor-zh/gong-xian-zhe-zhi-nan/ui-biao-zhun/component-patterns/) 用于使用示例
* 理解 [RTK Query](/docs/contributor/contributor-zh/gong-xian-zhe-zhi-nan/ui-biao-zhun/rtk-query/) 用于缓存管理


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/contributor/contributor-zh/gong-xian-zhe-zhi-nan/ui-biao-zhun/web3-integration.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
