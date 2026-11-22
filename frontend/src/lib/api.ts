// Thin fetch wrapper used by every admin screen (technical-specs.md §11 for
// correlation ids, specs.md §11 for the session cookie).
import { browser } from '$app/env';

export class ApiError extends Error {
	status: number;
	correlationId?: string;

	constructor(status: number, correlationId?: string, message?: string) {
		super(message ?? `Erreur ${status}`);
		this.status = status;
		this.correlationId = correlationId;
	}
}

async function request<T>(path: string, init?: RequestInit): Promise<T> {
	const response = await fetch(`/api${path}`, {
		credentials: 'include',
		headers: { 'Content-Type': 'application/json', ...(init?.headers ?? {}) },
		...init
	});

	if (response.status === 401) {
		// A full navigation, not goto(): the login screen doesn't exist as a
		// SvelteKit route yet (specs.md §0, planned next), and a hard
		// redirect also clears any in-memory state, which is the right
		// behavior for "the session just died" anyway.
		if (browser) window.location.href = '/connexion';
		throw new ApiError(401, undefined, 'Session expirée, reconnexion nécessaire.');
	}

	let body: unknown;
	try {
		body = await response.json();
	} catch {
		body = undefined;
	}

	if (!response.ok) {
		const correlationId = response.headers.get('x-correlation-id') ?? undefined;
		const bodyError =
			body && typeof body === 'object' && 'error' in body && typeof body.error === 'string'
				? body.error
				: undefined;
		const message =
			bodyError ??
			(response.status >= 500 ? 'Une erreur technique est survenue.' : `Erreur ${response.status}`);
		throw new ApiError(response.status, correlationId, message);
	}

	return body as T;
}

export const api = {
	get: <T>(path: string) => request<T>(path),
	post: <T>(path: string, body?: unknown) =>
		request<T>(path, {
			method: 'POST',
			body: body !== undefined ? JSON.stringify(body) : undefined
		}),
	patch: <T>(path: string, body?: unknown) =>
		request<T>(path, {
			method: 'PATCH',
			body: body !== undefined ? JSON.stringify(body) : undefined
		})
};
