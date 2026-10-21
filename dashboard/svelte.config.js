import adapter from '@sveltejs/adapter-static';

// Hash routing: the build is one `index.html` and its assets, so a page's path
// never collides with a route of the control API it is served beside.
export default {
	kit: {
		adapter: adapter({ fallback: 'index.html' }),
		router: { type: 'hash' }
	}
};
