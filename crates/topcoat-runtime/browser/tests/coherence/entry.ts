import { type Expression, resolveJson } from "../../src/expression/resolve";
import { SignalRegistry } from "../../src/signal-registry";
import { Panic } from "../../src/surrogate/panic";
import { FixtureContext } from "./fixture";
import { observe } from "./observe";

export { setServerUsizeBits } from "../../src/surrogate/integer";

/**
 * Resolves production-generated expression data against the expressions
 * module loaded before this script, then runs it in an independent runtime
 * context.
 */
export function execute(data: string, invoke: boolean): string {
	const cx = new FixtureContext(new SignalRegistry());
	let value: unknown;
	try {
		value = resolve(data)(cx);
		if (invoke) {
			if (typeof value !== "function") {
				throw new Error("Expected a compiled closure");
			}
			value = value();
		}
	} catch (error) {
		if (error instanceof Panic) {
			return JSON.stringify({ kind: "Panic", value: error.message });
		}
		if (error instanceof Error) {
			return JSON.stringify({ kind: "Exception", value: error.toString() });
		}
		throw error;
	}
	return JSON.stringify({ kind: "Return", value: observe(value) });
}

/** Invokes an async closure and observes its completed value. */
export async function executeAsync(data: string): Promise<string> {
	const cx = new FixtureContext(new SignalRegistry());
	let value: unknown;
	try {
		const run: unknown = resolve(data)(cx);
		if (typeof run !== "function") {
			throw new Error("Expected a compiled async closure");
		}
		const pending: unknown = run();
		if (!(pending instanceof Promise)) {
			throw new Error("Compiled async closure must return a promise");
		}
		value = await pending;
	} catch (error) {
		if (error instanceof Panic) {
			return JSON.stringify({ kind: "Panic", value: error.message });
		}
		if (error instanceof Error) {
			return JSON.stringify({ kind: "Exception", value: error.toString() });
		}
		throw error;
	}
	return JSON.stringify({ kind: "Return", value: observe(value) });
}

function resolve(data: string): Expression {
	return resolveJson(data, "coherence case");
}
