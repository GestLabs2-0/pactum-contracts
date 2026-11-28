import { findGlobalConfigPda } from "@GestLabs2-0/pactum-config";
import {
	fetchMaybeTreasuryConfig,
	findConfigPda,
	getInitializeTreasuryInstructionAsync,
} from "@GestLabs2-0/pactum-treasury";
import type { KeyPairSigner } from "@solana/kit";
import type { SolanaRpcType } from "../connection";
import { sendAndConfirmInstruction } from "../utils";

export async function initializePactumTreasury(
	connection: SolanaRpcType,
	payer: KeyPairSigner,
) {
	const [configPda] = await findConfigPda();
	const existing = await fetchMaybeTreasuryConfig(connection, configPda);
	if (existing.exists) {
		console.log(`TreasuryConfig already initialized at ${configPda}, skipping.`);
		return;
	}

	const [globalConfigPda] = await findGlobalConfigPda();
	const instruction = await getInitializeTreasuryInstructionAsync({
		authority: payer,
		config: configPda,
		globalConfig: globalConfigPda,
	});

	await sendAndConfirmInstruction(connection, payer, instruction);
}
