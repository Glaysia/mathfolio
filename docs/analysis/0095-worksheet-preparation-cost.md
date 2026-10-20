# 워크시트 생성 비용 분해

2026-10-19. 권위 제품 기준은 r94이며, 이번 변경은 `cfg(test)` 계측·캡처·회귀뿐이다. 원본 런타임과 GUI는 실행하지 않았고 생산 최적화는 적용하지 않았다. r94 archive와 manifest는 수정하지 않았다. archive SHA256은 `dee4f7f880b43e1d76b6e9ee4da1fa18a397e695114331017c91b6ddfb10f284`이다.

## 입력과 측정 경계

`actual_open_first_input_profile`의 실제 save → 새 `WorksheetApp::open` → viewport 일부 editor → 첫 Text → Enter 경로를 사용했다. `MATHCRAFT_WORKSHEET_CAPTURE`를 명시한 별도 실행에서 Text 6개 fixture의 `WorksheetXmlCache::build_prepared(&math)` 직전 입력을 JSON으로 보존했다. 따라서 원본 import, PortableTree 재직렬화, disabled 제외, opaque/table/fault 처리 후의 실제 `(id,left,top,MathML)`이다. 저장된 결과 캐시가 있는/없는 scalar 및 matrix 쌍은 이 단계 입력 바이트가 동일하다. 원본 1015/1016은 각 80개 계산용 조각이며 그중 표 1개는 캐시하지 않는 기존 경로다.

캡처 파일 I/O가 들어간 실행의 프레임 시간은 성능 결론에 사용하지 않는다. 캡처 실행 36개는 기존 계약만 검증했다: empty/focus/stimulus 준비·계산 0회, dirty Enter만 준비 1회, global error 0건.

compat unit-test harness는 이 입력을 직접 재생한다. 6 fixture × cold / warm-same / warm-value-edit × 7회 = 126회이며, 각 회에서 활성 계측/비활성 계측의 순서를 교대한다. warm은 별도 캐시를 먼저 생성한 뒤 재호출한다. warm-value-edit는 첫 plain `<ml:real>`의 값을 `123`으로 바꾼다. 각 측정 결과는 ordinary `worksheet_xml`과 전체 바이트를 대조하고, 생성된 영역 ID·좌표·math 범위는 DOM oracle과 대조한다. 검증·파일 읽기·캐시 사전 준비는 timed build 밖이다.

warm-same은 builder 자체를 강제로 호출한 비용이다. 실제 앱은 동일 입력이나 좌표만 바뀐 입력에서 상위 signature/geometry 캐시로 builder를 생략할 수 있으므로 이 수치를 매 프레임 비용으로 해석하면 안 된다. full source preparation, display DOM decode, 계산 worker 시간도 이번 범위 밖이다.

## 결과

단위 ms, 각 7회 중앙값이다. 비활성 열은 동일 test binary에서 phase recorder를 끈 값이며 test-only TLS 체크는 남아 있다. 비-test 제품 빌드에는 모듈·시계·TLS·분기가 생성되지 않는다.

| 입력 | 조각 | cold 비활성 | cold 활성 | warm 동일 비활성 | warm 값 수정 비활성 | 수정 시 cache hit |
|---|---:|---:|---:|---:|---:|---:|
| 4k scalar, saved cache 있음 | 4000 | 27.279 | 29.482 | 5.994 | 5.846 | 3999 |
| 4k scalar, saved cache 없음 | 4000 | 27.486 | 28.621 | 5.972 | 5.926 | 3999 |
| 100개 16×16 matrix, cache 있음 | 100 | 7.451 | 7.357 | 0.441 | 0.491 | 99 |
| 100개 16×16 matrix, cache 없음 | 100 | 7.682 | 7.785 | 0.473 | 0.644 | 99 |
| 원본 1015_LCC_S | 80 | 2.083 | 2.012 | 0.566 | 0.574 | 78 |
| 원본 1016_LCC_S | 80 | 1.989 | 2.015 | 0.556 | 0.534 | 78 |

활성 계측 cold 세부값:

| 입력 | retain | order | globals | lookup | wrap/parse | label walk | label patch | payload | assemble | 미계측 |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| 4k, cache 있음 | .122 | .260 | .144 | .243 | 9.264 | 9.865 | 2.165 | 2.321 | 3.327 | 1.964 |
| 4k, cache 없음 | .134 | .263 | .141 | .229 | 8.942 | 9.337 | 1.978 | 2.206 | 3.266 | 1.935 |
| matrix, cache 있음 | .003 | .009 | .042 | .005 | 4.006 | 2.396 | .149 | .232 | .412 | .117 |
| matrix, cache 없음 | .004 | .008 | .044 | .005 | 4.165 | 2.460 | .148 | .312 | .443 | .124 |
| 1015 | .002 | .005 | .004 | .004 | .610 | 1.091 | .082 | .042 | .109 | .054 |
| 1016 | .002 | .005 | .004 | .004 | .637 | 1.093 | .081 | .043 | .109 | .051 |

`retain`은 active-ID 집합과 제거된 cache entry 정리, `order`는 중복 ID·좌표·budget 검증과 stable geometric sort, `globals`는 globalDefine 사전 처리다. `lookup`은 exact source/dependency 검증과 hit의 definition 재적용·labelled 문자열 clone을 포함한다. `wrap/parse`는 namespace context 포장과 roxmltree 파싱이다. `label walk`는 구조 검증·bound/local scope·identifier decoding/추론·patch 문자열 직렬화·definition 적용을 포함하며, 중첩된 `label patch` 시간은 뺐다. `label patch`는 wrapped 문자열 clone·역순 replace·context 제거다. `payload`는 source/labelled 보존, dependency payload 산정과 cache 삽입/제거/eviction이다. `assemble`은 최종 region/math wrapper·좌표 문자열·metadata 범위·전체 source 조립이다. spec-table의 내부 라벨 작업은 `label walk` 전체에 남는다.

각 sample에서 `sum(exclusive phases) + unaccounted == total`을 ns 단위로 확인했다. 표의 **열 중앙값은 서로 더할 수 없다**. 미계측에는 phase 사이의 실행·계측 자체 기록 비용, metadata/vector 초기화와 일부 drop/deallocation 등이 남는다. 4k에서 약 1.9 ms다. 수만 번의 시계/TLS 접근 때문에 활성 계측이 cold 4k에서 약 1–2 ms 더 걸렸고, 작은 fixture는 실행 순서·allocator·CPU 상태 잡음으로 활성 값이 더 작기도 하다. 이 차이를 특정 phase에서 임의로 빼지 않았다. headless optimized dev/test profile CPU 자료이며 GPU/window latency, 제품 release 처리량, r93 대비 통제 개선량을 뜻하지 않는다.

4k warm 값 수정은 parse/label miss가 1개뿐이다. 활성 계측에서 cache lookup 약 1.7–2.0 ms, assemble 약 2.8–3.0 ms가 남는다. 원본 warm은 cached math 79개 중 78개 hit와 uncached 표 1개를 처리하며 표의 label walk 약 .34–.36 ms가 남는다. 표의 cache 제외는 기존 계약이며 이번 계측이 바꾸지 않았다.

## 다음 제품 변경 후보

가장 작은 독립 후보는 output assembler의 임시 문자열 제거다. 현재 `<math resultRef=...>{math}</math>` 전체를 `format!`으로 만든 뒤 최종 output에 다시 복사한다. 정확히 같은 wrapper 바이트를 write/push하고 fragment를 직접 붙이면 label/전역 환경을 바꾸지 않는다. cold 4k 약 3.3 ms, warm 약 3 ms가 속한 구간이지만 그 전체가 제거 가능한 비용이라는 뜻은 아니다. 좌표 `Display`, `-0`, UTF-8 범위, table 분기, source budget/error 시점까지 differential로 고정한 뒤 작은 실제 패치 전후 측정이 필요하다.

그다음 cold 후보는 label walk의 반복 identifier decode 및 문자열 생성이다. definition LHS는 walk의 `identifier_parts`, `declaration_label` 내부 decode, 마지막 definition 반영 때의 decode와 declaration_label을 거친다. `serialize(NativeIdentifier)`는 `escape`의 연속 replace와 임시 문자열을 만든다. 같은 node의 검증 결과를 지역적으로 재사용하거나 동일 escaping을 한 번에 쓰는 방법을 검토할 수 있다. 그러나 explicit/contextual/None(`*`)/SYSTEM/함수·indexed LHS/서브스크립트의 validation과 오류 순서를 그대로 지켜야 한다. 현재 약 10 ms 중 어느 부분이 이 후보인지는 아직 측정하지 않았으므로 절감량을 약속하지 않는다.

더 큰 후보는 cold 4k 약 9 ms의 fragment wrap/parse를 공유 parse로 바꾸는 것이다. fragment별 namespace·malformed rejection·global prepass·table fallback·원본 바이트 provenance를 함께 바꾸므로 작은 다음 패치로 권하지 않는다. cache payload는 약 2.2 ms지만 이를 생략하면 warm 비용·메모리 보존 계약이 달라진다. order/global은 합쳐 약 .4 ms여서 우선순위가 낮다. worker로 옮기는 설계나 지연 display decode는 이번 변경에서 다루지 않았다.

## 검증과 재현 자료

새 일반 회귀 2개가 timing 활성/비활성의 cold/warm/reorder/UTF-8/table source·metadata 일치, NaN 좌표·duplicate ID·malformed XML·invalid contextual flag 오류와 다음 실행 초기화를 검증한다. compat 전체와 app/compat all-target Clippy `-D warnings`도 통과했다. 제품 의미 패치는 없다.

- 테스트: `editor_bridge/worksheet_profile.rs`의 `actual_open_worksheet_build_phase_profile` (ignored).
- 입력: `artifacts/rea/worksheet-preparation-inputs-20261019/*.json`.
- 캡처 계약 로그: `artifacts/rea/worksheet-preparation-capture-20261019.log`.
- 126 sample 원시 로그: `artifacts/rea/worksheet-preparation-profile-20261019.log`.
- 합계·hit/parse count·baseline SHA 검증과 중앙값: `artifacts/rea/worksheet-preparation-summary-20261019.json`.
- 집계 스크립트: `artifacts/rea/summarize-worksheet-preparation-20261019.py`.

캡처는 `MATHCRAFT_WORKSHEET_CAPTURE`에 절대 출력 폴더를 지정하고 app의 기존 `actual_open_first_input_profile --ignored --nocapture`를 실행한다. 동일 폴더를 지정하고 compat의 위 profile을 실행하면 된다. 환경변수를 지정하지 않은 기존 앱 측정은 파일 I/O 없이 유지된다. r94 archive/manifest와 r93/r94 성능 로그·0090/0094는 덮어쓰지 않았다.
