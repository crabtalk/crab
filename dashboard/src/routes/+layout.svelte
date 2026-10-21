<script>
	import { ModeWatcher, toggleMode } from 'mode-watcher';
	import '../../../www/src/app.css';
	import '../dashboard.css';

	let { children } = $props();
</script>

<ModeWatcher />

<header>
	<a class="wordmark" href="#/">bermd</a>
	<button class="ghost" onclick={toggleMode}>theme</button>
</header>

<main>
	{@render children()}
</main>

<style>
	header {
		position: sticky;
		top: 0;
		z-index: 1;
		display: flex;
		align-items: center;
		justify-content: space-between;
		height: var(--header);
		padding: 0 24px;
		background: var(--bg);
		border-bottom: 1px solid var(--line);
	}

	.wordmark {
		font-weight: 600;
		color: var(--strong);
	}

	main {
		max-width: 1120px;
		margin: 0 auto;
		padding: 24px;
	}
</style>
