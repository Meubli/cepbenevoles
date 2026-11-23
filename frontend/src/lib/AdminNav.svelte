<script lang="ts">
	import { page } from '$app/state';
	import { resolve } from '$app/paths';

	// Every admin screen from mockups/wireframes.html's sidebar (screens
	// s3-s10), in the same order. Only Événements/Mapping form/Admins are
	// real routes today; the rest aren't built yet (cf. docs/HANDOFF.md) and
	// render disabled rather than as dead links or being silently dropped
	// from the nav — a preview of the final shape, not a lie about what
	// works today.
	const betweenEvenementsAndMapping = ['Bénévoles'];
	const betweenMappingAndAdmins = ['Générer planning'];

	const besoinsHref = resolve('/(admin)/besoins');
	const responsablesHref = resolve('/(admin)/responsables');
	const creneauxHref = resolve('/(admin)/creneaux');
	const commissionsHref = resolve('/(admin)/commissions');
	const evenementsHref = resolve('/(admin)/evenements');
	const mappingHref = resolve('/(admin)/mapping');
	const adminsHref = resolve('/(admin)/admins');
	const synchroHref = resolve('/(admin)/synchro');
</script>

<nav class="admin-nav wf" aria-label="Navigation admin">
	<a class="admin-nav-item" class:sel={page.url.pathname === besoinsHref} href={besoinsHref}>
		Besoins
	</a>
	<a class="admin-nav-item" class:sel={page.url.pathname === creneauxHref} href={creneauxHref}>
		Créneaux
	</a>
	<a
		class="admin-nav-item"
		class:sel={page.url.pathname === commissionsHref}
		href={commissionsHref}
	>
		Commissions
	</a>
	<a class="admin-nav-item" class:sel={page.url.pathname === evenementsHref} href={evenementsHref}>
		Événements
	</a>
	{#each betweenEvenementsAndMapping as label (label)}
		<span class="admin-nav-item disabled" aria-disabled="true">{label}</span>
	{/each}
	<a class="admin-nav-item" class:sel={page.url.pathname === synchroHref} href={synchroHref}>
		Synchro form
	</a>
	<a class="admin-nav-item" class:sel={page.url.pathname === mappingHref} href={mappingHref}>
		Mapping form
	</a>
	{#each betweenMappingAndAdmins as label (label)}
		<span class="admin-nav-item disabled" aria-disabled="true">{label}</span>
	{/each}
	<a class="admin-nav-item" class:sel={page.url.pathname === adminsHref} href={adminsHref}>
		Admins
	</a>
	<a
		class="admin-nav-item"
		class:sel={page.url.pathname === responsablesHref}
		href={responsablesHref}
	>
		Responsables
	</a>
</nav>
