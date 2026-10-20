# crab-html

Text out of HTML, `no_std`: the fields feeds and APIs carry, and the `<meta>` of a page.

- `text(html)`: HTML as text. Tags are dropped, a block boundary becomes a line break, a link becomes `[text](href)`, and entities are decoded.
- `unescape(s)`: `s` with its entities decoded and nothing else changed.
- `meta(html, property)`: the `content` of a page's first `<meta>` whose `property` or `name` is `property`, entities decoded and whitespace collapsed.

## License

Apache-2.0
