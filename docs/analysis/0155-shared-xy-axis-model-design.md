# 제안: 공유 XY 축 식을 보존하는 최소 모델

2026-10-19. **구현 제안**이며 원본 기능 구현 완료 기록이 아니다. r154 제품 소스는 수정하지 않았다. 현재 PlotSpec, plots::evaluate_in_context, InlinePlotEdit, modal 편집, native_plot/source compare와 [0151](0151-native-xy-axis-pairing-contract.md)의 공유 IEquationItem 계약을 비교했다.

## 문제와 권고안

현재 PlotSpec은 legacy x_expression/y_expression과 traces의 explicit x/y pair를 가진다. resolved_traces는 owned pair 목록을 만들어 평가·일부 저장 경로에 전달한다. 이 모델로 native의 X=[x0],Y=[y0,y1]을 두 pair로 펼치면 x0 편집의 공유 의미가 사라진다. 단순 문자열 dedup도 원래 독립적으로 작성한 동일한 식을 원치 않게 연결한다.

권고안은 **권위 있는 source를 하나의 mode로 두고**, shared source에서는 순서 있는 x/y 식 목록을 직접 저장하는 것이다. 새 중복 mirror를 유지하지 않는다.

```text
PlotSpec
  source: PlotSource
  width, height
  x_axis, y_axis                 // 표시/축 설정: 식 source와 별도

PlotSource
  LegacySingle { x, y }          // 이전 파일 의미 그대로
  LegacyPairs { wire_legacy, pairs }
  AxisLists {
    next_equation_id,
    x: Vec<AxisEquation { id, source }>,
    y: Vec<AxisEquation { id, source }>,
    appearances: Vec<TraceAppearance { label, style }>
  }
```

EquationId는 plot 소유 범위의 단조 증가 ID이며 native에 없던 UUID를 원본 identity라고 주장하지 않는다. 행 편집·이동·draft/IME 대상을 안정적으로 가리키기 위한 독립 내부 identity다. 두 목록은 각각1..16개, 전체 trace 수는 max(nx,ny), appearances 길이는 그 수와 같아야 한다. 식 source는 현재 지원 문자열을 우선 사용하고, 별도 구조 편집 도입 시 그 타입을 바꾸더라도 identity·목록 소유권은 유지한다.

legacy Rust field를 그대로 두고 Option<AxisLists>를 추가하는 방식은 수정 줄 수가 적지만 모드 간 동기화와 직접 field 편집이 다시 중복 권위를 만든다. 이 방식을 임시로 쓰더라도 validate가 mixed source를 거절하고 legacy fields를 wire adapter 내부로 숨겨야 한다. 장기적으로는 위 enum과 좁은 접근 API가 더 작은 의미 모델이다.

## Backward compatibility와 서명

JSON wire는 custom serde adapter로 구분한다. 기존 x_expression/y_expression/traces 파일은 그대로 읽어 LegacySingle/LegacyPairs로 보존한다. 아직 migration하지 않은 legacy mode는 기존 field 순서·기본값·inactive legacy mirror 값을 그대로 직렬화하여 기존 cache/signature oracle을 유지한다. wire_legacy는 이전 파일 bytes/의미 보존용이며 앱 편집 권위가 아니다.

새 파일은 axis_equations 같은 단일 새 source object를 쓰고 legacy pair source fields와 동시에 받지 않는다. unknown fields·중복 identity·잘못된 next_id·0개 목록·개수 초과를 거절한다. 기존 binary가 새 파일까지 읽는 forward compatibility를 약속하는 제안은 아니다. 새 버전이 이전 파일을 읽고 저장할 수 있는 backward compatibility다.

migration은 명시 편집 트랜잭션 또는 native import 때 한 번 한다.

* LegacySingle → x/y 식 각각 하나.
* LegacyPairs N개 → x N개/y N개, 전부 별도 ID. **같은 문자열도 합치지 않는다.** 기존 독립 편집 의미를 바꾸지 않는다.
* native shared source → 원래 x/y 개수와 순서를 그대로 보존하고 각 native 식 슬롯에 내부 ID를 부여한다. 패키지 resultRef/region-id를 equation ID로 재사용하지 않는다.

계산 서명은 PlotSpec에 대해 일반 serde 경로를 사용하므로 새 source graph가 포함된다. 같은 pair 값이어도 공유1/N과 독립N/N은 다른 작성 source다. 이전 cache를 source graph 변경 후 synchronized로 위장하지 않는다. conservative invalidation으로 이전 곡선을 Pending 표시로 유지할 수 있으며 source 수정 자체가 계산 실행을 의미하지 않는다.

## Pair는 borrowed view로만 만든다

`trace_bindings() -> iterator<TraceBinding { index, x_id, y_id, &x_source, &y_source, &appearance }>`를 제안한다. i번째 binding은 min-index 규칙으로 만든다. 이는 평가·렌더링용 view이며 저장·편집에 되쓰지 않는다.

resolved_traces는 기존 호출자를 위한 읽기 전용 adapter로 잠시 남길 수 있지만 새 shared source를 다시 authoritative Vec<PlotTraceSpec>으로 저장해서는 안 된다. evaluator/native writer/cache compare는 borrowed binding 또는 축 목록을 직접 받아야 한다. style/label은 trace appearance에 한 번, 식은 각 축 목록에 한 번만 둔다.

native writer는 실제 x/y 목록을 각각 한 번 쓴다. parser는 x/y 식 개수와 trace count=max(nx,ny)를 검증하고 원본 ordinal styles/resultRef를 보존한다. raw unknown metadata는 기존대로 Opaque다. same_source는 local EquationId를 다른 파일과 비교하지 않되 **목록 길이·식 순서·공유 구조**를 비교해야 한다. resolved pair 값만 비교하여1/N과N/N을 같다고 만들지 않는다.

## 현재 UI의 변경 지점

InlinePlotEdit는 lazy draft clone·Enter/focus commit·IME 억제 구조를 유지한다. 대상만 bool x에서 `AxisEquationKey { axis, id }`로 바꾼다. primary_x/y 접근자는 각 목록의 첫 항목을 가리키는 adapter가 된다. IME checkpoint도 ID를 기억하여 중간 행 추가/삭제 후 다른 식에 늦은 IME 이벤트를 적용하지 않는다. 조합 중 대상 삭제는 명시 cancel로 처리한다.

현재 modal의 traces.iter_mut에 있는 trace별 x/y 편집 칸은 x 목록과 y 목록 편집으로 바꾼다. shared 식을 trace마다 복제한 TextEdit를 두지 않는다. trace style 행에는 연결된 식 identity를 읽기 전용으로 보여 주거나 원본 축 행으로 이동하게 한다. inline의 기본 두 칸은 첫 항목을 편집하며 나머지 식은 동일 source 목록을 가리킨다. 전체 원본 축 목록 배치·탐색을 구현하기 전에는 UI parity를 주장하지 않는다.

0151의 Shift+Enter/Enter 구분을 유지한다. 명확한 첫 단계는 불균등 native import·기존 식 수정·공유 재계산·동일 목록 native save다. 임의 중간 삽입/삭제의 trace style/identity continuity는 아직 완전히 확정되지 않았으므로 이를 새 auto-style 정책으로 조용히 대체하지 않는다. 그 명령은 별도 evidence와 회귀로 확장한다.

## 평가 소유권: 공유 identity가 1회 실행 증거는 아니다

현재 evaluate_in_context는 resolved_traces를 순회하여 evaluate_trace를 각각 호출한다. evaluate_trace는 x/y를 모두 initial query한 뒤, x가 bare name이고 y가 이를 참조하면 initial y 결과를 버리고 sample마다 y를 다시 평가한다. shared vector x가 N trace에 사용되면 x parse/query도 N번 반복한다. 이 비용과 난수·고비용 함수의 실행 횟수는 새 모델에서 명시적으로 다뤄야 한다.

다만 **원본이 공유 식을 반드시 한 번만 실행한다는 양성은 아직 없다.** 원본 TraceModel1281/1287은 trace별 RL에 식 RL을 포함한다. PlotCalculatable1116/1117→ControlsCore.NodeHandleMapper0496/0498은 cached value를 읽는 것이 아니라 오류 위치/node handle 표기를 재매핑한다. 원본 engine가 공유 RL을 memoize하는지와 난수 함수의 정확한 실행 횟수는 이 정적 UI 모델만으로 확정할 수 없다.

따라서 다음을 구분하는 평가 계획이 필요하다.

첫 구현 단계는 **현재 per-trace 수치 평가 의미를 유지**한다. shared identity 도입과 parse 결과 공유만 먼저 다루며, 아래 수치 결과 재사용 항목은 후속 검토다. 난수·사용자 함수의 실행 횟수를 모델 개편에 끼워 바꾸지 않는다.

1. parse/검증 AST는 commit당 EquationId별 한 번 준비한다. 이것은 수치 실행을 바꾸지 않는다.
2. 안정된 기존 worksheet binding으로부터 range/vector를 읽는 경우는 동일 계산 context 안에서 재사용할 수 있다. 단순 문자열 같음으로 별개의 equation ID를 합치지 않는다.
3. 사용자 함수·난수·시간·외부 입력을 포함할 수 있는 식의 실행 결과는 purity/원본 ownership 확인 없이 전역 memoize하지 않는다. 초기 공유 source 지원에서 기존 per-trace 평가를 유지하는 편이 잘못된 once-only claim보다 정직하다.
4. pointwise 함수 결과를 재사용한다면 키는 EquationId만이 아니라 x binding identity, 실제 sample vector/domain, scaling, worksheet 계산 generation을 포함해야 한다. 동일 shared y가 서로 다른 x와 짝지어지면 결과가 달라질 수 있다.
5. commit 안에서만 캐시하고 문맥/식/domain/단위가 바뀌면 폐기한다. 문서 열기·saved/native curve 복원에는 새 식 평가를 추가하지 않는다.

미사용 initial y query를 제거하는 refactor도 난수 호출 횟수를 바꾸므로 별도 회귀와 독립 정책/원본 근거를 구분하여 검토한다. source identity 모델 도입과 evaluator 의미 변경을 한꺼번에 숨기지 않는다. 원본 once-only가 나중에 확인되면 같은 planner에 명시 ownership을 연결할 수 있다.

원본 불균등 목록 import, 공유 소유권을 유지하는 primary 식 수정, 목록 형태를 그대로 native 저장하는 흐름을 첫 기능 단계로 삼는다. append/Shift+Enter·중간 삭제의 source association과 trace appearance continuity는 그 뒤에 분리할 수 있다. 첫 단계에서 지원하지 않는 명령을 독립 pair 편집으로 우회시키지 않는다.

## 제안 검증 순서

legacy wire/signature golden 보존 →1/N 및N/1 import/save source 개수 보존 →shared 행 하나 수정 시 해당 trace만 함께 Pending/commit →동일 문자열인 독립 ID는 독립 수정 →IME/Undo/Enter atomicity →cache source graph 불일치 거절 →기존 range/vector/quickplot 계산 회귀 순서로 확인한다. 난수/고비용 호출 count는 구현의 관찰값으로 기록하고 원본 횟수라고 부르지 않는다.

추가 원본 원시 근거는 ignored `feedback/native-xy-model-evaluation-ownership-cil-20261019.txt`, `native-xy-model-node-mapper-cil-20261019.txt`에 있다. NodeHandleMapper0498 IL SHA=`97e15b9d557b0562789f4b3fb83c974e18eeea6ec8d4cbeebb989ef8baa212f9`; PlotCalculatable1116 SHA=`1f20ea396008251a2a877515adaf7b3db1683840d839df201b47085faf6c73e0`. 기존0146/0151 REA identity와 설치 bytes를 재검증했다. 이 문서는 설계 제안이며 제품 코드는 변경하지 않았다.
