# Web service benchmarks

Generate synthetic fixtures, then run each service from the repository root:

```sh
python3 services/web/markdown/bench/generate.py /tmp/crab-web-bench
crab bench services/web/seo check --cases /tmp/crab-web-bench/seo.jsonl --runs 10
crab bench services/web/markdown read --cases /tmp/crab-web-bench/markdown.jsonl --runs 10
```

The workloads include normal pages, accepted boundary inputs, structural and size refusals, and a simulated 100 ms HTTP response. No upstream requests are sent. These fixtures do not establish coverage for every HTML input. Costs reflect the current budgets and the reported rate source; repeat after changing a budget.
