# MISSING_FOR_DRUPAL.md — what Drupal 11 needed that ferro lacked

Drupal 11.4.8 (`drupal/recommended-project`), drush 13.8.0, SQLite, in the dev image
(`bench/drupal/install.sh`). One row per missing extension, class, function or engine
construct, in the order the install hit them. **Fixed** rows name their `.phpt` in
`baseline/repro/`; open rows say what blocks.

| # | missing | kind | hit at | status |
|---:|---|---|---|---|
| 1 | `ext/gd` in the **oracle** image | oracle | `composer create-project` refuses (`drupal/core` requires `ext-gd`) | fixed: `docker/Dockerfile` builds gd into PHP 8.5.7 |
| 2 | a trait used by an included trait, when it needs autoloading: the autoloader got its name lowercased (`robo\common\configawaretrait`) and PSR-4 found no file | engine (lowering) | drush `Drush\Config\ConfigAwareTrait` | fixed — `trait-in-trait-autoload-case.phpt` |
| 3 | `isset/[]/[]=/unset` on `$this[...]` (an `ArrayAccess` object indexing itself) | engine (compiler) | `consolidation/annotated-command` `AnnotationData::has()` | fixed — `this-arrayaccess-self.phpt` |
| 4 | a by-reference builtin given a call result (`array_shift($input->getArgument(...))`, `reset(f())`): PHP sends the temporary with a notice | engine (compiler) | drush `SiteInstallCommands::validate()` | fixed — `byref-builtin-call-result.phpt` |
| 5 | `RecursiveCallbackFilterIterator` | SPL class | `ExtensionDiscovery::scanDirectory()` | fixed — `recursive-callback-filter-iterator.phpt` |
| 6 | `RecursiveDirectoryIterator` ignored `CURRENT_AS_SELF` and `KEY_AS_FILENAME` | SPL behaviour | `ExtensionDiscovery::scanDirectory()` | fixed — `recursive-directory-iterator-flags.phpt` |
| 7 | `RecursiveIteratorIterator` was eager (walked the whole tree at `rewind()`): wrong with `CURRENT_AS_SELF`, hooks never ran, `getInnerIterator()` returned the root | SPL behaviour | same | fixed: lazy port of Zend's state machine — `recursive-iterator-iterator-lazy.phpt` |
| 8 | `SplFileInfo::openFile()` (and getATime/getCTime/getInode/getOwner/getGroup/getType/isExecutable/getLinkTarget/getFileInfo/getPathInfo) | SPL class | `ExtensionDiscovery` reads each `.info.yml` | fixed — `splfileinfo-openfile.phpt` |
| 9 | `mb_convert_encoding()` on an array (element by element, keys included) | builtin | Symfony Console `splitStringByWidth()` while rendering an error | fixed — `mb-convert-encoding-array.phpt` |
| 10 | `__serialize()` without `__unserialize()`: unmangled names not restored into private (and inherited private) properties; payload order changed by the class layout | engine (serialize) | Symfony DI `ResolveInstanceofConditionalsPass` round-trips every `ChildDefinition` → "service has no class" | fixed — `serialize-unmangled-private-roundtrip.phpt` |
| 11 | `$x =& f()` where `&f()` is declared in a file included later (`$batch =& batch_get()`) | engine (compiler) | `install_run_task()` | fixed — `ref-assign-from-later-declared-function.phpt` |
| 12 | backed enum case values that are constant expressions (`case Info = -1`, `1 << 3`, `self::P . 'x'`) | engine (const folding) | `RequirementSeverity::from()` | fixed — `enum-case-constant-expression.phpt` |
| 13 | `Fiber::suspend()` across any Rust-level nesting (inside a generator, a callback, a magic method): runtime panic; also `Fiber::throw()` and traces through fibers | engine (fibers) | installer batch / renderer | fixed: a native stack per fiber (corosensei, owner decision, DECISION_KERNEL §8) — `fiber-native-stack.phpt` |
| 14 | `DOMImplementation::createDocumentType()`; `createDocument()` with a doctype | ext/dom | masterminds/html5 `DOMTreeBuilder` | fixed — `dom-create-document-type.phpt` |
| 15 | `extension_loaded('zlib')` false though the zlib functions exist | extension list | install requirements ("PHP extensions: zlib") | fixed |
| 16 | XPath `namespace::` axis | ext/dom | masterminds/html5 `OutputRules::namespaceAttrs()` | parsed, evaluates empty (no effect on HTML output) — **open**: D-17 |
| 17 | a method-call result sent to a by-reference parameter (`NestedArray::setValue($this->getValues(), …)` with `&getValues()`) | engine (compiler) | `FormState::setValue()` | fixed — `byref-param-method-call-result.phpt` |
| 18 | `$v = &C::m()` (static method call) | engine (compiler) | `FormState::getValue()` | fixed — `ref-assign-static-call.phpt` |
| 19 | `Pdo\Sqlite::createCollation()` | ext/pdo_sqlite | `sqlite\Connection::open()` (NOCASE_UTF8) | fixed — `pdo-sqlite-create-collation.phpt` |
| 20 | `$GLOBALS[$name]` sent to a by-reference parameter | engine (compiler) | `SettingsEditor::rewrite()` | fixed — `byref-param-globals-dynamic-key.phpt` |
| 21 | 58 `STREAM_*` constants (`STREAM_URL_STAT_QUIET`, `STREAM_MKDIR_RECURSIVE`, `STREAM_META_*`, …) | constants | `FileSecurity::writeFile()` via a stream wrapper | fixed |
| 22 | `Class::$p = &$x` and `return self::$p;` from a by-reference function | engine (lowering + op) | Views `Page::setPageRenderArray()` | fixed — `static-prop-ref-bind-and-return.phpt` |
| 23 | the same trait method reached directly and through another trait was a "collision" | engine (traits) | Layout Builder `OverridesEntityForm` | fixed — `trait-same-method-two-paths.phpt` |
| 24 | the encoding list `"auto"` | mbstring | egulias/email-validator `EmailLexer::getType()` | fixed — `mb-encoding-list-auto.phpt` |
| 25 | `stream_get_wrappers()` did not list user wrappers (70 "already defined" warnings) | builtin | `StreamWrapperManager::registerWrapper()` | fixed — `stream-wrappers-and-object-concat-assign.phpt` |
| 26 | `$a[k] .= x` when the element is a `Stringable` object: wrote "Class" + warning | engine (VM) | `_batch_populate_queue()` init message | fixed — same test |
| 27 | `file_put_contents()` on a user stream wrapper | stream wrappers | `.htaccess` in `public://` / `temporary://` | fixed — `user-wrapper-file-put-contents.phpt` |
| 28 | chmod/touch/chown/chgrp/unlink/rename/mkdir/rmdir on a user stream wrapper | stream wrappers | `FileSecurity::writeFile()` chmod | fixed — `user-wrapper-fs-mutators.phpt` |

| 29 | `Closure::fromCallable()` / first-class callable on a class not loaded yet: no autoload | engine | `Unicode::strcasecmp(...)` in the SQLite driver on the first web request | fixed — `first-class-callable-autoload.phpt` |
| 30 | `preg_match()` byte offsets on a multibyte subject with a non-`/u` pattern (anchored `/A` never matched) | pcre | Twig lexer on `haven’t` in Olivero's `get-started.html.twig` | fixed — `preg-offset-multibyte-anchored.phpt` |
| 31 | `strtr()` with `Stringable` objects among the replacement values | builtin | `DrupalConsoleLogger` placeholders | fixed — `strtr-stringable-values.phpt` |
| 32 | `Fiber::suspend()`/`getCurrent()` called through an instance (`$fiber->suspend()`) | fibers | `EntityStorageBase::loadMultiple()` | fixed — `fiber-static-via-instance.phpt` |
| 33 | a `$this->prop` argument to an `[$obj, 'm']` / method-closure callable arrived as NULL (deferred place resolved in the callee) | engine (calls) | `HelpBlock` invoking `hook_help` | fixed — `callable-array-property-arg.phpt` |
| 34 | references lost in argument unpacking (`$f(...[&$vars, …])`) and in array-literal spreads; by-value parameters of a spread call received references | engine (calls) | `ModuleHandler::invoke()` with theme preprocess `[&$variables, …]` (Olivero preprocess had no effect) | fixed — `spread-references.phpt` |
| 35 | `DOMDocument::saveXML($node, LIBXML_NOEMPTYTAG)` | ext/dom | `ActiveLinkResponseFilter` (the active menu link's `<a>` was dropped) | fixed — `dom-savexml-noemptytag.phpt` |
| 36 | the cli-server sent its own `Date` header next to the script's | SAPI | every Symfony response | fixed (one-shot and worker) |

## Front page (step 2)

`bench/drupal/frontpage.sh`: the oracle's `php -S` and `ferro -S` (one-shot) serve copies of one
install (`/scratch/drupal-base`, see `bench/drupal/README.md`). After stripping per-request tokens
(form tokens, random view DOM ids, the port, the `Date` header), `/` cold, `/` warm, `/node` and
`/user/login` are **byte-identical, headers included** (4/4). One-shot ferro is slow on them
(warm `/`: 0.28 s vs 0.008 s): every request re-lexes, re-parses and re-compiles Drupal; the
worker mode is what removes that.

| 37 | `__toString()` not called for a `Stringable` subject in `preg_match`/`preg_match_all`/`preg_split`/`mb_ereg*` | pcre | `Unicode::validateUtf8()` on a `ViewsRenderPipelineMarkup` | fixed |
| 38 | `return $o->m();` / `return C::m();` in a `function &f()` noticed even when `m()` returns by reference | engine (compiler) | `Select::havingConditions()`, `BlockPluginCollection::get()` | fixed — `return-byref-call-result.phpt` |
| 39 | the worker server sent its own `Content-Length` next to the application's | SAPI (worker) | every Symfony response in worker mode | fixed |
| 40 | `ReflectionClass::getProperties()` ignored its filter and omitted static properties; `getDefaultProperties()` returned current static values; a static with a non-constant default read NULL through reflection | reflection | the worker's static-reset sweep | fixed — `reflection-static-properties.phpt` |
| 41 | `$map[$k][] = $v` through `WeakMap` / a by-reference `ArrayAccess::offsetGet()` (variable- and property-rooted) | engine (VM) | Twig `ExpressionParsers::getPrecedenceChanges()` | fixed — `arrayaccess-byref-offsetget-nested-write.phpt` |
| 42 | `touch()` on an existing directory | builtin | Drupal's Twig cache (`MTimeProtectedFastFileStorage`) | fixed — `touch-existing-directory.phpt` |
| 43 | `DOMDocument::saveXML()` wrote namespace declarations in source order (libxml2 writes them first) | ext/dom | `RssResponseRelativeUrlFilter` (`/rss.xml`) | fixed — `dom-savexml-nsdecl-first.phpt` |

## Worker mode (step 2)

`bench/drupal/drupal-worker.php` boots one `DrupalKernel` per worker and loops `handle()` /
`send()` / `terminate()`; Drupal is not patched. `bench/drupal/worker-leaks.sh` sends the same
10-request sequence (`/user/login`, `/`, `/rss.xml`, a 404, `/user/password`, `/node`, a query
string, repeats) to the oracle's one-shot `php -S` (a fresh process per request: the reference)
and to ONE ferro worker, and compares every response byte for byte after token stripping.
FrankenPHP's worker mode (`dunglas/frankenphp`, PHP 8.5.11) running the same controller is the
reference for *worker* semantics: on every reset variant below it produced the same page sizes
as ferro, so what follows is Drupal's state, not the engine's.

| reset after each request | ferro | FrankenPHP | what leaks |
|---|---:|---|---|
| none (naive loop) | 1/10 | same failures | the first route works; any later route that renders afresh returns an **empty body** (views pages), `/rss.xml` a 404 served from the dynamic page cache — service state in the container |
| a new kernel per request | 2/10 | same | pages render, but HTML ids carry `--2`, `--3` suffixes (`Html::$seenIds`) — process statics |
| `$kernel->resetContainer()` | 3/10 | — | container state is gone; the statics below remain |
| `resetContainer()` + every changed Drupal static | 10/10 | — | — |
| **`resetContainer()` + `Renderer::$contextCollection`, `Html::$seenIds`, `Html::$seenIdsInit`** (the `recipe` reset) | **10/10** | same sizes as the oracle | — |

So a Drupal 11 worker needs, after `terminate()`: **`$kernel->resetContainer()`** (Drupal has no
`kernel.reset`/`ResetInterface` service resetter; rebuilding the container from its cached
definition is the only way to drop request state held by services), and a reset of **three
statics**: `Drupal\Core\Render\Renderer::$contextCollection` (render contexts keyed by request),
`Drupal\Component\Utility\Html::$seenIds` and `::$seenIdsInit` (unique-id bookkeeping: without
the reset every HTML id on a repeated block gets a suffix). Other statics change per request but
do not affect output (`Views::$translationManager`, `Views::$handlerTypes`,
`UrlHelper::$allowedProtocols`, `Html::$classes`, `Page::$pageRenderArray`, `Timer::$timers`) or
are process infrastructure the booted kernel depends on (`Settings::$instance`,
`Database::$connections`/`$databaseInfo`, `FileCache*`, `DrupalKernel::$isEnvironmentInitialized`):
resetting those breaks the next request (500).

A fourth static joined the recipe in session 7: `ReverseContainer::$recordedServices` grows by
~245 entries per request with every `resetContainer()` — on both engines — a memory and time leak
that never shows in the output.

## Throughput (sessions 8–9)

Front page, anonymous, `page_cache` uninstalled, every server on a copy of the same base,
token-stripped bodies identical to php-fpm's (`bench/drupal/bench-wrk-drupal.sh`, 12-CPU VM shared
with wrk; files in `bench/results/`):

| server | 4 workers | 8 workers |
|---|---:|---:|
| nginx + php-fpm 8.5.7 + opcache | 753 | 1,425 |
| FrankenPHP 1.12.7 worker (`recipe`) | 184 | 340 |
| ferro worker (`recipe`) | 72 | 141 |
| ferro classic pool (`--workers N`, a fresh Vm per request) | 161 | 294 |

Both worker runtimes **degrade from run to run** under the recipe (FrankenPHP 549 → 340 → 278,
ferro 216 → 141 → 114 at 8 workers), so state still accumulates in Drupal itself after
`resetContainer()`; with that reset, a worker is slower than a classic per-request server for
Drupal — on both engines. Classic-pool scaling (`bench/drupal/scaling.sh`): 52 req/s for one
worker, 73 % per-worker efficiency at 8, **identical with 8 threads or 8 processes** (php-fpm:
86 %), so the per-thread caches are not what costs the scaling.

## Result

`drush site:install standard` (SQLite) completes under ferro: **14.5 s vs 2.1 s** for the oracle
(single run, dev image). The install log's diagnostics are identical to the oracle's (one
"sqlite3 not found" warning on both). The two installed databases have the same 39 tables, the
same row counts, identical `config` rows once UUIDs are normalised, and differ elsewhere only
in per-install values (timestamps, the cron key, the CSS/JS query string, the admin password's
salt — each engine's hash verifies `admin` under both).

Found on the way and registered (not needed by the install): `get_defined_constants()` is missing;
a builtin `TypeError` names `false` as `bool` (D-18); an error raised in a callee invoked from
prelude code can name the caller's file (D-19).
