# XY 식별 단위·scaling 소스 설계안

2026-10-19. **구현 제안이며 제품 완료 기록이나 추가 원본 동작 증거가 아니다.** r172 동결 중 기존 Rust와 [0170 원본 계약](0170-native-xy-unit-scaling-contract.md), [0161 입력·순회 계약](0161-native-xy-expression-layout-contract.md)을 읽어 작성했다. 제품 Rust 수정·빌드·GUI·스크린샷·원본 엔진/CLR 실행은 하지 않았다. 기존 REA 증거를 재사용하며 새 원본 실행 결과를 주장하지 않는다.

## 먼저 고정할 의미

원본 PlotEquation은 expression과 scaling을 따로 소유한다. 0170의 Plots0327/033d는 expression 다음 user scaling의 두 Math를 쓰고, 0339는 placeholder scaling을 기본 상태로 취급한다. 0328은 비어 있거나 사용자 변경이 아닌 scaling을 계산 요청의 숫자1로 보낸다. 원본 trace1281은 X/Y 식과 **각 식의** X/Y scaling을 xyplot2에 전달한다. 이는 전역 X축/Y축의 공통 단위 설정이나 단순 tick-label 문자열이 아니다.

성공 결과의 units를 식의 표시 컨트롤에 합치는0352/035d/035e는 보관한 사용자 소스를 복원하면서 수행한다. 따라서 표시 중인 자동 단위를 사용자 source scaling으로 다시 저장하면 안 된다. 사용자가 입력한 숫자0.1과 계산 뒤 보이는0.1 m도 무조건 동일한 source가 아니다. 기본 scaling 없음은 명시 UNIT m과 구분한다.

0170의 설치 도움말 예제는 UNIT m, UNIT in, `Scale(Number("0.1"), UNIT m)`의 소스와 식별 scaling 소유권을 입증한다. 도움말의 약79와20은 정확한79 in = 2 m 식을 입증하지 않는다. complex point를 실수부만 취해 그리거나 원본 xyplot2의 모든 차원 검사·affine unit·음수/0 scaling 정책을 추정할 근거도 아니다.

## 최소 authoritative 모델

일반 식 String 전체를 지금 enum으로 교체할 필요는 없다. 이번 첫 범위는 **기존 plain expression subset + typed scaling**이다. 이름 label/subscript가 있는 일반 axis expression은 기존처럼 native editable codec에서 거절하여 Opaque 원문으로 보존한다. 그것까지 가져오려면 별도의 typed expression 모델이 필요하며 scaling String 도입으로 우회하지 않는다.

제안 공개 API는 다음과 같다. 이름은 구현 단계에서 확정한다.

```rust
// private inner, validated deserialize; JSON은 PortableTree 객체를 투명하게 저장.
pub struct PlotScaling(Box<PortableTree>);
impl PlotScaling {
    pub fn from_tree(tree: PortableTree) -> Result<Option<Self>, String>;
    pub fn as_tree(&self) -> &PortableTree;
    pub fn into_tree(self) -> PortableTree;
}
// from_tree의 단독 Placeholder는 명시적인 source clear(None)다.
// PH가 들어 있는 미완성 복합식은 별도 incomplete source이며 숫자1이 아니다.

PlotAxisEquation { id, source: String, scaling: Option<PlotScaling> }
PlotTraceSpec { x_expression, y_expression, /* 기존 label/style */
                x_scaling: Option<PlotScaling>, y_scaling: Option<PlotScaling> }
PlotSource::Legacy { x_expression, y_expression, traces,
                     x_scaling: Option<PlotScaling>, y_scaling: Option<PlotScaling> }
```

* AxisLists scaling은 해당 equation ID의 소스다. shared X 슬롯 하나가 여러 Y와 짝지어지면 그 X scaling도 같은 하나의 source를 참조한다. trace마다 복제해 새 authority를 만들지 않는다.
* Legacy의 실제 traces가 있으면 **각 pair의 x_scaling/y_scaling**이 활성 소스다. fallback x/y 식과 fallback scaling은 원본 old 모델처럼 비활성이지만 저장 시 보존한다. traces가 비었을 때만 fallback pair가 활성이다. `PlotAxisSpec`에 전역 scaling을 추가하지 않는다.
* `promote_legacy_trace`는 fallback 식과 scaling을 함께 첫 pair로 옮긴다. `into_axis_lists`는 실제 pair 각각의 식+scaling을 새 ID 슬롯에 그대로 옮긴다. 텍스트가 같다는 이유로 dedup하지 않는다. remove/reorder는 scaling도 같은 소유 슬롯과 함께 이동한다.
* 기존 `resolved_traces()`의 owned String 복사 경로를 더 확장하기보다, evaluator/export용 borrowed view `PlotEquationRef { expression: &str, scaling: Option<&PlotScaling> }`, `PlotPairRef { x, y, appearance }`를 추가한다. authored axis iterator는 원래 슬롯 수를 유지하고 pair iterator만 기존 pairing을 적용한다. 이 API는 평가 횟수나 memoization을 규정하지 않는다.

Box는 각 trace에 큰 NodeKind/PortableTree를 inline으로 넣어 모든 CanvasContent/curve 캐리어를 키우지 않기 위한 선택이다. 실제 size/heap 비용은 구현 시 측정한다. 이 설계만으로 메모리 절감을 인증하지 않는다.

## old JSON·서명 보존

새 필드의 absent는 None이며 `default + skip_serializing_if(None)`로 생략한다. Legacy custom PlotSpec serializer의 기존 필드 순서, trace 기존 네 필드 순서, AxisLists 기존 equation의 id/source 순서를 None일 때 바꾸지 않는다. 따라서 old None 문서의 JSON bytes와 계산 서명/SHA는 정확히 같아야 한다. geometry carrier와 기존 labels/line style까지 포함한 고정 golden으로 검증한다. 자동 migration으로 AxisLists를 만들거나 숫자1/빈 PortableTree를 삽입하면 이 조건을 깨뜨린다.

Some일 때만 새 scaling 키가 나타난다. canonical Root PH는 None으로 저장하되, 명시적인 숫자1은 Some(Number("1"))로 유지한다. 이는 absent와 명시 source를 합치지 않기 위함이다. 새 키의 explicit null·중복·unknown 필드 처리는 기존 strict carrier 정책과 맞춰 **입력에서 거절**, 출력에서 absent만 쓰는 안을 권장한다. tree deserialization만으로 검증됐다고 취급하지 않는다.

Some scaling의 모든 노드·label·contextual·literal suffix·순서가 source comparison 및 calculation signature에 포함돼야 한다. UNIT m과 VARIABLE m, contextual true/false/attr 부재, suffix None/Some, Scale와 Multiply를 plain 문자열/동일 숫자값으로 합치지 않는다. `native_plot::same_source`, `matches_retained_source`, 앱 saved-origin 비교, document calculation signature와 background generation guard를 함께 갱신한다. 원본 scaling을 읽지 못하던 구형 모델에 새 의미를 소급 추정하는 legacy adapter는 만들지 않는다.

## 기존 structural bridge를 재사용하는 경계

`mathcraft-document`는 이미 mathcraft-editor와 mathcraft-compat에 의존하므로 새 순환 의존성은 필요 없다. `PortableTree::validate`, `Editor::from_portable`, `set_expressions_only(true)`, `editor_bridge::tree_from_mathml[_node]`와 `tree_to_mathml`를 재사용한다. AnnotatedIdentifier의 labels, NativeIdentifier의 labels/contextual/subscript가 소스 의미를 보존한다. `mathcraft_core::parse(text)`나 일반 formatter를 거쳐 scaling tree를 재생성하지 않는다.

주의할 점은 expressions_only UI 플래그가 import된 모든 tree의 side effect 부재를 입증하지 않는다는 것이다. bridge는 region-root Evaluate/Define 등도 저장할 수 있다. PlotScaling 생성자는 별도 expression-only validator로 정의/전역정의/최상위 Evaluate·symbolic command 등 source statement를 거절해야 한다. 최초 평가 subset은 숫자·Identifier 계열·Group·지원 산술/Scale/Call 등 확인한 노드만 허용하고, 프로그램/적분 등 나머지를 원본에서 금지된다고 주장하지 않는다. 미지원 원본은 Opaque로 남긴다. 미완성 tree를 draft로 편집하는 것과 native writer가 안전하게 저장 가능한지 검증하는 것도 분리한다.

정책 예산은 기존 최대 depth128/nodes16384/atom16384 bytes를 넘지 않는다. 별도로 plot 전체 scaling tree bytes/nodes 합계 예산을 설정해32개 슬롯이 각자 최대치를 소비하는 확대를 막는다. JSON은 기존 serde recursion/입력 bytes 제한부터 적용하고 validate 이후만 설치한다. 예산 수치는 구현 정책이며 원본 한도가 아니다.

0170 도움말의 완전한 plot에는 현재 codec 밖 viewport/형식/marker도 있으므로 scaling 하나를 지원했다고 전체 원본 region을 typed import할 수 있다고 주장하지 않는다. 원본 전체는 계속 Opaque/raw로 보존할 수 있고, 두 Math source 계약은 추출한 식 fragment와 별도 최소 supported fixture에서 검증한다.

native reader는 strict checked XML의 두 번째 `<ws:math>`를 typed tree로 읽는다. 알려진 resultRef는 inert하게 별도 보존하되 unknown attrs, formattingOverride/resultFormat 같은 모델에 없는 형식, comment/PI 등 현재 strict codec 범위를 우회해 지우지 않는다. bridge가 owning region 보존을 전제로 생략하는 presentation sibling을 plot reader가 무조건 허용하면 안 된다. 첫 번째 expression은 기존 plain round-trip 검사를 계속 적용한다.

writer는 expression을 첫 Math, None이면 PH 또는 Some tree를 두 번째 Math로 쓴다. native에는 managed writer의 두 Math 양성 경로부터 지원한다. schema의 한 Math 허용만으로 missing scaling의 원본 reader 의미를 단정하거나 한 Math 문서를 조용히 완성하지 않는다. 일반 subtree의 bridge round-trip을 먼저 검증하고, representation-only canonicalization도 실제 emitted XML의 source attrs를 보존하는 경우만 허용한다. group·label·명시 contextual false를 임의 생략하지 않는다.

원래 source가 같은 export는 root의 raw-preservation 경로로 Math resultRef/unknown metadata를 계속 보존한다. 일부만 이해한 scaling을 새 최소 plot XML로 덮어쓰지 않는다.

## 입력 transaction과 표시 metadata

기존 InlinePlotEdit의 한 aggregate draft와 Enter/focus/Save/F5/Undo 수명을 유지한다. scaling은 단순 TextEdit<String>로 편집하지 않고 expression-only Editor/formula 입력 어댑터로 편집한다. label 버튼/CtrlQ, literal subscript/CtrlMinus, selection/IME·clipboard가 typed tree에 반영돼야 한다. 한쪽만 텍스트로 flatten해서 label을 다시 추론하는 입력은 허용하지 않는다.

필드 키를 `(axis, equation_id, Expression|Scaling)`으로 확장한다. native 목록 순서는 `Y0.expr → Y0.scaling → Y1.expr → Y1.scaling → … → X0.expr → X0.scaling …`이며 내부 wrap은 없다(0161). Legacy는 실제 pair index+기존 exact source 세대 검증에 scaling까지 포함한다. 구성 변경 뒤 이전 ordinal의 IME/caret/draft를 다른 슬롯에 붙이지 않는다. ordinary typing/preedit는 committed source·curve·generation·history를 바꾸지 않고, Enter/기존 command가 검증된 candidate를 한 번 설치한다. structural editor 내부 Undo와 plot aggregate Undo 사이 우선순위는 기존 math adapter를 확인해 테스트로 정한다.

자동 표시 단위는 `PlotAxisPresentation` 같은 별도 캐리어에 둔다. user tree, 계산 결과 units, 보여줄 합성 tree를 분리하고 표시에 보인 m을 None source에 써 넣지 않는다. 이 metadata는 source signature에 참여하지 않으며 own 저장 캐시와 native 결과 units reader로만 복원한다. source/context guard가 실패하면 표시 metadata도 Pending 또는 unavailable이다. 원본 unit-system/base-unit 자동 선택의 완전한 재현을 주장하지 않는다.

## 계산·결과 캐시는 별도 완료 조건

현 `plots::evaluate_trace`는 plain String을 core Expr로 파싱하고 `real_number`가 unitful 값을 거절한다. 필드를 저장할 수 있게 만든 뒤 이 evaluator가 scaling을 무시한 채 Ready를 만들게 해서는 안 된다. UI를 활성화하기 전에 typed scale을 처리하거나 해당 trace만 명시적으로 unsupported/Incomplete로 남기는 adapter가 필요하다. None은 source의 기본 scaling 입력1에 해당하지만 unitful 원래 Y를 dimensionless로 입증하지 않는다.

최소 계산 view는 scaling tree를 기존 context에 side effect 없는 임시 수치 query로 넘긴다. 결과 scalar·finite·실수·차원 검사는 Quantity/기존 engine 연산을 재사용해야 한다. help의 m/in/0.1 m 선형 subset을 독립 구현할 수는 있지만 단순 문자열 단위표로 label을 무시하거나 모든 offset/복소/음수/0 정책을 원본 오류 ID로 포장하지 않는다. shared slot 평가 횟수나 once-only memoization도 아직 원본 계약으로 확정하지 않는다.

기본 scale1 또는 dimensionless0.1의 경우 나머지 units가 결과 metadata에 남아 자동 표시에 참여할 수 있다. 따라서 모든 결과에 `physical_value / user_scale` 후 무차원일 것을 강제하는 알고리즘도 원본 전체 동등성이 아니다. 선형 explicit unit ratio로 무차원임을 증명한 좁은 case와 남은 unit metadata case를 구분해야 한다. 이를 위한 future `PlotAxisSample { coordinates, units }`/`PlotAxisPresentation` 표현은 별도 계산·codec 설계 항목이다.

현재 `native_plot_result::finite_unitless_xml/read_result`는 유한 f64만으로 단위 유무를 추론하지 않는다. 이번 source 설계에서 unitMonomial reader/writer를 확장하거나 native united payload를 unitless Ready로 승격하지 않는다. source는 지원해도 cache units가 미지원이면 raw 원문은 유지하고 cold-open outcome을 unavailable/Pending으로 남긴다. fresh writer는 양축 좌표가 실제 무차원이라는 caller 증명일 때만 기존 codec을 사용할 수 있다. unit-bearing 캐시는 units10 이름·유리수 지수·system까지 별도 typed metadata와 독립 XSD/serializer 근거로 구현해야 한다. 현재 strict own plot snapshot v1/v2의 points/style DTO를 단위 캐리어인 것처럼 재해석하지 않는다. cache metadata 확장은 별도 version 또는 검증된 sidecar와 source/context guard를 설계한 뒤 한다.

source commit은 기존 per-trace invalidate/실패 슬롯의 이전 curve 제거/Manual old Pending curve 수명을 따르며, 비교 키에 scaling이 반드시 들어간다. shared X scaling 변경으로 영향받는 trace와 source/context 전체 invalidation 정책을 일원화한다. 새 소스를 받아들이면서 구형 NativeMathml origin marker나 Sync 좌표를 유지하는 shortcut은 금지한다. cold open은 engine query0을 유지한다.

## 구현 전 통과시킬 검증

1. **호환 golden:** old Legacy/AxisLists/geometry JSON bytes·서명/SHA 동일, None 생략, 숫자1과 None 구분. 새 키 duplicate/null/unknown·tree arity/depth/총합 예산 실패는 원본 source/redo/캐시 불변.
2. **소유권:** shared X1/Y2에서 X scaling 하나의 ID와 source만 존재, 서로 다른 Y scaling 유지. Legacy 실제 pair별 scaling 및 inactive fallback round-trip, promote/migration/remove/reorder가 label/tree를 해당 슬롯과 함께 이동. equal expression+different scaling dedup 금지.
3. **native source:** 0170 도움말의 원본 plotEquation에서 UNIT m, UNIT in, `Scale(0.1, UNIT m)` 두 Math fragment를 추출하여 source label/tree를 비교한다. unsupported viewport/형식이 있는 전체 원본 region은 Opaque/raw 보존을 검증한다. 별도로 해당 두 Math를 최소 supported plot에 넣은 **독립 조립 fixture**에서 strict reader→typed source→writer→XSD→reader를 검증한다. 이를 원본 전체 출력 표본으로 부르지 않는다. 실제 도움말 원문·독립 fixture·Mathcraft 생성 MCDX를 구분하고, source-same export의 raw reference/metadata 보존도 검사한다.
4. **label·suffix 반례:** UNIT m 대 VARIABLE m, 같은 철자 변수 정의 shadow, explicit/contextual flags, NativeIdentifier literal suffix를 저장·편집·재수입해 손실0. plain source serializer가 수용 못하는 첫 Math는 Opaque 유지. scaling PH clear와 incomplete 복합 PH는 fake1/0으로 계산하지 않는다.
5. **실제 입력:** 보조 equation scaling의 label 선택·CtrlMinus·Unicode/IME·Tab 양방향·Enter·focus exit·Escape·Save/F5·UndoRedo. 계산 전 source/curve/Pending/history 불변, commit1회, 다른 plot/삭제한 ID에 늦은 IME가 적용되지 않음. scaling 표시 units update만으로 draft/history/source 변경0.
6. **독립 선형 계산 단계:** 물리2 m에 대한 m/in/0.1 m 표시 좌표를 Quantity 계산으로 검증한다(각각2, 약78.74015748,20; help79는 반올림). 축 source sampling과 Xscale가 연결되는 경우도 확인하되 원본 미복구 xyplot2 일반식을 재현했다고 주장하지 않는다. 단위 호환/복소/0 등 미지원 반례는 해당 trace만 실패·unavailable로 남기고 다른 성공 curve ordinal 유지.
7. **cold/cache·성능:** own/native cold open math 실행0, raw unitful cache 미지원 fallback 유지, edited scaling 뒤 old Sync cache 재사용 금지, source-same native copy 보존. idle에서 per-frame MathML/JSON parse·PortableTree/Editor 전체 clone 없이 stable ID별 retained editor를 재사용한다. 16+16 fields와 긴 source, Pending previous curve 및 기존 finite tessellation을 CPU-only로 검증한다.

첫 구현을 source 모델·native round-trip·typed 입력·독립 계산·unit-bearing cache로 나누되, 지원하지 않는 계산을 source UI 뒤에 숨겨 완료 처리하지 않는다. 현재 제품의 unitless plain-expression 범위와 앞으로 지원할 범위를 인벤토리에 따로 적는다.


## Source 구현 체크포인트

위 본문은 설계와 검증 계획이다. 아래는 그중 document source 범위만 구현한 로컬 체크포인트이며 runtime/UI/단위 캐시 완료 기록이 아니다.

* `canvas::PlotScaling`은 private Box<PortableTree>와 validated transparent serde를 갖는다. `from_tree`의 단독 PH는 None, 명시1은 Some이며 Some의 null/PH carrier·중복·unknown 키를 거절한다. statements는 source factory에서 거절하고 지원 수식만 bridge로 왕복한다. labels/contextual/literal suffix를 plain parse로 바꾸지 않는다.
* AxisEquation.scaling, TraceSpec.x_scaling/y_scaling, Legacy fallback x_scaling/y_scaling이 추가됐고 None은 생략한다. promote/migrate/insert/remove가 scale을 해당 source ID와 함께 보존한다. Legacy fallback은 inactive 상태도 own JSON·signature에 보존한다.
* 공개 borrowed API는 `PlotEquationRef { expression, scaling }`, `PlotPairRef { x, y, label, style }`, `PlotSpec::equation_sources(x: bool)`, `paired_sources()`다. `set_axis_scaling(axis,index,value)`는 검증 후 원자적으로 설치하며 실패/같은 source는 원래 모델을 유지한다. `PlotScaling::is_complete`는 tree의 PH 존재를 확인하는 helper이며 원본 엔진 성공을 인증하지 않는다.
* 독립 자원 제한은 scaling 하나1024nodes, plot의 활성·비활성 scaling 합계8192nodes/128KiB(JSON source)다. 기존 PortableTree 깊이·atom 제한도 유지한다. native XML 전체의 기존256KiB/8192 XMLnode 제한은 별도이며 생성 범위 밖은 export가 거절한다. 이러한 수치는 원본 제한이 아니다.
* native 두 번째 Math는 typed scaling으로 읽고 그대로 쓰며 첫 Math의 기존 plain-expression guard는 유지한다. 동일 source 비교는 scaling tree까지 포함한다. unknown formatting/attrs/comment, 첫 Math의 지원 밖 label·syntax는 typed 최소 plot으로 축소하지 않는다.
* 새 source tests는 정상8개와 명시 실행 fixture1개다. old Trace의 네 필드를 고정한 독립 serializer와 hard-coded wire를 통해 None JSON·기존 SHA 호환을 확인했다. 공유 X의 borrowed scale pointer, Legacy migrate/remove, label/context/suffix, aggregate budget 실패, 명시1/PH, strict serde/native guards도 확인했다.

로컬 document 전체 **219 PASS/7 ignored**, all-target Clippy 및 소유 Rust rustfmt 검사를 통과했다(동시 작성된 unit/v3 모듈의 검사가 포함된 당시 체크포인트). 최종 Windows/Linux·앱·MCDX/PDF 통합 수치는 별도 통합 문서에서 기록해야 한다.

[검증 스크립트](../../artifacts/rea/validate-xy-scaling-source-20261019.py)와 [결과](../../artifacts/rea/xy-scaling-source-fixtures-20261019/validation.json)는 생성한 최소 source-only XML3개가 offline worksheet XSD를 통과하고, 설치 도움말 원문 SHA 및 보관한 원본 region SHA가 일치하며, 각 두 번째 Math의 구조·label이 실제 help fragment와 같음을 기록한다. XSD는 기존 offline schema bundle을 사용했다. 이 XML은 Mathcraft가 만든 독립 조립 fixture이고 원본 앱 출력이나 unit-bearing 계산 캐시 표본이 아니다. 원본 전체 도움말 plot의 지원 밖 viewport/형식을 없애 typed import했다는 주장은 없다.

현재 후속 구현·검증은 [0176](0176-xy-physical-units-implementation.md)과 [0178 r180](0178-xy-units-integration.md)에 기록한다. 이 문서의 당시 설계/미구현 진술과 최신 지원 범위를 구분한다.
