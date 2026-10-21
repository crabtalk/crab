<script>
	import { untrack } from 'svelte';
	import * as api from '$lib/api.js';
	import { time } from '$lib/format.js';
	import { Paged } from '$lib/paged.svelte.js';

	/** `call` limits the logs to those emitted in one call. */
	let { program, call = null } = $props();

	let topic = $state('');
	const logs = new Paged((query) => api.logs(program, query));

	/** Reads `topic` untracked: a topic applies on submit, not as it is typed. */
	function filters() {
		const query = new URLSearchParams();
		const words = untrack(() => topic);
		if (words) query.set('topic', words);
		if (call !== null) query.set('call', call);
		return query.toString();
	}

	$effect(() => {
		logs.load(filters());
	});
</script>

{#if call === null}
	<form class="row" onsubmit={(event) => (event.preventDefault(), logs.load(filters()))}>
		<label>Topic <input bind:value={topic} placeholder="any" /></label>
		<button class="ghost">Refresh</button>
	</form>
{/if}

{#if logs.error}<p class="error">{logs.error}</p>{/if}

<table>
	<thead>
		<tr><th class="num">Seq</th><th>At</th><th class="num">Call</th><th>Topics</th><th>Data</th></tr>
	</thead>
	<tbody>
		{#each logs.items as log (log.seq)}
			<tr>
				<td class="num">{log.seq}</td>
				<td>{time(log.at)}</td>
				<td class="num">{log.call ?? ''}</td>
				<td>{#each log.topics as one}<span class="tag">{one}</span>{/each}</td>
				<td><code>{log.data}</code></td>
			</tr>
		{:else}
			<tr><td colspan="5" class="muted">No logs.</td></tr>
		{/each}
	</tbody>
</table>

{#if logs.next}
	<p><button class="ghost" onclick={() => logs.load(logs.next, true)}>Newer</button></p>
{/if}
