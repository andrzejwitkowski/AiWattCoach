# Graphify Labeling And Wiki Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Nadać techniczne etykiety community w `graphify-out/`, wygenerować wiki i odświeżyć artefakty graphify.

**Architecture:** Plan opiera się na istniejącym `graphify-out/.graphify_analysis.json` i `graphify-out/.graphify_extract.json`. Najpierw budujemy słownik labeli z ręcznych override'ow i heurystyk, potem regenerujemy raport, HTML, JSON i wiki z jednego spójnego zestawu etykiet.

**Tech Stack:** Python, graphify, NetworkX, markdown

---

### Task 1: Build community labels

**Files:**
- Modify: `graphify-out/.graphify_analysis.json` indirectly through regenerated outputs
- Create: `graphify-out/.graphify_labels.json`

**Step 1: Define manual overrides**

- Zmapować top community do nazw technicznych.

**Step 2: Define heuristic fallback**

- Oprzeć fallback o dominujące prefiksy identyfikatorow i source files.

**Step 3: Persist labels**

- Zapisać finalny słownik do `graphify-out/.graphify_labels.json`.

### Task 2: Regenerate graphify artifacts

**Files:**
- Modify: `graphify-out/GRAPH_REPORT.md`
- Modify: `graphify-out/graph.json`
- Modify: `graphify-out/graph.html`

**Step 1: Load extraction and analysis**

- Odczytać istniejące dane graphify.

**Step 2: Rebuild graph with labels**

- Użyć `build_from_json`, `cluster`, `score_all`, `generate`, `to_json`, `to_html`.

**Step 3: Verify files exist**

- Potwierdzić, że wszystkie trzy artefakty powstały.

### Task 3: Generate wiki

**Files:**
- Create: `graphify-out/wiki/index.md`
- Create: `graphify-out/wiki/*.md`

**Step 1: Call graphify wiki exporter**

- Użyć `graphify.wiki.to_wiki(...)` z labelami i god nodes.

**Step 2: Verify wiki output**

- Sprawdzić `index.md` i kilka stron community.

### Task 4: Summarize exploration outputs

**Files:**
- Read: `graphify-out/GRAPH_REPORT.md`

**Step 1: Extract key sections**

- Odczytać `God Nodes`, `Surprising Connections`, `Suggested Questions`.

**Step 2: Present concise summary**

- Zwrócić użytkownikowi skrót i najciekawsze pytania do dalszej eksploracji.
