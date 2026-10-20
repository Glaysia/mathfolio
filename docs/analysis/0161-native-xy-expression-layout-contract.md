# XY 축 식 목록의 배치·포커스·크기 계약

2026-10-19. Prime12.0.0.1 설치 CIL/REA metadata와 en-US satellite BAML을 정적으로 조사했다. GUI·스크린샷·원본 runtime·CLR은 실행하지 않았다. 제품 코드나 원본 리소스를 제품에 복제하지 않았다. ignored artifact의 리소스 추출본은 구조·해시 확인용이다. [0157](0157-native-xy-slot-authoring-contract.md)의 슬롯 source/trace 계약과 [0149](0149-native-xy-domain-and-commit-contract.md)의 계산 commit 계약을 잇는다.

## 실제 목록은 독립적인 table layout이다

EquationList의 BAML은 EquationListContents 이름을 EquationListGrid로 지정하고 Columns=1, ColumnAlignment=LeftAlign을 설정한다. EquationListContents ctor00ef도 Columns=1, ColumnSpacing=8.0, RowSpacing=1.0, ColumnAlignment enum0을 설정한다. 따라서 축 source 목록 하나를 항상 한 줄짜리 문자열로 합치는 원본 근거는 없다. 기본1column은 식 슬롯들을 세로로 배치하는 table이며, 목록 폭에 따라 다중 column으로 재배치하는 경로가 있다.

EquationList.WidthListChanged00ad는 MaxListWidth>0일 때 EquationListContents.AdjustNumberOfColumnsToWidth00f0을 호출하여 Columns를 갱신한다. LayoutManager.GetNumberOfColumnsFitWidth00ee는 현재 measure 정보를 clone하고 columns를 늘리거나 줄여 MatrixSize.Width가 요청 폭에 들어오는 열 수를 찾는다. columns는 최소1이고 cell count 이상으로 계속 증가하지 않는다. 폭·식 측정값에 따른 table 결과이며 고정 column count나 슬롯 개수만의 규칙이 아니다. 일반적인 다중 column table의 모든 cell row/column index 및 decimal alignment 세부 알고리즘은 base TableLayoutManager 전체를 이번 범위에서 해석하지 않았다.

목록 외곽 EquationList BAML margin은4,0,4,0이고 EquationListItem margin은3이다. 이 값들과 ColumnSpacing8/RowSpacing1은 다른 레벨의 spacing이므로 하나의 고정 line-height로 합산하지 않는다. BAML 원시 숫자 property ID 중 -49는 FrameworkElement.Margin이다. 실제 font/math desired size와 trace indicator 측정이 최종 행 높이에 참여한다.

각 PlotEquation 내부는 별도의 EquationListContents Columns=2, ColumnSpacing=0이며 `_exp` EquationControl과 `_scaling` ScaledEquationControl을 갖는다. expression은 AllowExpressionsOnly=true이고 scaling은 native units/scaling 칸이다. 이2column은 **축 식 슬롯 목록의 columns**와 다르다. 식마다 scaling 칸을 없애고 공통 단위 문자열 하나로 대체하는 구조가 아니다. scaling BAML 초기 Opacity=0/visibility bindings의 모든 focus/error 전이를 이번 조사에서 확정하지 않았으므로 항상 숨김이라고 단정하지 않는다.

## Graph와 축 목록 placeholder

PlotControl BAML 자체는 PlotDictionary.xaml을 참조한다. 해당 dictionary의 XY template에는7 column/10 row grid가 있고, 열3/4와 행3/4가 각각0.5*이며 나머지는 Auto다. 다음은 BAML의 Grid.Column/Row/ColumnSpan/RowSpan을 해석한 구조다. 기본 WPF known property ID는 -61/-63/-62/-64 순서다.

| Template part | Column | Row | ColumnSpan | RowSpan | 의미 |
| --- | --- | --- | --- | --- | --- |
| PART_PlotGraph |3|3|2|2|중앙 graph |
| PART_X0 |3|6|2|기본1|graph 아래 X 식 위치0 |
| PART_X1 |3|1|2|기본1|graph 위 X 식 위치1 |
| PART_Y0 |1|1|기본1|6|왼쪽 Y 식 위치0 |
| PART_SharedY1 |6|1|기본1|6|오른쪽 Y 식 위치1 |

X2..5/Y2..5는 graph 관련 위치와 margin converter를 추가로 갖는다. 이번 문서는 그 이동·overlap 모드를 기본 위치와 섞어 새 UI를 발명하지 않는다. XYPlotStrategy.InitializeLegendPositions06e6은 이 part들을 각각 index 배열에 넣고, PlotControl.OnApplyTemplate0217은 저장된 position index의 placeholder에 실제 목록을 연결한다.

**fresh constructor와 loaded native source를 구분해야 한다.** XYPlotStrategy.GetInitialPlaceholderIndices06ea는 x=0,y=1,title=1,plotLegend=0을 반환한다. PlotControl ctor01b0은 이를 set_XLegendPositionIndex/set_YLegendPositionIndex에 전달한다. set_YLegendPositionIndex025a는 새 part를 PlotItem.Move05ae의 첫 인자로 넘기고, Move는 새 part.Content=this, 이전 part.Content=null로 이동한다. 따라서 이 설치 코드의 fresh constructor 정적 경로는 X0/아래와 Y1/오른쪽이다.

반면 native deserialization finished0a8c는 model.rangeAxis/PrimaryResultAxis.LegendPositionIndex를 다시 view에 적용한다. native position0인 Y 목록은Y0/왼쪽이다. 기존 설치 도움말의 default Y legend Left 문구([0151](0151-native-xy-axis-pairing-contract.md)에 hash 보존)와 fresh constructor Y1은 일치하지 않는다. 이 상충을 숨기고 모든 원본 문서의 default가 오른쪽 또는 왼쪽이라고 일반화하지 않는다. 실제 source에 저장된 index와 fresh 생성 경로를 분리하고, 사용자 실행을 통해 배치를 확인했다는 주장도 하지 않는다. RTL/기타 theme override에 의한 전체 runtime 최종 위치는 이번 범위에서 부재 증명하지 않았다.

## 슬롯 추가와 outer region/graph 크기

식 추가 후 EquationList가 item을 Insert하고 SetMinMaxWidth를 갱신하는 사실은0157을 따른다. SetMinMaxWidth00c7은 실제 item desired/actual width와 ItemMargin으로 MaxListWidth, 목록 MinWidth/MaxWidth를 관리한다. ArrangeEquationsHorizontally00c6은 별도 helper로 실제 식 폭 합계와 margin을 사용한다. 이 helper의 존재만으로 일반 새 XY 목록을 항상 가로1행으로 배치한다고 판단하지 않는다.

PlotControl.XEquationsSizeChanged01d0와 YEquationsSizeChanged01d1은 공통 EquationListOrPlotLegendSizeChanged01d3을 호출한다. 이 handler는 X/plot legend overflow와 Y 위치0/1 overflow를 확인한다. MakeSureXEquationsNotOutsidePlot01d5는 X 위치0/1에서 실제 grid column 폭과 목록 폭+margin+placeholder padding을 비교한다. 식 목록 자체 크기 변경 branch에서는 overflow만큼 outer Width를 늘린다. plot resize branch에서는 좌우 목록 폭을 줄이는 별도 정책과 `_xEquationSizeChangingDueToPlotSizeChanged` 상태가 있다.

MakeSureYEquationsNotOutsidePlot01d8은 loaded grid의 해당 placeholder 행들의 실제 높이 합계와 목록 ActualHeight+margin+padding을 비교한다. 양의 overflow가 있고 해당 guard를 만족하면 outer Height에 overflow를 더한다. 기본 side 위치의 여러 식은 우선 세로공간 안에 배치되고, 그 공간보다 커지면 plot 외곽 높이를 보정하는 경로다. **Y 슬롯 하나당 고정 n픽셀을 더하는 코드가 아니다.**

GraphSizeChanged01d9는 graph의 실제 old/new delta, `_plotSizeChanging`, `_resizeInProgress`, 식 visibility 변경 상태에 따라 outer Width/Height를 보정하고 model.GraphSize를 갱신한다. CorrectHeightIfXEquationSizeIsChanged01db와 KeepGraphMinSize01dc도 참여한다. 따라서 source graph-size와 outer region 크기는 서로 다른 값이며, 슬롯 추가 때 graph까지 항상 확대하거나 항상 그대로 둔다는 단일 등식은 이번 CIL만으로 확정하지 않는다. async WPF measure/event 순서 전체와 주어진 font/긴 식의 최종 픽셀 높이는 미확정이다.

Graph minimum의 기본 항은 GridCellSize.Width/Height의4배다(02b1/02b2). 보이는 legend/title/axis-title의 minimum이 추가 max 제약에 참여할 수 있고, XY strategy06e9는 여러 placeholder minimum을 합쳐 outer MinWidth/MinHeight를 계산한다. Mathcraft의 고정 graph panel 크기를 이 원본의 모든 source/label 상태에 대한 정확한 최소치라고 부르지 않는다.

## Tab/Shift+Tab: expression과 scaling이 독립 포커스 대상

EquationCommands Traverse/BackwardsTraverse의 Tab/ShiftTab binding은0151을 재사용한다. PlotControlBase.BindCommands0ad8은 이 명령을 OnTraverseForward/Backward0b62/0b63에 실제 연결한다. 일반 active plot equation의 경로는 다음과 같다.

1. PlotControl.BuildEquationList01c2는 **result(Y) 목록 먼저, range(X) 목록 다음**으로 AddEquationsFromList0b6d를 호출한다.
2. 0b6d는 각 source index마다 expression을 추가하고, WOUnit 타입이 아닌 경우 scaling을 그 뒤에 추가한다.
3. GetNextTraversalItem0b69는 ActivePlotEquation.ActiveEquation==ActiveEquation인 경우 이 목록을 선택하고 wrap=false로0b6a에 넘긴다.
4. forward는+1, backward는−1이며 다음 항목이 없으면 CanExecute=false다. 내부에서 첫 항목으로 강제순환하지 않는다.
5. OnTraverse0b6c는 대상 ActivateCommand를 실행하고 방향을 담은 TraversedFocusEvent를 보낸다.

일반 XY 식의 유효한 내부 순서는 `Y0.expr → Y0.scaling → Y1.expr → Y1.scaling → … → X0.expr → X0.scaling → …`이고 ShiftTab은 역순이다. source 식 목록 index 순서로 구성되며 화면을 가로·세로로 재배치했다고 다른 계산 순서나 식 identity로 바뀌지 않는다. 축 tick/marker traversal은 다른 목록과 wrap=true를 사용하는 분기이므로 이 식 슬롯 동작과 섞지 않는다.

PlotEquation.OnTraversedFocusEvent0311은 forward에서 NavigateRegionStart0361, backward에서 NavigateRegionEnd0360을 호출한다. start는 expression을 focus하고 시작으로, end는 scaling을 focus하고 끝으로 이동한다. 이것을 모든 자식 scaling Tab 진입 때 expression으로 강제 되돌리는 handler로 오독하지 않는다. CommonRoutedEvents cctor019c IL002e–0048은 TraversedFocus를 RoutingStrategy enum2(**Direct**)로 등록한다. OnTraverse가 선택한 expression/scaling 컨트롤에 발생시킨 이벤트가 PlotEquation 부모에 bubble하는 경로가 아니다. PlotControl 자체로 밖에서 진입하는 OnTraversedFocusEvent01c5는 forward의 첫Y 식, backward의 마지막X 식을 선택해 포커스를 위임하고 해당 PlotEquation에 직접 이벤트를 발생시킨다. 식 내부의 정확한 native caret 선택 폭/IME selection까지 이 정적 목록 순서만으로 확정하지 않는다.

목록 처음/끝에서의 CanExecute=false 이후 worksheet 전체 Tab fallback 및 plot 바깥 선택·border focus 순서는 이번 조사에서 미확정이다. Tab을 새 source 슬롯 추가나 Enter commit으로 바꾸는 원본 근거는 없다.

## Enter/Escape와 border의 확인된 한계

PlotControlBase0ad8은 EquationCommands.Commit을 preemptive binding으로 연결하고 lambda0bae는 Commit만 호출한다. PlotControl.Commit01f3은 internal calculatables를 commit하고 Y/X 식 목록의 CommitAllEquationListWithoutEvent를 호출한다. 이 본문에 새 슬롯 생성, 다음 슬롯 선택, border 이동 또는 plot 종료는 없다. 따라서 이 **commit 본문 자체**는 Enter가 새 식 슬롯을 만드는 경로가 아님을 뒷받침한다. ShiftEnter source 삽입과 계산 Enter를 구분한다.

그러나 EquationControl/worksheet의 Enter preview, IME 조합, expression/scaling focus loss와 다른 routed command 우선순위를 이 문서에서 모두 추적하지 않았다. 모든 Enter가 반드시 현재 editor에 머물거나 반드시 border로 나간다는 결론은 미확정이다. Escape의 plot equation→plot border→worksheet 선택 단계와 commit/cancel 의미도 이번 selected PlotControl/PlotEquation methods만으로 확정하지 않았다. 앱에서 Escape를 draft 취소로 연결했다는 사실을 원본 계약으로 역투영하지 않는다.

## 증거 identity와 제품 적용 범위

REA layout strategy=`ev_79889b8af3ca1a6068ac40fa809e8b5fe2f964e17779033613f17e85e232cf52`, focus/table=`ev_1f22d14c55bc69a92952b6f43f87d16c4ad3ea848b35d91f4bd989f393317acc`, PlotItem.Move=`ev_de6f5e0ab5d78c47a0908c835d46a15419531ebe4cb5a726851a2adcc7f366df`.

Plots.dll SHA=`4e5067e58e69c68e8152ce677d5ac77a6587e00f6295bda5cc8ddff18f6685eb`, MVID=`cecd333a-bf3e-4423-b74b-7ff799618db1`. en-US satellite/resource 단위 SHA는 ignored `native-xy-layout-resources-20261019.json`. ledger `native-xy-layout-method-ledger-20261019.json`에 선택 method token/IL SHA와 설치 bytes 재검증을 저장했다. BAML decoded view는 같은 prefix JSON, CIL은`feedback/native-xy-layout-*-cil-20261019.txt`이다. Direct TraversedFocus 추가 REA=`ev_d7f9b862164e85562e7265ad75c3ffc980e9d791b5b5f029660df57a4736e91b`, Core.CommonRoutedEvents019c SHA=`05d4b217da3b8f61987c397b890b88dfc49d791584d71cd435fd4bec301e3233`.

지원 가능한 좁은 다음 단계는 source slot/단위 칸을 분리한 목록, 실제 source 순서와 stable identity, 검증된 내부 Tab 순서, measured list overflow 기반 외곽 geometry다. 완전한 theme/pixel parity나 Enter/Escape boundary fidelity는 추가 조사 없이 주장하지 않는다. 원본 리소스는 분석 증거로만 보관했으며 제품 asset으로 사용하지 않았다.
