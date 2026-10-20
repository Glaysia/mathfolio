# 계산 입력 준비 비용 2차 분석

2026-10-16. [0058](0058-input-and-calculation-latency.md)의 quadratic 정의 복제 제거 이후에도 남아 있던 cold 준비와 drop 지연을 조사했다. GUI, 스크린샷, 원본 실행 없이 Rust CPU 테스트와 기존 정적 REA 계약을 사용했다. 원본 drag 중 transform/drop 시 모델 확정 경계와 수식 draft/Enter 확정 경계를 바꾸지 않는다.

## 단계별 재현

계산 준비를 signature·편집기 검증·MathML 직렬화·문맥 라벨 처리와 worksheet 생성·표시용 tree 복원으로 나눴다. 계측은 `cfg(test)`의 thread-local recorder에만 존재하며 제품 실행 파일에는 시계·환경 변수 검사·계측 저장소가 포함되지 않는다.

Windows Cargo test profile(자체 코드 opt-level 1, dependency opt-level 2), 단일 headless 실행이다. release 프레임률이나 실제 창의 체감 성능으로 해석하지 않는다. 이전 문서의 다른 시각 실행값 대신 이번 단계 계측을 전후 기준으로 삼는다.

| 작업 | signature | 검증 | 직렬화 | worksheet/라벨 | 표시 tree 복원 | 총 시간 |
|---|---:|---:|---:|---:|---:|---:|
| 수정 전 4,000 scalar cold | 5.343 | 3.694 | 6.131 | 34.006 | 30.583 | 80.279 ms |
| 수정 전 4,000 scalar 좌표 변경 | 4.799 | 2.233 | 6.463 | 30.099 | 31.003 | 75.296 ms |
| 수정 후 4,000 scalar cold | 5.628 | 5.345 | 10.043 | 21.515 | 11.953 | 54.574 ms |
| 수정 후 같은 문맥 순서의 좌표 변경 | 4.840 | 0.517 | 1.321 | 2.671 | 0 | 9.436 ms |
| 수정 후 숫자 하나 변경 | 4.792 | 0.448 | 1.000 | 20.837 | 11.694 | 39.033 ms |

cold 직렬화는 안전한 cache를 만들기 위해 원래 tree와 XML을 보존하는 비용도 포함한다. 따라서 각 단계가 모두 빨라졌다고 주장하지 않는다. 실제로 큰 비용이었던 전체 문서의 반복 XML parse를 줄였고, 문맥이 그대로인 배치 변경에서는 그 작업을 반복하지 않는다.

16×16 행렬 100개는 cold 48.315→39.249 ms, 같은 문맥 순서의 배치 변경 46.344→5.136 ms였다. 실제 1015 문서(107개 영역)는 cold 5.496→3.912 ms, 수식 영역의 수평 이동은 0.730 ms였다. 수정 전 실제 문서의 두 번째 계측은 text 영역을 옮겨 계산 입력이 그대로인 cache hit였으므로, 실제 문서 이동의 전후 값으로 사용하지 않는다.

## 중복 parse 제거

기존 코드는 완성한 worksheet XML을 한 번 parse한 뒤 각 `math` 노드를 namespace가 있는 standalone 문자열로 복사하고 다시 parse해 `PortableTree`로 만들었다. 4,000개 입력에서는 이 표시 복원 단계만 약 31 ms였다.

`tree_from_mathml_node`는 이미 parse된 `roxmltree::Node`를 받는다. 기존 string bridge와 동일한 wrapper 속성·자식 개수·구조 tree 검증을 사용한다. MathML 해석의 NodeKind match 본체를 별도로 구현하지 않는다. 상속 namespace를 유지하면서 영역마다 문자열과 XML document를 새로 만들 필요가 없어졌다. 이 교정만 적용했을 때 해당 단계는 약 11 ms였다.

`worksheet_xml`의 global 정의 prepass도 모든 fragment를 XML로 parse했다. XML local name `globalDefine`에는 character/entity escape를 쓸 수 없으므로 그 정확한 바이트가 없는 fragment는 global 정의일 수 없다. 이런 fragment는 전역 탐색 parse만 생략한다. 전체 fragment 검증과 문맥 라벨링은 기존 경로가 계속 수행한다. 문자열 안에 우연히 `globalDefine`이 있는 false positive는 기존 parser 경로를 사용한다. 잘못된 XML을 성공시키는 필터가 아니다.

## 재사용의 정확한 경계

raw MathML 직렬화 cache는 region ID와 **PortableTree 전체 값 동등성**으로 검사한다. 이름·값·단위·명시 라벨·contextual metadata·literal subscript 등 어떤 tree 값이든 바뀌면 다시 직렬화한다. 메모리 주소, 길이, arena 번호만으로 source가 같다고 판단하지 않는다. 제거된 영역의 cache는 정리한다. 이를 위해 영역별 원래 tree와 raw XML을 추가 보존하므로 메모리를 더 쓰는 선택이며, worker에 전달하는 완성 source는 기존처럼 Arc로 공유한다.

배치 전용 cache는 다음 조건이 모두 맞을 때만 이미 생성된 worksheet의 `left`/`top` attribute 값을 교체한다.

- 활성 statement들의 ID와 전체 raw XML이 입력 순서까지 정확히 같다.
- `worksheet_region_order`로 계산한 문맥 라벨 순서가 같다. 이는 실제 `worksheet_xml`이 사용하는 validation과 stable top/left 정렬 함수를 공유한다. 임의의 다른 정렬을 cache에만 추가하지 않는다. 이 좌표들은 이미 import/load가 끝난 worksheet 좌표이며 원본의 grid-load 지원 범위를 확대하지 않는다.
- 문서 width/height/grid의 bit 값과 보존한 native archive 전체 바이트가 같다.
- opaque solveblock이 없다. solveblock의 위치/내부 scope 의존성을 추측해 재사용하지 않는다.

한 번 parse한 XML에서 top-level region의 좌표 attribute byte range를 보존한다. 바뀐 숫자를 정렬한 range에 따라 한 번 선형 복사하여 worker에 **새 좌표를 가진 XML**을 보낸다. 전체 문서를 다시 parse해 라벨 tree를 만들지 않는다. 식의 계산 결과, engine scope, pending 상태를 cache hit라는 이유로 재사용하는 변경은 아니다. source가 바뀌면 기존 worker가 다시 update한다.

내용이나 순서·설정이 바뀌면 기존 전체 준비 경로로 돌아간다. explicit identifier label을 지우거나 cached contextual label을 강제로 고정하는 정규화는 넣지 않았다. 새 정의/배치에 따라 단위와 변수가 다시 해석되는 경계를 보존한다.

## 전체 입력 경로 측정과 한계

production과 같이 worker를 켠 기존 headless 이벤트 benchmark도 실행했다.

| 입력 | cold retain | key frame | commit submit | clean 전체 계산 submit |
|---|---:|---:|---:|---:|
| 4,000 scalar | 50.800 ms | 0.823 ms | 46.622 ms | 6.703 ms |
| 16×16 행렬 100개 | 40.106 ms | 0.753 ms | 26.286 ms | 5.033 ms |
| 실제 1015 | 3.614 ms | 0.348 ms | 2.983 ms | 0.288 ms |

실제 4,000개 영역의 move-handle press→60개 delta→drop benchmark는 press 1.729 ms, delta 총 100.331 ms(평균 1.672 ms/frame), drop 44.041 ms였다. 이전 동일 경로의 drop은 73.602 ms였다. 중간 source/dirty/undo/worker submission은 0, drop 제출은 1회다. 순수 배치 준비의 9.436 ms를 이 전체 drag-drop 시간으로 대신 제시하지 않는다. 진단에서는 raw XML·설정·archive가 모두 같았지만 원래 같은 row/tie에 있던 두 정의의 순서가 실제로 바뀌었다(`old_order=[0,2,…]`, `new_order=[2,0,…]`). cache가 올바르게 전체 경로로 돌아간 경우다. contextual metadata에 의한 false positive라는 초기 추정은 진단 결과 배제했다.

별도의 실제 pointer benchmark는 정의 4,000개를 서로 다른 row에 놓고 y delta 0의 수평 이동을 수행했다. 기존 순서 변경 benchmark를 대체하지 않았다. 이 경로는 press 1.465 ms, 60개 delta 총 56.774 ms(평균 0.946 ms/frame), drop 12.555 ms였다. delta 중 source·dirty·undo·worker 제출 불변과 drop 제출 1회를 검사하고, cache의 baseline source Arc가 그대로 유지되어 실제 재사용 경로를 탔음을 확인했다. 테스트는 `benchmark_4k_drag_preserving_definition_order`다. 큰 문서의 내용 변경과 문맥 순서 변경에서는 여전히 전체 준비가 남는다.

보존 로그:

- `artifacts/rea/input-second-pass-before-20261016.log`
- `artifacts/rea/input-second-pass-node-20261016.log`
- `artifacts/rea/input-second-pass-geometry-20261016.log`
- `artifacts/rea/input-second-pass-after-20261016.log`
- `artifacts/rea/input-second-pass-events-20261016.log`
- `artifacts/rea/input-second-pass-drag-20261016.log`
- `artifacts/rea/input-second-pass-drag-same-order-20261016.log`
- `artifacts/rea/input-second-pass-drag-diagnostic-20261016.log`

계측은 `cargo test -p mathcraft-app preparation_phase_profile --locked --offline -- --ignored --nocapture`로 재현한다. 전체 이벤트/drag 테스트 이름은 [0058](0058-input-and-calculation-latency.md)에 기록되어 있다.

## 정합성 검증

기존 준비 회귀 3개와 새 cache 회귀 5개가 통과했다. 반복 이동·긴 숫자 좌표·동일 길이 source 변경·order 변경·grid/page 설정·같은 크기 archive 변경·opaque solveblock fallback을 검사한다. cache 결과는 cache 전체를 버리고 생성한 XML 및 라벨 tree와 비교한다. 정의 이동에 따른 unit→variable 라벨 변화와 ORIGIN 정의 순서에 따른 실제 계산 결과도 확인한다.

새 bridge 회귀 3개는 string/Node 입력의 namespace 및 검증 결과 동등성, 뒤에 있는 global 정의, malformed XML 거부, 문자열 false positive, 정확한 stable order/좌표 validation을 확인했다. 기존 worker 7개, background 통합 8개, drag transaction 5개도 다시 실행해 통과했다. app의 all-targets Clippy는 `-D warnings`로 통과했다. profile·전체 입력·순서 변경 drag·순서 보존 drag benchmark도 직접 실행해 통과했다. 전체 workspace 검증은 다른 병렬 구현을 포함한 source freeze 이후 별도로 수행해야 한다. 실제 GUI의 모든 지연이 없어졌다고 판단하지 않는다.
