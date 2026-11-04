"""Write synthetic SEO and Markdown benchmark workloads to an output directory."""

import argparse
import json
from pathlib import Path

START = "<!doctype html><html lang='en'><head><title>Fixture</title><meta name='description' content='Fixture'></head><body><main>"
END = "</main></body></html>"
MAX_BYTES = 512 * 1024
URL = "https://fixture.example/page"


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("output", type=Path)
    output = parser.parse_args().output
    output.mkdir(parents=True, exist_ok=True)
    units = {
        "ordinary": ("<h1>Guide</h1><p>Hello <a href='/next'>next</a></p>", 8192),
        "text-limit": ("A readable sentence with some words. ", MAX_BYTES),
        "dense-elements": ("<p>x</p>", MAX_BYTES),
        "relative-links": ("<a href='/x'>x</a><img src='/x' alt='x'>", MAX_BYTES),
        "headings": ("<h1>x</h1>", MAX_BYTES),
        "malformed-tables": ("<table><tr><td>x", MAX_BYTES),
        "scripts": ("<script>const x = 1;</script>", MAX_BYTES),
    }
    bodies = {
        name: START + unit * ((size - len(START) - len(END)) // len(unit)) + END
        for name, (unit, size) in units.items()
    }
    attribute_tag = "<div " + "".join(f"a{i}='x' " for i in range(128)) + ">text</div>"
    bodies.update({
        "nested": "<div>" * 8000 + "x" + "</div>" * 8000,
        "oversized": "x" * (MAX_BYTES + 1),
        "near-nodes": START + "<p>x</p>" * 5990 + END,
        "near-depth": START + "<div>" * 120 + "text" + "</div>" * 120 + END,
        "deep-many-nodes": START + "<div>" * 120 + "<p>x</p>" * 5850 + "</div>" * 120 + END,
        "large-readable": START + "<script>" + "x" * (380 * 1024) + "</script>" + "A" * (127 * 1024) + END,
        "many-attributes": START + "<div " + "".join(f"a{i}='x' " for i in range(12000)) + ">text</div>" + END,
        "large-metadata": "<head><title>" + "T" * (300 * 1024) + "</title><meta name='description' content='" + "D" * (200 * 1024) + "'></head><body><main>text</main></body>",
        "long-link": START + "<a href='/" + "x" * (500 * 1024) + "'>text</a>" + END,
        "long-canonical": "<head><link rel='canonical' href='/" + "x" * (500 * 1024) + "'></head><body><main>text</main></body>",
        "large-heading-report": START + ("<h1>" + "x" * 13200 + "</h1>") * 20 + END,
        "near-attributes": START + attribute_tag * 93 + END,
        "total-attributes": START + attribute_tag * ((MAX_BYTES - len(START) - len(END)) // len(attribute_tag)) + END,
    })
    for name, body in bodies.items():
        (output / f"{name}.html").write_text(body)
    rejected = {"dense-elements", "relative-links", "headings", "malformed-tables", "scripts", "nested", "many-attributes", "total-attributes"}
    for service in ("seo", "markdown"):
        cases = []
        for name in [*bodies, "network-wait"]:
            response = {"method": "GET", "url": URL, "status": 200,
                        "headers": {"content-type": "text/html"},
                        "body_file": f"{'ordinary' if name == 'network-wait' else name}.html"}
            if name == "network-wait":
                response["delay_ms"] = 100
            case = {"name": name, "args": {"url": URL}, "http": [response]}
            if name in rejected:
                case["expected_error"] = "HTML"
            elif name == "oversized":
                case["expected_error"] = "512 KiB"
            elif service == "markdown" and name in {"text-limit", "long-link", "large-heading-report"}:
                case["expected_error"] = "128 KiB"
            elif service == "seo" and name in {"large-metadata", "long-canonical", "large-heading-report"}:
                case["expected_error"] = "256 KiB"
            cases.append(case)
        (output / f"{service}.jsonl").write_text("\n".join(map(json.dumps, cases)) + "\n")
    print(output)


if __name__ == "__main__":
    main()
