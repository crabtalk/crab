# SEO benchmarks

Generate synthetic fixtures, then run the service from the repository root:

```sh
python3 services/web/seo/bench/generate.py /tmp/crab-seo-bench
crab bench services/web/seo check --cases /tmp/crab-seo-bench/seo.jsonl --runs 10
```

Workloads also cover a near-512 KiB JSON-LD graph, many JSON-LD blocks, and canonical, hreflang, social, robots and description evidence.

The workloads include normal pages, accepted boundary inputs, structural and size refusals, and a simulated 100 ms HTTP response. No upstream requests are sent. These fixtures do not establish coverage for every HTML input. Costs reflect the current budgets and the reported rate source; repeat after changing a budget.
