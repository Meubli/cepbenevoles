<script lang="ts">
	import { onMount } from 'svelte';
	import '#lib/styles/admin.css';
	import AdminNav from '#lib/AdminNav.svelte';
	import { api, ApiError } from '#lib/api';
	import type { LayoutProps } from './$types';

	let { children }: LayoutProps = $props();

	type SessionInfo = { email: string; is_superadmin: boolean };

	let session = $state<SessionInfo | null>(null);
	let loggingOut = $state(false);

	onMount(async () => {
		try {
			session = await api.get<SessionInfo>('/session');
		} catch (e) {
			// A 401 here already triggers api.ts's hard redirect to
			// /connexion; nothing left to show in that case.
			if (!(e instanceof ApiError && e.status === 401)) throw e;
		}
	});

	async function logout() {
		loggingOut = true;
		try {
			await api.post('/auth/logout');
		} finally {
			// Always land back on the login screen, even if the request
			// failed — staying on an admin screen with a possibly-dead
			// session is worse than a redundant redirect.
			window.location.href = '/connexion';
		}
	}
</script>

<div class="admin-shell">
	<div class="topbar">
		<div class="topbar-inner">
			<span class="brand">CEP·BÉNÉVOLES</span>
			{#if session}
				<div class="session-box">
					<span>{session.email}</span>
					{#if session.is_superadmin}
						<span class="role-badge">superadmin</span>
					{/if}
					<button class="btn-wf" disabled={loggingOut} onclick={logout}>Déconnexion</button>
				</div>
			{/if}
		</div>
	</div>
	<main class="admin-main">
		<div class="admin-layout">
			<AdminNav />
			<div class="panel wf">
				{@render children()}
			</div>
		</div>
	</main>
</div>
