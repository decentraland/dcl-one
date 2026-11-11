# Integração Web3

Esta página aborda padrões para integrar funcionalidade blockchain com Redux e RTK Query, incluindo o tratamento de conexões de carteira, transações e eventos on-chain.

## Princípio Central: Mantenha Objetos Web3 Fora do Redux


> [!CAUTION]
> **Nunca armazene estes no Redux:**
> 
> * `window.ethereum`
> * Instâncias de Provider (`ethers.Provider`, `Web3Provider`)
> * Instâncias de Signer
> * Conexões WebSocket
> * Instâncias de contratos
> * `AbortController` instâncias
> 
> Estes não são serializáveis e violam os princípios do Redux.


## Arquitetura Recomendada

### Contexto Web3 (Fora do Redux)

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
      throw new Error('MetaMask não instalado');
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

  // Escutar mudanças na conta
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
    throw new Error('useWeb3 deve ser usado dentro de Web3Provider');
  }
  return context;
}
```

### Redux Slice para o Estado Web3

Armazene apenas o estado Web3 serializável no Redux:

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

// Selectors
export const selectAccount = (state: RootState) => state.web3.account;
export const selectChainId = (state: RootState) => state.web3.chainId;
export const selectIsConnected = (state: RootState) => state.web3.isConnected;
export const selectPendingTxs = (state: RootState) => 
  Object.values(state.web3.pendingTxs);
```

## Sincronizando o Contexto com o Redux

Conecte o contexto Web3 ao Redux:

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

  // Sincronizar estado da conexão
  useEffect(() => {
    if (account && chainId) {
      dispatch(connected({ account, chainId }));
    } else {
      dispatch(disconnected());
    }
  }, [account, chainId, dispatch]);

  // Redefinir cache na mudança de conta/rede
  useEffect(() => {
    if (account || chainId) {
      // Opção 1: Redefinir todo o estado do client
      dispatch(client.util.resetApiState());
      
      // Opção 2: Invalidar tags específicas
      // dispatch(client.util.invalidateTags(['User', 'Parcels', 'Credits']));
    }
  }, [account, chainId, dispatch]);

  // Conexão automática na montagem se já estiver conectado
  useEffect(() => {
    const autoConnect = async () => {
      const wasConnected = localStorage.getItem('walletConnected');
      if (wasConnected && window.ethereum) {
        try {
          await connect();
        } catch (error) {
          console.error('Falha na conexão automática:', error);
        }
      }
    };

    autoConnect();
  }, [connect]);

  return null;
}
```

## Ciclo de Vida da Transação

### Enviando Transações

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
      throw new Error('Carteira não conectada');
    }

    // Obter instância do contrato
    const contract = ParcelContract.connect(signer);

    try {
      // Enviar transação
      const tx = await contract.transfer(parcelId, to);

      // Adicionar aos pendentes
      dispatch(txPending({
        hash: tx.hash,
        type: 'transfer',
        status: 'pending',
        timestamp: Date.now(),
      }));

      // Atualizar cache de forma otimista
      dispatch(
        client.util.updateQueryData('getParcel', { id: parcelId }, (draft) => {
          draft.owner = to;
        })
      );

      // Aguardar confirmação
      const receipt = await tx.wait();

      if (receipt.status === 1) {
        // Transação confirmada
        dispatch(txConfirmed(tx.hash));
        
        // Invalidar queries afetadas
        dispatch(client.util.invalidateTags([
          { type: 'Parcels', id: parcelId },
          'Parcels',
        ]));
      } else {
        // Transação falhou
        dispatch(txFailed(tx.hash));
        
        // Reverter atualização otimista
        dispatch(client.util.invalidateTags([
          { type: 'Parcels', id: parcelId },
        ]));
      }

      return receipt;
    } catch (error) {
      // Transação rejeitada ou falhou
      if (error.hash) {
        dispatch(txFailed(error.hash));
      }
      
      // Reverter atualização otimista
      dispatch(client.util.invalidateTags([
        { type: 'Parcels', id: parcelId },
      ]));
      
      throw error;
    }
  }, [signer, account, dispatch]);
}
```

### Usando em Componentes

```tsx
function TransferParcelButton({ parcelId }: { parcelId: string }) {
  const transferParcel = useTransferParcel();
  const [recipient, setRecipient] = useState('');
  const [isLoading, setIsLoading] = useState(false);

  const handleTransfer = async () => {
    setIsLoading(true);
    try {
      await transferParcel(parcelId, recipient);
      toast.success('Transferência bem-sucedida!');
      setRecipient('');
    } catch (error) {
      if (error.code === 4001) {
        toast.error('Transação rejeitada');
      } else {
        toast.error('Falha na transferência');
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
        placeholder="Endereço do destinatário"
      />
      <button onClick={handleTransfer} disabled={isLoading || !recipient}>
        {isLoading ? 'Transferindo...' : 'Transferir'}
      </button>
    </div>
  );
}
```

## Escutando Eventos On-Chain

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

    // Escutar eventos Transfer
    const handleTransfer = (from: string, to: string, tokenId: string) => {
      console.log(`Parcel ${tokenId} transferido de ${from} para ${to}`);
      
      // Invalidar queries afetadas
      dispatch(client.util.invalidateTags([
        { type: 'Parcels', id: tokenId },
        'Parcels',
      ]));
    };

    // Assinar eventos
    contract.on('Transfer', handleTransfer);

    // Limpeza
    return () => {
      contract.off('Transfer', handleTransfer);
    };
  }, [provider, dispatch]);
}
```

## RTK Query com Dados Web3

Crie endpoints que usem dados da blockchain:

```tsx
// src/features/nft/nft.client.ts
import { client } from '@/services/client';
import { ethers } from 'ethers';

export const nftClient = client.injectEndpoints({
  endpoints: (build) => ({
    // Híbrido: buscar da API e verificar on-chain
    getNFTWithOwnership: build.query<NFT, { id: string; account?: string }>({
      async queryFn({ id, account }, { getState }) {
        try {
          // Buscar metadados da API
          const response = await fetch(`/api/nfts/${id}`);
          const nft = await response.json();

          // Verificar a propriedade on-chain se uma conta for fornecida
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

## Estratégias de Invalidação de Cache

### Ao Mudar de Rede

```tsx
// Invalidar todos os dados quando a rede mudar
useEffect(() => {
  if (chainId) {
    dispatch(client.util.resetApiState());
  }
}, [chainId, dispatch]);
```

### Ao Mudar de Conta

```tsx
// Invalidar dados específicos do usuário
useEffect(() => {
  if (account) {
    dispatch(client.util.invalidateTags(['User', 'Credits', 'NFTs']));
  }
}, [account, dispatch]);
```

### Após a Confirmação da Transação

```tsx
// Invalidar dados relacionados após a transação
if (receipt.status === 1) {
  dispatch(client.util.invalidateTags([
    { type: 'Parcels', id: parcelId },
    'Parcels',
    'User', // Pode afetar o saldo do usuário
  ]));
}
```

## Melhores práticas

### 1. Separar Responsabilidades

```tsx
// ✅ Bom: Web3 no contexto, estado no Redux
const { signer } = useWeb3(); // Do contexto
const account = useAppSelector(selectAccount); // Do Redux

// ❌ Ruim: Tudo no Redux
const { signer, account } = useAppSelector(selectWeb3); // Não faça isso
```

### 2. Lidar com Estados de Transação

```tsx
// ✅ Bom: Acompanhar todos os estados
const tx = await contract.transfer(...);
dispatch(txPending(tx.hash));
const receipt = await tx.wait();
if (receipt.status === 1) {
  dispatch(txConfirmed(tx.hash));
} else {
  dispatch(txFailed(tx.hash));
}

// ❌ Ruim: Enviar e esquecer
await contract.transfer(...);
```

### 3. Atualizações Otimistas com Rollback

```tsx
// ✅ Bom: Otimista com rollback
dispatch(client.util.updateQueryData(...));
try {
  await tx.wait();
  dispatch(client.util.invalidateTags(...));
} catch {
  dispatch(client.util.invalidateTags(...)); // Rollback
}

// ❌ Ruim: Sem rollback
dispatch(client.util.updateQueryData(...));
await tx.wait(); // E se isso falhar?
```

### 4. Limpeza de Event Listeners

```tsx
// ✅ Bom: Limpar listeners
useEffect(() => {
  contract.on('Transfer', handler);
  return () => contract.off('Transfer', handler);
}, [contract]);

// ❌ Ruim: Vazamento de memória
useEffect(() => {
  contract.on('Transfer', handler);
}, [contract]);
```

## Próximos Passos

* Revisar [Testes e Performance](/docs/contributor/contributor-pt/guias-para-colaboradores/padroes-de-ui/testing-and-performance/) para otimização
* Veja [Padrões de Componentes](/docs/contributor/contributor-pt/guias-para-colaboradores/padroes-de-ui/component-patterns/) para exemplos de uso
* Entenda [RTK Query](/docs/contributor/contributor-pt/guias-para-colaboradores/padroes-de-ui/rtk-query/) para gerenciamento de cache


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/contributor/contributor-pt/guias-para-colaboradores/padroes-de-ui/web3-integration.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
