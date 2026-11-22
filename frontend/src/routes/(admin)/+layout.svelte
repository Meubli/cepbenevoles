<script lang="ts">
	import '#lib/styles/admin.css';
	import AdminNav from '#lib/AdminNav.svelte';
	import type { LayoutProps } from './$types';

	let { children }: LayoutProps = $props();
</script>

<div class="admin-shell">
	<main class="admin-main">
		<div class="admin-layout">
			<AdminNav />
			<div class="panel wf">
				{@render children()}
			</div>
		</div>
	</main>
</div>
