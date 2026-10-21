<script>
	import * as api from '$lib/api.js';
	import { time } from '$lib/format.js';

	/** `call` limits the logs to those emitted in one call. */
	let { program, call = null } = $props();

	let topic = $state('');
	let items = $state([]);
	let next = $state(null);
	let error = $state('');

	function filters() {
		const query = new URLSearchParams();
		if (topic) query.set('topic', topic);
		if (call !== null) query.set('call', call);
		return query.toString();
	}

	async function load(query, more) {
		try {
			const page = await api.logs(program, query);
			items = more ? [...items, ...page.items] : page.items;
			next = page.next;
			error = '';
		} catch (e) {
			error = e.message;
		}
	}

	$effect(() => {
		load(filters(), false);
	});
</script>

{#if call === null}
	<div class="row">
		<label>Topic <input bind:value={topic} placeholder="any" /></label>
		<button class="ghost" onclick={() => load(filters(), false)}>Refresh</button>
	</div>
{/if}

{#if error}<p class="error">{error}</p>{/if}

<table>
	<thead>
		<tr><th class="num">Seq</th><th>At</th><th class="num">Call</th><th>Topics</th><th>Data</th></tr>
	</thead>
	<tbody>
		{#each items as log (log.seq)}
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

{#if next}
	<p><button class="ghost" onclick={() => load(next, true)}>Newer</button></p>
{/if}
