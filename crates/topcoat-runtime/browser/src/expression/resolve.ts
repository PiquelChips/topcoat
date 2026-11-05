import type { Context } from "./context";

/** A runtime expression, computed from the runtime context. */
export type Expression<T = unknown> = (cx: Context) => T;

/**
 * A site's function from the expressions module: one parameter per hole,
 * each an expression its body calls where it needs the hole's value.
 */
export type Site = (cx: Context, ...holes: Expression[]) => unknown;

/**
 * Sites by id. The server's expressions module defines them before the
 * runtime starts; nothing else does, so markup cannot add code.
 */
export type SiteRegistry = Record<string, Site>;

declare global {
	var __topcoat_expressions: SiteRegistry | undefined;
}

/**
 * Resolves an expression the server rendered as data: `["<id>", ...holes]`
 * applies a site to its holes, `{"v": value}` hydrates a captured value.
 *
 * Expression data never contains source. An id missing from the
 * expressions module fails instead of being evaluated.
 */
export function resolve<T = unknown>(
	data: unknown,
	label: string,
): Expression<T> {
	return resolveNode(data, label) as Expression<T>;
}

/** Parses expression data from an attribute value, then resolves it. */
export function resolveJson<T = unknown>(
	json: string,
	label: string,
): Expression<T> {
	let data: unknown;
	try {
		data = JSON.parse(json);
	} catch (cause) {
		throw new Error(`Invalid expression data in ${label}`, { cause });
	}
	return resolve<T>(data, label);
}

function resolveNode(data: unknown, label: string): Expression {
	if (Array.isArray(data)) {
		const [id, ...holes] = data as unknown[];
		if (typeof id !== "string") {
			throw new Error(`Invalid expression site in ${label}`);
		}
		const site = lookup(id, label);
		const resolved = holes.map((hole) => resolveNode(hole, label));
		return (cx) => site(cx, ...resolved);
	}
	if (typeof data === "object" && data !== null && "v" in data) {
		const { v } = data;
		return (cx) => cx.hydrate(v);
	}
	throw new Error(`Invalid expression data in ${label}`);
}

function lookup(id: string, label: string): Site {
	const sites = globalThis.__topcoat_expressions;
	if (sites === undefined) {
		throw new Error(
			`Cannot resolve ${label}: the expressions module is not loaded. Load it before the runtime script.`,
		);
	}
	const site = Object.hasOwn(sites, id) ? sites[id] : undefined;
	if (typeof site !== "function") {
		throw new Error(
			`Unknown expression site ${id} in ${label}. The page and the expressions module come from different builds.`,
		);
	}
	return site;
}
