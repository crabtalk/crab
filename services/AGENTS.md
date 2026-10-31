# Provider services

- Use the provider's treg catalog entry as the default scope: implement its complete listed tool set and supported options. Do not reduce it to a few sample operations or expand it to the provider's entire API.
- Before coding, state the tool list, catalog job mappings and exclusions. This is a scope statement, not a mandatory approval step.
- Consult Monid when treg has no entry. If neither covers the provider, propose a concrete tool set before implementation.
- Interpret “complete” or “fulfill” as completing that established scope. Propose additions separately; implement them only when the user requests or accepts them.
- Use upstream documentation to verify contracts within scope. Discovering another endpoint does not add it to scope.
- Follow existing service patterns. Add generators, shared frameworks or new dependencies only when the agreed tool set requires them.
- Use treg as a coverage reference; implement from upstream documentation rather than copying its code or catalog files.

- Reuse `berm_lang::http::Client` for provider headers, HTTP failures and JSON decoding. Keep provider-specific response envelopes and partial failures in the service.
