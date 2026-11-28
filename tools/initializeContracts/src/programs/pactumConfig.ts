import {
	fetchMaybeGlobalConfig,
	findGlobalConfigPda,
	getInitializeConfigInstructionAsync,
} from "@GestLabs2-0/pactum-config";
import { address, type KeyPairSigner } from "@solana/kit";
import type { CLIArgs } from "../config";
import type { SolanaRpcType } from "../connection";
import { sendAndConfirmInstruction } from "../utils";

const SYSTEM_PROGRAM_DEFAULT = "11111111111111111111111111111111";

export async function initializePactumConfig(
	connection: SolanaRpcType,
	payer: KeyPairSigner,
	args: CLIArgs,
) {
	if (!args.mintAddress) {
		throw new Error("Debes pasar --mint-address");
	}
	const [configPda] = await findGlobalConfigPda();
	const existing = await fetchMaybeGlobalConfig(connection, configPda);
	if (existing.exists) {
		console.log(`GlobalConfig already initialized at ${configPda}, skipping.`);
		return;
	}

	const instruction = await getInitializeConfigInstructionAsync({
		authority: payer,
		globalConfig: configPda,
		usdcMint: address(args.mintAddress),
		protocolFeeBps: args.protocolFeeBps ?? 100,
		maxProtocolFee: args.maxProtocolFee ?? 0n,
		disputeEscalationFee: args.disputeEscalationFee ?? 0n,
		sasProgram: address(args.sasProgram ?? SYSTEM_PROGRAM_DEFAULT),
		sasCredential: address(args.sasCredential ?? SYSTEM_PROGRAM_DEFAULT),
	});

	await sendAndConfirmInstruction(connection, payer, instruction);
}
