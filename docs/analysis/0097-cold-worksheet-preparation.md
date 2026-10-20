# Cold 워크시트 생성 개선

2026-10-19. [0095](0095-worksheet-preparation-cost.md)의 실제 입력·계측 baseline을 보존하고 XML 조립 및 cold 조각 파싱을 비교했다. GUI·원본 런타임은 실행하지 않았다. 수식·라벨·계산 의미를 추가하거나 변경하는 작업이 아니다.

## 변경

1. 최종 XML 조립에서 `<math resultRef=...>{math}</math>` 전체의 임시 `format!` 문자열을 만들지 않고 같은 wrapper를 write/push한 뒤 math를 직접 append한다. 좌표 직렬화, region 순서, metadata 범위, table 분기, 8 MiB 출력 검사는 유지한다.
2. 처음 비어 있는 `WorksheetXmlCache`의 **다수 소형 조각**만 `<batch xmlns=WS xmlns:ml=ML>` 아래 독립 `<context>`로 묶어 한 번 파싱한다. 기존 geometric order/global prepass 후 같은 `label_parsed_fragment`에 각 context node를 공급하고, 결과는 같은 cache entry로 보존한다. warm cache는 기존 조각별 lookup/parse다.
3. labeller는 context의 range만 복사하고 모든 patch range에서 `context.range().start`를 뺀다. 전체 batch 문자열을 조각마다 clone하거나 다른 context를 포함해 slice하지 않는다. 표는 기존 table labeller에 원래 node와 공유 source를 전달한다.

batch를 파싱할 수 없거나 생성 시 기록한 context range·개수가 파싱된 직접 자식과 다르면 **기존 조각별 경로**를 실행한다. 따라서 batch parser가 낸 다른 위치의 오류를 노출하지 않으며, 첫 오류 문자열·그 전까지의 cache payload·global prepass·label 순서·출력 budget 시점을 기존 경로와 대조한다. semantic label 오류는 같은 labeller가 같은 순서로 내며, cache 부분 상태도 대조한다.

## 선택 조건과 메모리 비용

배치 대상은 초기 cache empty, 조각 256개 이상, 평균 raw fragment ≤1 KiB, 최대 fragment ≤8 KiB다. 합친 context XML은 2 MiB 이하, roxmltree 파싱의 실제 node limit은 100,000이다. dependency parser는 node limit 검사 전에 literal `<`/`=` 개수로 node/attribute Vec를 reserve하므로, 그 입력 hint도 wrapper를 포함해 각각 100,000 이하로 미리 제한한다. 조건에 걸리면 입력을 거부하는 것이 아니라 기존 경로를 사용한다.

256/크기 cutoff는 측정에서 발견한 dense matrix 악화를 피하기 위한 보수적인 workload 선택값이다. 보편적으로 최적인 분기점이라는 주장이 아니다. 큰 조각과 표가 섞인 문서, node/byte cap, 실패한 batch에는 기존 parse 비용이 남고 실패한 시도의 추가 비용도 생길 수 있다. 여러 조각 DOM을 한동안 보존하므로 old per-fragment보다 transient peak가 높다. source cap과 parser/reserve cap은 그 추가 경로를 제한하지만 내부 Vec capacity·allocator·RSS가 node 수와 같다는 뜻은 아니다. retained cache payload와 RSS도 구분한다.

## 측정 방법과 보존 기준

baseline은 계측만 추가한 r95 compat unit EXE이며 SHA256 `9efa5ad901a1f19f20d0d0e3f4a99a08259817fba3cd04139a486be8f4c40302`다. 기존 r94 archive/manifest와 r93/r94/0095의 결과는 변경하지 않았다.

입력은 [0095](0095-worksheet-preparation-cost.md)의 실제 save/fresh-open/Text→Enter에서 캡처한 `&math` 6개를 그대로 재생한다. cold/warm-same/warm-value-edit 각 7회, EXE마다 5회 process 교대하므로 표의 각 셀은 35개 sample의 중앙값이다. 각 sample에서 활성 계측과 비활성 recorder 순서를 교대하고, ordinary `worksheet_xml` 전체 바이트 및 생성 ID·좌표·math range DOM oracle을 검사한다. 값 수정은 첫 plain `<ml:real>`을 `123`으로 바꾸며 새로운 end-to-end UI 측정은 아니다.

기본 비교 시간은 **recorder 비활성** test profile CPU 시간이다. 활성 계측의 시계/TLS overhead와 parse 호출 수 변화 때문에 활성 total은 성과 수치로 사용하지 않는다. non-test 제품 빌드에는 시계/TLS 계측이 없다. warm-same은 상위 앱의 signature/geometry 생략 경로를 강제로 우회한 builder 비용이고, 전체 Enter/worker/GPU/window latency가 아니다. phase 중앙값은 더할 수 없으며 sample별 `sum(exclusive phases)+unaccounted=total`을 별도로 확인한다.

## 중간 후보와 채택 판단

최초 baseline / assembler-only / 무조건 cold-batch의 5회 교대는 1,890 sample이었다. 4k scalar cold는 cache 있음 28.118 → 26.363 → 21.028 ms, cache 없음 28.189 → 26.088 → 21.357 ms였다. 그러나 100개 16×16 matrix는 7.591 → 7.587 → **8.653 ms**, cache 없음 7.489 → 7.333 → **8.380 ms**로 악화됐다. 이 무조건 cold-batch 후보는 채택하지 않았다. 그 EXE와 raw/JSON은 `cold-worksheet-pre-gate-*` 및 최초 후보 이름으로 보존했다. 이후 크기 조건을 포함한 bounded 후보를 별도 EXE로 다시 비교했다.

최종 bounded EXE SHA256은 `5455e79b87f283e509d02e1457c7f214f3e99e6fbcbeb24e3f6f08c2e94fcdb6`다. baseline/최종 후보 5회 교대 1,260 sample 결과는 아래와 같다. 단위 ms, recorder 비활성 중앙값이다.

| 입력 | cold 이전 | cold 최종 | warm 동일 이전→최종 | warm 값 수정 이전→최종 |
|---|---:|---:|---:|---:|
| 4k scalar, saved cache 있음 | 28.376 | 21.794 | 6.373→5.035 | 6.102→5.209 |
| 4k scalar, saved cache 없음 | 27.519 | 21.292 | 5.972→5.325 | 6.336→5.163 |
| 100개 16×16 matrix, cache 있음 | 7.641 | 7.377 | .453→.345 | .449→.429 |
| 100개 16×16 matrix, cache 없음 | 7.466 | 7.348 | .390→.355 | .492→.410 |
| 원본 1015 | 1.918 | 1.863 | .527→.497 | .512→.485 |
| 원본 1016 | 1.804 | 1.814 | .519→.473 | .521→.487 |

4k cold는 6.582 ms(23.2%) / 6.227 ms(22.6%) 감소했다. matrix/원본은 작은 region count 조건 때문에 기존 per-fragment 파싱이고 assembler만 바뀐다. 1016 cold의 .010 ms 차이를 개선으로 포장하지 않는다. 이 수치는 builder 구간의 개선이며 전체 Enter, 파일 열기나 계산 완료 시간의 감소량이 아니다.

활성 계측의 원인 분해(단위 ms):

| 입력·상태 | wrap/parse 이전→최종 | assemble 이전→최종 | label walk 이전→최종 | 계측 total 이전→최종 | 미계측 이전→최종 |
|---|---:|---:|---:|---:|---:|
| 4k cache, cold | 9.237→4.619 | 3.348→2.386 | 9.710→9.750 | 29.520→22.904 | 1.934→1.188 |
| 4k no cache, cold | 9.059→4.531 | 3.192→2.189 | 9.503→9.318 | 28.517→22.010 | 1.920→1.211 |
| 4k cache, warm 값 수정 | .008→.013 | 3.122→2.120 | .007→.006 | 6.549→5.244 | .541→.489 |
| 4k no cache, warm 값 수정 | .008→.013 | 3.053→2.064 | .007→.006 | 6.549→5.158 | .552→.491 |

cold parse 호출 수는 4,000→1이며 lookup miss를 각각 확인하는 작업도 cold batch에서는 필요 없다. 라벨 walk 약 9–10 ms는 남아 있다. warm은 3,999 hit와 1 fragment miss로 기존 경로를 유지한다. 전후 input/output bytes 및 retained cache payload는 모든 fixture·상태에서 같았다. 예를 들어 cold 4k raw XML 합 529,781 bytes, 최종 worksheet 989,018 bytes, 보존 payload 1,790,452 bytes이며 값 수정 후에는 각각 529,782 / 989,019 / 1,790,454 bytes다. 보존 payload가 같아도 shared DOM으로 인해 transient peak는 늘 수 있다.

각 sample의 exact source·metadata, phase 합, hit/parse count, bytes/payload 검증이 모두 통과했다. observer overhead가 서로 다른 활성 total의 차이를 직접 성과로 쓰지 않았고, 최종 판단은 교대 실행의 recorder 비활성 값과 오류/바이트 oracle을 함께 사용했다. assembler와 bounded batch를 채택하고, dense matrix에도 무조건 적용한 중간 후보는 제외한다.

## 회귀와 근거 파일

새 batch 회귀 7개가 아래를 검사한다.

- context offset 기준 patch, UTF-8 이름·𝑥·한글·emoji·rich span namespace, escaped `globalDefine` false positive와 late globalDefine/앞선 query, MathML range와 generated metadata.
- malformed XML·주입된 context delimiter·UTF-16 XML declaration의 old first-error 문자열과 부분 cache payload.
- enabled/disabled table의 definition 효과와 valid XML의 semantic failure.
- 255/256 조각 경계, 256개 16×16 matrix, 큰 단일 조각의 fallback과 정확한 bytes/payload.
- source/실제 node/parser reserve hint cap의 fallback, 8 MiB output 오류 전에 전체 label/cache를 처리하는 기존 순서.

실측 harness는 profile마다 exact source/metadata oracle을 수행하고 input/output bytes·retained payload·hit/parse counts·phase 합을 교차 검증한다. 최종 통합 XSD/원본 export 검증은 부모 작업의 같은 source snapshot에서 수행한다.

소스 동결 후 compat 전체 274 pass / 0 fail / 5 ignored, compat all-target Clippy `-D warnings`를 확인했다. 이 집계에는 동시에 추가된 원본 프로그램 기능의 회귀도 포함된다. 로그는 `cold-worksheet-compat-tests-20261019.log`, `cold-worksheet-compat-clippy-20261019.log`다.
이후 같은 bulk Unicode/namespace 회귀에 late global/query 입력을 추가해 focused 1 pass를 확인했다(`cold-worksheet-final-global-focused-20261019.log`). 생산 코드는 변하지 않았으며 측정용 frozen EXE도 덮어쓰지 않았다.

- `artifacts/rea/cold-worksheet-before-r95-20261019.exe`
- `artifacts/rea/cold-worksheet-assembler-candidate-20261019.exe`
- `artifacts/rea/cold-worksheet-batch-candidate-20261019.exe` (pre-gate; matrix 악화 후보)
- `artifacts/rea/cold-worksheet-bounded-candidate-20261019.exe`
- `artifacts/rea/cold-worksheet-pre-gate-alternating-20261019.{json,log}`
- `artifacts/rea/cold-worksheet-bounded-alternating-20261019.{json,log}`
- `artifacts/rea/measure-cold-worksheet-20261019.py`
- `artifacts/rea/measure-cold-worksheet-bounded-20261019.py`

새 배치 외 원본 기능 변경과 앱 source 수정은 이 작업 범위에 포함하지 않았다.
