---
id: TC-462
title: repeated symbol query on unchanged content hits the content-hash cache without an LSP round-trip
type: scenario
status: unimplemented
validates:
  features: []
  adrs: []
phase: 1
runner: cargo-test
runner-args: -p decision-cli ft_179_symbol_cache_content_hash
runner-timeout: 600
observes:
- file
- stdout
---

## Description

Two identical `get_document_outline` queries for an unchanged fixture file: the second must be served from the on-disk content-hash cache. Asserts on **file** (the cache directory contains an entry keyed by the fixture's relative path + content hash; after touching the file's content the entry is invalidated and a fresh one appears) and **stdout** (service instrumentation records exactly one LSP round-trip for the two queries; two after the content change).
