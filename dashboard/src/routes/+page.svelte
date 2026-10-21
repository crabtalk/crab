<script>
	import * as api from '$lib/api.js';

	let programs = $state(null);
	let error = $state('');

	let file = $state(null);
	let name = $state('');
	let tools = $state('');
	let crons = $state('');
	let force = $state(false);
	let deploying = $state(false);
	let deployed = $state(null);
	let refused = $state('');

	async function load() {
		try {
			programs = await api.programs();
			error = '';
		} catch (e) {
			error = e.message;
		}
	}

	$effect(() => {
		load();
	});

	function pick(event) {
		file = event.currentTarget.files?.[0] ?? null;
		if (file && !name) name = file.name.replace(/\.wasm$/, '').toLowerCase().replaceAll('.', '_');
	}

	async function deploy(event) {
		event.preventDefault();
		deploying = true;
		refused = '';
		deployed = null;
		try {
			deployed = await api.deploy(name, file, { public: tools, crons, force });
			await load();
		} catch (e) {
			refused = e.message;
		} finally {
			deploying = false;
		}
	}
</script>

<h1>Programs</h1>

{#if error}
	<p class="error">{error}</p>
{:else if programs === null}
	<p class="muted">Loading…</p>
{:else if programs.length === 0}
	<p class="muted">Nothing is deployed.</p>
{:else}
	<table>
		<thead>
			<tr><th>Name</th><th>Version</th><th>Description</th><th class="num">Public tools</th></tr>
		</thead>
		<tbody>
			{#each programs as program (program.name)}
				<tr>
					<td><a href="#/programs/{program.name}">{program.name}</a></td>
					<td>{program.version}</td>
					<td class="muted">{program.description}</td>
					<td class="num">{program.tools.length}</td>
				</tr>
			{/each}
		</tbody>
	</table>
{/if}

<h2>Deploy</h2>

<form class="panel" onsubmit={deploy}>
	<div class="row">
		<label>Image <input type="file" accept=".wasm" onchange={pick} required /></label>
		<label>Name <input bind:value={name} pattern="[a-z0-9_\-]+" required /></label>
		<label>Public tools <input bind:value={tools} placeholder="echo,search" /></label>
		<label>Crons <input bind:value={crons} placeholder={'{"tick": "*/5 * * * *"}'} /></label>
		<label><span><input type="checkbox" bind:checked={force} /> force</span></label>
		<button disabled={!file || deploying}>{deploying ? 'Deploying…' : 'Deploy'}</button>
	</div>
	{#if refused}
		<p class="error">{refused}</p>
	{/if}
	{#if deployed}
		<p>
			Deployed <a href="#/programs/{deployed.program.name}">{deployed.program.name}</a>
			<span class="digest">{deployed.program.digest}</span>
		</p>
		{#if deployed.breaks?.length}
			<p class="muted">It broke:</p>
			<ul>
				{#each deployed.breaks as broke}<li>{broke}</li>{/each}
			</ul>
		{/if}
	{/if}
</form>
