ALTER TABLE relay_agents ADD COLUMN handle TEXT NOT NULL DEFAULT '';
WITH RECURSIVE normalized(id, owner, rest, slug) AS (
    SELECT id, owner, lower(name), '' FROM relay_agents
    UNION ALL
    SELECT id, owner, substr(rest, 2), slug || CASE WHEN substr(rest, 1, 1) GLOB '[a-z0-9_-]' THEN substr(rest, 1, 1) ELSE '-' END FROM normalized WHERE rest != ''
), candidates AS (
    SELECT id, owner, CASE WHEN substr(trim(slug, '-'), 1, 48) GLOB '*[a-z]*' THEN substr(trim(slug, '-'), 1, 48) ELSE 'agent' END AS handle FROM normalized WHERE rest = ''
), ranked AS (
    SELECT id, handle, row_number() OVER (PARTITION BY owner, handle ORDER BY id) AS rank FROM candidates
)
UPDATE relay_agents SET handle = (SELECT handle || CASE WHEN rank = 1 THEN '' ELSE '-' || lower(hex(randomblob(6))) END FROM ranked WHERE ranked.id = relay_agents.id);
CREATE UNIQUE INDEX relay_agents_handle ON relay_agents(owner, handle);
