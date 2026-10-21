<script>
	import { goto } from '$app/navigation';
	import { page } from '$app/state';
	import * as api from '$lib/api.js';
	import { bytes, day, number, time } from '$lib/format.js';
	import Calls from '$lib/Calls.svelte';
	import Logs from '$lib/Logs.svelte';
	import Schedule from '$lib/Schedule.svelte';
	import Tool from '$lib/Tool.svelte';

	/** How often the live counts are read again, in milliseconds. */
	const LIVE_EVERY = 2000;

	const tabs = ['tools', 'calls', 'logs', 'schedule', 'usage', 'meters'];

	let name = $derived(page.params.name);
	let tab = $state('tools');

	let listing = $state(null);
	let runtime = $state(null);
	let terms = $state(null);
	let usage = $state(null);
	let meters = $state(null);
	let live = $state(null);
	let error = $state('');
	let calls = $state();

	async function load() {
		try {
			[listing, runtime, terms, usage, meters] = await Promise.all([
				api.listing(name),
				api.runtime(name),
				api.terms(name),
				api.usage(name),
				api.meters(name)
			]);
			error = '';
		} catch (e) {
			error = e.message;
		}
	}

	$effect(() => {
		load();
	});

	$effect(() => {
		const program = name;
		const read = () =>
			api
				.live(program)
				.then((now) => (live = now))
				.catch(() => {});
		read();
		const timer = setInterval(read, LIVE_EVERY);
		return () => clearInterval(timer);
	});

	function called() {
		load();
		calls?.refresh();
	}

	async function undeploy() {
		if (!confirm(`Undeploy ${name}?`)) return;
		try {
			await api.undeploy(name);
			goto('#/');
		} catch (e) {
			error = `${e.status}: ${e.message}`;
		}
	}

	let tools = $derived([...(listing?.program.tools ?? []), ...(runtime?.tools ?? [])]);
</script>

{#if error}
	<p class="error">{error}</p>
{/if}

{#if listing}
	{@const program = listing.program}
	<div class="title">
		<div>
			<h1>{program.name} <span class="muted">{program.version}</span></h1>
			<p class="muted">{program.description}</p>
			<p class="digest">{program.digest}</p>
		</div>
		<button class="danger" onclick={undeploy}>Undeploy</button>
	</div>

	{#if live}
		<div class="live">
			<div class="panel"><span class="muted">Running</span><strong>{live.running}</strong></div>
			<div class="panel"><span class="muted">Memory</span><strong>{bytes(live.memory)}</strong></div>
			<div class="panel"><span class="muted">Stored</span><strong>{bytes(live.stored)}</strong></div>
			<div class="panel">
				<span class="muted">Instructions</span><strong>{number(live.instructions.total)}</strong>
			</div>
			<div class="panel">
				<span class="muted">Rows r/w</span>
				<strong>{number(live.read.total)} / {number(live.written.total)}</strong>
			</div>
			<div class="panel"><span class="muted">Logged</span><strong>{bytes(live.logged.total)}</strong></div>
			<div class="panel"><span class="muted">Served</span><strong>{number(listing.served.total)}</strong></div>
		</div>
	{/if}

	{#if runtime?.unresolved}
		<p class="error">Unresolved deps: <code>{JSON.stringify(runtime.unresolved)}</code></p>
	{/if}

	<nav>
		{#each tabs as one}
			<button class:ghost={tab !== one} onclick={() => (tab = one)}>{one}</button>
		{/each}
	</nav>

	{#if tab === 'tools'}
		{#key name}
			{#each tools as tool (tool.name)}
				<Tool program={name} {tool} budget={terms?.tools[tool.name]?.budget} onchange={called} />
			{:else}
				<p class="muted">No tools.</p>
			{/each}
		{/key}
		{#if runtime?.events.length}
			<h2>Events</h2>
			<pre><code>{JSON.stringify(runtime.events, null, 2)}</code></pre>
		{/if}
		{#if runtime}
			<h2>Deps</h2>
			<pre><code>{JSON.stringify(runtime.deps, null, 2)}</code></pre>
		{/if}
	{:else if tab === 'calls'}
		<Calls bind:this={calls} program={name} {tools} />
	{:else if tab === 'logs'}
		<Logs program={name} />
	{:else if tab === 'schedule'}
		<Schedule program={name} />
	{:else if tab === 'usage'}
		<table>
			<thead>
				<tr>
					<th>Kind</th><th>Item</th><th>Source</th><th>Caller</th><th>Outcome</th>
					<th class="num">Calls</th><th>Last</th>
				</tr>
			</thead>
			<tbody>
				{#each usage ?? [] as use (`${use.kind} ${use.item} ${use.source} ${use.caller} ${use.outcome}`)}
					<tr>
						<td>{use.kind}</td>
						<td>{use.item}</td>
						<td>{use.source}</td>
						<td>{use.caller}</td>
						<td class="outcome-{use.outcome}">{use.outcome}</td>
						<td class="num">{use.calls}</td>
						<td>{time(use.last * 1000)}</td>
					</tr>
				{:else}
					<tr><td colspan="7" class="muted">No callers in the last 30 days.</td></tr>
				{/each}
			</tbody>
		</table>
	{:else if tab === 'meters'}
		<table>
			<thead>
				<tr>
					<th>Day</th><th class="num">Calls</th><th class="num">Failed</th><th class="num">Instructions</th>
					<th class="num">Memory</th><th class="num">Read</th><th class="num">Written</th>
					<th class="num">Proxied</th><th class="num">Sent</th><th class="num">Logged</th><th class="num">Stored</th>
				</tr>
			</thead>
			<tbody>
				{#each meters ?? [] as one (one.day)}
					<tr>
						<td>{day(one.day)}</td>
						<td class="num">{number(one.calls)}</td>
						<td class="num">{number(one.failed)}</td>
						<td class="num">{number(one.instructions)}</td>
						<td class="num">{bytes(one.memory)}</td>
						<td class="num">{number(one.read)}</td>
						<td class="num">{number(one.written)}</td>
						<td class="num">{bytes(one.proxied)}</td>
						<td class="num">{bytes(one.sent)}</td>
						<td class="num">{bytes(one.logged)}</td>
						<td class="num">{bytes(one.stored)}</td>
					</tr>
				{:else}
					<tr><td colspan="11" class="muted">Nothing metered in the last 30 days.</td></tr>
				{/each}
			</tbody>
		</table>
	{/if}
{:else if !error}
	<p class="muted">Loading…</p>
{/if}

<style>
	.title {
		display: flex;
		justify-content: space-between;
		align-items: start;
		gap: 16px;
	}

	.title p {
		margin: 4px 0;
	}

	.live {
		display: grid;
		grid-template-columns: repeat(auto-fill, minmax(140px, 1fr));
		gap: 8px;
		margin: 16px 0;
	}

	.live .panel {
		display: grid;
		gap: 2px;
		padding: 10px 12px;
		font-size: 13px;
	}

	.live strong {
		font-size: 17px;
		font-variant-numeric: tabular-nums;
	}

	nav {
		display: flex;
		gap: 6px;
		margin: 16px 0;
	}
</style>
