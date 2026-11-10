export type CreditLineRequest = {
  usdPriceCents: number;
  tradeId?: string;
  contractAddress?: string;
  itemId?: string;
  name?: string;
  source?: "website" | "client";
};

export type AuthorizedCredit = {
  id: string;
  amount: string;
  availableAmount: string;
  expiresAt: number;
  signature: string;
  contract: string;
};

export type CreditAuthorization = {
  credit: AuthorizedCredit;
  maxCreditedValue: string;
  usdCents: number;
  oracleRate: string;
};

export type CreditBatchRequest = { items: CreditLineRequest[]; source?: "website" | "client" };

export type CreditBatchLine = {
  usdCents: number;
  tradeId?: string;
  contractAddress?: string;
  itemId?: string;
  name?: string;
};

export type CreditBatchAuthorization = CreditAuthorization & { lines: CreditBatchLine[] };

export type CancelCreditsRequest = { salts: string[] };
export type CancelCreditsResult = { released: number; retired: number; spendableAtSeconds: number | null };

export type CreditSubmissionRequest = { salts: string[]; txHash: string };
export type CreditSubmissionResult = { recorded: number };

export type PackOrderRequest = { packId: string; timezone?: string; source?: string };
export type PackOrderCreated = { orderId: string; url: string };
export type PackOrderStatus =
  | { status: "processing" }
  | { status: "failed"; error: string }
  | { status: "credited"; creditsGranted: number; newBalance: number };
