import { createAction } from '@reduxjs/toolkit';

import { AuthServerProvider } from '/@/lib/auth';

/** Dispatched whenever the signed-in wallet changes; every wallet-scoped slice resets on it. */
export const accountChanged = createAction('account/changed');

/** `createAsyncThunk` condition: a wallet-scoped thunk runs only for the signed-in account. */
export const isSignedInAccount = <Arg extends { address: string }>({ address }: Arg) =>
  AuthServerProvider.getAccount()?.toLowerCase() === address.toLowerCase();
