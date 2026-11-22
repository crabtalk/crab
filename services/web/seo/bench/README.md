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
cargo nextest run --manifest-path services/web/seo/tests/runtime/Cargo.toml --no-capture -E 'test(=thousand_page_fixture)'
```

The fixture builds the Wasm service and uses caller SQLite storage and 1,000 HTML pages with 24 internal links each. It first establishes a baseline, then runs an unchanged audit with 304 responses and a changed audit with 200 HTML responses and revised titles, descriptions and headings. Both measured runs simulate 100 ms per page response. To reuse a built image, set `BERM_TEST_IMAGE_CRAB_SEO` to the image from `crab build services/web/seo`.

Measured on arm64 macOS on 2026-11-21. Workflow totals come from the attempt meter and include eight workflow control invocations. Elapsed time includes admitting and executing the workflow.

| 1,000 checked pages | Audit batches | Rows read | Rows written | Guest CPU (µs) | Invocations | Elapsed (s) |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| Nothing changed | 0 | 35,311 | 5 | 112,824 | 38 | 4.241 |
| Every page changed | 100 | 146,386 | 2,005 | 421,400 | 138 | 9.135 |

### Tool dispatches

| Run | Step | Rows read | Rows written | Guest CPU (µs) | Invocations |
| --- | --- | ---: | ---: | ---: | ---: |
| Unchanged | Check | 2,025 | 0 | 39,567 | 25 |
| Unchanged | Analyze | 30,273 | 1 | 56,161 | 1 |
| Unchanged | Save | 3 | 2 | 4,060 | 1 |
| All changed | Check | 3,025 | 1,000 | 314,094 | 25 |
| All changed | Select | 105,979 | 0 | 564 | 1 |
| All changed | Audit | 6,100 | 1,000 | 35,892 | 100 |
| All changed | Analyze | 30,273 | 1 | 54,466 | 1 |
| All changed | Save | 4 | 2 | 3,884 | 1 |

The unchanged run wrote zero page rows. Its five writes cover creating, updating and deleting the run row, plus saving the report and cursor together. The changed run wrote ten scan rows per audit batch on average, in addition to the check's 1,000 observation writes.

After the baseline, the 1,000 scans occupied 1,594,340 bytes of JSON. The `pages` table occupied 2,052,096 bytes and the caller database occupied 2,543,616 bytes, including indexes, the report and free pages.
