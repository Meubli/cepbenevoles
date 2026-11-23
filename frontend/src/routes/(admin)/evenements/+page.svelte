<script lang="ts">
	import { onMount } from 'svelte';
	import { api, ApiError } from '#lib/api';
	import { onLive } from '#lib/live';

	type Evenement = {
		id: string;
		nom: string;
		date_debut: string;
		date_fin: string;
		lieu: string | null;
		archive: boolean;
		sheet_id: string | null;
		bloc_formulaire: string | null;
	};

	type FormState = {
		nom: string;
		date_debut: string;
		date_fin: string;
		lieu: string;
		sheet_id: string;
		bloc_formulaire: string;
	};

	function emptyForm(): FormState {
		return { nom: '', date_debut: '', date_fin: '', lieu: '', sheet_id: '', bloc_formulaire: '' };
	}

	type Bloc = { value: string; label: string };

	let events = $state<Evenement[]>([]);
	let blocs = $state<Bloc[]>([]);
	let loading = $state(true);
	let error = $state<string | null>(null);

	let editingId = $state<string | null>(null);
	let editForm = $state<FormState>(emptyForm());

	let creating = $state(false);
	let createForm = $state<FormState>(emptyForm());

	// See (admin)/commissions/+page.svelte's refresh() for why this is split
	// from load(): a live-refresh tick must not toggle `loading` (which
	// would unmount the Modifier/Nouvel événement form an admin has open).
	async function refresh() {
		try {
			[events, blocs] = await Promise.all([
				api.get<Evenement[]>('/evenements'),
				api.get<Bloc[]>('/blocs-formulaire')
			]);
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
		return onLive(['evenements'], () => void refresh());
	});

	// sheet_id values already used by non-archived events, with the names
	// of the events using each one — lets an admin reuse the right Sheet
	// instead of retyping it (specs.md §3.3).
	function sheetIdSuggestions(): { sheetId: string; eventNames: string }[] {
		const byId: Record<string, string[]> = {};
		for (const e of events) {
			if (e.archive || !e.sheet_id) continue;
			(byId[e.sheet_id] ??= []).push(e.nom);
		}
		return Object.entries(byId).map(([sheetId, names]) => ({
			sheetId,
			eventNames: names.join(', ')
		}));
	}

	function toInput(e: Evenement): FormState {
		return {
			nom: e.nom,
			date_debut: e.date_debut,
			date_fin: e.date_fin,
			lieu: e.lieu ?? '',
			sheet_id: e.sheet_id ?? '',
			bloc_formulaire: e.bloc_formulaire ?? ''
		};
	}

	function toBody(f: FormState) {
		return {
			nom: f.nom,
			date_debut: f.date_debut,
			date_fin: f.date_fin,
			lieu: f.lieu || null,
			sheet_id: f.sheet_id || null,
			bloc_formulaire: f.bloc_formulaire || null
		};
	}

	function startEditing(e: Evenement) {
		editingId = e.id;
		editForm = toInput(e);
	}

	async function saveEdit(id: string) {
		error = null;
		try {
			await api.patch(`/evenements/${id}`, toBody(editForm));
			editingId = null;
			await refresh();
		} catch (e) {
			error = e instanceof ApiError ? e.message : 'Erreur inattendue';
		}
	}

	async function toggleArchive(e: Evenement) {
		error = null;
		try {
			await api.patch(`/evenements/${e.id}/archive`, { archive: !e.archive });
			await refresh();
		} catch (e2) {
			error = e2 instanceof ApiError ? e2.message : 'Erreur inattendue';
		}
	}

	async function createEvent() {
		error = null;
		if (!createForm.nom.trim() || !createForm.date_debut || !createForm.date_fin) {
			error = 'Nom, date début et date fin sont obligatoires.';
			return;
		}
		try {
			await api.post('/evenements', toBody(createForm));
			creating = false;
			createForm = emptyForm();
			await refresh();
		} catch (e) {
			error = e instanceof ApiError ? e.message : 'Erreur inattendue';
		}
	}
</script>

<div class="panel wf">
	<div class="toolbar-row" style="margin-top:0">
		<h3>Événements ({events.length})</h3>
		{#if !creating}
			<button class="btn-wf primary" onclick={() => (creating = true)}>+ Nouvel événement</button>
		{/if}
	</div>
	<p class="note">
		Archiver un événement le masque des écrans actifs sans le rendre lecture seule (cf. specs.md
		§12.6).
	</p>

	{#if error}
		<p role="alert" class="error-note">{error}</p>
	{/if}

	{#if loading}
		<p>Chargement…</p>
	{:else}
		{#each events as e (e.id)}
			<div class="comm-row" data-testid="evenement-row" data-nom={e.nom}>
				<div class="comm-main">
					<span style="font-weight:600">{e.nom}</span>
					<span class="wf-label">{e.date_debut} – {e.date_fin}{e.lieu ? ` · ${e.lieu}` : ''}</span>
					<span class="wf-label">
						{#if e.sheet_id}
							Sheet source : {e.sheet_id}
						{:else}
							<span style="color:var(--accent)">
								Aucune sheet source — saisie manuelle uniquement
							</span>
						{/if}
					</span>
				</div>
				<div class="comm-meta">
					<span class="pill" class:archived={e.archive}>{e.archive ? 'archivé' : 'actif'}</span>
					<button class="btn-wf" onclick={() => startEditing(e)}>Modifier</button>
					<button
						class="btn-wf"
						class:danger={!e.archive}
						class:primary={e.archive}
						onclick={() => toggleArchive(e)}
					>
						{e.archive ? 'Désarchiver' : 'Archiver'}
					</button>
				</div>
			</div>

			{#if editingId === e.id}
				<div style="margin-top:12px; padding-top:16px; border-top:1px solid var(--line)">
					<h3>Modifier — {e.nom}</h3>
					<div class="field-grid">
						<div class="field wf">
							<label>
								Nom
								<input class="input-wf" bind:value={editForm.nom} maxlength="200" required />
							</label>
						</div>
						<div class="field wf">
							<label>
								Date début
								<input class="input-wf" type="date" bind:value={editForm.date_debut} required />
							</label>
						</div>
						<div class="field wf">
							<label>
								Date fin
								<input class="input-wf" type="date" bind:value={editForm.date_fin} required />
							</label>
						</div>
						<div class="field wf">
							<label>
								Lieu
								<input class="input-wf" bind:value={editForm.lieu} maxlength="200" />
							</label>
						</div>
						<div class="field wf">
							<label>
								ID Google Sheet source <span class="optional-tag">— facultatif</span>
								<input
									class="input-wf"
									bind:value={editForm.sheet_id}
									list="sheet-id-suggestions"
								/>
							</label>
							{#if editForm.sheet_id}
								<label>
									Bloc du formulaire
									<select class="input-wf" bind:value={editForm.bloc_formulaire}>
										<option value="">— bloc du formulaire —</option>
										{#each blocs as b (b.value)}
											<option value={b.value}>{b.label}</option>
										{/each}
									</select>
								</label>
							{/if}
						</div>
					</div>
					<button class="btn-wf primary" style="margin-top:12px" onclick={() => saveEdit(e.id)}>
						Enregistrer
					</button>
					<button class="btn-wf" style="margin-top:12px" onclick={() => (editingId = null)}>
						Annuler
					</button>
				</div>
			{/if}
		{/each}

		<datalist id="sheet-id-suggestions">
			{#each sheetIdSuggestions() as s (s.sheetId)}
				<option value={s.sheetId}>{s.sheetId} — utilisé par {s.eventNames}</option>
			{/each}
		</datalist>

		<p class="note" style="margin-top:14px">
			Un événement archivé n'apparaît plus dans les sélecteurs d'événement des autres écrans (seul
			cet écran continue de le lister).
		</p>

		{#if creating}
			<div style="margin-top:22px; padding-top:16px; border-top:1px solid var(--line)">
				<h3>Nouvel événement</h3>
				<div class="field-grid">
					<div class="field wf">
						<label>
							Nom
							<input class="input-wf" bind:value={createForm.nom} maxlength="200" required />
						</label>
					</div>
					<div class="field wf">
						<label>
							Date début
							<input class="input-wf" type="date" bind:value={createForm.date_debut} required />
						</label>
					</div>
					<div class="field wf">
						<label>
							Date fin
							<input class="input-wf" type="date" bind:value={createForm.date_fin} required />
						</label>
					</div>
					<div class="field wf">
						<label>
							Lieu
							<input class="input-wf" bind:value={createForm.lieu} maxlength="200" />
						</label>
					</div>
					<div class="field wf">
						<label>
							ID Google Sheet source <span class="optional-tag">— facultatif</span>
							<input
								class="input-wf"
								bind:value={createForm.sheet_id}
								list="sheet-id-suggestions"
							/>
						</label>
						{#if createForm.sheet_id}
							<label>
								Bloc du formulaire
								<select class="input-wf" bind:value={createForm.bloc_formulaire}>
									<option value="">— bloc du formulaire —</option>
									{#each blocs as b (b.value)}
										<option value={b.value}>{b.label}</option>
									{/each}
								</select>
							</label>
						{/if}
					</div>
				</div>
				<p class="note" style="margin-top:10px">
					Autocomplétion sur les <code>sheet_id</code> déjà utilisés par des événements non archivés ;
					champ libre, toute nouvelle valeur reste acceptée après vérification d'accès.
				</p>
				<button class="btn-wf primary" onclick={createEvent}>Créer l'événement</button>
				<button class="btn-wf" onclick={() => (creating = false)}>Annuler</button>
			</div>
		{/if}
	{/if}
</div>
