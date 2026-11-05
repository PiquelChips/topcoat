import { afterEach, expect, it } from "vitest";

import { Effect, flushEffects } from "../reactivity";
import { SignalRegistry } from "../signal-registry";
import { type Bool, F64 } from "../surrogate";
import { Context } from "./context";
import { type Expression, resolve, resolveJson, type Site } from "./resolve";

const previous = globalThis.__topcoat_expressions;

afterEach(() => {
	globalThis.__topcoat_expressions = previous;
});

function register(sites: Record<string, Site>): void {
	globalThis.__topcoat_expressions = Object.assign(Object.create(null), sites);
}

function value(expression: Expression, cx: Context): number {
	return (expression(cx) as F64).dehydrate();
}

it("applies a site to holes it calls in place, at every use", () => {
	const calls: string[] = [];
	register({
		sum: (cx, $0, $1) => {
			calls.push("body");
			return ($0?.(cx) as F64).add($0?.(cx) as F64).add($1?.(cx) as F64);
		},
		two: (cx) => {
			calls.push("hole");
			return cx.hydrate(2);
		},
	});
	const cx = new Context(new SignalRegistry());

	const expression = resolveJson('["sum",["two"],{"v":3}]', "test");
	expect(calls).toEqual([]);
	expect(value(expression, cx)).toBe(7);
	expect(calls).toEqual(["body", "hole", "hole"]);
});

it("leaves a hole uncalled on a branch that does not use it", () => {
	register({
		choose: (cx, $0, $1) =>
			(cx.hydrate(true) as Bool).dehydrate() ? $0?.(cx) : $1?.(cx),
		fail: () => {
			throw new Error("untaken branch evaluated");
		},
	});
	const cx = new Context(new SignalRegistry());

	expect(value(resolve(["choose", { v: 1 }, ["fail"]], "test"), cx)).toBe(1);
});

it("tracks the signals a hole reads in the expression using it", () => {
	register({
		read: (cx) => cx.signal("a").get(),
		double: (cx, $0) => ($0?.(cx) as F64).mul(cx.hydrate(2) as F64),
	});
	const registry = new SignalRegistry();
	registry.insert("a", new F64(1));
	const cx = new Context(registry);
	const expression = resolve(["double", ["read"]], "test");

	const seen: number[] = [];
	const effect = new Effect(() => seen.push(value(expression, cx)));
	effect.run();
	cx.signal("a").set(new F64(4));
	flushEffects();
	effect.dispose();
	expect(seen).toEqual([2, 8]);
});

it("fails with the id of a site missing from the expressions module", () => {
	register({});
	expect(() => resolve(["0123456789abcdef"], "event @click")).toThrow(
		"Unknown expression site 0123456789abcdef in event @click",
	);
	globalThis.__topcoat_expressions = undefined;
	expect(() => resolve(["0123456789abcdef"], "binding :value")).toThrow(
		"the expressions module is not loaded",
	);
});

it("rejects data that is not a site or a captured value", () => {
	register({});
	for (const data of ["() => alert(1)", 1, null, [1], {}]) {
		expect(() => resolve(data, "test")).toThrow("Invalid expression");
	}
	expect(() => resolveJson("alert(1)", "test")).toThrow(
		"Invalid expression data in test",
	);
});
