import {
	appendTransactionMessageInstructions,
	assertIsFullySignedTransaction,
	assertIsSendableTransaction,
	assertIsSignature,
	assertIsTransactionWithinSizeLimit,
	compileTransaction,
	createTransactionMessage,
	getBase64EncodedWireTransaction,
	type Instruction,
	type KeyPairSigner,
	pipe,
	setTransactionMessageFeePayerSigner,
	setTransactionMessageLifetimeUsingBlockhash,
} from "@solana/kit";
import type { SolanaRpcType } from "./connection";

export async function confirmTx(connection: SolanaRpcType, signature: string) {
	assertIsSignature(signature);
	for (let i = 0; i < 20; i++) {
		const res = await connection.getSignatureStatuses([signature]).send();

		const status = res.value[0];

		if (status) {
			if (status.err) {
				throw new Error(`Transaction failed: ${JSON.stringify(status.err)}`);
			}

			if (
				status.confirmationStatus === "confirmed" ||
				status.confirmationStatus === "finalized"
			) {
				console.log("✅ Transaction confirmed:", status.confirmationStatus);
				return;
			}
		}

		await new Promise((r) => setTimeout(r, 500));
	}

	throw new Error("Transaction not confirmed in time");
}

export async function sendAndConfirmInstruction(
	connection: SolanaRpcType,
	payer: KeyPairSigner,
	instruction: Instruction,
) {
	const { value: latestBlockhash } = await connection
		.getLatestBlockhash()
		.send();

	const transaction = pipe(
		createTransactionMessage({ version: 0 }),
		(tx) => setTransactionMessageFeePayerSigner(payer, tx),
		(tx) => setTransactionMessageLifetimeUsingBlockhash(latestBlockhash, tx),
		(tx) => appendTransactionMessageInstructions([instruction], tx),
		(tx) => compileTransaction(tx),
	);

	assertIsTransactionWithinSizeLimit(transaction);

	const signatures = await payer.signTransactions([transaction]);
	const signedTx = {
		...transaction,
		signatures: {
			...transaction.signatures,
			...signatures[0],
		},
	};
	assertIsFullySignedTransaction(signedTx);
	assertIsSendableTransaction(signedTx);

	const base64EncodedTx = getBase64EncodedWireTransaction(signedTx);

	const signature = await connection
		.sendTransaction(base64EncodedTx, {
			encoding: "base64",
			maxRetries: 5n,
			preflightCommitment: "confirmed",
		})
		.send();

	console.log("Transaction signature:", signature);
	await confirmTx(connection, signature);
}
