import type { Context } from "./context";
import type { SiteRegistry } from "./resolve";

let next = 0;

/**
 * Registers `fn` as a site, as the server's expressions module would, and
 * returns expression data applying it, escaped for an attribute value or a
 * marker comment. For tests.
 */
export function site(fn: (cx: Context) => unknown): string {
	const id = `test${next++}`;
	globalThis.__topcoat_expressions ??= Object.create(null);
	(globalThis.__topcoat_expressions as SiteRegistry)[id] = fn;
	return JSON.stringify([id])
		.replaceAll("&", "&amp;")
		.replaceAll('"', "&quot;")
		.replaceAll("<", "&lt;")
		.replaceAll(">", "&gt;");
}
