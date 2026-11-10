@AGENTS.md

## Mandatory: graphify first

This is a required rule, not a suggestion. The project has a knowledge graph in
`graphify-out/` (`graph.json`, `GRAPH_REPORT.md`).

- Before searching the code with grep/Glob/Read, look it up in the graph:
  `graphify query "<question>"`, `graphify explain "<name>"`,
  `graphify path "<A>" "<B>"`. Fall back to grep/Read only for what the graph
  does not answer, or to read the exact lines it points to.
- After changing code, refresh the graph with `graphify update .` (AST only, no
  LLM, a few seconds) before finishing the task, so the next session sees the
  current code.
- If `graphify-out/graph.json` is missing, build it with `/graphify .` first.
