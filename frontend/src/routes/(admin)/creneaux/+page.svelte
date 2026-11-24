<script lang="ts">
	import { onMount } from 'svelte';
	import { api, ApiError } from '#lib/api';
	import { onLive } from '#lib/live';
	import { createSaveFlash } from '#lib/saveFeedback.svelte';

	type Evenement = {
		id: string;
		nom: string;
		date_debut: string;
		date_fin: string;
		archive: boolean;
	};
	type Commission = {
		id: string;
		nom: string;
		grille_creneaux: 'generale' | 'propre';
		couleur: string;
	};

	type Creneau = {
		id: string;
		commission_id: string | null;
		jour: string;
		heure_debut: string;
		heure_fin: string;
		ordre: number;
	};
	type MappingEntry = { jour: string; slot_form: string; creneau_applicatif_id: string };
	type CreneauRepas = { id: string; jour: string; nom: string; creneau_ids: string[] };
	type ScreenData = {
		creneaux: Creneau[];
		mapping: MappingEntry[];
		creneaux_repas: CreneauRepas[];
	};

	const CRENEAU_REPAS_MIN_MEMBRES = 2;

	const SLOTS = ['matin', 'midi', 'aprem', 'soir'] as const;

	// Backend sérialise un chrono::NaiveTime en "HH:MM:SS" ; <input type=time>
	// veut "HH:MM".
	function toInputTime(t: string): string {
		return t.slice(0, 5);
	}
	function toApiTime(t: string): string {
		return t.length === 5 ? `${t}:00` : t;
	}

	// Dates manipulées en UTC uniquement (Date.UTC + setUTCDate) — construire
	// avec `new Date("YYYY-MM-DDT00:00:00")` (heure locale) puis formater avec
	// toISOString() (UTC) décale le jour d'un cran dans un fuseau ≠ UTC+0.
	function parseIsoDateUtc(s: string): Date {
		const [y, m, d] = s.split('-').map(Number);
		return new Date(Date.UTC(y, m - 1, d));
	}

	function joursEvenement(ev: Evenement): string[] {
		const out: string[] = [];
		const d = parseIsoDateUtc(ev.date_debut);
		const end = parseIsoDateUtc(ev.date_fin);
		while (d <= end) {
			out.push(d.toISOString().slice(0, 10));
			d.setUTCDate(d.getUTCDate() + 1);
		}
		return out;
	}

	// "Ven 30/04", pour les boutons de sélection du jour (même esprit que le
	// sélecteur de jour de l'écran Planning, mockups/wireframes.html s1).
	function formatJourShort(iso: string): string {
		const d = parseIsoDateUtc(iso);
		const weekday = d
			.toLocaleDateString('fr-FR', { weekday: 'short', timeZone: 'UTC' })
			.replace('.', '');
		const dm = d.toLocaleDateString('fr-FR', { day: '2-digit', month: '2-digit', timeZone: 'UTC' });
		return `${weekday.charAt(0).toUpperCase()}${weekday.slice(1)} ${dm}`;
	}

	let evenements = $state<Evenement[]>([]);
	let commissions = $state<Commission[]>([]);
	let selectedEvenementId = $state<string | null>(null);
	let selectedJour = $state<string | null>(null);
	let selectedCommissionPropreId = $state<string | null>(null);
	let data = $state<ScreenData | null>(null);

	let loading = $state(true);
	let error = $state<string | null>(null);

	// Formulaire d'ajout de créneau, un seul à la fois (clé = "generale" ou
	// l'id de la commission à grille propre affichée).
	let addKey = $state<string | null>(null);
	let addForm = $state({ heure_debut: '', heure_fin: '' });

	let editingId = $state<string | null>(null);
	let editForm = $state({ heure_debut: '', heure_fin: '' });

	let dupliquerOuvert = $state(false);
	let dupliquerSourceId = $state('');

	let dupliquerJourOuvert = $state(false);
	let dupliquerJourSourceId = $state('');

	// État local du mapping form -> créneaux, reconstruit à chaque chargement
	// puis modifié localement avant un "Enregistrer" qui remplace tout
	// (PATCH /creneaux/:id/mapping, specs.md §4.1).
	let mappingState = $state<Record<string, string[]>>({});
	let mappingDirty = $state(false);

	function mappingKey(jour: string, slot: string): string {
		return `${jour}|${slot}`;
	}

	function rebuildMappingState(d: ScreenData) {
		const next: Record<string, string[]> = {};
		for (const m of d.mapping) {
			const key = mappingKey(m.jour, m.slot_form);
			next[key] = [...(next[key] ?? []), m.creneau_applicatif_id];
		}
		mappingState = next;
		mappingDirty = false;
	}

	async function refreshScreenData() {
		if (!selectedEvenementId) return;
		try {
			const d = await api.get<ScreenData>(`/creneaux/${selectedEvenementId}`);
			data = d;
			if (!mappingDirty) rebuildMappingState(d);
			const ev = evenements.find((e) => e.id === selectedEvenementId);
			if (ev) {
				const jours = joursEvenement(ev);
				if (!selectedJour || !jours.includes(selectedJour)) selectedJour = jours[0] ?? null;
			}
		} catch (e) {
			error = e instanceof ApiError ? e.message : 'Erreur inattendue';
		}
	}

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
			if (!selectedCommissionPropreId) {
				selectedCommissionPropreId = comms.find((c) => c.grille_creneaux === 'propre')?.id ?? null;
			}
		} catch (e) {
			error = e instanceof ApiError ? e.message : 'Erreur inattendue';
		}
	}

	async function load() {
		loading = true;
		error = null;
		await refreshRefs();
		await refreshScreenData();
		loading = false;
	}

	onMount(() => {
		void load();
		return onLive(['creneaux', 'evenements', 'commissions'], () => {
			void refreshRefs();
			void refreshScreenData();
		});
	});

	function selectedEvenement(): Evenement | undefined {
		return evenements.find((e) => e.id === selectedEvenementId);
	}

	function selectEvenement(id: string) {
		selectedEvenementId = id;
		selectedJour = null;
		addKey = null;
		editingId = null;
		dupliquerOuvert = false;
		dupliquerJourOuvert = false;
		data = null;
		void refreshScreenData();
	}

	function creneauxDe(commissionId: string | null, jour: string): Creneau[] {
		return (data?.creneaux ?? [])
			.filter((c) => c.commission_id === commissionId && c.jour === jour)
			.sort((a, b) => a.ordre - b.ordre || a.heure_debut.localeCompare(b.heure_debut));
	}

	function startAdd(key: string) {
		editingId = null;
		addKey = key;
		addForm = { heure_debut: '', heure_fin: '' };
	}

	async function submitAdd(commissionId: string | null, jour: string) {
		if (!selectedEvenementId || !addForm.heure_debut || !addForm.heure_fin) return;
		error = null;
		try {
			await api.post(`/creneaux/${selectedEvenementId}`, {
				commission_id: commissionId,
				jour,
				heure_debut: toApiTime(addForm.heure_debut),
				heure_fin: toApiTime(addForm.heure_fin)
			});
			addKey = null;
			await refreshScreenData();
		} catch (e) {
			error = e instanceof ApiError ? e.message : 'Erreur inattendue';
		}
	}

	function startEdit(c: Creneau) {
		addKey = null;
		editingId = c.id;
		editForm = { heure_debut: toInputTime(c.heure_debut), heure_fin: toInputTime(c.heure_fin) };
	}

	async function submitEdit(c: Creneau) {
		if (!selectedEvenementId) return;
		error = null;
		try {
			await api.patch(`/creneaux/${selectedEvenementId}/${c.id}`, {
				commission_id: c.commission_id,
				jour: c.jour,
				heure_debut: toApiTime(editForm.heure_debut),
				heure_fin: toApiTime(editForm.heure_fin),
				ordre: c.ordre
			});
			editingId = null;
			await refreshScreenData();
		} catch (e) {
			error = e instanceof ApiError ? e.message : 'Erreur inattendue';
		}
	}

	async function removeCreneau(c: Creneau) {
		if (!selectedEvenementId) return;
		error = null;
		try {
			await api.delete(`/creneaux/${selectedEvenementId}/${c.id}`);
			await refreshScreenData();
		} catch (e) {
			error = e instanceof ApiError ? e.message : 'Erreur inattendue';
		}
	}

	let repasAddOpen = $state(false);
	let repasAddForm = $state<{ nom: string; ids: string[] }>({ nom: '', ids: [] });
	let editingRepasId = $state<string | null>(null);
	let repasEditForm = $state<{ nom: string; ids: string[] }>({ nom: '', ids: [] });

	function creneauxRepasDe(jour: string): CreneauRepas[] {
		return (data?.creneaux_repas ?? []).filter((r) => r.jour === jour);
	}

	function startAddRepas() {
		editingRepasId = null;
		repasAddOpen = true;
		repasAddForm = { nom: '', ids: [] };
	}

	function toggleRepasAddId(id: string) {
		repasAddForm = {
			...repasAddForm,
			ids: repasAddForm.ids.includes(id)
				? repasAddForm.ids.filter((i) => i !== id)
				: [...repasAddForm.ids, id]
		};
	}

	async function submitAddRepas(jour: string) {
		if (!selectedEvenementId || !repasAddForm.nom.trim()) return;
		error = null;
		try {
			await api.post(`/creneaux/${selectedEvenementId}/repas`, {
				jour,
				nom: repasAddForm.nom.trim(),
				creneau_ids: repasAddForm.ids
			});
			repasAddOpen = false;
			await refreshScreenData();
		} catch (e) {
			error = e instanceof ApiError ? e.message : 'Erreur inattendue';
		}
	}

	function startEditRepas(r: CreneauRepas) {
		repasAddOpen = false;
		editingRepasId = r.id;
		repasEditForm = { nom: r.nom, ids: [...r.creneau_ids] };
	}

	function toggleRepasEditId(id: string) {
		repasEditForm = {
			...repasEditForm,
			ids: repasEditForm.ids.includes(id)
				? repasEditForm.ids.filter((i) => i !== id)
				: [...repasEditForm.ids, id]
		};
	}

	async function submitEditRepas(r: CreneauRepas) {
		if (!selectedEvenementId || !repasEditForm.nom.trim()) return;
		error = null;
		try {
			await api.patch(`/creneaux/${selectedEvenementId}/repas/${r.id}`, {
				jour: r.jour,
				nom: repasEditForm.nom.trim(),
				creneau_ids: repasEditForm.ids
			});
			editingRepasId = null;
			await refreshScreenData();
		} catch (e) {
			error = e instanceof ApiError ? e.message : 'Erreur inattendue';
		}
	}

	async function removeRepas(r: CreneauRepas) {
		if (!selectedEvenementId) return;
		error = null;
		try {
			await api.delete(`/creneaux/${selectedEvenementId}/repas/${r.id}`);
			await refreshScreenData();
		} catch (e) {
			error = e instanceof ApiError ? e.message : 'Erreur inattendue';
		}
	}

	function commissionsPropres(): Commission[] {
		return commissions.filter((c) => c.grille_creneaux === 'propre');
	}

	function creneauxTotal(commissionId: string): number {
		return (data?.creneaux ?? []).filter((c) => c.commission_id === commissionId).length;
	}

	function autresCommissionsPropresAvecCreneaux(excludeId: string): Commission[] {
		return commissionsPropres().filter((c) => c.id !== excludeId && creneauxTotal(c.id) > 0);
	}

	function selectCommissionPropre(id: string) {
		selectedCommissionPropreId = id;
		addKey = null;
		editingId = null;
		dupliquerOuvert = false;
	}

	async function dupliquer() {
		if (!selectedEvenementId || !selectedCommissionPropreId || !dupliquerSourceId) return;
		error = null;
		try {
			await api.post(`/creneaux/${selectedEvenementId}/dupliquer`, {
				source_commission_id: dupliquerSourceId,
				target_commission_id: selectedCommissionPropreId
			});
			dupliquerOuvert = false;
			dupliquerSourceId = '';
			await refreshScreenData();
		} catch (e) {
			error = e instanceof ApiError ? e.message : 'Erreur inattendue';
		}
	}

	// Jours de l'événement ayant déjà au moins un créneau de grille générale
	// — seuls candidats valides comme source d'une duplication de jour.
	function joursAvecCreneauxGenerale(): string[] {
		const ev = selectedEvenement();
		if (!ev) return [];
		return joursEvenement(ev).filter((j) => creneauxDe(null, j).length > 0);
	}

	async function dupliquerJour() {
		if (!selectedEvenementId || !selectedJour || !dupliquerJourSourceId) return;
		error = null;
		try {
			await api.post(`/creneaux/${selectedEvenementId}/dupliquer-jour`, {
				source_jour: dupliquerJourSourceId,
				target_jour: selectedJour
			});
			dupliquerJourOuvert = false;
			dupliquerJourSourceId = '';
			await refreshScreenData();
		} catch (e) {
			error = e instanceof ApiError ? e.message : 'Erreur inattendue';
		}
	}

	function toggleMapping(jour: string, slot: string, creneauId: string) {
		const key = mappingKey(jour, slot);
		const current = mappingState[key] ?? [];
		const next = { ...mappingState };
		next[key] = current.includes(creneauId)
			? current.filter((id) => id !== creneauId)
			: [...current, creneauId];
		mappingState = next;
		mappingDirty = true;
	}

	const mappingSaved = createSaveFlash();

	async function saveMapping() {
		if (!selectedEvenementId) return;
		error = null;
		const entries: MappingEntry[] = [];
		for (const [key, ids] of Object.entries(mappingState)) {
			const [jour, slot_form] = key.split('|');
			for (const creneau_applicatif_id of ids) {
				entries.push({ jour, slot_form, creneau_applicatif_id });
			}
		}
		try {
			await api.patch(`/creneaux/${selectedEvenementId}/mapping`, entries);
			mappingDirty = false;
			await refreshScreenData();
			mappingSaved.trigger();
		} catch (e) {
			error = e instanceof ApiError ? e.message : 'Erreur inattendue';
		}
	}
</script>

<div class="screen-head">
	<h2>Admin — créneaux applicatifs</h2>
	<span class="screen-tag">Gabarit + grilles propres + créneaux repas + mapping form</span>
</div>
<p class="note">
	Créneaux par jour (gabarit par défaut, modifiable), mapping avec les cases du formulaire, et
	grilles propres par commission.
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

	{#if data && selectedEvenement()}
		{@const ev = selectedEvenement()}
		{@const jours = ev ? joursEvenement(ev) : []}

		<div class="btn-group" style="margin:0 0 16px">
			{#each jours as jour (jour)}
				<button
					class="btn-wf"
					class:primary={selectedJour === jour}
					onclick={() => (selectedJour = jour)}
				>
					{formatJourShort(jour)}
				</button>
			{/each}
		</div>

		{#if selectedJour}
			<div class="panel wf" style="margin-bottom:16px">
				<h3>{formatJourShort(selectedJour)} — créneaux du jour</h3>

				{#if creneauxDe(null, selectedJour).length === 0}
					{@const candidats = joursAvecCreneauxGenerale().filter((j) => j !== selectedJour)}
					{#if candidats.length > 0}
						<div class="toolbar-row" style="margin:8px 0; justify-content:space-between">
							{#if dupliquerJourOuvert}
								<div class="btn-group">
									{#each candidats as j (j)}
										<button
											class="btn-wf"
											class:primary={dupliquerJourSourceId === j}
											onclick={() => (dupliquerJourSourceId = j)}
										>
											{formatJourShort(j)}
										</button>
									{/each}
								</div>
								<button class="btn-wf primary" onclick={dupliquerJour}>Dupliquer</button>
								<button class="btn-wf" onclick={() => (dupliquerJourOuvert = false)}>
									Annuler
								</button>
							{:else}
								<button class="btn-wf" onclick={() => (dupliquerJourOuvert = true)}>
									Dupliquer les créneaux d'un autre jour
								</button>
							{/if}
						</div>
					{/if}
				{/if}

				<div class="field-grid">
					{#each creneauxDe(null, selectedJour) as c (c.id)}
						{#if editingId === c.id}
							<div class="field wf">
								<span class="wf-label">Début</span>
								<input class="input-wf wf" type="time" bind:value={editForm.heure_debut} />
								<span class="wf-label">Fin</span>
								<input class="input-wf wf" type="time" bind:value={editForm.heure_fin} />
								<button class="btn-wf primary" onclick={() => submitEdit(c)}>OK</button>
								<button class="btn-wf" onclick={() => (editingId = null)}>Annuler</button>
							</div>
						{:else}
							<div class="field wf">
								<span class="wf-label">Début</span>
								<span class="input-wf wf creneau-value">{toInputTime(c.heure_debut)}</span>
								<span class="wf-label">Fin</span>
								<span class="input-wf wf creneau-value">{toInputTime(c.heure_fin)}</span>
								<div class="field-actions">
									<button
										class="btn-wf icon-btn"
										onclick={() => startEdit(c)}
										aria-label="Modifier"
										title="Modifier"
									>
										<svg
											width="14"
											height="14"
											viewBox="0 0 24 24"
											fill="none"
											stroke="currentColor"
											stroke-width="2"
											stroke-linecap="round"
											stroke-linejoin="round"
										>
											<path d="M11 4H4a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h14a2 2 0 0 0 2-2v-7" /><path
												d="M18.5 2.5a2.121 2.121 0 0 1 3 3L12 15l-4 1 1-4Z"
											/>
										</svg>
									</button>
									<button
										class="btn-wf danger icon-btn"
										onclick={() => removeCreneau(c)}
										aria-label="Supprimer"
										title="Supprimer"
									>
										<svg
											width="14"
											height="14"
											viewBox="0 0 24 24"
											fill="none"
											stroke="currentColor"
											stroke-width="2"
											stroke-linecap="round"
										>
											<path d="M18 6 6 18" /><path d="M6 6l12 12" />
										</svg>
									</button>
								</div>
							</div>
						{/if}
					{/each}
					{#if addKey === 'generale'}
						<div class="field wf">
							<span class="wf-label">Début</span>
							<input class="input-wf wf" type="time" bind:value={addForm.heure_debut} />
							<span class="wf-label">Fin</span>
							<input class="input-wf wf" type="time" bind:value={addForm.heure_fin} />
							<button class="btn-wf primary" onclick={() => submitAdd(null, selectedJour ?? '')}>
								Ajouter
							</button>
							<button class="btn-wf" onclick={() => (addKey = null)}>Annuler</button>
						</div>
					{:else}
						<button class="field wf add-tile" onclick={() => startAdd('generale')}>
							+ ajouter un créneau
						</button>
					{/if}
				</div>

				<p class="note" style="margin-top:10px">
					Mapping form → app :
					{#each SLOTS as slot (slot)}
						<span class="pill">{slot}</span>
						{#if (mappingState[mappingKey(selectedJour, slot)]?.length ?? 0) > 0}
							{mappingState[mappingKey(selectedJour, slot)]
								.map((id) => {
									const c = data?.creneaux.find((x) => x.id === id);
									return c ? `${toInputTime(c.heure_debut)}–${toInputTime(c.heure_fin)}` : '?';
								})
								.join(', ')}
						{:else}
							<em>non mappé</em>
						{/if}
					{/each}
				</p>
				<div class="mapping-editor">
					{#each SLOTS as slot (slot)}
						<div class="mapping-slot-row">
							<span class="wf-label mapping-slot-label">{slot}</span>
							<div class="btn-group">
								{#each creneauxDe(null, selectedJour) as c (c.id)}
									<button
										class="btn-wf"
										class:primary={mappingState[mappingKey(selectedJour, slot)]?.includes(c.id) ??
											false}
										onclick={() => toggleMapping(selectedJour ?? '', slot, c.id)}
									>
										{toInputTime(c.heure_debut)}–{toInputTime(c.heure_fin)}
									</button>
								{/each}
								{#if creneauxDe(null, selectedJour).length === 0}
									<span class="note" style="margin:0">Aucun créneau général ce jour.</span>
								{/if}
							</div>
						</div>
					{/each}
					<button
						class="btn-wf primary"
						class:success={mappingSaved.active}
						style="margin-top:8px"
						onclick={saveMapping}
					>
						{mappingSaved.active ? '✓ Enregistré' : 'Enregistrer le mapping'}
					</button>
				</div>
			</div>

			<div class="panel wf" style="margin-bottom:16px">
				<h3>Créneaux repas <span class="wf-label">(grille générale)</span></h3>
				<p class="note" style="margin:4px 0 10px">
					Liste nommée de créneaux, min. {CRENEAU_REPAS_MIN_MEMBRES}, variable par jour et par
					événement : la génération automatique n'affecte jamais un bénévole à la totalité des
					créneaux d'un même créneau repas, pour lui garantir une pause (une affectation manuelle
					peut, elle, le faire). Non applicable aux commissions à grille propre.
				</p>

				{#each creneauxRepasDe(selectedJour) as r (r.id)}
					{#if editingRepasId === r.id}
						<div class="field-grid" style="margin-bottom:6px">
							<div class="field wf">
								<span class="wf-label">Nom</span>
								<input class="input-wf wf" bind:value={repasEditForm.nom} />
							</div>
						</div>
						<div class="btn-group" style="margin-bottom:6px">
							{#each creneauxDe(null, selectedJour) as c (c.id)}
								<button
									class="btn-wf"
									class:primary={repasEditForm.ids.includes(c.id)}
									onclick={() => toggleRepasEditId(c.id)}
								>
									{toInputTime(c.heure_debut)}–{toInputTime(c.heure_fin)}
								</button>
							{/each}
						</div>
						<div class="toolbar-row" style="margin:0 0 14px">
							<button class="btn-wf primary" onclick={() => submitEditRepas(r)}>OK</button>
							<button class="btn-wf" onclick={() => (editingRepasId = null)}>Annuler</button>
						</div>
					{:else}
						<div class="wf-label" style="margin-bottom:6px">{r.nom}</div>
						<div class="btn-group" style="margin-bottom:12px">
							{#each r.creneau_ids as id (id)}
								{@const c = data?.creneaux.find((x) => x.id === id)}
								{#if c}
									<button class="btn-wf primary" onclick={() => startEditRepas(r)}>
										{toInputTime(c.heure_debut)}–{toInputTime(c.heure_fin)}
									</button>
								{/if}
							{/each}
							<button
								class="btn-wf danger icon-btn"
								onclick={() => removeRepas(r)}
								aria-label="Supprimer ce créneau repas"
								title="Supprimer ce créneau repas"
							>
								✕
							</button>
						</div>
					{/if}
				{/each}

				{#if repasAddOpen}
					<div class="field-grid" style="margin-bottom:6px">
						<div class="field wf">
							<span class="wf-label">Nom</span>
							<input class="input-wf wf" bind:value={repasAddForm.nom} placeholder="Déjeuner" />
						</div>
					</div>
					<div class="btn-group" style="margin-bottom:6px">
						{#each creneauxDe(null, selectedJour) as c (c.id)}
							<button
								class="btn-wf"
								class:primary={repasAddForm.ids.includes(c.id)}
								onclick={() => toggleRepasAddId(c.id)}
							>
								{toInputTime(c.heure_debut)}–{toInputTime(c.heure_fin)}
							</button>
						{/each}
						{#if creneauxDe(null, selectedJour).length === 0}
							<span class="note" style="margin:0">Aucun créneau général ce jour.</span>
						{/if}
					</div>
					<div class="toolbar-row" style="margin:0 0 14px">
						<button
							class="btn-wf primary"
							disabled={repasAddForm.ids.length < CRENEAU_REPAS_MIN_MEMBRES ||
								!repasAddForm.nom.trim()}
							onclick={() => submitAddRepas(selectedJour ?? '')}
						>
							Ajouter
						</button>
						<button class="btn-wf" onclick={() => (repasAddOpen = false)}>Annuler</button>
					</div>
				{:else}
					<button class="btn-wf" onclick={startAddRepas}>
						+ ajouter un créneau repas — {formatJourShort(selectedJour)}
					</button>
				{/if}
			</div>

			<div class="panel wf">
				<h3>Grille de créneaux par commission</h3>
				<p class="note" style="margin:4px 0 10px">
					Par défaut une commission suit la grille générale ci-dessus. On peut la basculer sur une
					grille propre dans l'écran Commissions ; chaque commission à grille propre a ses
					<em>propres</em> créneaux, non partagés avec une autre.
				</p>

				{#each commissions as comm (comm.id)}
					<div class="comm-row">
						<div class="comm-main">
							<span>
								<span class="color-swatch" style="background:{comm.couleur}"></span>
								{comm.nom}
							</span>
						</div>
						<span class="grille-badge" class:propre={comm.grille_creneaux === 'propre'}>
							grille {comm.grille_creneaux === 'propre' ? 'propre' : 'générale'}
						</span>
					</div>
				{/each}

				{#if commissionsPropres().length === 0}
					<p class="note" style="margin-top:10px">
						Aucune commission n'utilise une grille propre — bascule dans l'écran Commissions.
					</p>
				{:else}
					<div style="margin-top:12px; padding-top:12px; border-top:1px solid var(--line)">
						<div class="wf-label" style="margin:0 0 8px">Commission</div>
						<div class="btn-group" style="margin:0 0 14px">
							{#each commissionsPropres() as comm (comm.id)}
								<button
									class="btn-wf"
									class:primary={selectedCommissionPropreId === comm.id}
									style="border-left: 4px solid {comm.couleur}"
									onclick={() => selectCommissionPropre(comm.id)}
								>
									{comm.nom}
								</button>
							{/each}
						</div>

						{#if selectedCommissionPropreId}
							{@const autres = autresCommissionsPropresAvecCreneaux(selectedCommissionPropreId)}
							{#if autres.length > 0}
								<div class="wf-label" style="margin:0 0 8px">
									Dupliquer depuis
									<span class="optional-tag">
										— utile en démarrant une nouvelle grille propre
									</span>
								</div>
								<div class="toolbar-row" style="margin:0 0 14px; justify-content:space-between">
									{#if dupliquerOuvert}
										<div class="btn-group">
											{#each autres as src (src.id)}
												<button
													class="btn-wf"
													class:primary={dupliquerSourceId === src.id}
													style="border-left: 4px solid {src.couleur}"
													onclick={() => (dupliquerSourceId = src.id)}
												>
													{src.nom}
												</button>
											{/each}
										</div>
										<button class="btn-wf primary" onclick={dupliquer}>Dupliquer</button>
										<button class="btn-wf" onclick={() => (dupliquerOuvert = false)}>
											Annuler
										</button>
									{:else}
										<button class="btn-wf" onclick={() => (dupliquerOuvert = true)}>
											Dupliquer une grille existante
										</button>
									{/if}
								</div>
							{/if}

							<div class="wf-label" style="margin-bottom:8px">
								Ses créneaux — {formatJourShort(selectedJour)}
							</div>
							<div class="field-grid">
								{#each creneauxDe(selectedCommissionPropreId, selectedJour) as c (c.id)}
									{#if editingId === c.id}
										<div class="field wf">
											<span class="wf-label">Début</span>
											<input class="input-wf wf" type="time" bind:value={editForm.heure_debut} />
											<span class="wf-label">Fin</span>
											<input class="input-wf wf" type="time" bind:value={editForm.heure_fin} />
											<button class="btn-wf primary" onclick={() => submitEdit(c)}>OK</button>
											<button class="btn-wf" onclick={() => (editingId = null)}>Annuler</button>
										</div>
									{:else}
										<div class="field wf">
											<span class="wf-label">Début</span>
											<span class="input-wf wf creneau-value">{toInputTime(c.heure_debut)}</span>
											<span class="wf-label">Fin</span>
											<span class="input-wf wf creneau-value">{toInputTime(c.heure_fin)}</span>
											<div class="field-actions">
												<button
													class="btn-wf icon-btn"
													onclick={() => startEdit(c)}
													aria-label="Modifier"
													title="Modifier"
												>
													<svg
														width="14"
														height="14"
														viewBox="0 0 24 24"
														fill="none"
														stroke="currentColor"
														stroke-width="2"
														stroke-linecap="round"
														stroke-linejoin="round"
													>
														<path
															d="M11 4H4a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h14a2 2 0 0 0 2-2v-7"
														/><path d="M18.5 2.5a2.121 2.121 0 0 1 3 3L12 15l-4 1 1-4Z" />
													</svg>
												</button>
												<button
													class="btn-wf danger icon-btn"
													onclick={() => removeCreneau(c)}
													aria-label="Supprimer"
													title="Supprimer"
												>
													<svg
														width="14"
														height="14"
														viewBox="0 0 24 24"
														fill="none"
														stroke="currentColor"
														stroke-width="2"
														stroke-linecap="round"
													>
														<path d="M18 6 6 18" /><path d="M6 6l12 12" />
													</svg>
												</button>
											</div>
										</div>
									{/if}
								{/each}
								{#if addKey === selectedCommissionPropreId}
									<div class="field wf">
										<span class="wf-label">Début</span>
										<input class="input-wf wf" type="time" bind:value={addForm.heure_debut} />
										<span class="wf-label">Fin</span>
										<input class="input-wf wf" type="time" bind:value={addForm.heure_fin} />
										<button
											class="btn-wf primary"
											onclick={() => submitAdd(selectedCommissionPropreId, selectedJour ?? '')}
										>
											Ajouter
										</button>
										<button class="btn-wf" onclick={() => (addKey = null)}>Annuler</button>
									</div>
								{:else}
									<button
										class="field wf add-tile"
										onclick={() => startAdd(selectedCommissionPropreId ?? '')}
									>
										+ ajouter un créneau
									</button>
								{/if}
							</div>
							<p class="note" style="margin-top:10px">
								Ces horaires ne s'appliquent qu'à la commission sélectionnée ci-dessus (et à ses
								postes, qui suivent tous la grille de leur commission).
							</p>
						{/if}
					</div>
				{/if}
			</div>
		{/if}
	{/if}
{/if}

<style>
	.creneau-value {
		display: flex;
		align-items: center;
		justify-content: center;
		min-width: 60px;
	}
	.field-actions {
		display: flex;
		gap: 6px;
		justify-content: flex-end;
		margin-top: 2px;
	}
	.add-tile {
		justify-content: center;
		align-items: center;
		color: var(--muted);
		font-size: 12px;
		cursor: pointer;
	}
	.mapping-editor {
		margin-top: 12px;
		padding-top: 12px;
		border-top: 1px dashed var(--line);
	}
	.mapping-slot-row {
		display: flex;
		align-items: center;
		gap: 10px;
		margin-bottom: 8px;
	}
	.mapping-slot-label {
		min-width: 50px;
	}
</style>
