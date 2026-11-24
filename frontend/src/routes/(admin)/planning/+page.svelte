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
	type Souhait = { commission_id: string; poste_id: string | null };
	type BenevoleJour = {
		soumission_id: string;
		nom: string;
		prenom: string;
		permis_b: boolean;
		polyvalent: boolean;
		creneau_ids: string[];
		souhaits: Souhait[];
	};
	type Affectation = {
		id: string;
		soumission_id: string;
		creneau_applicatif_id: string;
		commission_id: string;
		poste_id: string | null;
		verrouille: boolean;
	};

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
	let creneauxJour = $state<Creneau[]>([]);
	let benevoles = $state<BenevoleJour[]>([]);
	let affectations = $state<Affectation[]>([]);

	let selectedEvenementId = $state<string | null>(null);
	let selectedJour = $state<string | null>(null);

	let loading = $state(true);
	let error = $state<string | null>(null);
	let saving = $state(false);

	// Cellule sur laquelle le menu d'affectation est ouvert, clé =
	// "<soumissionId>|<creneauId>" — un seul menu ouvert à la fois.
	let assignKey = $state<string | null>(null);
	let assignCommissionId = $state<string | null>(null);
	let assignPosteId = $state<string | null>(null);

	function cellKey(soumissionId: string, creneauId: string): string {
		return `${soumissionId}|${creneauId}`;
	}

	// Seules les commissions à grille générale sont assignables depuis ce
	// tableau — les colonnes ne couvrent que la grille générale du jour
	// (choix fait avec l'utilisateur ; Plateau, à grille propre, n'a pas sa
	// place dans cette vue pour l'instant).
	function commissionsAssignables(): Commission[] {
		return commissions.filter((c) => c.grille_creneaux === 'generale');
	}

	function commissionNom(id: string): string {
		return commissions.find((c) => c.id === id)?.nom ?? '?';
	}
	function commission(id: string): Commission | undefined {
		return commissions.find((c) => c.id === id);
	}
	function posteNom(commissionId: string, posteId: string): string {
		return commission(commissionId)?.postes.find((p) => p.id === posteId)?.nom ?? '?';
	}

	function affectationFor(soumissionId: string, creneauId: string): Affectation | undefined {
		return affectations.find(
			(a) => a.soumission_id === soumissionId && a.creneau_applicatif_id === creneauId
		);
	}

	async function refresh() {
		if (!selectedEvenementId || !selectedJour) return;
		try {
			const [creneauxData, benevolesData, affectationsData] = await Promise.all([
				api.get<{ creneaux: Creneau[] }>(`/creneaux/${selectedEvenementId}`),
				api.get<BenevoleJour[]>(`/planning/${selectedEvenementId}/benevoles?jour=${selectedJour}`),
				api.get<Affectation[]>(`/planning/${selectedEvenementId}/affectations?jour=${selectedJour}`)
			]);
			creneauxJour = creneauxData.creneaux
				.filter((c) => c.commission_id === null && c.jour === selectedJour)
				.sort((a, b) => a.ordre - b.ordre || a.heure_debut.localeCompare(b.heure_debut));
			benevoles = benevolesData;
			affectations = affectationsData;
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
		await refresh();
		loading = false;
	}

	onMount(() => {
		void load();
		return onLive(['planning', 'evenements', 'creneaux', 'commissions'], () => {
			void refreshRefs();
			void refresh();
		});
	});

	function selectEvenement(id: string) {
		selectedEvenementId = id;
		selectedJour = null;
		assignKey = null;
		void refreshRefs().then(refresh);
	}

	function selectJour(jour: string) {
		selectedJour = jour;
		assignKey = null;
		void refresh();
	}

	function openAssign(soumissionId: string, creneauId: string) {
		assignKey = cellKey(soumissionId, creneauId);
		assignCommissionId = null;
		assignPosteId = null;
	}

	function closeAssign() {
		assignKey = null;
	}

	function selectAssignCommission(id: string) {
		assignCommissionId = id;
		assignPosteId = null;
	}

	async function confirmAssign(soumissionId: string, creneauId: string) {
		if (!selectedEvenementId || !assignCommissionId) return;
		error = null;
		saving = true;
		try {
			await api.post(`/planning/${selectedEvenementId}/affectations`, {
				soumission_id: soumissionId,
				creneau_applicatif_id: creneauId,
				commission_id: assignCommissionId,
				poste_id: assignPosteId
			});
			assignKey = null;
			await refresh();
		} catch (e) {
			error = e instanceof ApiError ? e.message : 'Erreur inattendue';
		} finally {
			saving = false;
		}
	}

	async function removeAffectation(a: Affectation) {
		if (!selectedEvenementId) return;
		error = null;
		try {
			await api.delete(`/planning/${selectedEvenementId}/affectations/${a.id}`);
			await refresh();
		} catch (e) {
			error = e instanceof ApiError ? e.message : 'Erreur inattendue';
		}
	}
</script>

<div class="screen-head">
	<h2>Planning — événement / jour</h2>
	<span class="screen-tag">Affectation manuelle — grille générale</span>
</div>
<p class="note">
	Sélection événement + jour, puis tableau bénévoles × créneaux de la grille générale. Cellule
	cliquable pour affecter ou retirer une commission à la main — toute affectation posée ici est
	verrouillée (§6.3), la génération automatique n'existe pas encore.
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
					onclick={() => selectJour(jour)}
				>
					{formatJourShort(jour)}
				</button>
			{/each}
		</div>

		{#if selectedJour}
			{#if creneauxJour.length === 0}
				<p class="note">
					Aucun créneau de grille générale ce jour — configurez-le dans l'écran Créneaux.
				</p>
			{:else if benevoles.length === 0}
				<p class="note">Aucun bénévole n'a déclaré de disponibilité ce jour.</p>
			{:else}
				<div class="pill" style="margin-bottom:10px">{benevoles.length} bénévoles ce jour</div>
				<div class="grid-wrap">
					<table class="req-table planning-table">
						<thead>
							<tr>
								<th>Bénévole</th>
								{#each creneauxJour as c (c.id)}
									<th>{creneauLabel(c)}</th>
								{/each}
							</tr>
						</thead>
						<tbody>
							{#each benevoles as b (b.soumission_id)}
								<tr>
									<td style="text-align:left">
										{b.nom} · {b.prenom}
										{#if !b.permis_b}
											<span class="wf-label">(sans permis B)</span>
										{/if}
									</td>
									{#each creneauxJour as c (c.id)}
										{@const a = affectationFor(b.soumission_id, c.id)}
										{@const key = cellKey(b.soumission_id, c.id)}
										<td>
											{#if a}
												<span
													class="cell-tag"
													style="border-left-color:{commission(a.commission_id)?.couleur ??
														'var(--line)'}"
												>
													{commissionNom(a.commission_id)}{a.poste_id
														? ` · ${posteNom(a.commission_id, a.poste_id)}`
														: ''}
												</span>
												<button
													class="btn-wf danger icon-btn"
													style="margin-left:4px"
													onclick={() => removeAffectation(a)}
													aria-label="Retirer"
													title="Retirer"
												>
													✕
												</button>
											{:else if b.creneau_ids.includes(c.id)}
												<button class="btn-wf" onclick={() => openAssign(b.soumission_id, c.id)}>
													+
												</button>
											{:else}
												<span class="cell-empty">—</span>
											{/if}

											{#if assignKey === key}
												<div class="panel wf assign-popover">
													<div class="wf-label" style="margin-bottom:6px">
														Affecter {b.nom} · {b.prenom} — {creneauLabel(c)}
													</div>
													{#if b.souhaits.length > 0}
														<p class="note" style="margin:0 0 8px">
															Souhaits : {b.souhaits
																.map((s) =>
																	s.poste_id
																		? `${commissionNom(s.commission_id)} · ${posteNom(s.commission_id, s.poste_id)}`
																		: commissionNom(s.commission_id)
																)
																.join(', ')}
														</p>
													{/if}
													<div class="btn-group" style="margin-bottom:8px">
														{#each commissionsAssignables() as comm (comm.id)}
															<button
																class="btn-wf"
																class:primary={assignCommissionId === comm.id}
																style="border-left: 4px solid {comm.couleur}"
																onclick={() => selectAssignCommission(comm.id)}
															>
																{comm.nom}
															</button>
														{/each}
													</div>
													{#if assignCommissionId && (commission(assignCommissionId)?.postes.length ?? 0) > 0}
														<div class="wf-label" style="margin-bottom:6px">Poste</div>
														<div class="btn-group" style="margin-bottom:8px">
															<button
																class="btn-wf"
																class:primary={assignPosteId === null}
																onclick={() => (assignPosteId = null)}
															>
																Aucun poste précis
															</button>
															{#each commission(assignCommissionId)?.postes ?? [] as poste (poste.id)}
																<button
																	class="btn-wf"
																	class:primary={assignPosteId === poste.id}
																	onclick={() => (assignPosteId = poste.id)}
																>
																	{poste.nom}
																</button>
															{/each}
														</div>
													{/if}
													<div class="toolbar-row" style="margin:0">
														<button
															class="btn-wf primary"
															disabled={!assignCommissionId || saving}
															onclick={() => confirmAssign(b.soumission_id, c.id)}
														>
															Affecter
														</button>
														<button class="btn-wf" onclick={closeAssign}>Annuler</button>
													</div>
												</div>
											{/if}
										</td>
									{/each}
								</tr>
							{/each}
						</tbody>
					</table>
				</div>
			{/if}
		{/if}
	{/if}
{/if}

<style>
	.planning-table td {
		position: relative;
	}
	.assign-popover {
		position: absolute;
		top: 100%;
		left: 0;
		z-index: 6;
		min-width: 260px;
		margin-top: 4px;
		text-align: left;
	}
</style>
