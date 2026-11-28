import { findGlobalConfigPda } from "@GestLabs2-0/pactum-config";
import {
	fetchMaybeEscrowConfig,
	findEscrowConfigPda,
	getInitializeEscrowInstructionAsync,
} from "@GestLabs2-0/pactum-escrow";
import type { KeyPairSigner } from "@solana/kit";
import type { SolanaRpcType } from "../connection";
import { sendAndConfirmInstruction } from "../utils";

export async function initializePactumEscrow(
	connection: SolanaRpcType,
	payer: KeyPairSigner,
) {
	const [escrowConfigPda] = await findEscrowConfigPda();
	const existing = await fetchMaybeEscrowConfig(connection, escrowConfigPda);
	if (existing.exists) {
		console.log(
			`EscrowConfig already initialized at ${escrowConfigPda}, skipping.`,
		);
		return;
	}

	const [globalConfigPda] = await findGlobalConfigPda();
	const instruction = await getInitializeEscrowInstructionAsync({
		authority: payer,
		config: escrowConfigPda,
		globalConfig: globalConfigPda,
	});

	await sendAndConfirmInstruction(connection, payer, instruction);
}
