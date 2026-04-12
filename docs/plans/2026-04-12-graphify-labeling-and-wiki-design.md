# Graphify Labeling And Wiki Design

## Goal

Uczytelnić wygenerowany `graphify-out/` przez nadanie technicznych nazw community, wygenerowanie wiki do nawigacji agentowej oraz odświeżenie raportu i artefaktów wizualnych.

## Scope

- Nazwać najważniejsze community ręcznie, na podstawie dominujących węzłów i plików.
- Nazwać pozostałe community heurystycznie z technicznym, precyzyjnym stylem.
- Wygenerować `graphify-out/wiki/` z `index.md`, stronami community i stronami god nodes.
- Przebudować `GRAPH_REPORT.md`, `graph.json` i `graph.html` z nowymi labelami.
- Wyciągnąć sekcje eksploracyjne z raportu: god nodes, surprising connections, suggested questions.

## Naming Strategy

- Priorytet mają największe i najbardziej centralne community.
- Nazwy mają być techniczne i zwięzłe, np. `Training Context Packing`, `Timeline Event Proof`, `Mongo Timeline Repository`.
- Dla mniejszych community fallback bierze dominujący prefiks identyfikatorow i mapuje go na czytelny label.
- Jeśli heurystyka nie daje mocnego wyniku, zostaje bezpieczny fallback `Community N`.

## Wiki Strategy

- Wiki generowane przez `graphify.wiki.to_wiki(...)` do `graphify-out/wiki/`.
- `index.md` ma byc punktem wejscia dla agentow.
- Strony community i god nodes korzystaja z tych samych labeli co raport i HTML.

## Verification

- Sprawdzic obecność `graphify-out/wiki/index.md`, `GRAPH_REPORT.md`, `graph.json`, `graph.html`.
- Potwierdzic, że raport pokazuje nazwy community zamiast samych `Community N` dla top klastrow.
- Odczytac i streścić sekcje `God Nodes`, `Surprising Connections`, `Suggested Questions`.
