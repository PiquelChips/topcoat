// @vitest-environment happy-dom

import { afterEach, expect, it, vi } from "vitest";
import { site } from "../expression/testing";
import { flushEffects } from "../reactivity";
import { RUNTIME_HEADER } from "../render/request";
import { Runtime } from "../runtime";
import type { Event, F64, Procedure } from "../surrogate";
import type { Bool } from "../surrogate/bool";

type Log = { log: string[] };

const globals = globalThis as typeof globalThis & Log;

let runtime: Runtime | null = null;

afterEach(() => {
	runtime?.navigation.dispose();
	runtime?.page.dispose();
	runtime = null;
	vi.unstubAllGlobals();
	vi.restoreAllMocks();
	sessionStorage.clear();
	document.body.innerHTML = "";
});

const declaration = (id: string, value: number) =>
	`<!--::topcoat::signal({"t":"signal","id":"${id}","v":${value}})-->`;

/** A mount handler that records `name` in `globalThis.log`. */
const logs = (name: string) =>
	`data-topcoat-on:mount="${site(() => (e: Event) => globals.log.push(`${name}:${String(e.event_type)}`))}"`;

function start(body: string): Runtime {
	globals.log = [];
	document.body.innerHTML = body;
	runtime = new Runtime();
	runtime.start(document);
	return runtime;
}

/** Lets queued mount events, requests, and effects run. */
async function settle(): Promise<void> {
	for (let i = 0; i < 5; i += 1) {
		flushEffects();
		await new Promise((resolve) => setTimeout(resolve, 0));
	}
	flushEffects();
}

it("runs after hydration, once every signal in the document is declared", async () => {
	const rt = start(`
		<main data-topcoat-on:mount="${site((cx) => () => cx.signal("late").increment())}"></main>
		${declaration("late", 1)}
		<p><!--::topcoat::expr::start(${site((cx) => cx.signal("late").get())})-->1<!--::topcoat::expr::end--></p>
	`);
	const count = () => (rt.context.signal("late").get() as F64).dehydrate();

	// Hydration itself does not run the handler.
	expect(count()).toBe(1);
	await settle();
	expect(count()).toBe(2);
	expect(document.querySelector("p")?.textContent).toBe("2");
	await settle();
	expect(count()).toBe(2);
});

it("dispatches in document order without reaching ancestors' handlers", async () => {
	start(`<div ${logs("outer")}><span ${logs("inner")}></span></div>`);
	await settle();
	expect(globals.log).toEqual(["outer:mount", "inner:mount"]);
});

it("runs again for content that a server render replaces, but not outside it", async () => {
	const rt = start(`
		<p ${logs("page")}></p>
		<!--::topcoat::region::start(ab)-->
			<p ${logs("region")}></p>
		<!--::topcoat::region::end(ab)-->
	`);
	await settle();
	rt.page.applySwap("ab", `<p ${logs("swapped")}></p>`, null);
	await settle();
	expect(globals.log).toEqual(["page:mount", "region:mount", "swapped:mount"]);
});

it("skips content that is replaced before its handler runs", async () => {
	const rt = start(`
		<!--::topcoat::region::start(ab)-->
			<p ${logs("first")}></p>
		<!--::topcoat::region::end(ab)-->
	`);
	rt.page.applySwap("ab", `<p ${logs("second")}></p>`, null);
	await settle();
	expect(globals.log).toEqual(["second:mount"]);
});

it("reruns the page when a handler changes a signal the server read", async () => {
	const fetch = vi.fn(async () => new Promise<Response>(() => {}));
	vi.stubGlobal("fetch", fetch);
	start(`
		${declaration("a", 1)}
		<!--::topcoat::dep("a")-->
		<main data-topcoat-on:mount="${site((cx) => () => cx.signal("a").increment())}"></main>
	`);
	await settle();
	expect(fetch).toHaveBeenCalledTimes(1);
	expect(fetch).toHaveBeenCalledWith(
		expect.any(String),
		expect.objectContaining({
			headers: expect.objectContaining({ [RUNTIME_HEADER]: "true" }),
		}),
	);
});

it("formats times, shows a saved message, and resumes an interrupted operation on load", async () => {
	// The handlers of the runtime example's `mount` page, as its expressions
	// module defines them.
	const formatTimes = site(() => () => {
		for (const time of document.querySelectorAll<HTMLElement>("time[data-ms]"))
			time.textContent = new Date(Number(time.dataset.ms)).toLocaleString();
	});
	const showFlash = site((cx) => () => {
		const message = sessionStorage.getItem("flash") ?? "";
		sessionStorage.removeItem("flash");
		cx.signal("flash").set(cx.hydrate(message));
	});
	const resume = site((cx) => async () => {
		if (sessionStorage.getItem("moved") === null) return;
		const moved = cx.signal("moved");
		const moveOne = cx.hydrate({
			t: "Procedure",
			path: "/procedures/move_one",
		}) as Procedure<[unknown], unknown>;
		moved.set(cx.hydrate(Number(sessionStorage.getItem("moved"))));
		while (
			((moved.get() as F64).lt(cx.hydrate(5.0) as F64) as Bool).dehydrate()
		) {
			const next = await moveOne.call(moved.get());
			sessionStorage.setItem("moved", String(next));
			moved.set(next);
		}
		sessionStorage.removeItem("moved");
	});

	const moves: unknown[] = [];
	vi.stubGlobal("fetch", async (_url: string, init: RequestInit) => {
		const [moved] = JSON.parse(String(init.body)) as [number];
		moves.push(moved);
		return new Response(JSON.stringify(moved + 1));
	});
	sessionStorage.setItem("flash", "username is now ada");
	sessionStorage.setItem("moved", "3");
	const ms = Date.UTC(2026, 9, 20, 12, 30);
	const rt = start(`
		<!--::topcoat::signal({"t":"signal","id":"flash","v":""})-->
		${declaration("moved", 0)}
		<section data-topcoat-on:mount="${formatTimes}"><time data-ms="${ms}">${ms}</time></section>
		<section data-topcoat-on:mount="${showFlash}"></section>
		<section data-topcoat-on:mount="${resume}"></section>
	`);
	await settle();

	expect(document.querySelector("time")?.textContent).toBe(
		new Date(ms).toLocaleString(),
	);
	expect(String(rt.context.signal("flash").get())).toBe("username is now ada");
	expect(sessionStorage.getItem("flash")).toBe(null);
	// The operation continues from the saved progress instead of starting over.
	expect(moves).toEqual([3, 4]);
	expect((rt.context.signal("moved").get() as F64).dehydrate()).toBe(5);
	expect(sessionStorage.getItem("moved")).toBe(null);
});

it("does not run handlers when hydration fails part-way", async () => {
	globals.log = [];
	document.body.innerHTML = `
		<p ${logs("before")}></p>
		<p><!--::topcoat::expr::start(${site((cx) => cx.signal("missing").get())})--><!--::topcoat::expr::end--></p>
	`;
	runtime = new Runtime();
	expect(() => runtime?.start(document)).toThrow();
	await settle();
	// A failed navigation loads the page again, which would run them twice.
	expect(globals.log).toEqual([]);
});
