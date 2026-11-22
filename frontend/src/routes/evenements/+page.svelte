<script lang="ts">
	import { onMount } from 'svelte';
	import { api, ApiError } from '#lib/api';

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

	let events = $state<Evenement[]>([]);
	let loading = $state(true);
	let error = $state<string | null>(null);

	let editingId = $state<string | null>(null);
	let editForm = $state<FormState>(emptyForm());

	let creating = $state(false);
	let createForm = $state<FormState>(emptyForm());

	async function load() {
		loading = true;
		error = null;
		try {
			events = await api.get<Evenement[]>('/evenements');
		} catch (e) {
			error = e instanceof ApiError ? e.message : 'Erreur inattendue';
		} finally {
			loading = false;
		}
	}

	onMount(load);

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
			await load();
		} catch (e) {
			error = e instanceof ApiError ? e.message : 'Erreur inattendue';
		}
	}

	async function toggleArchive(e: Evenement) {
		error = null;
		try {
			await api.patch(`/evenements/${e.id}/archive`, { archive: !e.archive });
			await load();
		} catch (e2) {
			error = e2 instanceof ApiError ? e2.message : 'Erreur inattendue';
		}
	}

	async function createEvent() {
		error = null;
		try {
			await api.post('/evenements', toBody(createForm));
			creating = false;
			createForm = emptyForm();
			await load();
		} catch (e) {
			error = e instanceof ApiError ? e.message : 'Erreur inattendue';
		}
	}
</script>

<h2>Admin — événements</h2>
<p>
	Archiver un événement le masque des écrans actifs sans le rendre lecture seule (cf. specs.md
	§12.6).
</p>

{#if error}
	<p role="alert">{error}</p>
{/if}

{#if loading}
	<p>Chargement…</p>
{:else}
	<table>
		<thead>
			<tr>
				<th>Nom</th>
				<th>Dates</th>
				<th>Lieu</th>
				<th>Statut</th>
				<th>Sheet source</th>
				<th></th>
			</tr>
		</thead>
		<tbody>
			{#each events as e (e.id)}
				<tr>
					{#if editingId === e.id}
						<td><input bind:value={editForm.nom} /></td>
						<td>
							<input type="date" bind:value={editForm.date_debut} />
							<input type="date" bind:value={editForm.date_fin} />
						</td>
						<td><input bind:value={editForm.lieu} /></td>
						<td>{e.archive ? 'archivé' : 'actif'}</td>
						<td>
							<input bind:value={editForm.sheet_id} list="sheet-id-suggestions" />
						</td>
						<td>
							<button onclick={() => saveEdit(e.id)}>Enregistrer</button>
							<button onclick={() => (editingId = null)}>Annuler</button>
						</td>
					{:else}
						<td>{e.nom}</td>
						<td>{e.date_debut} – {e.date_fin}</td>
						<td>{e.lieu ?? ''}</td>
						<td>{e.archive ? 'archivé' : 'actif'}</td>
						<td>{e.sheet_id ?? '—'}</td>
						<td>
							<button onclick={() => startEditing(e)}>Modifier</button>
							<button onclick={() => toggleArchive(e)}>
								{e.archive ? 'Désarchiver' : 'Archiver'}
							</button>
						</td>
					{/if}
				</tr>
			{/each}
		</tbody>
	</table>

	<datalist id="sheet-id-suggestions">
		{#each sheetIdSuggestions() as s (s.sheetId)}
			<option value={s.sheetId}>{s.sheetId} — utilisé par {s.eventNames}</option>
		{/each}
	</datalist>

	<div>
		{#if creating}
			<h3>Nouvel événement</h3>
			<label>Nom <input bind:value={createForm.nom} /></label>
			<label>Date début <input type="date" bind:value={createForm.date_debut} /></label>
			<label>Date fin <input type="date" bind:value={createForm.date_fin} /></label>
			<label>Lieu <input bind:value={createForm.lieu} /></label>
			<label>
				ID Google Sheet source
				<input bind:value={createForm.sheet_id} list="sheet-id-suggestions" />
			</label>
			<button onclick={createEvent}>Créer l'événement</button>
			<button onclick={() => (creating = false)}>Annuler</button>
		{:else}
			<button onclick={() => (creating = true)}>+ Nouvel événement</button>
		{/if}
	</div>
{/if}
