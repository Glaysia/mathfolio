# 빈 XY 작성과 trace별 결과 보존 설계

2026-10-19. [0163 원본 계약](0163-native-empty-and-partial-plot-contract.md), [0157 슬롯 identity](0157-native-xy-slot-authoring-contract.md), [0148 결과 codec](0148-native-xy-result-contract.md)를 현재 앱/문서 경로에 연결하는 **독립 구현 설계**다. 원본의 enum/error/native payload를 새로 추정한 문서가 아니다. 이 설계를 정리하는 동안 제품은 r160 검증 source를 유지했으며, 구현은 이후 별도 gate로 검증한다. GUI·원본 runtime·CLR은 사용하지 않는다.

## 원본과 현재 공백

0163의 NewPlot null 경로는 빈 X/Y 각1slot과 trace1개를 만든다. EquationList.AddItem은 nonempty 문자열일 때만 식을 설정하므로 기본 PH를 남긴다. 현재 `CanvasDocument::add_plot`은 `PlotSpec::default()`의 x/sin(x) sample을 넣는다. 새 작성은 별도 `blank_xy()`를 사용하고, 기존 Default·old JSON·명시 식 parameter는 보존한다. 빈 문자열은 현재 native source codec의 `source_tree`에서 이미 MathML PH로 저장된다. 공백 문자열이나 임의 parse 실패를 PH로 조용히 정규화하지 않는다.

0163의 TraceModel1279/128d/128f는 실패한 **자신의 points만** 지운다. 현재 `plots.rs::evaluate_in_context`는 첫 `evaluate_trace(...)?`에서 plot 전체를 Err로 반환한다. `native_plot_source::saved_row`도 한 cache가 없으면 전체 None, 진단/unsupported이면 전체 Err를 반환한다. `export::plot_results::literals`는 Vec<String> 전체 성공을 요구한다. 이 세 경로 모두 순서를 유지하는 부분 결과가 필요하다.

빈 식의 조건부 자동 SetEquation 경로1243은 commit을 생략하지만, 명시 PlotControl.Commit01f3/iterator14f3는 PH trace도 요청한다. native engine의 PH result/error 코드는 미확정이다. 아래 Incomplete는 **작성/표시 상태**이며 native 성공값·0·sin·인증된 engine error가 아니다. 명시 Enter/F5의 완전한 PH engine 동등성은 별도 미확정으로 남긴다.

## 최소 runtime API

`Result<PlotData,String>`의 outer Err는 spec/context/전체 구조가 유효하지 않아 trace association 자체를 만들 수 없는 경우에 남긴다. 일반 한 trace의 parse/evaluation 오류는 모든 ordinal을 채우며 수집한다. shared X가 실패하면 그 X를 사용하는 각 pair가 각자 실패할 수 있다. 동일 문자열을 dedup/memoize해 평가 의미를 바꾸지 않는다.

```rust
struct PlotData { traces: Vec<TraceOutcome> }
enum TraceOutcome {
    Ready(PlotTraceData),
    Failed(String),
    Pending { previous: Option<PlotTraceData> },
    Incomplete,
    Unavailable(String),
}
```

Ready의 PlotTraceData는 기존 points/implicit_domain/label/style이다. Failed는 실제 이번 trace 실패이며 이전 curve를 표시하지 않는다. Pending만 이전 inert curve를 선택적으로 보유한다. Unavailable은 지원하지 못한 원본 presentation/metadata의 진단으로, 원본 engine 실패와 구별하며 원문은 NativePackage에 남긴다. print 차단 등 기존 지원 경계는 has_unavailable()로 유지한다.

공개 helper는 `TraceOutcome::curve()/is_pending()/diagnostic()`, `PlotData::first_curve()/has_failures()/has_pending()/has_unavailable()/invalidate()/to_saved()/from_saved()`다. first_curve는 Option을 반환한다. 기존 첫 trace alias `.points`를 유지하며 실패 slot에 빈 가짜 curve를 넣거나 첫 성공 curve를 trace0인 것처럼 옮기지 않는다.

* traces 길이는 resolved source trace 수와 같고 원래 ordinal을 유지한다. 성공 slot만 filter해서 style/ref와 zip하지 않는다.
* paint는 원래 ordinal로 색·legend·진단 귀속을 정한다. Ready/Pending(previous)만 그린다. 오류 하나가 다른 Ready를 가리지 않는다. 정확한 원본 opacity/오류 색/auto-range는 이 모델 계약의 완료 범위가 아니다.
* source/context invalidation은 Ready→Pending(previous), Failed/Unavailable/Incomplete→Pending(None)으로 명시 전환한다. 기존 Pending은 유지한다. `pending_regions.insert(id)`만 하고 slot을 Ready로 남기지 않는다.
* topology 변경(count/identity/reorder)은 root가 기존/새 spec을 비교해 pending slot을 재구성한다. identity 대응이 증명되지 않으면 이전 curve를 다른 ordinal에 붙이지 않는다. AxisLists의 equation ID와 Legacy ordinal을 문자열 동일성으로 대체하지 않는다.
* parent row의 Pending은 aggregate다. 정상 native cold-open의 Ready+Pending 조합과 실제 upstream invalidation을 혼동하지 않는다. save/export는 slot freshness를 읽고, upstream invalidation은 모든 slot에 먼저 반영한다. 임의 global Pending 해제로 일부 stale slot을 승격하지 않는다.

## 저장 wire와 typed reader

all-Ready는 기존 v1 JSON 문자열을 그대로 쓴다. 기존 네 curve field와 numeric lexeme/golden을 유지한다. mixed state만 v2로 저장한다.

```json
{"version":2,"traces":[
  {"state":"ready","data":{"points":[[1,2]],"implicit_domain":false,"label":"","style":{}}},
  {"state":"failed","message":"y축: …"},
  {"state":"pending","previous":null},
  {"state":"incomplete"},
  {"state":"unavailable","message":"원본 결과 지원 필요"}
]}
```

위 style의 `{}`는 설명상 생략이며 실제 wire에서는 **기존 완전한 PlotTraceStyle 객체**가 필수다. pending.previous는 null 또는 같은 완전한 Curve다. variant별 필수 key/unknown key를 엄격히 검증한다. source-bound reader는 slot 수와 순서를 current spec에 대조한다. Ready의 label/style은 해당 source와 맞아야 한다. Pending의 이전 curve는 역사적 표시이므로 현재 style과 무조건 같다고 주장하지 않는다.

한 payload 2MiB, 1–16slots, curve당 2,000points/finite 좌표, 기존 label/style 제한, diagnostic 16KiB를 유지/추가한다. diagnostic 한계는 독립 저장 정책이다. 실패 시 데이터를 조용히 자르지 않는다. runtime의 CurvePoints는 이동/Arc 공유로 복원하고 prepared render cache는 저장하지 않는다. 빈 points나 unknown state를 성공/0으로 수선하지 않는다.

root 소유 document DTO(제안 이름 `SavedPlotPresentation::parse_for_spec`/`to_json`)가 v1/v2를 정규화하면 native writer가 같은 규칙을 재사용할 수 있다. 앱은 own snapshot을 open/save 때만 decode/encode하며 draw에서 JSON·MathML을 다시 파싱하지 않는다. v1을 복원하면 모든 curve를 Ready로 읽고, 실제 aggregate Pending invalidation이 있으면 호출자가 명시 전환한다. v2의 mixed Pending을 이유로 모든 Ready sibling을 자동 폐기하지 않는다.

## native resultRef와 writer action

기존 `plot_source`가 최종 생성한 `Vec<resultRef>`와 outcome 전체 Vec를 ordinal로 결합한다. ref의 XML 정수 정규화·중복 owner/ref·source/context signature·unknown carrier guard는 유지한다. slot 누락으로 뒤 ref가 당겨지는 형태는 금지한다.

문서 writer에는 `Vec<Option<String>>`보다 다음처럼 의미가 구분된 ordered action이 필요하다.

| action | 의미 |
| --- | --- |
| PreserveOriginal | 동일 source/context에 결합된 원본 payload/status/metadata를 lossless 보존 |
| WriteCurve(xml) | 인증된 current Ready의 finite unitless Trace2dResult를 해당 ref에 Synchronized로 설치 |
| PendingKeepPayload | 아직 새 계산이 없으므로 기존 inert curve와 metadata를 보존하며 Pending 표시 |
| ClearCurveToPending | fresh Failed/새 Incomplete처럼 현재 curve를 지워야 하는 slot. 알려진 curve payload를 제거하고 Pending carrier만 남김 |

현재 `saved_results::upsert(None)`은 Pending 속성만 바꾸고 **old payload를 남긴다**. 이것을 fresh Failed의 clear action으로 쓰면 native reopen에서 실패한 예전 곡선이 다시 나타난다. 반대로 generic Pending 전체를 clear하면 기존 수동/미확정 표시가 사라진다. 두 action을 구분해야 한다.

new native 오류 메시지·NodeHandle·EmptyResult/빈 Trace2dResult를 아직 조립하지 않는다. Empty native resultData의 dispatch early-return은 live ResultManager를 빈 SetResult로 지운다는 근거가 아니다. ClearCurveToPending은 파일의 old payload를 제거해 fresh reopen에서 재생되는 것을 막는 독립 정책이며 live engine clear와 구분한다. own v2는 Failed 진단을 정확히 보관할 수 있지만 native export는 지원하는 Pending-empty carrier로 old curve 부활을 막는 제한을 명시한다. 해당 carrier가 unknown metadata를 품은 원본이면 전체를 새 것으로 바꾸지 말고, 증명된 source/ref 범위에서 patch하거나 안전하게 지원 거절한다. 새 resultRef에는 옛 다른 trace payload를 복사하지 않는다.

native reader도 각 ref를 별도로 Ready/Pending/Unavailable로 복원한다. 한 missing/unsupported ref가 성공 sibling을 지우지 않는다. unknown source/중복 owner로 association 자체가 불명확한 경우만 owner 단위 보존 경계에 남긴다. native error payload의 정확한 형식이 추가 확인되면 해당 slot에만 Failed를 복원하며, 수식이나 cache MathML을 실행하지 않는다.

## 새 빈 plot의 배치와 분리한 범위

0163은 새 source의 X 아래/Y 오른쪽을 확인했다. `ws:*Axis/@legend-position`과 `axisLine/@legendWidth`가 source 필드이며 axis 선 위치와 다르다. 새 blank는 explicit X=PlotBoundaryBottom/Y=PlotBoundaryRight가 필요하다. 기존 absent Y는 schema Left 기본값과 raw reader current-state의 차이가 있으므로 오른쪽으로 조용히 고쳐 쓰지 않는다. optional layout 필드와 blank constructor는 source 모델 worker의 별도 소유이며 runtime outcome 구현이 임의 재해석하지 않는다.

## 필수 회귀와 작업 분담

1. 실제 새 삽입은 PH1/1·trace1·curve0이고, own/native 저장·cold open 계산0. 명시 sample 식을 준 경로와 legacy old JSON은 바뀌지 않는다.
2. Ready/Failed/Ready 세 trace에서 양쪽 성공 curve와 원래 스타일/ref를 보존한다. 첫/중간/마지막 실패, 모든 실패, shared-X 실패, blank slot 추가를 구분한다.
3. Pending에는 이전 curve를 유지하다가 해당 fresh Failed가 오면 그 curve만 제거한다. Invalidated Ready는 save/native export에서 fresh로 승격되지 않는다. topology 변화는 ref/slot 압축 없이 처리한다.
4. all-Ready v1 bytes golden, mixed v2 own cold-open 계산0, native 성공/실패/성공의 ref→payload 매핑·실패 old payload 부활 방지, unknown 진단 원문 보존을 검증한다.
5. render 좌표·finite tessellation·CurvePoints lazy/Arc 공유 회귀와 Manual Enter/F5·generation/cancel guards를 유지한다.

소유는 worker의 `plots.rs`/새 `plots/trace_outcome.rs`와 필요한 calculation_worker 타입 적응, root의 worksheet/cache lifecycle·native restore·export writer·document DTO다. 제품 변경은 이 API를 확정한 다음 진행한다. 원본 PH engine 결과, 모든 axis-global 오류의 영향 범위와 완전한 native 오류 serialization은 구현 테스트 통과와 별개로 미확정에 남긴다.


## 구현 상태와 검증 (동결 전 소유 범위)

`plots.rs`/`plots/trace_outcome.rs`에 위 ordered enum과 표시/계산 수집을 반영했다. `from_saved_versioned`는 root의 `plot_snapshot::SavedPlotSnapshot::parse`를 재사용하므로 variant 필수 key·중복 key·budget 판정이 문서 DTO와 같다. `from_saved`는 version이 필요 없는 기존 호출자용 wrapper다. v1의 기존 inert empty-curve acceptance도 유지하며 v2 Pending.previous로 이행할 수 있다. 이는 새 PH를 Ready empty curve로 생성한다는 뜻이 아니며 fresh native finite codec은 empty curve를 성공값으로 승격하지 않는다.

실패가 첫/중간/마지막인 경우와 공통 X 실패, PH-middle/blank 작성 상태, 두 성공 curve 사이의 오류 ordinal 표시를 검사했다. 오류 뒤 trace를 당겨 색/ref를 다시 붙이지 않는다. prepared points/serialization/IME/input의 기존 회귀를 포함해 **plots 34 PASS**, **calculation_worker 7 PASS**, **app all-target Clippy PASS**, 소유 파일 포맷 검사 PASS다. 마지막 Clippy 정리는 의도적으로 유지한 compatibility accessor/wrapper의 비테스트 unused 허용뿐이며 동작 변경이 아니다. Root의 blank constructor/layout·lifecycle·native reader/writer·양 OS 전체 통합 검증은 별도 소유/기록이다.
