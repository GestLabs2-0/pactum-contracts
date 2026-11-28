import { findGlobalConfigPda } from "@GestLabs2-0/pactum-config";
import {
	fetchMaybeDisputeConfig,
	findDisputeConfigPda,
	getInitializeDisputeConfigInstructionAsync,
} from "@GestLabs2-0/pactum-disputes";
import type { KeyPairSigner } from "@solana/kit";
import type { CLIArgs } from "../config";
import type { SolanaRpcType } from "../connection";
import { sendAndConfirmInstruction } from "../utils";

export async function initializePactumDisputes(
	connection: SolanaRpcType,
	payer: KeyPairSigner,
	args: CLIArgs,
) {
	const [disputeConfigPda] = await findDisputeConfigPda();
	const existing = await fetchMaybeDisputeConfig(connection, disputeConfigPda);
	if (existing.exists) {
		console.log(
			`DisputeConfig already initialized at ${disputeConfigPda}, skipping.`,
		);
		return;
	}

	const [globalConfigPda] = await findGlobalConfigPda();
	const instruction = await getInitializeDisputeConfigInstructionAsync({
		disputeConfig: disputeConfigPda,
		globalConfig: globalConfigPda,
		authority: payer,
		p2pWindowSecs: args.p2pWindowSecs ?? 172_800,
		voteWindowSecs: args.voteWindowSecs ?? 172_800,
		revealWindowSecs: args.revealWindowSecs ?? 86_400,
		arbiterSlaSlashBps: args.arbiterSlaSlashBps ?? 500,
		noFaultFeeBps: args.noFaultFeeBps ?? 5_000,
	});

	await sendAndConfirmInstruction(connection, payer, instruction);
}
