<script lang="ts">
	import { onMount } from 'svelte';
	import { api, ApiError } from '#lib/api';
	import { onLive } from '#lib/live';

	type Evenement = { id: string; nom: string; archive: boolean };
	type Commission = {
		id: string;
		nom: string;
		couleur: string;
		besoin_chiffre: boolean;
	};
	type ResponsableEntry = {
		commission_id: string;
		soumission_id: string;
		nom: string;
		prenom: string;
		created_at: string;
	};
	type SoumissionResult = { id: string; nom: string; prenom: string };

	function formatDate(iso: string): string {
		return new Date(iso).toLocaleDateString('fr-FR', {
			day: 'numeric',
			month: 'short',
			year: 'numeric'
		});
	}

	let evenements = $state<Evenement[]>([]);
	let commissions = $state<Commission[]>([]);
	let responsables = $state<ResponsableEntry[]>([]);
	let selectedEvenementId = $state<string | null>(null);
	let selectedCommissionId = $state<string | null>(null);

	let searchQuery = $state('');
	let searchResults = $state<SoumissionResult[]>([]);
	let selectedSoumission = $state<SoumissionResult | null>(null);

	let loading = $state(true);
	let error = $state<string | null>(null);

	async function refreshRefs() {
		try {
			const [evs, comms] = await Promise.all([
				api.get<Evenement[]>('/evenements'),
				api.get<Commission[]>('/commissions')
			]);
			evenements = evs;
			commissions = comms;
			if (!selectedEvenementId && evs.length > 0) {
				selectedEvenementId = evs.find((e) => !e.archive)?.id ?? evs[0].id;
			}
			if (!selectedCommissionId && comms.length > 0) {
				selectedCommissionId = comms[0].id;
			}
		} catch (e) {
			error = e instanceof ApiError ? e.message : 'Erreur inattendue';
		}
	}

	async function refreshResponsables() {
		if (!selectedEvenementId) return;
		try {
			responsables = await api.get<ResponsableEntry[]>(`/responsables/${selectedEvenementId}`);
		} catch (e) {
			error = e instanceof ApiError ? e.message : 'Erreur inattendue';
		}
	}

	async function load() {
		loading = true;
		error = null;
		await refreshRefs();
		await refreshResponsables();
		loading = false;
	}

	onMount(() => {
		void load();
		return onLive(['responsables', 'evenements', 'commissions'], () => {
			void refreshRefs();
			void refreshResponsables();
		});
	});

	function selectEvenement(id: string) {
		selectedEvenementId = id;
		resetSearch();
		void refreshResponsables();
	}

	function selectCommission(id: string) {
		selectedCommissionId = id;
		resetSearch();
	}

	function resetSearch() {
		searchQuery = '';
		searchResults = [];
		selectedSoumission = null;
	}

	function commissionNom(id: string): string {
		return commissions.find((c) => c.id === id)?.nom ?? '?';
	}

	function responsablesDe(commissionId: string): ResponsableEntry[] {
		return responsables.filter((r) => r.commission_id === commissionId);
	}

	function autresCommissionsDe(soumissionId: string, excludeCommissionId: string): string[] {
		return responsables
			.filter((r) => r.soumission_id === soumissionId && r.commission_id !== excludeCommissionId)
			.map((r) => commissionNom(r.commission_id));
	}

	async function runSearch() {
		if (!selectedEvenementId || searchQuery.trim().length === 0) {
			searchResults = [];
			return;
		}
		try {
			searchResults = await api.get<SoumissionResult[]>(
				`/responsables/${selectedEvenementId}/soumissions?q=${encodeURIComponent(searchQuery.trim())}`
			);
		} catch (e) {
			error = e instanceof ApiError ? e.message : 'Erreur inattendue';
		}
	}

	function pickSoumission(s: SoumissionResult) {
		selectedSoumission = s;
		searchQuery = `${s.nom} ${s.prenom}`;
		searchResults = [];
	}

	async function addResponsable() {
		if (!selectedEvenementId || !selectedCommissionId || !selectedSoumission) return;
		error = null;
		try {
			await api.post(`/responsables/${selectedEvenementId}`, {
				commission_id: selectedCommissionId,
				soumission_id: selectedSoumission.id
			});
			resetSearch();
			await refreshResponsables();
		} catch (e) {
			error = e instanceof ApiError ? e.message : 'Erreur inattendue';
		}
	}

	async function removeResponsable(r: ResponsableEntry) {
		if (!selectedEvenementId) return;
		error = null;
		try {
			await api.delete(
				`/responsables/${selectedEvenementId}/${r.commission_id}/${r.soumission_id}`
			);
			await refreshResponsables();
		} catch (e) {
			error = e instanceof ApiError ? e.message : 'Erreur inattendue';
		}
	}
</script>

<div class="screen-head">
	<h2>Admin — responsables de commission</h2>
	<span class="screen-tag">Par événement</span>
</div>
<p class="note">
	Un bénévole désigné responsable d'une commission est occupé sur tous les créneaux de cette
	commission pour tout l'événement, même sans affectation explicite. Il peut en plus être affecté à
	un poste de sa propre commission. Son nom porte le badge <span class="resp-badge">R</span>
	partout où il apparaît.
</p>

{#if error}
	<p role="alert" class="error-note">{error}</p>
{/if}

{#if loading}
	<p>Chargement…</p>
{:else if evenements.length === 0}
	<p class="note">Aucun événement — créez-en un dans l'écran Événements d'abord.</p>
{:else}
	<div class="btn-group" style="margin:0 0 14px">
		{#each evenements as ev (ev.id)}
			<button
				class="btn-wf"
				class:primary={selectedEvenementId === ev.id}
				onclick={() => selectEvenement(ev.id)}
			>
				{ev.nom}{ev.archive ? ' (archivé)' : ''}
			</button>
		{/each}
	</div>

	<div class="panel wf" style="margin-bottom:18px">
		<h3>Couverture par commission</h3>
		<p class="note" style="margin:4px 0 10px">
			Une commission sans responsable bloque la génération (hors commissions sans besoin chiffré).
		</p>
		{#each commissions as comm (comm.id)}
			{@const count = responsablesDe(comm.id).length}
			<div class="comm-row">
				<span>
					<span class="color-swatch" style="background:{comm.couleur}"></span>
					{comm.nom}
					{#if !comm.besoin_chiffre}<span class="wf-label">(sans besoin chiffré)</span>{/if}
				</span>
				{#if !comm.besoin_chiffre}
					<span class="pill">—</span>
				{:else}
					<span class="pill" class:error-pill={count === 0}>
						{count} responsable{count > 1 ? 's' : ''}
					</span>
				{/if}
			</div>
		{/each}
	</div>

	<div class="wf-label" style="margin:14px 0 8px">Commission</div>
	<div class="btn-group" style="margin-bottom:14px">
		{#each commissions as comm (comm.id)}
			<button
				class="btn-wf"
				class:primary={selectedCommissionId === comm.id}
				style="border-left: 4px solid {comm.couleur}"
				onclick={() => selectCommission(comm.id)}
			>
				{comm.nom}
			</button>
		{/each}
	</div>

	{#if selectedCommissionId}
		{@const comm = commissions.find((c) => c.id === selectedCommissionId)}
		{#if comm}
			<h3>Responsables — commission <em>{comm.nom}</em></h3>
			{#each responsablesDe(comm.id) as r (r.soumission_id)}
				{@const autres = autresCommissionsDe(r.soumission_id, comm.id)}
				<div class="comm-row">
					<div class="comm-main">
						<span style="font-weight:600">
							{r.nom} · {r.prenom} <span class="resp-badge">R</span>
						</span>
						<span class="wf-label">
							{#if autres.length > 0}
								responsable aussi de {autres.join(', ')} — cumul autorisé
							{:else}
								responsable depuis le {formatDate(r.created_at)}
							{/if}
						</span>
					</div>
					<div class="comm-meta">
						<button class="btn-wf danger" onclick={() => removeResponsable(r)}>Retirer</button>
					</div>
				</div>
			{:else}
				<p class="note">Aucun responsable pour cette commission.</p>
			{/each}

			<div style="margin-top:18px; padding-top:16px; border-top:1px solid var(--line)">
				<h3>Ajouter un responsable à <em>{comm.nom}</em></h3>
				<div class="toolbar-row" style="position:relative">
					<input
						class="input-wf wf"
						style="flex:1; min-width:160px"
						type="search"
						placeholder="🔍 Rechercher un bénévole…"
						bind:value={searchQuery}
						oninput={() => {
							selectedSoumission = null;
							void runSearch();
						}}
					/>
					<button class="btn-wf primary" disabled={!selectedSoumission} onclick={addResponsable}>
						Ajouter
					</button>
				</div>
				{#if searchResults.length > 0}
					<div class="panel wf search-results">
						{#each searchResults as s (s.id)}
							<button class="search-row" onclick={() => pickSoumission(s)}>
								{s.nom} · {s.prenom}
							</button>
						{/each}
					</div>
				{/if}
				<p class="note" style="margin-top:8px">
					Refusé si le bénévole est déjà responsable d'une autre commission dont au moins un créneau
					chevauche ceux de {comm.nom} (sauf s'il est responsable des deux).
				</p>
			</div>
		{/if}
	{/if}
{/if}

<style>
	.error-pill {
		border-color: var(--accent);
		color: var(--accent);
	}
	.search-results {
		margin-top: 6px;
		padding: 6px;
		display: flex;
		flex-direction: column;
		gap: 3px;
	}
	.search-row {
		text-align: left;
		padding: 6px 8px;
		border-radius: 4px;
		font-size: 12px;
		background: none;
		border: none;
		color: inherit;
		cursor: pointer;
	}
	.search-row:hover {
		background: var(--accent-weak);
	}
</style>
