import { findGlobalConfigPda } from "@GestLabs2-0/pactum-config";
import {
	fetchMaybeCoreConfig,
	findCoreConfigPda,
	getInitializeCoreConfigInstructionAsync,
} from "@GestLabs2-0/pactum-core";
import type { KeyPairSigner } from "@solana/kit";
import type { SolanaRpcType } from "../connection";
import { sendAndConfirmInstruction } from "../utils";

export async function initializePactumCore(
	connection: SolanaRpcType,
	payer: KeyPairSigner,
) {
	const [coreConfigPda] = await findCoreConfigPda();
	const existing = await fetchMaybeCoreConfig(connection, coreConfigPda);
	if (existing.exists) {
		console.log(`CoreConfig already initialized at ${coreConfigPda}, skipping.`);
		return;
	}

	const [globalConfigPda] = await findGlobalConfigPda();
	const instruction = await getInitializeCoreConfigInstructionAsync({
		coreConfig: coreConfigPda,
		globalConfig: globalConfigPda,
		authority: payer,
	});

	await sendAndConfirmInstruction(connection, payer, instruction);
}
