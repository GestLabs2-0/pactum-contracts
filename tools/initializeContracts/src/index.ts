import { parseArgs } from "./config";
import { createConnection } from "./connection";
import { initializePactumConfig } from "./programs/pactumConfig";
import { initializePactumCore } from "./programs/pactumCore";
import { initializePactumDisputes } from "./programs/pactumDisputes";
import { initializePactumEscrow } from "./programs/pactumEscrow";
import { initializePactumTreasury } from "./programs/pactumTreasury";
import { loadKeypair } from "./wallet";

async function main() {
	const args = parseArgs();

	const keypair = await loadKeypair(args.keypair);
	const connection = createConnection(args);

	switch (args.program) {
		case "all":
			await initializePactumConfig(connection, keypair, args);
			await initializePactumCore(connection, keypair);
			await initializePactumTreasury(connection, keypair);
			await initializePactumEscrow(connection, keypair);
			await initializePactumDisputes(connection, keypair, args);
			break;
		case "pactum-config":
		case "config":
			await initializePactumConfig(connection, keypair, args);
			break;
		case "pactum-core":
		case "core":
			await initializePactumCore(connection, keypair);
			break;
		case "pactum-treasury":
		case "treasury":
			await initializePactumTreasury(connection, keypair);
			break;
		case "pactum-escrow":
		case "escrow":
			await initializePactumEscrow(connection, keypair);
			break;
		case "pactum-disputes":
		case "disputes":
			await initializePactumDisputes(connection, keypair, args);
			break;
		default:
			throw new Error(`Programa no soportado: ${args.program}`);
	}
}

main().catch((err) => {
	console.error(err);
	process.exit(1);
});
