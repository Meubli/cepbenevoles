<script lang="ts">
	import { onMount } from 'svelte';
	import { api, ApiError } from '#lib/api';
	import { onLive } from '#lib/live';

	type Evenement = {
		id: string;
		nom: string;
		date_debut: string;
		date_fin: string;
		archive: boolean;
	};
	type Poste = { id: string; nom: string };
	type Commission = {
		id: string;
		nom: string;
		grille_creneaux: 'generale' | 'propre';
		besoin_chiffre: boolean;
		a_des_postes: boolean;
		couleur: string;
		postes: Poste[];
	};
	type Creneau = {
		id: string;
		commission_id: string | null;
		jour: string;
		heure_debut: string;
		heure_fin: string;
		ordre: number;
	};
	type BesoinCommissionEntry = {
		creneau_applicatif_id: string;
		commission_id: string;
		nb_requis: number;
	};
	type BesoinPosteEntry = { creneau_applicatif_id: string; poste_id: string; nb_requis: number };
	type BesoinsData = { commission: BesoinCommissionEntry[]; poste: BesoinPosteEntry[] };

	// Backend sérialise un chrono::NaiveTime en "HH:MM:SS".
	function toInputTime(t: string): string {
		return t.slice(0, 5);
	}

	// Dates manipulées en UTC uniquement (cf. écran Créneaux — même piège
	// d'off-by-one avec new Date(iso) en heure locale).
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

	function formatJourShort(iso: string): string {
		const d = parseIsoDateUtc(iso);
		const weekday = d
			.toLocaleDateString('fr-FR', { weekday: 'short', timeZone: 'UTC' })
			.replace('.', '');
		const dm = d.toLocaleDateString('fr-FR', { day: '2-digit', month: '2-digit', timeZone: 'UTC' });
		return `${weekday.charAt(0).toUpperCase()}${weekday.slice(1)} ${dm}`;
	}

	function creneauLabel(c: Creneau): string {
		return `${toInputTime(c.heure_debut)}–${toInputTime(c.heure_fin)}`;
	}

	let evenements = $state<Evenement[]>([]);
	let commissions = $state<Commission[]>([]);
	let creneaux = $state<Creneau[]>([]);
	let selectedEvenementId = $state<string | null>(null);
	let selectedJour = $state<string | null>(null);
	let selectedCommissionPropreId = $state<string | null>(null);

	let loading = $state(true);
	let error = $state<string | null>(null);
	let saving = $state(false);

	// État local des inputs, clé = "<creneauId>|<commissionId ou posteId>".
	// Reconstruit à chaque chargement (pas d'édition concurrente à préserver
	// — même choix que le formulaire de bornes de l'écran Créneaux).
	let formCommission = $state<Record<string, string>>({});
	let formPoste = $state<Record<string, string>>({});

	function keyC(creneauId: string, commissionId: string): string {
		return `${creneauId}|${commissionId}`;
	}

	function rebuildForms(d: BesoinsData) {
		const fc: Record<string, string> = {};
		for (const b of d.commission)
			fc[keyC(b.creneau_applicatif_id, b.commission_id)] = String(b.nb_requis);
		formCommission = fc;
		const fp: Record<string, string> = {};
		for (const b of d.poste) fp[keyC(b.creneau_applicatif_id, b.poste_id)] = String(b.nb_requis);
		formPoste = fp;
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
				selectedCommissionPropreId =
					comms.find((c) => c.grille_creneaux === 'propre' && c.besoin_chiffre)?.id ?? null;
			}
		} catch (e) {
			error = e instanceof ApiError ? e.message : 'Erreur inattendue';
		}
	}

	async function refreshScreenData() {
		if (!selectedEvenementId) return;
		try {
			const [creneauxData, besoinsData] = await Promise.all([
				api.get<{ creneaux: Creneau[] }>(`/creneaux/${selectedEvenementId}`),
				api.get<BesoinsData>(`/besoins/${selectedEvenementId}`)
			]);
			creneaux = creneauxData.creneaux;
			rebuildForms(besoinsData);
			const ev = evenements.find((e) => e.id === selectedEvenementId);
			if (ev) {
				const jours = joursEvenement(ev);
				if (!selectedJour || !jours.includes(selectedJour)) selectedJour = jours[0] ?? null;
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
		return onLive(['besoins', 'creneaux', 'evenements', 'commissions'], () => {
			void refreshRefs();
			void refreshScreenData();
		});
	});

	function selectEvenement(id: string) {
		selectedEvenementId = id;
		selectedJour = null;
		void refreshScreenData();
	}

	function creneauxDe(commissionId: string | null, jour: string | null): Creneau[] {
		if (!jour) return [];
		return creneaux
			.filter((c) => c.commission_id === commissionId && c.jour === jour)
			.sort((a, b) => a.ordre - b.ordre || a.heure_debut.localeCompare(b.heure_debut));
	}

	function commissionsGenerale(): Commission[] {
		return commissions.filter((c) => c.besoin_chiffre && c.grille_creneaux === 'generale');
	}

	function commissionsPropresChiffrees(): Commission[] {
		return commissions.filter((c) => c.besoin_chiffre && c.grille_creneaux === 'propre');
	}

	function totalPostes(creneauId: string, commission: Commission): number {
		return commission.postes.reduce(
			(sum, p) => sum + (parseInt(formPoste[keyC(creneauId, p.id)] ?? '0', 10) || 0),
			0
		);
	}

	function besoinCommissionValue(creneauId: string, commissionId: string): number {
		return parseInt(formCommission[keyC(creneauId, commissionId)] ?? '0', 10) || 0;
	}

	async function saveCommissions(commissionsToSave: Commission[], jour: string | null) {
		if (!selectedEvenementId || !jour) return;
		saving = true;
		error = null;
		try {
			const calls: Promise<unknown>[] = [];
			for (const comm of commissionsToSave) {
				for (const c of creneauxDe(comm.grille_creneaux === 'propre' ? comm.id : null, jour)) {
					const raw = formCommission[keyC(c.id, comm.id)];
					if (raw === undefined || raw === '') continue;
					calls.push(
						api.patch(`/besoins/${selectedEvenementId}/commission`, {
							creneau_applicatif_id: c.id,
							commission_id: comm.id,
							nb_requis: parseInt(raw, 10) || 0
						})
					);
				}
				if (comm.a_des_postes) {
					for (const c of creneauxDe(comm.grille_creneaux === 'propre' ? comm.id : null, jour)) {
						for (const p of comm.postes) {
							const raw = formPoste[keyC(c.id, p.id)];
							if (raw === undefined || raw === '') continue;
							calls.push(
								api.patch(`/besoins/${selectedEvenementId}/poste`, {
									creneau_applicatif_id: c.id,
									poste_id: p.id,
									nb_requis: parseInt(raw, 10) || 0
								})
							);
						}
					}
				}
			}
			await Promise.all(calls);
			await refreshScreenData();
		} catch (e) {
			error = e instanceof ApiError ? e.message : 'Erreur inattendue';
		} finally {
			saving = false;
		}
	}
</script>

<div class="screen-head">
	<h2>Admin — besoins par commission</h2>
	<span class="screen-tag">Saisie effectifs requis</span>
</div>
<p class="note">
	Nombre de bénévoles nécessaires, par commission et par créneau applicatif, pour l'événement
	sélectionné. Répartition par poste possible pour les commissions qui en ont. Les commissions "sans
	besoin chiffré" n'apparaissent pas ici (cf. écran Commissions).
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

	{#if selectedEvenementId}
		{@const ev = evenements.find((e) => e.id === selectedEvenementId)}
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
			{@const creneauxJour = creneauxDe(null, selectedJour)}
			<div class="panel wf" style="margin-bottom:16px">
				<h3>{formatJourShort(selectedJour)} — grille générale</h3>
				{#if creneauxJour.length === 0}
					<p class="note" style="margin-top:8px">
						Aucun créneau général ce jour — configurez-le dans l'écran Créneaux.
					</p>
				{:else if commissionsGenerale().length === 0}
					<p class="note" style="margin-top:8px">
						Aucune commission à besoin chiffré sur la grille générale.
					</p>
				{:else}
					<div style="overflow-x:auto">
						<table class="req-table">
							<thead>
								<tr>
									<th>Commission</th>
									{#each creneauxJour as c (c.id)}
										<th>{creneauLabel(c)}</th>
									{/each}
								</tr>
							</thead>
							<tbody>
								{#each commissionsGenerale() as comm (comm.id)}
									<tr>
										<td>
											<span class="color-swatch" style="background:{comm.couleur}"></span>
											{comm.nom}
											{#if comm.a_des_postes}<span class="wf-label">(a des postes)</span>{/if}
										</td>
										{#each creneauxJour as c (c.id)}
											<td>
												<input
													class="input-wf wf req-input"
													type="number"
													min="0"
													bind:value={formCommission[keyC(c.id, comm.id)]}
												/>
											</td>
										{/each}
									</tr>
								{/each}
							</tbody>
						</table>
					</div>
					<button
						class="btn-wf primary"
						style="margin-top:12px"
						disabled={saving}
						onclick={() => saveCommissions(commissionsGenerale(), selectedJour)}
					>
						Enregistrer les besoins
					</button>

					{#each commissionsGenerale().filter((c) => c.a_des_postes) as comm (comm.id)}
						<div style="margin-top:22px; padding-top:16px; border-top:1px solid var(--line)">
							<h3 style="font-size:12.5px">
								Répartition par poste — commission <em>{comm.nom}</em>
							</h3>
							<p class="note" style="margin:4px 0 10px">
								La somme des postes sur un créneau ne doit jamais dépasser le besoin total de la
								commission sur ce créneau.
							</p>
							<div style="overflow-x:auto">
								<table class="req-table">
									<thead>
										<tr>
											<th>Poste</th>
											{#each creneauxJour as c (c.id)}
												<th>{creneauLabel(c)}</th>
											{/each}
										</tr>
									</thead>
									<tbody>
										{#each comm.postes as p (p.id)}
											<tr>
												<td>{p.nom}</td>
												{#each creneauxJour as c (c.id)}
													<td>
														<input
															class="input-wf wf req-input"
															type="number"
															min="0"
															bind:value={formPoste[keyC(c.id, p.id)]}
														/>
													</td>
												{/each}
											</tr>
										{/each}
										<tr>
											<td style="font-weight:600">Total postes</td>
											{#each creneauxJour as c (c.id)}
												<td style="font-weight:600">{totalPostes(c.id, comm)}</td>
											{/each}
										</tr>
										<tr>
											<td style="color:var(--muted)">Besoin commission (ci-dessus)</td>
											{#each creneauxJour as c (c.id)}
												<td style="color:var(--muted)">{besoinCommissionValue(c.id, comm.id)}</td>
											{/each}
										</tr>
									</tbody>
								</table>
							</div>
						</div>
					{/each}
				{/if}
			</div>

			<div class="panel wf">
				<h3>Besoins — commissions à grille propre</h3>
				<p class="note" style="margin:4px 0 10px">
					Chaque commission à grille propre a ses propres créneaux (cf. écran Créneaux) — besoin
					saisi séparément pour chacune.
				</p>

				{#if commissionsPropresChiffrees().length === 0}
					<p class="note">Aucune commission à grille propre avec besoin chiffré.</p>
				{:else}
					<div class="btn-group" style="margin:0 0 14px">
						{#each commissionsPropresChiffrees() as comm (comm.id)}
							<button
								class="btn-wf"
								class:primary={selectedCommissionPropreId === comm.id}
								style="border-left: 4px solid {comm.couleur}"
								onclick={() => (selectedCommissionPropreId = comm.id)}
							>
								{comm.nom}
							</button>
						{/each}
					</div>

					{#if selectedCommissionPropreId}
						{@const comm = commissions.find((c) => c.id === selectedCommissionPropreId)}
						{#if comm}
							{@const creneauxComm = creneauxDe(comm.id, selectedJour)}
							{#if creneauxComm.length === 0}
								<p class="note">
									Aucun créneau propre ce jour pour {comm.nom} — configurez-le dans l'écran Créneaux.
								</p>
							{:else}
								<div style="overflow-x:auto">
									<table class="req-table">
										<thead>
											<tr>
												<th>Commission</th>
												{#each creneauxComm as c (c.id)}
													<th>{creneauLabel(c)}</th>
												{/each}
											</tr>
										</thead>
										<tbody>
											<tr>
												<td>
													<span class="color-swatch" style="background:{comm.couleur}"></span>
													{comm.nom}
												</td>
												{#each creneauxComm as c (c.id)}
													<td>
														<input
															class="input-wf wf req-input"
															type="number"
															min="0"
															bind:value={formCommission[keyC(c.id, comm.id)]}
														/>
													</td>
												{/each}
											</tr>
										</tbody>
									</table>
								</div>
								<button
									class="btn-wf primary"
									style="margin-top:12px"
									disabled={saving}
									onclick={() => saveCommissions([comm], selectedJour)}
								>
									Enregistrer les besoins
								</button>

								{#if comm.a_des_postes}
									<div style="margin-top:22px; padding-top:16px; border-top:1px solid var(--line)">
										<h3 style="font-size:12.5px">
											Répartition par poste — commission <em>{comm.nom}</em>
										</h3>
										<div style="overflow-x:auto">
											<table class="req-table">
												<thead>
													<tr>
														<th>Poste</th>
														{#each creneauxComm as c (c.id)}
															<th>{creneauLabel(c)}</th>
														{/each}
													</tr>
												</thead>
												<tbody>
													{#each comm.postes as p (p.id)}
														<tr>
															<td>{p.nom}</td>
															{#each creneauxComm as c (c.id)}
																<td>
																	<input
																		class="input-wf wf req-input"
																		type="number"
																		min="0"
																		bind:value={formPoste[keyC(c.id, p.id)]}
																	/>
																</td>
															{/each}
														</tr>
													{/each}
													<tr>
														<td style="font-weight:600">Total postes</td>
														{#each creneauxComm as c (c.id)}
															<td style="font-weight:600">{totalPostes(c.id, comm)}</td>
														{/each}
													</tr>
													<tr>
														<td style="color:var(--muted)">Besoin commission (ci-dessus)</td>
														{#each creneauxComm as c (c.id)}
															<td style="color:var(--muted)">
																{besoinCommissionValue(c.id, comm.id)}
															</td>
														{/each}
													</tr>
												</tbody>
											</table>
										</div>
									</div>
								{/if}
							{/if}
						{/if}
					{/if}
				{/if}
			</div>
		{/if}
	{/if}
{/if}
