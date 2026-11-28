export type CLIArgs = {
	keypair: string;
	cluster?: string;
	rpcUrl?: string;
	program: string;
	mintAddress?: string;
	protocolFeeBps?: number;
	maxProtocolFee?: bigint;
	disputeEscalationFee?: bigint;
	sasProgram?: string;
	sasCredential?: string;
	p2pWindowSecs?: number;
	voteWindowSecs?: number;
	revealWindowSecs?: number;
	arbiterSlaSlashBps?: number;
	noFaultFeeBps?: number;
};

export function parseArgs(): CLIArgs {
	const args = process.argv.slice(2);

	const getArg = (name: string) => {
		const index = args.indexOf(name);
		if (index === -1) return undefined;
		return args[index + 1];
	};

	const keypair = getArg("--keypair");
	const program = getArg("--program") ?? "all";

	if (!keypair) throw new Error("Debes pasar --keypair");

	const protocolFeeBps = getArg("--protocol-fee-bps") ?? getArg("--fee-bps");
	const maxProtocolFee = getArg("--max-protocol-fee");
	const disputeEscalationFee = getArg("--dispute-escalation-fee");
	const p2pWindowSecs = getArg("--p2p-window-secs");
	const voteWindowSecs = getArg("--vote-window-secs");
	const revealWindowSecs = getArg("--reveal-window-secs");
	const arbiterSlaSlashBps = getArg("--arbiter-sla-slash-bps");
	const noFaultFeeBps = getArg("--no-fault-fee-bps");

	return {
		keypair,
		program,
		cluster: getArg("--cluster"),
		rpcUrl: getArg("--rpc-url"),
		mintAddress: getArg("--mint-address"),
		protocolFeeBps: protocolFeeBps ? parseInt(protocolFeeBps, 10) : undefined,
		maxProtocolFee: maxProtocolFee ? BigInt(maxProtocolFee) : undefined,
		disputeEscalationFee: disputeEscalationFee
			? BigInt(disputeEscalationFee)
			: undefined,
		sasProgram: getArg("--sas-program"),
		sasCredential: getArg("--sas-credential"),
		p2pWindowSecs: p2pWindowSecs ? parseInt(p2pWindowSecs, 10) : undefined,
		voteWindowSecs: voteWindowSecs ? parseInt(voteWindowSecs, 10) : undefined,
		revealWindowSecs: revealWindowSecs
			? parseInt(revealWindowSecs, 10)
			: undefined,
		arbiterSlaSlashBps: arbiterSlaSlashBps
			? parseInt(arbiterSlaSlashBps, 10)
			: undefined,
		noFaultFeeBps: noFaultFeeBps ? parseInt(noFaultFeeBps, 10) : undefined,
	};
}
