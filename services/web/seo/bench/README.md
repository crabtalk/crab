# SEO benchmarks

Generate synthetic fixtures, then run the service from the repository root:

```sh
python3 services/web/seo/bench/generate.py /tmp/crab-seo-bench
crab bench services/web/seo check --cases /tmp/crab-seo-bench/seo.jsonl --runs 10
```

Workloads also cover a near-512 KiB JSON-LD graph, many JSON-LD blocks, and canonical, hreflang, social, robots and description evidence.

The workloads include normal pages, accepted boundary inputs, structural and size refusals, and a simulated 100 ms HTTP response. No upstream requests are sent. These fixtures do not establish coverage for every HTML input. Costs reflect the current budgets and the reported rate source; repeat after changing a budget.

## Site audit

Run the local fixture from the repository root.

```sh
cargo nextest run --manifest-path services/web/seo/tests/runtime/Cargo.toml --no-capture
```

The runtime fixture uses the built Wasm service, caller SQLite storage, and 1,000 HTML pages with 24 internal links per page. It reports the serialized scan bytes, the SQLite pages occupied by the `pages` table, and the caller database's logical file size. A repeat audit with simulated 100 ms conditional responses reports elapsed workflow time and the check steps' metered CPU, rows, and peak linear memory. To reuse an image, set `BERM_TEST_IMAGE_CRAB_SEO` to the image from `crab build services/web/seo`.

With 100 ms responses, the 25 check batches completed the conditional workflow in 4.60 seconds. They used 91,044 µs of guest CPU in total. The highest measurements in one batch were 4,141 µs of guest CPU, 121 rows read, 80 rows written and 1,900,544 bytes of linear memory.

A diagnostic build used the same HTTP batches inside one check invocation. With higher diagnostic limits it checked all 1,000 pages in 3.22 seconds of workflow time and used 64,177 µs of guest CPU, 3,001 row reads, 2,000 row writes and 1,966,080 bytes of linear memory. The time was within the 60-second chain limit. The reads and writes exceeded the deployed per-step limits of 2,000 and 100.

The 1,000 scans occupied 1,594,340 bytes of JSON. The `pages` table occupied 2,052,096 bytes and the caller database occupied 4,726,784 bytes, including indexes, audit reports and free pages.
