# Plain MathML serialization cost

2026-10-19. This is a bounded serialization optimization and independent headless CPU investigation. It adds no mathematical behavior and moves no preparation into a worker. GUI, screenshots and the original runtime were not used. Existing r113 artifacts are unchanged.

## Preserved baseline and scope

The r113 archive `mathcraft-implicit-scale-20261019-r113.tar` is SHA-256 `998f776187615f6146aff66ccb44da79f70c1f967b0b657a11e2d8bdd2a7895b`. The retained actual-open harness `implicit-scale-open-input-after-r113-20261019.exe` is `7455dff0ec57b8e7984a774c1f1408ec9ca3b9e2eb3650767fae333703f41d73`. Both identities were checked before this investigation. The same six Enter XML captures from `worksheet-preparation-inputs-20261019` remain inputs.

The r113 app's serialization phase includes source validation, creating/looking up serialized proofs, tree cloning, copying raw strings and constructing region/proof maps. Its worksheet phase includes sparse label planning, labelled serialization, projection, lookup/payload and output assembly. Timing an isolated serializer is therefore not interchangeable with either app phase. The existing exact JSON signature is unchanged.

## Function audit and chosen change

The original recursive `serialize_with_plan` creates a String per node, temporary escaped strings and formatted parent strings. Matrix cells are each formatted, copied into a content String and copied into the matrix String. Call/Index collect and join vectors, with the significant fallible evaluation order **arguments before callee/base**. Other specialized forms have their own validation and XML ordering.

The candidate adds `editor_bridge/serialization_writer.rs`: the existing proven plain subset writes tags, literal spans and escaped text directly into one output String. Matrix traversal remains column-major using `row * cols + col` AST child indices. Decimal dimension formatting writes into that buffer. Both raw proof creation and labelled emission use it; sparse plans and the shared label resolver are unchanged. The root namespace, identifier attributes, NativeIdentifier contextual flag, XAML namespace order, entities, Unicode and numeric spelling remain exact. The final String sheds capacity slack before becoming retained proof source. No early XML budget check changes failure timing; the builder's existing 8 MiB output boundary remains authoritative.

This is **two independent single-buffer emissions**, not a claim that raw and contextual-labelled output are fused into one pass. Raw bytes must exist before the worksheet's global prepass and contextual label environment. A future identifier-range trace would need to retain AST child-path identity and byte ranges for every raw source revision, including XAML and column-major matrices; it must not match AST/XML preorder positions. That metadata and retention cost are unnecessary to obtain the measured benefit here and are deferred.

The original serializer remains independent production fallback and test oracle. Functions, programs, calculus, symbolic forms, depth/XML-normalization uncertainty and unsupported proof forms retain their previous path. Call and Index also use that serializer for their individual subtrees, preserving the exact arguments-first error order rather than inserting another validation traversal. They therefore retain some temporary allocation. The writer retains numeric parsing and depth errors. Public raw conversion still runs structural validation and contextual validation in the same order before selecting a writer. Label projection still validates its final tree. No validation contract was removed.

`PreparationCache::serialize_math` also previously inserted a new cloned proof, then compared its tree against the exact input from which it had just been cloned. Vacant/Occupied entry handling removes only that first-insert equality scan. Occupied entries retain complete structural equality, including numeric spelling and identifier metadata; cached failures retain their original source guard. A saved proof remains immutable when the same region ID changes or fails. Tree cloning itself is retained as the durable cache/source identity guard.

## Isolated CPU method

`fused-serialization-microprofile-20261019.exe` SHA-256 `2d15d6a71df06de6efae71b4cd1ae64f1b8464b67c7cf6254c19a6b3dd95c62b` contains the unchanged recursive serializer and candidate. The ignored function profile uses the same executable, ten alternating legacy/writer orders per capture, with all phase recorders disabled. XML input parsing, sparse-plan construction and fixture setup happen outside intervals. Raw serialization, labelled serialization, one structural validation and one tree clone are separate intervals. Their outputs are checked against the independent legacy serializer. The writer's eligibility/context scans, Arc allocation, app cache bookkeeping, worksheet order/prepass and display adoption are not in these serializer intervals.

The capture subset is 4,000 trees for each scalar input, 100 for each matrix input and 79 for each original; the remaining original region is raw fallback. Units are ms. Each column is its own median; the raw+labelled column uses each run's sum before taking the median.

| Capture | Raw legacy→writer | Labelled legacy→writer | Paired raw+labelled legacy→writer | Validation, writer control | Tree clone, writer control |
|---|---:|---:|---:|---:|---:|
| 4k/cache | 3.189→2.261 | 3.874→2.602 | 7.092→4.884 | .751 | .758 |
| 4k/no cache | 3.184→2.297 | 3.926→2.580 | 7.165→4.864 | .743 | .750 |
| 100 matrices/cache | 4.594→1.275 | 4.725→1.265 | 9.400→2.559 | .598 | 1.677 |
| 100 matrices/no cache | 4.673→1.287 | 4.686→1.292 | 9.401→2.585 | .603 | 1.712 |
| Original1015 | .374→.103 | .338→.110 | .720→.213 | .038 | .068 |
| Original1016 | .332→.095 | .322→.101 | .654→.196 | .037 | .067 |

Validation and clone controls show remaining cost; they are not eliminated or to be summed with the app's independent phase medians. This is an optimized CPU test profile, not release desktop/GPU/complete calculation latency. The frozen executable predates formatting and a test-only `black_box` unit-argument lint repair; production operations are identical. Final integration rebuilds the frozen source independently.

The same executable also ran the existing captured builder profile. Its comparison is **DOM builder versus typed+writer**, not an isolated r113 typed builder versus r117 writer comparison. It excludes raw serialization/proof construction. Cold DOM→typed+writer ms: scalars/cache 23.193→13.570, no cache22.544→13.123; matrices/cache12.018→5.900, no cache11.807→5.540; original1015 1.783→.781,1016 1.767→.723. Retained worksheet payload remains exactly 1,790,452 / 1,152,672 / 155,864 / 155,866 bytes respectively. Warm original1015 regresses .389→.432 ms and1016 .384→.385 ms; scalar and matrix samples improve. Neither this secondary comparison nor separate r113 microprofile runs establish a controlled whole-Enter saving.

Raw logs and all120 samples per profile are `fused-serialization-{function,builder}-profile-20261019.{log,json}`. No process RSS or allocation-count measurement was made. One output buffer can still reallocate while growing; Call/Index fallback and strings in the sparse plan/projection remain. The existing cache payload budget and exact cache contents are unchanged.

## Differential gates and final integration

The existing eight typed differential tests now also compare every proof's raw source byte-for-byte with the independent recursive serializer. They cover all eligible plain editor kinds, matrix child identity, units/None/contextual/explicit labels, globals, Unicode/XAML/entities, order/deletion/new IDs, stale proof fallback, malformed XML/first-error partial cache state, conservative depth/normalization fallback, 8 MiB output failure and 32 MiB payload eviction. A new focused writer test compares explicit leaf attributes/entities and fallible Call/Index/matrix/Eval child order and depth errors. The captured six-fixture full-DOM tree/source/retention oracle passes. Compat library:70 passed,9 ignored at initial candidate check. The app proof-cache regression additionally exercises same-tree reuse, `-0`→`0`, failure caching and same-ID recovery while retaining an old proof.

Prepared final runner: `artifacts/rea/measure-fused-serialization-open-input-20261019.py`, retained r113 before identity as above, fresh after `fused-serialization-open-input-after-r117-20261019.exe`, output `fused-serialization-open-input-alternating-20261019.{json,log}`. It preserves six fixtures×six events×five alternating runs per executable, zero preparation before Enter, one preparation only for dirty Enter, generation/draft/global-error contracts and zero evaluation on open. It explicitly removes worksheet-capture I/O from timed runs. This auxiliary runner remains preparation-only and was not executed; the final integration below uses the parenthesis-prefix runner. Whole Enter, per-run event+Enter and worker residual are reported separately, and no 16 ms target is established by the isolated numbers.

## Final integration adoption

The auxiliary `fused-serialization` whole-frame runner above was not executed. Final measurement is unified under `parenthesis-open-input-alternating-20261019.{json,log}` and its phase summary in [0117](0117-parenthesis-integration.md), with five alternating runs per executable and 360 contracts passing. Source archive SHA is `fea698f24e4af0ba087d2399574a554064df6c895aa3c0693a683afad42ab34d`; final r117 app harness SHA is `df7f803114021833686cd39c7754f2b0644d3a3339bddda672b579f9a256e7b6`. The comparison includes original parenthesis/container behavior changes, so it is an integration comparison rather than an isolated estimate of this writer alone.

| Fixture | Enter ms r113→r117 | Per-run Text+Enter ms r113→r117 | Per-run worksheet+display ms r113→r117 |
|---|---:|---:|---:|
| 4k/cache | 37.320→32.927 | 39.779→35.236 | 19.265→16.949 |
| 4k/no cache | 35.687→32.756 | 38.062→35.103 | 18.372→16.513 |
| 100 matrices/cache | 28.084→20.506 | 33.073→25.698 | 9.815→5.874 |
| 100 matrices/no cache | 28.156→20.766 | 33.581→26.247 | 9.311→5.624 |
| Original1015 | 2.599→2.264 | 3.106→2.770 | 1.177→1.056 |
| Original1016 | 2.515→2.017 | 2.979→2.465 | 1.133→.884 |

This gate adopts the bounded writer. It does not reach 16 ms for the large Enter fixtures. Source signature remains about 6–7 ms; validation, immutable clones, cache bookkeeping/projection and calculator work remain. Ten of the 36 event-pair medians increased, all listed in 0117. Worker residual increased for matrices/cache11.160→11.557 ms, matrices/no-cache11.333→11.547 and original1016 1.632→1.837; residual excludes concurrent work and cannot be added to Enter as completion latency. Recorded open-through-Enter CPU sums for original1015 increased45.228→45.696 ms and1016 43.629→43.804, so native cold alignment/restore has not been fixed. Windows/peets each pass956 tests with45 ignored; native/reference/PDF/source identity are recorded in 0117. Neither test counts nor these timings establish overall product20% completion.
