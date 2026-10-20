# XY 사용자 축 한계·표시 범위·계산 확정

2026-10-19. 설치 Mathcad Prime12.0.0.1의 REA 관리 metadata와 실제 PE 본문을 정적으로 검증했다. GUI·스크린샷·원본 실행·CLR은 사용하지 않았다. 제품 코드는 변경하지 않았다. [0146](0146-native-xy-plot-export-contract.md)의 source-only plot 계약을 잇는다.

## 서로 다른 두 범위

**원본의 x축 사용자 한계는 단순 viewport 설정이 아니다.** TraceModel.GetParameterRange1282가 사용자 limit의 계산값을 읽고, get_Rl1281이 그 하한·상한을 실제 `xyplot2` 엔진 요청에 전달한다. 현재 PlotSpec.axis.range처럼 표시만 바꾸고 quickplot sampling을−10..10으로 유지하는 값을 원본 사용자 domain으로 저장하면 계산 의미가 달라진다.

| 원본 상태 | native 저장 | 계산 영향 |
| --- | --- | --- |
| 사용자 limit 식 | xyDomain의 startValue/endValue 안 직접 MathML expression | 유효한 x/range-axis limit 계산값은 trace parameter range로 전달된다. |
| 기본 limit | startValue/endValue 안 ml:placeholder | GetParameterRange는 StartDefault/EndDefault를 사용한다. 선형 기본값은0146의−10/10이다. |
| 현재 표시 범위 | xAxis/yAxis의 start/end 속성 | domain 식과 별도로 저장·복원한다. TraceModel.GetParameterRange는 이 속성의 backing Start/End를 읽지 않는다. |

따라서 기존 display-only range를 보존하려면 domain을 placeholder로 유지하고 axis start/end 속성에 저장하는 경로를 구분할 수 있다. 이를 원본의 사용자 한계 입력 기능이라고 부르지 않는다. 원본 사용자 한계를 구현하려면 sampling에 쓰는 limit과 viewport 상태를 별도 모델로 표현해야 한다.

## 엔진 입력에 전달되는 정확한 값

`TraceModel.GetParameterRange 0x06001282`는 RangeAxisModel을 읽는다. 일반 XY에서는 StartExpression/EndExpression을 각각 `GetXAxisLimitEditorValue1283`에 넘기며, 그 editor가 존재하고 IsValueValid이면 CalculatedValue를, 아니면 해당 axis StartDefault/EndDefault를 선택한다. angular axis는 별도 고정 interval 분기이므로 이 문서의 XY 양성에 섞지 않는다.

`AxisLimitEditor.IsValueValid002d`는 IsDefaultValue가 false이고 CalculatedValue가 NaN이 아닌지 확인한다. 이 getter 자체는 Infinity를 거르는 코드가 아니므로 독립 Rust 구현의 finite-only 제한과 원본 전체 오류 처리를 구분해야 한다. 잘못된 limit 오류가 어떤 경우에 trace fallback/default와 동시에 표시되는지 전체 error policy는 미확정이다.

`TraceModel.get_Rl1281`은 x/y 식 RL과 scaling RL을 읽고 GetParameterRange 결과를 사용한다. 실제 요청은 xyplot2에 x/y 식, 두 scaling, x scaling이 적용된 Lower/Upper, **NumberOfPoints−1**을 전달하는 eval/numeric 구조다. 이 메서드에 y-axis limit의 Lower/Upper를 같은 sampling 인자로 전달하는 경로는 없다.500점 model이면 엔진 인자는499이며, 이것만으로 엔진의 보간·끝점 배치·discontinuity 규칙 전체를 확정하지 않는다. defined range/vector가 있는 축에서 이 parameter interval이 언제 무시되거나 결합되는지는 native engine 내부 추가 범위다.

`AxisModelRangeChanged1229`는 density image DataRange와 plotting-point filter를 갱신한다. 이것을 곧바로 새 trace 계산 요청이라고 해석하지 않는다. `ResultAxisLimitValueChanged122a`는 SetAnalyzedData, `RangeAxisLimitValueChanged122f`는 Commit을 호출하는 메서드지만 후자의 실제 이벤트 구독 위치는 이번 retained call graph에서 확인하지 못했다. 메서드 이름만 보고 한계 입력 매 키마다 재계산된다고 주장하지 않는다. 실제 RL에 사용자 x limit 값이 들어간다는 양성과 전체 scheduling 연결의 미확정을 구분한다.

## Domain XML의 wrapper와 cache

AxisModel.GetXmlNode1068의 nondefault limit은 `AxisLimitEditor.RegionXmlNode`를 해당 startValue/endValue에 ImportNode한다. `AxisLimitEditor.get_RegionXmlNode0035`는 내부 EquationControl.GetRegionXmlNode의 **FirstChild**를 반환한다. 따라서 limit은 `<startValue><ml:real>1</ml:real></startValue>` 같은 직접 MathML이며 `<startValue><math>...</math></startValue>`가 아니다. 기본 limit은 직접 ml:placeholder다. secondTickValue는 기본이 아닐 때만 추가한다.

reader AxisModel.SetXmlNode1065도 각 value wrapper의 FirstChild를 editor.RegionXmlNode에 전달한다. AxisLimitEditor setter0036은 식을 복원한 뒤 LocalName이 placeholder인지로 IsDefaultValue를 정한다. ResultManager.UpdateResultRef는 value wrapper에서 optional 참조를 복원한다.0146의 resultRef 부재 허용은 여기에도 적용되며 source-only 저장에 가짜 계산값을 넣을 필요는 없다.

writer1068은 xyDomain auto-scale을 **항상 true**로 기록한다. 사용자 limit이 있다고 false를 자동으로 쓰는 근거는 없다. start/end axis 속성은 domain과 별도로 현재 표시값을 InvariantCulture로 쓴다. reader는 두 속성 각각의 존재를 검사하여 Start/End를 설정하므로 없으면 model 기본값을 유지할 수 있다.

독립 조립 fixture `artifacts/rea/native-xy-domain-derived-20261019.xml`은 x 사용자 limit1..3과 표시 범위0..5를 동시에 기록하고 y limit은 기본 PH로 둔다. 기존 로컬 worksheet50 schema 검사에 통과했다. 원본이 저장한 표본이나 원본 실행 검증이 아니며 source 해시·검증 record를 동명 JSON에 보존했다. limit 계산 이후 RL parameter가1..3을 선택한다는 판단은 위 메서드에 근거한다.

## 축 식 입력과 commit

원본 plot 식은 EquationControl 기반이므로 단순 SourceTextChanged와 계산 확정을 분리한다.

* PlotEquationBase.OnSubEquationExpressionChanged02ea는 ResetIsCommitted와 FireExpressionValueChanged를 수행한다. PlotEquation030f는 scaling 변경 상태·Undo 부수 상태를 처리한 뒤 base를 호출하며 이 경로에 trace 계산 요청은 없다.
* TraceModel.AttachAxisEquationEventHandlers1241은 plot equation RequestEvaluation에 EquationControlRequestEvaluation1238을 연결한다.1238은 PlotCalculatable.Commit을 호출한다. SExpressionValueChanged는 별도 OnTraceEquationSExpressionChanged에 연결되어 오류·committed 상태를 갱신한다.
* PlotEquation.OnRequestEvaluation0326은 Commit0357/0358로 간다. 기본 Commit은 먼저 FireRequestEvaluation035a를 수행하고, expression/scaling 내부 EquationControl을 이벤트 재진입 없이 commit한 뒤 IsCommitted=true로 만든다.
* PlotControl.Commit01f3은 InternalCalculatables를 commit하고 두 equation list를 Commit(false)로 확정한다. list 확정에서 별도 RequestEvaluation을 중복 발생시키지 않으려는 경로가 명시돼 있다.
* AxisLimitEditor.EquationChanged0058도 default flag·오류 상태를 바꾸며 직접 RequestEvaluation을 보내지 않는다. EqRequestEvaluation004c→Commit004d는 내부 수식을 이벤트 재진입 없이 commit하고 nondefault 계산값을 reset한 뒤 RequestEvaluation을 보낸다.

이 양성은 편집 중 조판/dirty 상태와 commit/계산 요청을 구분해야 한다는 [0042](0042-edit-commit-contract.md)의 일반 계약과 일치한다. Enter/focus가 모든 plot 하위 요소에 전달되는 최종 routed-command 우선순위, 미완성 식, scaling field 전환과 IME 전체는 이번 범위에서 완전히 추적하지 않았다. 식 내부 Enter 소비를 무시하여 전역 commit으로 바꾸는 근거도 아니다.

## 증거 식별자와 구현 판단

새 REA `ev_a2e2677ffd999b9a2f87b8d40e2a6c57e917b31751a5997a3744b4a282a6e468`는 AxisLimitEditor/PlotCalculatable을 확인했다. 기존0146의 AxisModel/TraceModel/PlotEquation/PlotControl evidence를 재사용했다. Plots.dll SHA=`4e5067e58e69c68e8152ce677d5ac77a6587e00f6295bda5cc8ddff18f6685eb`, MVID=`cecd333a-bf3e-4423-b74b-7ff799618db1`이다. ignored `native-xy-domain-method-ledger-20261019.json`과 `feedback/native-xy-domain-*-cil-20261019.txt`에 token/본문 SHA를 보존했다.

핵심 IL SHA:1281=`411108d88d7fd759568623dcb5663c5a5644c6edae6fe3492f846219ddbc60f2`,1282=`401baba13838e7a6795eee0ab93923acbd5fe08ed65cb4082d4b03bcdad6f68e`,0035=`af27236505559ea7f75a586c87d4f7f1754e0f0a066efd23f45d5d902047d27a`,0358=`863e0edaecb5d9346af6c0993c64062c1fbb779f83757bf56e902ccad4f8551b`.

당장 native_plot의 사용자 range unsupported guard를 단순히 제거해서는 안 된다. 표시 range 보존만 추가한다면 axis start/end로 한정하고 domain PH·샘플링을 유지한다. 원본 사용자 limit 작성까지 지원하려면 별도 source 표현과 sampling 연결·commit/결과 유지·원본 오류 경계를 함께 구현하고 검사해야 한다.
