import { sveltekit } from '@sveltejs/kit/vite';

// `BERMD` is the bermd `vite dev` sends the control API to.
const bermd = process.env.BERMD ?? 'http://127.0.0.1:7777';

export default {
	plugins: [sveltekit()],
	server: {
		fs: { allow: ['.', '../www/src'] },
		proxy: { '/programs': bermd }
	}
};
