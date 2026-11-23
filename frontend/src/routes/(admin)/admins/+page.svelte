<script lang="ts">
	import { onMount } from 'svelte';
	import { api, ApiError } from '#lib/api';
	import { onLive } from '#lib/live';

	type Admin = {
		id: string;
		nom: string | null;
		prenom: string | null;
		email: string;
		is_superadmin: boolean;
		created_at: string;
	};

	type FormState = { nom: string; prenom: string; email: string; is_superadmin: boolean };

	function emptyForm(): FormState {
		return { nom: '', prenom: '', email: '', is_superadmin: false };
	}

	let admins = $state<Admin[]>([]);
	let selfEmail = $state<string | null>(null);
	let loading = $state(true);
	let error = $state<string | null>(null);

	let creating = $state(false);
	let createForm = $state<FormState>(emptyForm());
	let createFieldErrors = $state<Record<string, string>>({});

	// See (admin)/commissions/+page.svelte's refresh() for why this is split
	// from load(): a live-refresh tick must not toggle `loading`.
	async function refresh() {
		try {
			const [adminsResult, session] = await Promise.all([
				api.get<Admin[]>('/admins'),
				api.get<{ email: string }>('/session')
			]);
			admins = adminsResult;
			selfEmail = session.email;
		} catch (e) {
			error = e instanceof ApiError ? e.message : 'Erreur inattendue';
		}
	}

	async function load() {
		loading = true;
		error = null;
		await refresh();
		loading = false;
	}

	onMount(() => {
		void load();
		return onLive(['admins'], () => void refresh());
	});

	function displayName(a: Admin): string {
		return a.nom && a.prenom ? `${a.nom} ${a.prenom}` : a.email;
	}

	async function promote(a: Admin) {
		error = null;
		try {
			await api.patch(`/admins/${a.id}`, { is_superadmin: true });
			await refresh();
		} catch (e) {
			error = e instanceof ApiError ? e.message : 'Erreur inattendue';
		}
	}

	async function remove(a: Admin) {
		error = null;
		try {
			await api.delete(`/admins/${a.id}`);
			await refresh();
		} catch (e) {
			error = e instanceof ApiError ? e.message : 'Erreur inattendue';
		}
	}

	async function createAdmin() {
		error = null;
		createFieldErrors = {};
		if (!createForm.nom.trim() || !createForm.prenom.trim() || !createForm.email.trim()) {
			error = 'Nom, prénom et email sont obligatoires.';
			return;
		}
		try {
			await api.post('/admins', createForm);
			creating = false;
			createForm = emptyForm();
			await refresh();
		} catch (e) {
			if (e instanceof ApiError && e.field) {
				createFieldErrors = { [e.field]: e.message };
			} else {
				error = e instanceof ApiError ? e.message : 'Erreur inattendue';
			}
		}
	}
</script>

<div class="panel wf">
	<div class="toolbar-row" style="margin-top:0">
		<h3>Admins ({admins.length})</h3>
		{#if !creating}
			<button class="btn-wf primary" onclick={() => (creating = true)}>+ Ajouter un admin</button>
		{/if}
	</div>
	<p class="note">Ajout/suppression d'admins et promotion superadmin. Réservé aux superadmins.</p>

	{#if error}
		<p role="alert" class="error-note">{error}</p>
	{/if}

	{#if loading}
		<p>Chargement…</p>
	{:else}
		{#each admins as a (a.id)}
			<div class="comm-row" data-testid="admin-row">
				<div class="comm-main">
					<span style="font-weight:600">{displayName(a)}</span>
					<span class="wf-label">{a.email}</span>
				</div>
				<div class="comm-meta">
					{#if a.is_superadmin}
						<span class="role-badge">superadmin</span>
					{:else}
						<span class="pill">admin</span>
					{/if}
					{#if a.email === selfEmail}
						<span class="pill">connecté</span>
					{:else}
						{#if !a.is_superadmin}
							<button class="btn-wf" onclick={() => promote(a)}>Promouvoir superadmin</button>
						{/if}
						<button class="btn-wf danger" onclick={() => remove(a)}>Retirer</button>
					{/if}
				</div>
			</div>
		{/each}

		{#if creating}
			<div style="margin-top:22px; padding-top:16px; border-top:1px solid var(--line)">
				<h3>Ajouter un admin</h3>
				<div class="field-grid">
					<div class="field wf">
						<label>
							Nom
							<input
								class="input-wf"
								bind:value={createForm.nom}
								maxlength="100"
								required
								oninput={() => (createFieldErrors = {})}
							/>
						</label>
						{#if createFieldErrors.nom}
							<p class="error-note" style="margin:4px 0 0">{createFieldErrors.nom}</p>
						{/if}
					</div>
					<div class="field wf">
						<label>
							Prénom
							<input class="input-wf" bind:value={createForm.prenom} maxlength="100" required />
						</label>
					</div>
					<div class="field wf">
						<label>
							Email
							<input
								class="input-wf"
								type="email"
								bind:value={createForm.email}
								maxlength="255"
								required
								oninput={() => (createFieldErrors = {})}
							/>
						</label>
						{#if createFieldErrors.email}
							<p class="error-note" style="margin:4px 0 0">{createFieldErrors.email}</p>
						{/if}
					</div>
					<div class="field wf" style="flex-direction:row; align-items:center; gap:6px">
						<label style="flex-direction:row; align-items:center; gap:6px; text-transform:none">
							<input type="checkbox" bind:checked={createForm.is_superadmin} />
							Superadmin
						</label>
					</div>
				</div>
				<button class="btn-wf primary" style="margin-top:12px" onclick={createAdmin}>
					Créer l'admin
				</button>
				<button class="btn-wf" style="margin-top:12px" onclick={() => (creating = false)}>
					Annuler
				</button>
				<p class="note" style="margin-top:8px">
					Pas de mot de passe à définir : le nouvel admin se connecte directement via son email +
					code reçu (écran Connexion).
				</p>
			</div>
		{/if}
	{/if}
</div>
