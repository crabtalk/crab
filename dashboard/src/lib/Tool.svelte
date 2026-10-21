<script>
	import { untrack } from 'svelte';
	import * as api from '$lib/api.js';
	import { number, skeleton } from '$lib/format.js';

	let { program, tool, budget, onchange } = $props();

	// Seeded once from the tool and its terms, then edited in place: the page
	// reloads both after every call, and that must not undo an edit.
	let args = $state(untrack(() => JSON.stringify(skeleton(tool.parameters), null, 2)));
	let caller = $state(untrack(() => (tool.anonymous ? '' : '{"id": "dev"}')));
	let running = $state(false);
	let answer = $state(null);
	let refused = $state('');

	let instructions = $state(untrack(() => budget?.instructions ?? 0));
	let rows = $state(untrack(() => budget?.rows ?? 0));

	async function run(event) {
		event.preventDefault();
		running = true;
		refused = '';
		answer = null;
		try {
			answer = await api.call(
				program,
				tool.name,
				JSON.parse(args),
				caller.trim() ? JSON.parse(caller) : null
			);
			onchange?.();
		} catch (e) {
			refused = e.status ? `${e.status}: ${e.message}` : e.message;
		} finally {
			running = false;
		}
	}

	async function save() {
		refused = '';
		try {
			await api.setBudget(program, tool.name, {
				instructions: Number(instructions),
				rows: Number(rows)
			});
			onchange?.();
		} catch (e) {
			refused = `${e.status}: ${e.message}`;
		}
	}

	/** The text a tool answered, pretty-printed when it is JSON. */
	function pretty(text) {
		try {
			return JSON.stringify(JSON.parse(text), null, 2);
		} catch {
			return text;
		}
	}
</script>

<details class="panel">
	<summary>
		<strong>{tool.name}</strong>
		{#each ['query', 'anonymous', 'deprecated', 'cron', 'stripe'] as flag}
			{#if tool[flag]}<span class="tag">{flag}</span>{/if}
		{/each}
		{#if tool.callers}<span class="tag">{tool.callers} only</span>{/if}
		<span class="muted">{tool.description}</span>
	</summary>

	{#if tool.public}
		<form onsubmit={run}>
			<label>Arguments <textarea rows="6" bind:value={args}></textarea></label>
			<label>
				X-Berm-Caller {tool.anonymous ? '(optional)' : ''}
				<input bind:value={caller} placeholder={'{"id": "dev"}'} />
			</label>
			<p><button disabled={running}>{running ? 'Running…' : 'Call'}</button></p>
		</form>
	{:else}
		<p class="muted">Not public: only its host calls it.</p>
	{/if}

	{#if refused}<p class="error">{refused}</p>{/if}
	{#if answer}
		{#if 'done' in answer.output}
			<p class="outcome-done">done</p>
			<pre><code>{pretty(answer.output.done)}</code></pre>
		{:else}
			<p class="outcome-failed">failed</p>
			<pre><code>{answer.output.failed}</code></pre>
		{/if}
		{#if answer.meter}
			<p class="muted">
				{number(answer.meter.instructions)} instructions · {number(answer.meter.read)} read ·
				{number(answer.meter.written)} written · {answer.meter.calls} calls
			</p>
		{/if}
	{/if}

	{#if budget}
		<div class="row">
			<label>Instruction budget <input type="number" bind:value={instructions} /></label>
			<label>Row budget <input type="number" bind:value={rows} /></label>
			<button class="ghost" onclick={save}>Set budget</button>
		</div>
	{/if}

	<h3>Parameters</h3>
	<pre><code>{JSON.stringify(tool.parameters, null, 2)}</code></pre>
	{#if tool.returns}
		<h3>Returns</h3>
		<pre><code>{JSON.stringify(tool.returns, null, 2)}</code></pre>
	{/if}
</details>

<style>
	details + :global(details) {
		margin-top: 8px;
	}

	summary {
		cursor: pointer;
	}

	summary strong {
		margin-right: 8px;
	}

	form {
		display: grid;
		gap: 8px;
		margin-top: 12px;
	}
</style>
