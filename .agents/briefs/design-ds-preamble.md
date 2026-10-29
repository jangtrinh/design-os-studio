# Design-system preamble for every UI brief

Include this block verbatim in each brief for a UI task (agy, Argon, Grok or any other worker). It is the design system as the registry holds it, in tiers: tokens (primitive, semantic, component) and UI (element, component, composite, region, screen, flow). Generate the three outputs when the task starts, from the repository root of the checkout you work in; never paste them from memory.

```bash
ui ds context --dir . --strict   # soul chain, roles, record names + tiers + tokens, the ENFORCEMENT preamble
ui ds docs --dir .               # per record: tier/rust/composes line, variants, resolved token values
ui registry list --json          # the closed list of names; a new UI may only use these (from the repo root; list takes no --dir)
```

Rule: a screen is assembled from registered records; a needed record that does not exist is a stop-and-ask, never an invention.
