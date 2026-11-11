# Integración Web3

Esta página cubre patrones para integrar funcionalidad de blockchain con Redux y RTK Query, incluyendo la gestión de conexiones de Wallet, transacciones y eventos en cadena.

## Principio fundamental: mantener los objetos de Web3 fuera de Redux


> [!CAUTION]
> **Nunca almacenes estos en Redux:**
> 
> * `window.ethereum`
> * Instancias de Provider (`ethers.Provider`, `Web3Provider`)
> * Instancias de Signer
> * Conexiones WebSocket
> * Instancias de Contract
> * `AbortController` instancias
> 
> Estos no son serializables y violan los principios de Redux.


## Arquitectura recomendada

### Contexto de Web3 (fuera de Redux)

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
      throw new Error('MetaMask no está instalado');
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

  // Escuchar cambios de cuenta
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
    throw new Error('useWeb3 debe usarse dentro de Web3Provider');
  }
  return context;
}
```

### Redux Slice para el estado de Web3

Almacena solo el estado serializable de Web3 en Redux:

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

// Selectores
export const selectAccount = (state: RootState) => state.web3.account;
export const selectChainId = (state: RootState) => state.web3.chainId;
export const selectIsConnected = (state: RootState) => state.web3.isConnected;
export const selectPendingTxs = (state: RootState) => 
  Object.values(state.web3.pendingTxs);
```

## Sincronizando el Contexto con Redux

Conecta el contexto de Web3 con Redux:

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

  // Sincronizar el estado de conexión
  useEffect(() => {
    if (account && chainId) {
      dispatch(connected({ account, chainId }));
    } else {
      dispatch(disconnected());
    }
  }, [account, chainId, dispatch]);

  // Restablecer la caché al cambiar de cuenta/cadena
  useEffect(() => {
    if (account || chainId) {
      // Opción 1: restablecer todo el estado del cliente
      dispatch(client.util.resetApiState());
      
      // Opción 2: invalidar etiquetas específicas
      // dispatch(client.util.invalidateTags(['User', 'Parcels', 'Credits']));
    }
  }, [account, chainId, dispatch]);

  // Auto-conectar al montarse si ya estaba conectado
  useEffect(() => {
    const autoConnect = async () => {
      const wasConnected = localStorage.getItem('walletConnected');
      if (wasConnected && window.ethereum) {
        try {
          await connect();
        } catch (error) {
          console.error('La auto-conexión falló:', error);
        }
      }
    };

    autoConnect();
  }, [connect]);

  return null;
}
```

## Ciclo de vida de la transacción

### Envío de transacciones

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
      throw new Error('Wallet no conectada');
    }

    // Obtener instancia de contrato
    const contract = ParcelContract.connect(signer);

    try {
      // Enviar transacción
      const tx = await contract.transfer(parcelId, to);

      // Añadir a pendientes
      dispatch(txPending({
        hash: tx.hash,
        type: 'transfer',
        status: 'pending',
        timestamp: Date.now(),
      }));

      // Actualizar caché de forma optimista
      dispatch(
        client.util.updateQueryData('getParcel', { id: parcelId }, (draft) => {
          draft.owner = to;
        })
      );

      // Esperar confirmación
      const receipt = await tx.wait();

      if (receipt.status === 1) {
        // Transacción confirmada
        dispatch(txConfirmed(tx.hash));
        
        // Invalidar consultas afectadas
        dispatch(client.util.invalidateTags([
          { type: 'Parcels', id: parcelId },
          'Parcels',
        ]));
      } else {
        // La transacción falló
        dispatch(txFailed(tx.hash));
        
        // Revertir la actualización optimista
        dispatch(client.util.invalidateTags([
          { type: 'Parcels', id: parcelId },
        ]));
      }

      return receipt;
    } catch (error) {
      // La transacción fue rechazada o falló
      if (error.hash) {
        dispatch(txFailed(error.hash));
      }
      
      // Revertir la actualización optimista
      dispatch(client.util.invalidateTags([
        { type: 'Parcels', id: parcelId },
      ]));
      
      throw error;
    }
  }, [signer, account, dispatch]);
}
```

### Uso en componentes

```tsx
function TransferParcelButton({ parcelId }: { parcelId: string }) {
  const transferParcel = useTransferParcel();
  const [recipient, setRecipient] = useState('');
  const [isLoading, setIsLoading] = useState(false);

  const handleTransfer = async () => {
    setIsLoading(true);
    try {
      await transferParcel(parcelId, recipient);
      toast.success('¡Transferencia exitosa!');
      setRecipient('');
    } catch (error) {
      if (error.code === 4001) {
        toast.error('Transacción rechazada');
      } else {
        toast.error('La transferencia falló');
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
        placeholder="Dirección del destinatario"
      />
      <button onClick={handleTransfer} disabled={isLoading || !recipient}>
        {isLoading ? 'Transfiriendo...' : 'Transferir'}
      </button>
    </div>
  );
}
```

## Escuchar eventos en cadena

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

    // Escuchar eventos de Transfer
    const handleTransfer = (from: string, to: string, tokenId: string) => {
      console.log(`Parcel ${tokenId} transferido de ${from} a ${to}`);
      
      // Invalidar consultas afectadas
      dispatch(client.util.invalidateTags([
        { type: 'Parcels', id: tokenId },
        'Parcels',
      ]));
    };

    // Suscribirse a los eventos
    contract.on('Transfer', handleTransfer);

    // Limpieza
    return () => {
      contract.off('Transfer', handleTransfer);
    };
  }, [provider, dispatch]);
}
```

## RTK Query con datos de Web3

Crea endpoints que usen datos de blockchain:

```tsx
// src/features/nft/nft.client.ts
import { client } from '@/services/client';
import { ethers } from 'ethers';

export const nftClient = client.injectEndpoints({
  endpoints: (build) => ({
    // Híbrido: obtener de la API y verificar en cadena
    getNFTWithOwnership: build.query<NFT, { id: string; account?: string }>({
      async queryFn({ id, account }, { getState }) {
        try {
          // Obtener metadatos de la API
          const response = await fetch(`/api/nfts/${id}`);
          const nft = await response.json();

          // Verificar la propiedad en cadena si se proporciona account
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

## Estrategias de invalidación de caché

### Al cambiar de red

```tsx
// Invalidar todos los datos cuando cambie la red
useEffect(() => {
  if (chainId) {
    dispatch(client.util.resetApiState());
  }
}, [chainId, dispatch]);
```

### Al cambiar de cuenta

```tsx
// Invalidar datos específicos del usuario
useEffect(() => {
  if (account) {
    dispatch(client.util.invalidateTags(['User', 'Credits', 'NFTs']));
  }
}, [account, dispatch]);
```

### Después de la confirmación de la transacción

```tsx
// Invalidar los datos relacionados después de la transacción
if (receipt.status === 1) {
  dispatch(client.util.invalidateTags([
    { type: 'Parcels', id: parcelId },
    'Parcels',
    'User', // Puede afectar el balance del usuario
  ]));
}
```

## Buenas prácticas

### 1. Separar responsabilidades

```tsx
// ✅ Bien: Web3 en el contexto, estado en Redux
const { signer } = useWeb3(); // Desde el contexto
const account = useAppSelector(selectAccount); // Desde Redux

// ❌ Mal: todo en Redux
const { signer, account } = useAppSelector(selectWeb3); // No hagas esto
```

### 2. Gestionar los estados de las transacciones

```tsx
// ✅ Bien: registrar todos los estados
const tx = await contract.transfer(...);
dispatch(txPending(tx.hash));
const receipt = await tx.wait();
if (receipt.status === 1) {
  dispatch(txConfirmed(tx.hash));
} else {
  dispatch(txFailed(tx.hash));
}

// ❌ Mal: enviar y olvidarse
await contract.transfer(...);
```

### 3. Actualizaciones optimistas con reversión

```tsx
// ✅ Bien: optimista con reversión
dispatch(client.util.updateQueryData(...));
try {
  await tx.wait();
  dispatch(client.util.invalidateTags(...));
} catch {
  dispatch(client.util.invalidateTags(...)); // Reversión
}

// ❌ Mal: sin reversión
dispatch(client.util.updateQueryData(...));
await tx.wait(); // ¿Y si esto falla?
```

### 4. Limpieza de event listeners

```tsx
// ✅ Bien: limpiar listeners
useEffect(() => {
  contract.on('Transfer', handler);
  return () => contract.off('Transfer', handler);
}, [contract]);

// ❌ Mal: fuga de memoria
useEffect(() => {
  contract.on('Transfer', handler);
}, [contract]);
```

## Próximos pasos

* Revisa [Pruebas y rendimiento](/docs/contributor/contributor-es/guias-para-colaboradores/estandares-de-ui/testing-and-performance/) para optimización
* Consulta [Patrones de Componentes](/docs/contributor/contributor-es/guias-para-colaboradores/estandares-de-ui/component-patterns/) para ejemplos de uso
* Entiende [RTK Query](/docs/contributor/contributor-es/guias-para-colaboradores/estandares-de-ui/rtk-query/) para la gestión de caché


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/contributor/contributor-es/guias-para-colaboradores/estandares-de-ui/web3-integration.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
