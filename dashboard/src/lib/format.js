const DAY = 86_400_000;

const count = new Intl.NumberFormat('en', { notation: 'compact', maximumFractionDigits: 1 });

export const number = (n) => count.format(n);

export function bytes(n) {
	const units = ['B', 'KiB', 'MiB', 'GiB', 'TiB'];
	let unit = 0;
	while (n >= 1024 && unit < units.length - 1) {
		n /= 1024;
		unit += 1;
	}
	return `${unit ? n.toFixed(1) : n} ${units[unit]}`;
}

/** Epoch milliseconds, as local date and time. */
export const time = (ms) => new Date(ms).toLocaleString();

/** Days since the Unix epoch, as an ISO date. */
export const day = (days) => new Date(days * DAY).toISOString().slice(0, 10);

/**
 * An argument object for a JSON Schema's `properties`, each set to an empty
 * value of its `type`.
 */
export function skeleton(schema) {
	const empty = { string: '', number: 0, integer: 0, boolean: false, array: [], object: {} };
	const args = {};
	for (const [key, property] of Object.entries(schema?.properties ?? {})) {
		const type = Array.isArray(property.type) ? property.type[0] : property.type;
		args[key] = type in empty ? structuredClone(empty[type]) : null;
	}
	return args;
}
