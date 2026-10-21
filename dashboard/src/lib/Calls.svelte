<script>
	import * as api from '$lib/api.js';
	import { number, time } from '$lib/format.js';
	import Logs from '$lib/Logs.svelte';

	let { program, tools } = $props();

	let tool = $state('');
	let outcome = $state('');
	let items = $state([]);
	let next = $state(null);
	let open = $state(null);
	let error = $state('');

	function filters() {
		const query = new URLSearchParams();
		if (tool) query.set('tool', tool);
		if (outcome) query.set('outcome', outcome);
		return query.toString();
	}

	async function load(query, more) {
		try {
			const page = await api.calls(program, query);
			items = more ? [...items, ...page.items] : page.items;
			next = page.next;
			error = '';
		} catch (e) {
			error = e.message;
		}
	}

	export function refresh() {
		open = null;
		return load(filters(), false);
	}

	$effect(() => {
		load(filters(), false);
	});
</script>

<div class="row">
	<label>
		Tool
		<select bind:value={tool}>
			<option value="">any</option>
			{#each tools as one (one.name)}<option>{one.name}</option>{/each}
		</select>
	</label>
	<label>
		Outcome
		<select bind:value={outcome}>
			<option value="">any</option>
			{#each ['done', 'failed', 'denied', 'refused'] as one}<option>{one}</option>{/each}
		</select>
	</label>
	<button class="ghost" onclick={refresh}>Refresh</button>
</div>

{#if error}<p class="error">{error}</p>{/if}

<table>
	<thead>
		<tr>
			<th class="num">#</th><th>At</th><th>Tool</th><th>Source</th><th>Caller</th><th>Outcome</th>
			<th class="num">ms</th><th class="num">Instructions</th><th class="num">Rows r/w</th><th class="num">Logs</th>
		</tr>
	</thead>
	<tbody>
		{#each items as call (call.id)}
			<tr>
				<td class="num">{call.id}</td>
				<td>{time(call.at)}</td>
				<td>{call.tool}</td>
				<td>{call.source}</td>
				<td>{call.caller}</td>
				<td class="outcome-{call.outcome}">{call.outcome}</td>
				<td class="num">{call.ms}</td>
				<td class="num">{call.meter ? number(call.meter.instructions) : ''}</td>
				<td class="num">{call.meter ? `${call.meter.read}/${call.meter.written}` : ''}</td>
				<td class="num">
					{#if call.logs}
						<button class="ghost" onclick={() => (open = open === call.id ? null : call.id)}>
							{call.logs}
						</button>
					{:else}0{/if}
				</td>
			</tr>
			{#if open === call.id}
				<tr><td colspan="10"><Logs {program} call={call.id} /></td></tr>
			{/if}
		{:else}
			<tr><td colspan="10" class="muted">No calls.</td></tr>
		{/each}
	</tbody>
</table>

{#if next}
	<p><button class="ghost" onclick={() => load(next, true)}>Older</button></p>
{/if}
