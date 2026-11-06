// Grants CREDITS_SIGNER_ROLE on dcl-one's CreditsManager on Amoy (./lib/contracts.mts) to the testnet
// stack's credits signer, so the credits the stack signs are accepted on chain (README, "Credits"). Amoy
// only: the chain, the contract and the role are fixed here, and the script refuses any other chain.
//
//   node grant-credits-signer.mts --check            reads only: who would be granted, and whether it already is
//   node grant-credits-signer.mts                    sends grantRole from the contract's admin wallet
//
// The grantee is the address of the key in .cache/dev/credits-signer.key (or E2E_CREDITS_SIGNER_KEY_FILE),
// or E2E_CREDITS_SIGNER when set. The admin key, a wallet holding the role's admin role on that contract,
// is read from the file CREDITS_ADMIN_KEY_FILE names, or typed at a hidden prompt. No key is printed.
import { existsSync, readFileSync, statSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { createPublicClient, createWalletClient, getAddress, http, isAddress, keccak256, parseAbi, toHex, type Hex } from "viem";
import { privateKeyToAccount } from "viem/accounts";
import { polygonAmoy } from "viem/chains";
import { AMOY_CREDITS_MANAGER as CREDITS_MANAGER } from "./lib/contracts.mts";
import { DEFAULT_RPC } from "./lib/wallet.mts";

const ROLE = keccak256(toHex("CREDITS_SIGNER_ROLE"));
const ABI = parseAbi([
  "function CREDITS_SIGNER_ROLE() view returns (bytes32)",
  "function getRoleAdmin(bytes32) view returns (bytes32)",
  "function hasRole(bytes32, address) view returns (bool)",
  "function grantRole(bytes32, address)",
]);
const STACK_KEY = fileURLToPath(new URL("../../.cache/dev/credits-signer.key", import.meta.url));
const checkOnly = process.argv.includes("--check");

function readKey(path: string, what: string): Hex {
  const key = readFileSync(path, "utf8").trim();
  if (!/^0x[0-9a-fA-F]{64}$/.test(key)) throw new Error(`${what} (${path}) does not hold a 0x-prefixed 32-byte private key`);
  return key as Hex;
}

/** The address to grant: E2E_CREDITS_SIGNER, else the address of the stack's signer key file. */
function grantee(): Hex {
  const given = process.env.E2E_CREDITS_SIGNER;
  if (given) {
    if (!isAddress(given)) throw new Error("E2E_CREDITS_SIGNER is not an address");
    return getAddress(given);
  }
  const file = process.env.E2E_CREDITS_SIGNER_KEY_FILE ?? STACK_KEY;
  if (!existsSync(file)) throw new Error(`no signer to grant: set E2E_CREDITS_SIGNER, or put the stack's key in ${file}`);
  return privateKeyToAccount(readKey(file, "the signer key file")).address;
}

/** A line typed without echo. */
async function hiddenPrompt(question: string): Promise<string> {
  const stdin = process.stdin;
  if (!stdin.isTTY) throw new Error("no terminal to prompt on: set CREDITS_ADMIN_KEY_FILE");
  process.stdout.write(question);
  stdin.setRawMode(true);
  stdin.resume();
  stdin.setEncoding("utf8");
  let typed = "";
  return new Promise((resolve, reject) => {
    const onData = (chunk: string) => {
      for (const ch of chunk) {
        if (ch === "\r" || ch === "\n") {
          stdin.setRawMode(false);
          stdin.pause();
          stdin.off("data", onData);
          process.stdout.write("\n");
          return resolve(typed.trim());
        }
        if (ch === "\u0003") {
          stdin.setRawMode(false);
          stdin.pause();
          stdin.off("data", onData);
          process.stdout.write("\n");
          return reject(new Error("cancelled"));
        }
        if (ch === "\u007f") typed = typed.slice(0, -1);
        else typed += ch;
      }
    };
    stdin.on("data", onData);
  });
}

async function adminKey(): Promise<Hex> {
  const file = process.env.CREDITS_ADMIN_KEY_FILE;
  if (file) {
    // A regular file holding the contract's admin key should be private; a pipe (<(pbpaste)) has no such mode.
    const st = statSync(file);
    if (st.isFile() && st.mode & 0o077) console.warn(`Warning: ${file} is readable by other users; chmod 600 it, or delete it once the role is granted.`);
    return readKey(file, "CREDITS_ADMIN_KEY_FILE");
  }
  const typed = await hiddenPrompt("Private key of the CreditsManager admin on Amoy (hidden): ");
  const key = typed.startsWith("0x") ? typed : `0x${typed}`;
  if (!/^0x[0-9a-fA-F]{64}$/.test(key)) throw new Error("that is not a 32-byte private key");
  return key as Hex;
}

async function main(): Promise<void> {
  const transport = http(process.env.E2E_RPC_AMOY ?? DEFAULT_RPC[polygonAmoy.id]);
  const amoy = createPublicClient({ chain: polygonAmoy, transport });
  const chainId = await amoy.getChainId();
  if (chainId !== polygonAmoy.id) throw new Error(`the RPC answers chain ${chainId}, not Amoy (${polygonAmoy.id})`);
  const onChainRole = await amoy.readContract({ address: CREDITS_MANAGER, abi: ABI, functionName: "CREDITS_SIGNER_ROLE" });
  if (onChainRole !== ROLE) throw new Error(`the contract's CREDITS_SIGNER_ROLE is ${onChainRole}, not ${ROLE}`);
  const adminRole = await amoy.readContract({ address: CREDITS_MANAGER, abi: ABI, functionName: "getRoleAdmin", args: [ROLE] });

  const signer = grantee();
  console.log(`CreditsManager ${CREDITS_MANAGER} on Amoy`);
  console.log(`signer to grant: ${signer}`);
  if (await amoy.readContract({ address: CREDITS_MANAGER, abi: ABI, functionName: "hasRole", args: [ROLE, signer] })) {
    console.log("It already holds CREDITS_SIGNER_ROLE. Nothing to do.");
    return;
  }
  console.log("It does not hold CREDITS_SIGNER_ROLE yet.");
  if (checkOnly) return;

  const admin = privateKeyToAccount(await adminKey());
  console.log(`admin: ${admin.address}`);
  if (!(await amoy.readContract({ address: CREDITS_MANAGER, abi: ABI, functionName: "hasRole", args: [adminRole, admin.address] }))) {
    throw new Error(`${admin.address} does not hold the admin role (${adminRole}) of CREDITS_SIGNER_ROLE on this contract`);
  }
  // Simulated first, so a revert costs nothing and names its reason.
  const { request } = await amoy.simulateContract({ account: admin, address: CREDITS_MANAGER, abi: ABI, functionName: "grantRole", args: [ROLE, signer] });
  const wallet = createWalletClient({ account: admin, chain: polygonAmoy, transport });
  const hash = await wallet.writeContract(request);
  console.log(`sent: https://amoy.polygonscan.com/tx/${hash}`);
  const receipt = await amoy.waitForTransactionReceipt({ hash });
  if (receipt.status !== "success") throw new Error(`the transaction reverted in block ${receipt.blockNumber}`);
  const granted = await amoy.readContract({ address: CREDITS_MANAGER, abi: ABI, functionName: "hasRole", args: [ROLE, signer] });
  if (!granted) throw new Error("the transaction succeeded but the signer still does not hold the role");
  console.log(`Granted in block ${receipt.blockNumber}: ${signer} now holds CREDITS_SIGNER_ROLE.`);
}

main().catch((e) => {
  console.error(`\n${(e as Error).message}`);
  process.exitCode = 1;
});
