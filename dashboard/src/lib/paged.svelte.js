/**
 * A list read a page at a time from `fetch(query)`, which answers
 * `{items, next}` as `api.calls` and `api.logs` do.
 */
export class Paged {
	items = $state([]);
	next = $state(null);
	error = $state('');
	#fetch;

	constructor(fetch) {
		this.#fetch = fetch;
	}

	async load(query, more = false) {
		try {
			const page = await this.#fetch(query);
			this.items = more ? [...this.items, ...page.items] : page.items;
			this.next = page.next;
			this.error = '';
		} catch (e) {
			this.error = e.message;
		}
	}
}
