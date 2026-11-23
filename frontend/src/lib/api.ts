// Thin fetch wrapper used by every admin screen (technical-specs.md §11 for
// correlation ids, specs.md §11 for the session cookie).
import { browser } from '$app/env';
import { clientId } from '#lib/live';

export class ApiError extends Error {
	status: number;
	correlationId?: string;
	// Names which input field the error applies to (e.g. "email"), when the
	// backend's error payload says so — lets a form show the message right
	// under that field instead of only in the screen's top-level banner.
	field?: string;

	constructor(status: number, correlationId?: string, message?: string, field?: string) {
		super(message ?? `Erreur ${status}`);
		this.status = status;
		this.correlationId = correlationId;
		this.field = field;
	}
}

type RequestOptions = RequestInit & {
	// The login screen itself calls /auth/verify-code and /session, both of
	// which answer 401 on a wrong code or no session yet — expected
	// outcomes there, not "the session just died" (which is what the
	// redirect below is for everywhere else).
	skipAuthRedirect?: boolean;
};

async function request<T>(path: string, init?: RequestOptions): Promise<T> {
	const response = await fetch(`/api${path}`, {
		credentials: 'include',
		headers: {
			'Content-Type': 'application/json',
			// Relayed back as the live event's `origin` (technical-specs.md §10)
			// so this tab can skip refetching on the echo of its own change.
			'X-Client-Id': clientId,
			...(init?.headers ?? {})
		},
		...init
	});

	if (response.status === 401 && !init?.skipAuthRedirect) {
		// A full navigation, not goto(): a hard redirect also clears any
		// in-memory state, which is the right behavior for "the session
		// just died" anyway.
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
		const bodyField =
			body && typeof body === 'object' && 'field' in body && typeof body.field === 'string'
				? body.field
				: undefined;
		const message =
			bodyError ??
			(response.status >= 500 ? 'Une erreur technique est survenue.' : `Erreur ${response.status}`);
		throw new ApiError(response.status, correlationId, message, bodyField);
	}

	return body as T;
}

export const api = {
	get: <T>(path: string, options?: { skipAuthRedirect?: boolean }) => request<T>(path, options),
	post: <T>(path: string, body?: unknown, options?: { skipAuthRedirect?: boolean }) =>
		request<T>(path, {
			...options,
			method: 'POST',
			body: body !== undefined ? JSON.stringify(body) : undefined
		}),
	patch: <T>(path: string, body?: unknown) =>
		request<T>(path, {
			method: 'PATCH',
			body: body !== undefined ? JSON.stringify(body) : undefined
		}),
	delete: <T>(path: string) => request<T>(path, { method: 'DELETE' })
};
