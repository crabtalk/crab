<script>
	import * as api from '$lib/api.js';
	import { time } from '$lib/format.js';

	let { program } = $props();

	let crons = $state(null);
	let wakes = $state(null);
	let edits = $state({});
	let error = $state('');

	async function load() {
		try {
			[crons, wakes] = await Promise.all([api.crons(program), api.wakes(program)]);
			edits = Object.fromEntries(crons.crons.map((job) => [job.tool, job.cron ?? '']));
			error = '';
		} catch (e) {
			error = e.message;
		}
	}

	async function set(tool) {
		try {
			await api.setCron(program, tool, edits[tool].trim() || null);
			await load();
		} catch (e) {
			error = e.message;
		}
	}

	$effect(() => {
		load();
	});
</script>

{#if error}<p class="error">{error}</p>{/if}

{#if crons}
	<h2>Crons {#if crons.held}<span class="tag">held</span>{/if}</h2>
	<table>
		<thead>
			<tr><th>Tool</th><th>Expression (UTC)</th><th>Next run</th><th class="num">Failures</th><th>Last</th></tr>
		</thead>
		<tbody>
			{#each crons.crons as job (job.tool)}
				<tr>
					<td>{job.tool}</td>
					<td>
						<form class="row" onsubmit={(event) => (event.preventDefault(), set(job.tool))}>
							<input bind:value={edits[job.tool]} placeholder="stopped" />
							<button class="ghost">Set</button>
						</form>
					</td>
					<td>{job.due ? time(job.due) : ''}</td>
					<td class="num">{job.failures}</td>
					<td>
						{#if job.last}
							<span class="outcome-{job.last.outcome}">{job.last.outcome}</span> {time(job.last.at)}
						{/if}
					</td>
				</tr>
			{:else}
				<tr><td colspan="5" class="muted">No cron tools.</td></tr>
			{/each}
		</tbody>
	</table>
{/if}

{#if wakes}
	<h2>Wakes {#if wakes.held}<span class="tag">held</span>{/if}</h2>
	<table>
		<thead>
			<tr><th>Tool</th><th>Due</th><th>By</th><th class="num">Failures</th><th>Arguments</th></tr>
		</thead>
		<tbody>
			{#each wakes.wakes as wake (`${wake.tool} ${wake.due} ${wake.args}`)}
				<tr>
					<td>{wake.tool}</td>
					<td>{time(wake.due)}</td>
					<td>{wake.by}</td>
					<td class="num">{wake.failures}</td>
					<td><code>{wake.args}</code></td>
				</tr>
			{:else}
				<tr><td colspan="5" class="muted">No wakes armed.</td></tr>
			{/each}
		</tbody>
	</table>
{/if}
