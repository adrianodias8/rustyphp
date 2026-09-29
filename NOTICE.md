# NOTICE

This repository is a fork of **php-rust** (`phpr`), a from-scratch
reimplementation of PHP 8.5 in Rust.

| | |
|---|---|
| Upstream project | php-rust (`phpr`) |
| Upstream repository | https://github.com/francescotinti/php-rust |
| Upstream author / copyright holder | Francesco Tinti — "phpr — Copyright (c) 2026 Francesco Tinti. All rights reserved." |
| Forked from | branch `main`, commit `9d4ef5ba3945545a0d7d9f53437b2d703ebd0ed9` (2026-09-29) |
| Upstream license | **PHP License, version 3.01** — see [`LICENSE`](LICENSE) and [`php-rust/LICENSE`](php-rust/LICENSE); SPDX `PHP-3.01` in `php-rust/Cargo.toml` |

## License — read this before redistributing

`PLAN.md` describes upstream as MIT. **It is not.** Upstream's `LICENSE`,
`README.md`, `AGENTS.md` and Cargo manifest all state the PHP License 3.01
(owner's decision dated 2026-09-20). Upstream's own notes record that its
website still says "MIT" and that the owner intends to change the license
later; until that happens in the upstream repository, the text in `LICENSE`
is what governs this fork.

Consequences that differ from MIT and that this fork must respect:

- Redistributions of source must retain the copyright notice, the list of
  conditions and the disclaimer; binary redistributions must reproduce them in
  the documentation (clauses 1–2).
- **Clause 3** — the name "PHP" must not be used to endorse or promote
  products derived from this software without prior written permission.
- **Clause 4** — products derived from this software may not be called "PHP",
  nor may "PHP" appear in their name, without prior written permission
  (the "Foo for PHP" form is allowed; "PHP Foo" / "phpfoo" is not).
- **Clause 6** — redistributions of any form must carry the acknowledgment
  required by the license text.

The fork's working name and any published artifact name therefore need a
decision from the project owner before anything is distributed. Nothing has
been published from this fork.

## What this fork keeps and what it changes

Kept from upstream, unmodified: all upstream source, documentation, diary,
session records, harnesses and license files. Upstream documents are not
edited or deleted; this fork's documents are added beside them.

Added by this fork (all at the repository root unless noted):
`NOTICE.md`, `PLAN.md`, `NOTES.md`, `ARCHITECTURE_NOTES.md`, `PROFILE.md`,
`DECISION_KERNEL.md`, `baseline/`, `bench/`, `docker/`.

## Third-party components

Upstream's dependencies are listed in `php-rust/Cargo.lock`. The `.phpt`
corpus and `Zend/bench.php` / `Zend/micro_bench.php` used for baselines and
benchmarks come from php-src (https://github.com/php/php-src, tag
`php-8.5.7`), which is checked out beside this repository and is not
redistributed here.
