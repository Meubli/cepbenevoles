// Live-refresh websocket client (technical-specs.md §10, generalized from
// Planning-only to every admin screen). Push-only: the server sends a bare
// resource tag (e.g. "commissions") whenever something changes server-side;
// this module just dispatches that tag to whichever page registered
// interest in it, so the page can refetch. No data ever travels over the
// socket itself — only `api.ts` does real reads/writes.
import { browser } from '$app/env';

type Listener = { tags: string[]; cb: () => void };

const listeners = new Set<Listener>();

// A random id for this browser tab, sent as `X-Client-Id` on every API
// request (api.ts) and echoed back by the server as the live event's
// `origin`. Lets this tab skip refetching on its own echo — it already
// refetched locally right after its own mutation — while every other tab
// (different id) still refetches normally. Not an identity: random per tab,
// unrelated to the logged-in admin.
export const clientId: string = (() => {
	try {
		return crypto.randomUUID();
	} catch {
		return Math.random().toString(36).slice(2);
	}
})();

let socket: WebSocket | null = null;
let reconnectDelayMs = 1000;
const MAX_RECONNECT_DELAY_MS = 15000;

function dispatch(tag: string) {
	for (const listener of listeners) {
		if (tag === '*' || listener.tags.includes(tag)) {
			listener.cb();
		}
	}
}

function connect() {
	if (!browser) return;

	const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
	socket = new WebSocket(`${protocol}//${window.location.host}/api/ws`);

	socket.addEventListener('open', () => {
		reconnectDelayMs = 1000;
	});

	socket.addEventListener('message', (event) => {
		if (typeof event.data !== 'string') return;
		let parsed: unknown;
		try {
			parsed = JSON.parse(event.data);
		} catch {
			return;
		}
		if (
			typeof parsed !== 'object' ||
			parsed === null ||
			!('tag' in parsed) ||
			typeof parsed.tag !== 'string'
		) {
			return;
		}
		const origin = 'origin' in parsed ? parsed.origin : null;
		// Skip this tab's own echo (see `clientId` above) — it already
		// refetched locally right after making the change.
		if (origin === clientId) return;
		dispatch(parsed.tag);
	});

	socket.addEventListener('close', () => {
		socket = null;
		// A gap in the connection means missed events — refetch everything
		// once reconnected rather than trying to track what was missed.
		setTimeout(() => {
			connect();
			dispatch('*');
		}, reconnectDelayMs);
		reconnectDelayMs = Math.min(reconnectDelayMs * 2, MAX_RECONNECT_DELAY_MS);
	});

	// A network drop surfaces as both an error and a close event; letting
	// close alone drive reconnection avoids scheduling it twice.
	socket.addEventListener('error', () => {
		socket?.close();
	});
}

/**
 * Registers `cb` to run whenever one of `tags` changes (or on a full
 * resync, e.g. after a reconnect). Opens the shared socket on first use.
 * Returns an unsubscribe function — call it from the page's `onMount`
 * cleanup (or `onDestroy`) so a closed page doesn't keep refetching.
 */
export function onLive(tags: string[], cb: () => void): () => void {
	const listener: Listener = { tags, cb };
	listeners.add(listener);

	if (browser && !socket) connect();

	return () => {
		listeners.delete(listener);
	};
}
