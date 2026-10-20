# 새 빈 XY Plot과 trace별 부분 결과 계약

2026-10-19. 설치 Prime12.0.0.1 REA metadata/CIL, shipped help, schema를 정적으로 조사했다. GUI·스크린샷·원본 engine 함수·runtime·CLR은 실행하지 않았고 제품 Rust 파일은 수정하지 않았다. [0157](0157-native-xy-slot-authoring-contract.md)의 슬롯/trace identity, [0161](0161-native-xy-expression-layout-contract.md)의 배치, [0148](0148-native-xy-result-contract.md)의 trace cache codec을 잇는다.

## New XY Plot는 샘플 수식을 주입하지 않는다

기존0023의 ribbon BAML은 WorksheetCommands.NewPlot/Ctrl+2를 확인했다. 이번에는 실제 WorksheetViewModel.SetupCommandBindings callback0b6e를 이어 확인했다. callback은 parameter를 XyEquation으로 cast하여 CreateXYPlotControl0a9c에 전달한다. 명시 XyEquation parameter가 있는 경로와 일반 parameter 없는 새 plot을 구분한다.

일반 null parameter에서는0a9c →0a20 →0a1f 경로다. 0a20은 **String.Empty/String.Empty**를 넘기고,0a1f는 control factory로 새 XYPlotControl을 만든 뒤 CompoundCommand/CommandScope 안에서 InitializePlotControl0a9a를 실행한다. Initialize는 빈 두 문자열의 XyEquation을 만들고 PlotControl.AddEquations028a로 전달한다. AddEquations는 range(X)축, primary result(Y)축에 각각 한 식을 추가한다.

EquationList.AddItem(string)00ba는 슬롯을 만든 다음 **String.IsNullOrEmpty가 false일 때만** 해당 식의 S-expression을 설정한다. 따라서 새 식 컨트롤의 기본 placeholder를 남긴다. native 새로운 plot에 `x`/`sin(x)` 또는0을 기본 source로 넣는 단계는 이 경로에 없다. XyEquation 기본 ctor06de도 빈 두 문자열을 보관한다. 명시 parameter에 실제 식이 들어온 경우에는 그것을 보존하는 별도 경로이므로 모든 NewPlot 호출을 무조건 PH로 덮어쓰지는 않는다.

기존0157의 presenter 연결에 따라 X/Y 각각1slot이 연결되면 trace model1개가 생긴다. 이 trace는 source association/style을 가진 작성 객체이며 계산된 sin curve가 아니다. source 슬롯, trace identity, 실제 points/result 유무를 분리해야 한다.

shipped `about_XY_plots.html` SHA=`224c27dfa28fd38ccf07fa0baa6e87b27dc58c0ca21aaa6aafa85f79b87fd165`는 empty plot을 삽입한 뒤 식을 정한다고 설명한다. `to_plot_functions.html` SHA=`ca0e754ddbc7b464f6ae6cdb81e64bdf490399314b61b4b0306677cbee6b0b2d`는 empty XY region, **Y placeholder 오른쪽/X placeholder 아래**, Enter 후 line trace 표시를 명시한다. 이는0161의 fresh constructor X0/Y1 배치를 지지하며 기존 about_traces의 defaultLeft 문구와 차이를 문서화한다.

factory0078의 최초 square size는2.4inch→DIP이지만 WorksheetViewModel.InitializePlotControl0a9a는 Width=PageBodyWidth/2, Height=Width로 덮어쓴다. 최초 factory 값만 원본의 최종 region 크기라고 고정하지 않는다. 최종 WPF measure와 native graph-size/outer region 분리는0161을 따른다.

## 빈 식의 자동 연결과 명시 Enter는 다르다

TraceModel.ContainsEmptyEquation1232는 x 또는 y의 IsPlotEquationEmpty를 확인한다. IsPlotEquationEmpty 자체는 expression과 scaling이 모두 비어 있어야 한다(0157). expression PH/scaling nonempty를 완성된 일반 식으로 계산한다는 뜻은 아니며, 여기의 empty predicate 범위만 확정한 것이다.

SetEquation123f는 식을 연결하고 IsCommitted=false로 바꾼 뒤 synchronization enum3인 경우 CommitIfEquationsNotEmpty1243을 호출한다. 1243은 ContainsEmptyEquation=true이면 commit하지 않는다. 따라서 새 PH 슬롯/association 변경의 이 조건부 자동 경로에서 가짜 기본 식으로 curve를 만들지 않는다.

**하지만 명시 Enter를 무조건 PH-skip이라고 구현할 근거는 없다.** PlotControl.Commit01f3은 InternalCalculatables iterator14f3의 항목 각각에 ICalculatable.Commit을 호출한다. iterator는 TraceModels를 empty filter 없이 전부 yield하고 axis limits/markers도 별도로 yield한다. TraceModels0274는 model traces를 visual order로 OrderBy할 뿐 PH를 제거하지 않는다. trace.Commit1289는 FireRequestEvaluation을 호출한다. 따라서 명시 plot commit의 요청 단계에는 빈 trace도 참여한다.

PH를 포함한 RL을 native engine가 어떤 result/error 코드로 반환하는지, blank Enter가 no-op으로 최적화되는 하위 engine 조건, PH source에 이전 cached curve가 있을 때의 전체 dispatch 순서는 이번 조사에서 미확정이다. 조건부1243을 전체 Enter 계약으로 확대하지 않으며, 반대로 PH를 성공한 빈 point 배열이나0 값으로 위조하지 않는다.

## 결과와 오류의 기본 소유자는 trace다

TraceModel1279 OnPlotCalculatableError는 base error event를 전달하고 해당 trace의 number-of-points mutable을false로 바꾼다. EngineError가 있으면 **this.ClearPoints128f**를 호출한다. equation-specific error도 PreprocessEquationSpecificError128d에서 this.ClearPoints를 호출한다. ClearPoints는 자신의 NumberOfPointsInResult=0, 자신의 analyzed data defaults, 자신의 새 빈 PointCollection 및 SetPlottingPoints를 설정한다. 다른 trace의 points collection이나 plot 전체 source 목록을 순회해 지우지 않는다.

PlotCalculatable.SetError1109는 이 calculatable의 ErrorSource/NodeHandle을 처리한다. matching child가 있으면 mapper로 NodeHandle을 매핑하고 해당 child에 SetError를 전달한 뒤 equation-specific 경로로 간다. 없으면 해당 calculatable의 OnPlotCalculatableError를 호출한다. TraceModel128e는 자신의 EquationError event를 발생시킨다. shared 식에 error 표시가 전달될 수 있다는 사실과 독립된 모든 curve 결과를 삭제하는 동작은 다르다.

성공 SetResult128b/DoSetResult128c도 자신의 points를 초기화하고 결과를 분석한다. EmptyResult, MatrixResult, Trace2DResult를 분기하며 성공한 분석에서 자신의 SetAnalyzedData129b→own SetPlottingPoints→ResultValid=true를 실행한다. 이 구조는 하나의 plot-wide Result 하나에 모든 trace를 넣었다가 첫 오류로 전체를 Err 처리하는 모델과 다르다.

TracePresenter ctor0abf는 정확한 한 model/view 쌍을 보관하고 그 model의 PlottingPointsChanged/PlotCalculatableError/EquationError/ResultValidChanged 등에 연결된다. ModelPlottingPointsChanged0ac9는 그 model.GetPlottingPointsForDisplay를 그 view.PlottingPoints로 전달한다. ModelResultValidChanged0ac2/ModelEquationError0ac7도 같은 view의 상태를 갱신한다. PlotTrace.DoOnRender0684는 자신의 renderer와 points를 사용하며 sibling trace failure를 조건으로 전체 렌더링을 생략하는 branch가 없다.

이 양성 범위에서 **다른 유효 trace의 결과를 한 trace 오류 때문에 전부 폐기하는 정책은 원본 소유권과 다르다.** 독립 Y 식 하나가 오류인 경우에도 성공한 trace의 source/style/points를 유지할 수 있는 상태 모델이 필요하다. 단일 shared X가 실제로 오류라면 그 X를 사용하는 여러 trace가 각자 실패할 수 있다. shared identity를 독립된 잘못된 문자열이나 sample fallback으로 바꾸지는 않는다.

axis limit/공통 scaling 오류, incompatible unit/style, plot type 전체 invalid 상태는 여러 trace에 영향을 줄 수 있다. 원본 전체 오류 domain을 여기서 한 trace의 일반 계산 실패와 같다고 단정하지 않는다. SetCommonRange0aa1도 관련 x 또는 y axis를 공유하는 trace들을 모아 axis range를 갱신하므로 서로 완전히 독립된 viewport라는 뜻이 아니다. 부분 실패 후 정확한 auto-range/opacity/error 색/곡선 표시 프레임 타이밍은 추가 범위다.

## source 변경 직후와 결과 도착 시점

0157 도움말은 축 식 삭제 뒤에도 Enter로 평가하기 전에는 trace가 사라지지 않는다고 설명한다. 이 조사에서도 source association의 SetIsCommitted(false), 계산 요청, SetError/SetResult, view points 전달을 서로 다른 단계로 확인했다. 따라서 source edit 시점에 곧바로 모든 이전 curves를 지우거나 성공한 sibling의 cache까지 버리는 것과, 해당 trace에 fresh error가 도착해 그 trace를 clear하는 것은 구분해야 한다.

불완전 PH 슬롯을 추가한 순간 기존 유효 trace는 같은 identity/points 소유권을 보존한다. 그러나 새 trace가 이전 다른 source의 curve를 재사용하거나, PH가 Synchronized 성공값이라고 표시하는 계약은 없다. 미완성 source와 계산 결과의 상태/순서를 보존하는 per-trace outcome 배열이 현재 앱의 whole-plot Result보다 적절하다. 구현 API의 이름/serialization은 독립 설계이며 원본에 없는 계산 error code를 새로 만들어 native error처럼 쓰지 않는다.

## native source/cache의 좁은 계약

원본 PlotEquation writer033d의 source는 expression용 math와 scaling용 math 두 개다(0146). 빈 expression은 MathML placeholder라는 source 구조를 유지해야 하며 trace나 math row를 생략해 뒤 ordinal을 당기지 않는다. source-only plot은 resultRef 없이도 지원되는 원본 shape다. trace의 resultRef는 자신의 result manager를 통해 자신의 resultData와 연결되고, source graph/trace style 배열의 ordinal·identity를 보존해야 한다(0148/0157).

TraceModel.SetError128a는 nonnull error일 때 자신의 nullable `_engineResultForSerialization` override를 지운 뒤 base.SetError를 실행한다. getter12a8은 override가 있으면 그것을, 없으면 자신의 current EngineResult를 선택한다. ShouldNotCopyFullResults에 따라 min/max/units만 복사하는 별도 branch도 있다. **이 nullable override clear를 native 오류 결과 전체를 저장하지 않는 정책으로 해석하지 않는다.** 정확한 error payload/상태는 공통 result serializer/manager 계약을 따라야 한다.

현재 bounded finite Trace2dResult codec은 nonempty finite points만 인증한다. native EmptyResult/빈 TraceData의 serializer shape와 XSD 차이는0148의 명시 한계다. PH/error trace에 가짜 finite cache를 쓰거나 유효 trace의 row를 재번호하여 모두 compact하는 근거가 아니다. native 오류 cache의 정확한 메시지/NodeHandle/동기상태를 이 분석에서 새로 조립하지 않았다. unknown 원본 payload/metadata는 보존해야 한다.

## 추가: 목록 위치의 native XML 필드

위치 source는 plot root의 show-axis-expressions/disable-calc와 별개다. AxisModel.GetXmlNode1068은 `ws:xAxis/@legend-position`, `ws:yAxis/@legend-position`에 enum.ToString을 쓴다. axisLine의 `position`은 **축 선 자체의 위치**로 다른 속성이다. `axisLine/@legendWidth`는 목록의 표시 폭이며 InvariantCulture double로 저장/복원한다.

| Index | xAxis legend-position | yAxis legend-position |
| --- | --- | --- |
|0|PlotBoundaryBottom|PlotBoundaryLeft|
|1|PlotBoundaryTop|PlotBoundaryRight|
|2|AxisAttachedTopRight|AxisAttachedTopLeft|
|3|AxisAttachedBottomRight|AxisAttachedTopRight|
|4|AxisAttachedTopLeft|AxisAttachedBottomLeft|
|5|AxisAttachedBottomLeft|AxisAttachedBottomRight|

ConvertLegendPositionEnumToLegendPositionIndex1069는 축 종류에 맞는 enum을 Enum.Parse하므로 대소문자/알 수 없는 값의 임의 canonicalization을 추가하지 않는다. raw SetXmlNode1065는 속성이 있을 때만 index를 변경하며 absent이면 현재 값을 유지한다. fresh PlotControl ctor의 X0/Y1은 presenter0a85/0a86을 통해 model에도 반영된다.

그러나 **source absent의 schema 기본값**은 worksheet50.xsd1282/1305 및2050/2073의 X=PlotBoundaryBottom/Y=PlotBoundaryLeft다. XSD-default augmentation이 정상 typed deserializer의 InnerXml에 언제 주입되는지까지 이번 조사에서 확인하지 않았다. 따라서 raw setter absent→current Y1이라는 도출을 전체 native open의 absent 결과라고 단정하지 않는다. schema가 없는 standalone AxisModel 초기값0, fresh control Y1, 명시 native Y0, schema-default absent를 구분한다. 새 빈 plot의 원본 fresh 배치를 저장하려면 명시 Y=PlotBoundaryRight를 보존하는 표현이 필요하며 기존 absent source를 오른쪽으로 조용히 고쳐 쓰지 않는다. 사용자에게 원본보다 새로운 layout 기능을 발명한다는 뜻은 아니다.

## 근거·미확정·검증 범위

새 REA plot init/presenter=`ev_34a64e22cd2ceebfef1cc9c7d926fcd967fa78df20b1f0b0e39af23ce6fc5793`, view=`ev_e36db5f42c15b1583574f32a8c509cf659cf7b13f66f3de370b73cf4c6d325f8`, internal calculatables iterator=`ev_fd81ce53349cee1fbd00b1309b1c93194c38a6b639606aec7428253675e600e8`, range predicate=`ev_bcb6de17a2080a48ebe1701e67ad17e3f198ff14d6c8aac40df6ae0960c51154`.

Plots.dll SHA=`4e5067e58e69c68e8152ce677d5ac77a6587e00f6295bda5cc8ddff18f6685eb`, MVID=`cecd333a-bf3e-4423-b74b-7ff799618db1`. Worksheet/EquationControl assembly identities와 선택 raw CIL SHA는 ignored `native-xy-empty-method-ledger-20261019.json`에 설치 bytes와 재대조하여 기록했다. help/enum/schema/source 및 resource identity도 같은 prefix에 별도 보존했다. artifact 중 exploratory placeholder worksheet0a5d는 이번 계약 근거로 사용하지 않았다.

미확정은 native engine의 PH 결과 코드/메시지, 모든 axis-global 오류의 영향 범위, 원본 error cache의 완전한 serialization/복원, async UI 프레임별 curve 상태, normal deserializer의 XSD default augmentation 경로다. 다음 구현은 blank source를 사용자 sample과 분리하고 trace별 source identity/ordinal 및 성공·빈작성·오류 상태를 보존하는 범위이며, 테스트 개수로 이 미확정을 완료 처리하지 않는다.
