-- CPU time in microseconds in place of wasm instructions, converted at 10,000
-- instructions a microsecond (measured 2026-10-21 under wasmtime 48 with fuel:
-- 4.7G to 18.4G a second across kernels on Apple silicon).

ALTER TABLE service_meters RENAME COLUMN instructions TO cpu;
UPDATE service_meters SET cpu = cpu / 10000;
ALTER TABLE service_terms RENAME COLUMN instructions TO cpu;
UPDATE service_terms SET cpu = MAX(cpu / 10000, 1) WHERE cpu IS NOT NULL;
UPDATE service_calls
   SET meter = json_set(json_remove(meter, '$.instructions'), '$.cpu',
                        json_extract(meter, '$.instructions') / 10000)
 WHERE json_extract(meter, '$.instructions') IS NOT NULL;
