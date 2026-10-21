// A client of bermd's control API, on the origin the dashboard is served from.

const CALLER = 'X-Berm-Caller';
const METER = 'Berm-Meter';

/** A request the API refused: its status and the `error` it answered. */
export class Refused extends Error {
	constructor(status, message) {
		super(message);
		this.status = status;
	}
}

async function send(path, init) {
	const response = await fetch(path, init);
	if (!response.ok) {
		const body = await response.json().catch(() => null);
		throw new Refused(response.status, body?.error ?? response.statusText);
	}
	return response;
}

async function get(path) {
	return (await send(path)).json();
}

/** The query `Link` names as `rel="next"`, or `null` on the last page. */
function next(response) {
	const match = /<\?([^>]*)>;\s*rel="next"/.exec(response.headers.get('Link') ?? '');
	return match ? match[1] : null;
}

async function page(path, query) {
	const response = await send(`${path}?${query}`);
	return { items: await response.json(), next: next(response) };
}

const at = (name) => `/programs/${encodeURIComponent(name)}`;

export const programs = () => get('/programs');
export const listing = (name) => get(at(name));
export const runtime = (name) => get(`${at(name)}/runtime`);
export const terms = (name) => get(`${at(name)}/terms`);
export const usage = (name) => get(`${at(name)}/usage`);
export const meters = (name) => get(`${at(name)}/meters`);
export const live = (name) => get(`${at(name)}/live`);
export const crons = (name) => get(`${at(name)}/crons`);
export const wakes = (name) => get(`${at(name)}/wakes`);

/** A page of calls, newest first; `query` is `CallPage` as a query string. */
export const calls = (name, query) => page(`${at(name)}/calls`, query);

/** A page of logs, oldest first; `query` is `Page` as a query string. */
export const logs = (name, query) => page(`${at(name)}/logs`, query);

/** Deploy `image` as `name`, answering `{program, breaks}`. */
export async function deploy(name, image, { public: tools, crons, force }) {
	const query = new URLSearchParams();
	if (tools) query.set('public', tools);
	if (crons) query.set('crons', crons);
	if (force) query.set('force', 'true');
	const response = await send(`${at(name)}?${query}`, { method: 'PUT', body: image });
	return response.json();
}

export async function undeploy(name) {
	await send(at(name), { method: 'DELETE' });
}

/**
 * Call `tool` with the argument object `args`, as `caller` when one is given,
 * answering its `{done}` or `{failed}` and the `Berm-Meter` it carried.
 */
export async function call(name, tool, args, caller) {
	const headers = { 'Content-Type': 'application/json' };
	if (caller) headers[CALLER] = JSON.stringify(caller);
	const response = await send(`${at(name)}/tools/${encodeURIComponent(tool)}`, {
		method: 'POST',
		headers,
		body: JSON.stringify(args)
	});
	const meter = response.headers.get(METER);
	return { output: await response.json(), meter: meter ? JSON.parse(meter) : null };
}

export async function setBudget(name, tool, budget) {
	await send(`${at(name)}/tools/${encodeURIComponent(tool)}/budget`, {
		method: 'PUT',
		headers: { 'Content-Type': 'application/json' },
		body: JSON.stringify(budget)
	});
}

/** Set a cron tool's expression; `null` stops it. */
export async function setCron(name, tool, cron) {
	await send(`${at(name)}/tools/${encodeURIComponent(tool)}/cron`, {
		method: 'PUT',
		headers: { 'Content-Type': 'application/json' },
		body: JSON.stringify({ cron })
	});
}
