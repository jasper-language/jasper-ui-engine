# Selected Blitz migration foundation

Pinned Git revision and archive SHA-256: revision.json. source.tar.gz is the
unaltered upstream archive; Apache-2.0 and MIT licenses are copied alongside.
Transitive licenses still require the resolved dependency inventory; this is not
an assertion that every dependency shares the top-level license.

The active native facade candidate selects blitz-dom and blitz-traits with default
features disabled, and optional blitz-paint for CPU diagnostics. It directly
constructs native trees; blitz-html, blitz-shell and blitz-net are not selected by
that target. The original HTML parsing probe is historical evidence only.
CPU paint is runnable but the whole adapter remains unaccepted; GPU integration
is still pending. Check the active pin/features with
scripts/check-blitz-native-dependencies.py.

Use existing upstream font injection, layout and input APIs; stop duplicating
these in a parallel Jasper implementation. Keep Jasper authority/transaction/
lifecycle checks outside the unproved third-party boundary. Node IDs are not
authentication. Treat DOM identity translation, input consumption and document
revision updates as owned adapter verification obligations.

System-host std/thread/resource requirements are not yet validated. No promise
of no_std/no-POSIX/no-thread operation is made. Canvas remains deferred.
