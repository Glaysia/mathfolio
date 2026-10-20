# XY 축 식 슬롯 삽입·삭제 계약

2026-10-19. Prime 12.0.0.1 설치 managed PE를 실행하지 않고 REA metadata와 설치 bytes의 CIL을 확인했다. GUI·스크린샷·원본 runtime·CLR은 사용하지 않았다. 제품 Rust 파일은 수정하지 않았다. [0151](0151-native-xy-axis-pairing-contract.md)의 pairing 계약과 [0155](0155-shared-xy-axis-model-design.md)의 source identity 제안을 잇는다.

## 명령과 삽입 위치

원본 Shift+Enter는 EquationCommands.InsertRow → EquationList.BindCommands0094 → HandleInsertItemToLegendCommand00d5 경로다. 00d5는 ContainsLabels가 false인 목록에서만 실행한다. 명령 바인딩에는 별도 CanExecute count-limit handler가 없다. 0151에서 확인한 EquationCommands0025의 WPF EnterLineBreak/ShiftEnter binding을 재사용한다. 설치 도움말도 Plots → Traces → Add Trace를 같은 축 식 작성 기능으로 설명한다. 이번 조사에서는 ribbon BAML을 다시 추출하지 않았으므로 두 진입점의 모든 routed priority가 동일하다는 추가 주장은 하지 않는다.

AddEquationByPosition00d3의 실제 분기는 다음과 같다.

| 현재 상태 | 새 슬롯 위치 |
| --- | --- |
| equation count가0 | index0 |
| 선택 식의 AtLeftEdge=true, expression S-expression이 정확히 `@PLACEHOLDER`와 다름 | 현재 index 앞 |
| 그 밖의 상태 | 현재 index+1 |

AtLeftEdge는 문자열 caret offset0을 앱에서 임의로 비교하는 속성이 아니다. PlotEquation0340은 **scaling 칸에 keyboard focus가 있으면 false**를 반환하고, 그 밖에는 expression EquationControl.AtLeftEdge를 반환한다. 따라서 scaling 칸 왼쪽에서도 뒤에 추가된다. PH 식은 왼쪽 경계여도 뒤에 추가된다. 부분선택·구조 선택의 AtLeftEdge 해석은 하위 EquationControl 계약을 따라야 하며 이번 문서가 모든 selection을 단순 offset0으로 정규화하지 않는다.

00d3은 기존 식 내용을 분할하거나 복사하지 않는다. 선택 index로 AddItem00ae → FireRequestAddEquation00bc를 호출하며 현재 선택 index도 이전 위치로 전달한다. PlotPresenter0a8e/0a8f가 AddItemCommand를 Undo buffer로 실행한다. 새 슬롯은 새 PlotEquation/EquationListItem이며 이전 식 identity가 바뀌는 대체가 아니다.

## 새 슬롯과 포커스

GetNewEquation00b0은 일반 축에서 PlotEquation과 ViewModel을 만든다. PlotEquation030c는 초기 ActiveEquation을 `_exp`로 설정한다(IL00c1–00c8). EmptyPlotEquationSExp0336은 expression/scaling 모두 PH인 형식을 만든다. constructor 전체에서 최초 PH 상태를 모두 생성하는 BAML/EquationControl 초기화까지 이번 범위에서 다시 검증하지 않았으며, 새 슬롯이 도움말의 axis placeholder라는 사실과 empty 식 formatter를 구분한다.

AddItemToLegend00af는 요청 index에 UI item을 Insert하고 ChangeSelectedItem으로 그 item을 선택한다. Shift+Enter handler00d5는 Dispatcher priority6에 callback149f를 예약한다. callback은 새 item.Equation.DelegateFocus만 호출하고, DelegateFocus035c는 ActiveEquation.Focus, 실패하면 UpdateLayout 후 다시 Focus한다. 따라서 새 expression 입력 컨트롤에 포커스를 주는 양성은 있다. PH의 실제 native NavState/UTF16 선택 폭, caret 픽셀 위치나 focus 직후 전부 선택 여부는 이 메서드들만으로 확정하지 않는다.

## 식·trace identity와 appearance

source 축에 식이 추가된 뒤 HandleEquationAdded0a94는 반대 축 목록이0이면 null을 반환한다. 그 밖에는 min-index 대응을 선택한다. 관련 trace count는 closure15bb가 **trace.GetAxis(sourceAxisIndex)==sourceAxis**인 trace만 세므로 단순 전역 trace 개수라고 바꾸지 않는다.

추가 후 source equation count−1이 관련 trace count와 같으면 새 trace가 필요한 분기다. 최초 Do에서 trace 인자가 null이면 AddTrace0a95를 호출한다. 이 helper는 model.AddTrace 후 TraceStyleSelector.ApplyNextStyle과 plot default trace type을 설정한다. 이미 보관된 trace를 넘기는 Redo/Undo 복구에서는 model.AddTrace(existingTrace) 경로를 사용하며 새 style을 다시 선택하지 않는다. view trace, equation membership, plot legend도 연결한다.

새 trace가 필요하지 않으면 source 축의 RearrangeTraces0a9c만 실행한다. 새 trace를 만든 경우에는 반대 축을 재배치한다. RearrangeTraces는 인접 식을 순서대로 처리한다.

* 현재 식 trace가0개이고 다음 식에 trace가 있으면 다음 식의 첫 trace 하나를 현재 식으로 이동한다.
* 현재 식 trace가1개이면 유지한다.
* 현재 식 trace가2개 이상이면 첫 trace를 남기고 나머지를 다음 식으로 순서대로 이동한다.

MoveFirstTrace0a9d와 MoveAllButFirstTrace0a9e는 기존 ITraceModel.SetEquation과 기존 view.SetEquation을 호출하고 membership만 이동한다. 이 과정은 기존 trace를 새 style로 재생성하는 경로가 아니다. 예를 들어 X=[x0], Y=[y0,y1], trace T0/T1인 상태에 X 끝 슬롯을 넣으면 새 trace 수를 늘리지 않고 x0의 T1이 새 X 식으로 이동하는 분기를 도출할 수 있다. 반면 X/Y가 각각1개인 상태에서 Y 끝 슬롯을 넣으면 새 trace가 생겨 다음 style을 받는다. 이 두 예는 위 정적 분기의 도출값이며 원본 실행 캡처라고 부르지 않는다.

AddItemCommand15a1은 새 item과 생성된 trace를 `_item`/`_trace` 필드에 보관한다. Undo15a2는 이전 선택 index를 RemoveEquation에 전달하고 HandleEquationRemoved를 실행한다. Redo15a3은 **동일 item**을 다시 삽입하고, 저장된 **동일 trace**를 HandleEquationAdded에 전달한다. 따라서 source 슬롯 삽입을 통째로 pair 목록 재생성 + ordinal style 재할당으로 구현하면 원본 객체·appearance continuity를 보존하지 못한다. 다만 complete plotting state의 렌더링 z-order, legend drag/reorder, 비primary 축까지 포함한 임의 insertion/deletion style continuity는 아직 이 좁은 계약 밖이다.

## Backspace/Delete와 유일한 슬롯

PlotEquation.HandleBackspace031c/HandleDelete031d는 sender가 PlotEquation이고 IsPlotEquationEmpty일 때만 FireRequestRemove를 호출하고 RoutedEvent.Handled=true로 설정한다. 방향은 각각0/1이다. IsPlotEquationEmpty0331은 **IsScalingEmpty && IsExpEmpty**다. expression만 PH이고 scaling/단위가 남아 있으면 이 empty-slot 제거 경로에 들어가지 않는다. 채워진 식에서 한 번 Backspace가 곧바로 축 슬롯 전체를 삭제한다는 계약은 없다.

EquationList.RequestRemoveItem00c0은 다음을 수행한다.

| 목록 상태 | 결과 |
| --- | --- |
| 슬롯1개, 이미 empty | 슬롯 유지; 제거/새 clear command 없음 |
| 슬롯1개, nonempty | ClearPlotEquationCommand를 Undo buffer에서 실행; 슬롯 identity 유지 |
| 슬롯2개 이상, Backspace(direction0) | 앞 슬롯이 있으면 그 index를 다음 선택으로, 첫 슬롯이면 같은 index(제거 후 successor) |
| 슬롯2개 이상, Delete(direction1) | successor가 있으면 같은 index, 마지막 슬롯이면 predecessor index |

RemoveItemAt00c2는 제거 전 events 해제, UI item 제거, 선택 변경, DelegateFocus를 실행한다. direction0이면 NavigateToEquationEnd(Direction.Right), direction1이면 NavigateToEquationStart를 실행한다. 이는 empty-slot 제거 후 인접 식의 end/start 포커스 양성이다. Undo 복구에 전달되는 direction4는 이 두 추가 navigation 분기를 실행하지 않는다. 세부 native text selection 복구까지 동일하다는 주장은 하지 않는다.

ClearPlotEquationCommand.Do007c는 nonempty일 때 CompoundCommand/CommandScope 안에서 EmptyPlotEquationSExp를 설정하고 NavigateRegionStart를 호출한다. 이미 empty이면 false를 반환한다. 따라서 유일한 채워진 슬롯의 Remove Trace 요청은 clear 동작과 구분되며, empty 키 처리에서 유일 PH를 zero-slot 목록으로 만들지 않는다.

## 실제 trace 제거와 Enter

HandleEquationRemoved0a97은 먼저 source axis에서 equation을 제거한다. 남은 전체 trace count가 반대 축 equation count보다 크면 제거한 식의 첫 trace를 model/view/legend/calculatable에서 제거하고 반대 축을 RearrangeTraces한다. 그렇지 않으면 기존 trace들을 source 축의 같은 index에 남은 식, 또는 제거 위치가 끝이면 남은 마지막 식으로 재연결한다. 제거된 마지막 식의 trace들은 새 마지막 식 뒤에 append하고, 중간 식의 trace들은 successor membership 앞쪽에 연결하는 분기가 구분된다. 모두 새 식 문자열 dedup이 아니라 실제 IEquationItem 소유권 조작이다.

RemoveItemCommand15a7은 제거된 item/trace를 저장한다. Undo15a8은 item을 재삽입하고 저장된 trace로 HandleEquationAdded를 호출한다. trace가 있으면 ResultManager.DispatchResult도 호출한다. 이처럼 원본은 이미 계산된 curve 복구를 새 계산과 구분한다. installed help는 식 삭제 뒤에도 Enter로 평가하기 전에는 trace가 사라지지 않는다고 설명한다. 즉 source/model association 갱신 시점과 최종 curve 표시/계산 갱신을 하나의 즉시 재계산으로 합치지 않는다. 모든 pending/error style·Undo 결과 curve 유지 타이밍을 이번 분석만으로 완전히 확정하지 않는다.

## 삽입 모델에 적용할 수 있는 조건과 style preset

다음 조건은 0a94 IL0075–00a9의 실제 비교를 우선한다: `source_equations_after.len() - 1 == count(trace.GetAxis(source_axis_index) == source_axis)`이면 trace를 추가하고, 아니면 기존 trace를 재연결한다. 정상적인 두 primary 축이 이미 min-index invariant를 만족하여 기존 trace 수가 max(oldSourceCount,otherCount)인 경우에 한해서 이는 **oldSourceCount >= otherCount이면 새 trace1개**, **oldSourceCount < otherCount이면 기존 trace만 이동**으로 단순화된다. 두 번째 식을 항상 만들거나, source 식을 넣을 때마다 항상 appearance 행을 추가하는 구현은 다르다. 반대 축이0인 경우는 별도 early return이므로 이 단순화 적용 대상이 아니다.

원본 association을 갖고 있는 상태에서 재배치해야 한다. 예를 들어 다음은 위 CIL로 도출되는 일반 primary-axis 예다.

| 삽입 전 | 삽입 | trace 변화 |
| --- | --- | --- |
| X=[x0], Y=[y0,y1]; T0=(x0,y0), T1=(x0,y1) | X 끝에 p | 기존 T0=(x0,y0), T1=(p,y1). style/identity 그대로, 신규 trace 없음 |
| X=[x0], Y=[y0,y1]; 같은 T0/T1 | X 앞에 p | T0=(p,y0), T1=(x0,y1). 새 식이 앞 trace를 인수, 신규 trace 없음 |
| X=[x0], Y=[y0]; T0=(x0,y0) | Y 끝에 p | T0 유지, 새 T1=(x0,p)가 다음 preset을 받음 |
| X=[x0], Y=[y0]; 같은 T0 | Y 앞에 p | 새 T1=(x0,p), 기존 T0=(x0,y0) 유지. ordinal 재계산을 이유로 T0 style을 T1로 복사하지 않음 |

trace display ordering 전체와 식별자 naming은 이 표의 pairing/identity와 별도다. 새 trace model은 model.AddTrace에서 추가되고 plot legend는 삽입 index를 받으므로 단일 Vec의 모든 ordinal을 동시에 덮어쓰는 방식이 완전한 원본 모델이라는 주장은 하지 않는다. appearance를 source 식 ID에 붙여 식과 같이 단순 이동시키는 것도 실제 trace identity와 다르다.

ApplyNextStyle0ace의 field040001f7은 TraceColorTable.Default다. 0ace는 currentColorIndex의 brush color, Symbol enum0, LineStyle enum1, LineWeight=1.0, SymbolFrequency=1.0, SymbolWeight=1.5를 설정하고 index를1 증가시킨 뒤 palette count에서0으로 되돌린다. 기존 enum 근거의 Symbol0=none/LineStyle1=Solid와 일치한다. TraceStyleSelector ctor0acf는 object ctor만 호출하므로 새 selector의 index는 CLR field 기본값0이다. selector 생성/import 초기화는 아래 후속 절에서 별도로 확인했다. 현재 trace 개수 modulo7를 모든 편집 세션에 일반화하지 않는다.

TraceColorTable ctor06d6은 WPF Brushes를 **DarkBlue, Black, Red, Yellow, Orange, Green, Blue** 순서로 넣는다. 별도 line-style 순환은 이 method에 없다. 실제 RGBA 값을 설치 WPF KnownColors까지 재수집하지 않았으므로 여기서는 명명된 brush 호출을 원시 양성으로 기록한다. 새 trace마다 thickness를1.5로 추정하면 안 된다:1.5는 SymbolWeight이며 LineWeight는1.0이다. Undo/Redo의 existingTrace 경로에는 ApplyNextStyle 호출이 없다는 앞 절의 계약을 유지한다. 아래 후속 조사로 selector lifetime/import 분기를 추가 확인했다.

새 REA style=`ev_6f65af9b5371f59c85b280a1489cb2d30c18f53f62531ea1a3d2a651700722e7`, palette=`ev_a2e086802867d4e6fab024cec682bc03ae565044dbf64fa1024fa10cd3bda4a9`. ApplyNextStyle0ace IL SHA=`85dfc08209b39b4254c1ba844072312e1bae0caaaf99d2faa4ca23c454f24577`; palette ctor06d6 SHA=`683d82fdc52e2d060943ef3a89616903b9e589c53a9d623742e68bf5a37bf5c7`. 원시 본문은 slot-style/slot-palette CIL 파일이다.

## 후속: selector lifetime과 native import

PlotPresenter ctor0a81 IL0016–001c는 새 TraceStyleSelector를 `_traceStyleSelector`에 저장한다. PlotControl ctor01b0은 새 PlotModel을 만든 뒤 IL0266–026d에 새 PlotPresenter를 연결한다. selector ctor0acf에는 index setter가 없고 instance integer field의 기본값0을 사용한다.

추가로 설치 Plots.dll의 현재 managed CIL body **5,046개 전부를 metadata parser로 디코드**하여 selector ctor, ApplyNextStyle, selector field 및 `_currentColorIndex`의 token 참조를 스캔했다. decode failure는0개다. selector 생성/저장은 PlotPresenter ctor에만 있고 ApplyNextStyle 호출은 presenter.AddTrace0a95에만 있다. `_currentColorIndex`를 쓰는 CIL은 ApplyNextStyle의 증가/7색 wrap뿐이다. Initialize/Reset/SetIndex 또는 삭제/Undo에서 index를 줄이는 메서드는 이 assembly의 static CIL에 없다. 이는 원본을 실행한 스캔이 아니며 private field reflection/native memory 수정을 포괄하는 부재 증명은 아니다.

따라서 **현 presenter lifetime에서 새 trace style이 실제 배정된 횟수 modulo7**이 next index다. trace 삭제·새 trace 삽입 Undo는 이 index를 되감지 않는다. 동일 trace를 복구하는 Redo/삭제 Undo는 ApplyNextStyle을 다시 호출하지 않는다. 현재 보이는 trace 개수 또는 마지막 trace 색상에서 next index를 역산하면 삭제/Undo 후 달라질 수 있다. 세션이 끝나고 새 presenter로 열리면 새 index0에서 시작한다.

일반 native open 경로는 PlotControl.SetRegionPersistentData02bd → model.set_PlotXmlNode114f → FireDeserializationStarted → createPlot1150 → FireDeserializationFinished다. 0a82는 두 deserialization 이벤트를 실제 구독한다. handler0a8b/0a8c는 축 위치/배경/legend geometry를 적용하며 selector/index를 만지지 않는다. 새 model1142는 빈 `_traces` List로 시작한다. createPlot은 실제 x/y 식을 번갈아 삽입하고 presenter 경로로 필요한 trace를 만든 뒤, 마지막에1155로 XML trace style/resultRef를 덮어쓴다. 이 overwrite가 palette index를 맞추거나 원본 색상을 검색하는 경로는 없다.

따라서 **fresh control에 정상적인1개 이상 primary x/y 목록을 처음 import하는 경로**에서는 max(nx,ny)번의 신규 style 배정 후 XML style을 복원하므로 next index는 max(nx,ny)%7로 도출된다. 이는 fresh import에 한정한 초기화 도출이며 색상 내용에 근거한 추정이 아니다. 이미 편집 중인 control에 set_PlotXmlNode를 반복하는 경우, 붙여넣기/별도 presenter 재사용 등은 이 fresh 초기 조건과 같다고 가정하지 않는다. 내부 next index는 원본 XML style 필드로 직렬화되지 않으므로 전체 작성 history를 현재 파일로부터 복구한다는 계약도 없다.

원시 증거는 `native-xy-slot-selector-reference-scan-20261019.json`과 재실행 가능한 동명 scan Python, `feedback/native-xy-slot-selector-lifetime-cil-20261019.txt`, `slot-import-{control,model,setter-main}-cil`이다. ctor0a81 SHA=`393d99d217556185d9a8dd8e94ca91801bd4e66856546e0c96595bbe04523f2b`, control01b0 SHA=`91bfa0656740e5154389539821141b6aa3a427c414e4619f77e00b8b8eaafadf`, setter02bd SHA=`25a650689cddcf0ae6795800b2bc5670091122a529a0d0207db5404c4ef1b6bc`.

## 후속: global append·membership·native writer 순서는 다르다

Model.AddTrace1169는 global `_traces` List.Add다. 그러나 XML writer1154 IL00e9는 이 List를 바로 순회하지 않고 **get_SortedTraces1153**를 순회하여 각 trace.RegionXmlNode를 쓴다. SortedTraces는 X 목록이 Y보다 길면 X, 그 밖에는 Y(동수도Y)를 고른다. 선택한 축의 식을 `IEquationItem.Equation` 목록으로 만들고 global trace들을 `chosenEquations.IndexOf(trace.GetEquation(chosenAxis))` 키의 LINQ OrderBy로 반환한다. MethodSpeccc=Select,cd=ToList,25=OrderBy와 두 lambda를 설치 metadata로 해결했다. OrderBy가 같은 키에 대해 stable인 성질은 LINQ 계약이지만, 현재 정상 min-index 두 primary 축의 긴 쪽은 trace마다 식이 하나씩 대응하므로 일반 예에서는 다른 ordinal 키를 갖는다.

예를 들어 X=[x0],Y=[y0]에 Y 앞 슬롯 p를 추가하면 global model List는 [Told,Tnew]가 될 수 있지만, source Y=[p,y0]에 대해 writer는 **[Tnew,Told]** 순서로 trace XML을 쓴다. 각각의 style/resultRef는 trace 객체를 따라간다. Reader1155는 생성된 `_traces[i]`에 XML trace[i]를 적용한다. 별도의 NodeHandle로 source-style 대응을 재탐색하는 코드가 아니라 writer가 canonical axis ordinal로 쓴 source/native trace 배열을 읽는 경로다.

또한 EquationItem.get_Traces10a7은 per-equation `_traces` List를 그대로 반환한다. ctor10a4가 생성한 List에 AddTrace10a8은 append, overload10a9는 index Insert, RemoveTrace10aa는 RemoveAt다. **이 membership 순서를 writer 정렬/global 생성 순서로 대체하지 않는다.** HandleEquationAdded는 source 식 membership에 새 trace를 append하고, 반대 식 membership에는 `max(0, insertedSourceIndex-(otherAxisCount-1))` 위치로 Insert한다. 이후 MoveFirstTrace/MoveAllButFirstTrace의 First/Skip(1)은 이 mutable List의 순서를 사용한다. 긴 쪽 앞 삽입 뒤 짧은 쪽 분할/합치기를 구현할 때 이 상태가 필요하다. 전체 pair를 다시 생성하면서 appearance를 ordinal로만 맞추는 모델은 원본 object membership 상태를 누락할 수 있다.

새 REA order=`ev_9636212b465c9b8f0afb8bd147cbab5c6c64d3badfcd004cc05df0f004d53f15`, membership=`ev_836157b2385ed32bb356a57f1b4a04a8e6dbc666039b63d4ff947372c6a7c1e4`. SortedTraces1153 SHA=`34ad473bdd45e8f93a473d023d82d3e86dde792018028cd687d771d83a1ef297`; key lambda169f SHA=`0ecd8567c6c2214ff349760e53dfbf11f3662fc4614ee31587b1998dfa1770dd`; writer1154 SHA=`1bfda8e41e5fe0414dc16dfb1c96e12c9b03ae4498e429d97368039d2325480f`. 원시 본문은 `slot-trace-order`, `slot-trace-order-selector`, `slot-membership` CIL 파일이다.

## 최대 개수와 명시적인 미확정 범위

검증한 Shift+Enter handler, AddEquationByPosition, FireRequestAddEquation, AddItemCommand.Do, AxisModel.AddEquation106f/1070에는16·50 등의 최대 개수 guard가 없다. AxisModel106f는 IList.Insert를 사용한다. 인접 trace 재배치도 목록 길이 기반이다. 설치 trace/axis-expression 도움말 검색에서도 수치 상한을 찾지 못했다. 따라서 **원본의 전역 최대 개수는 미확정**이며 현재 Mathcraft의16개 상한을 원본 제한이라고 표시하지 않는다. native schema 무제한과 이 경로의 guard 부재를 무한 작성 보장으로 바꾸지도 않는다. resource/engine/UI 외부 guard 전체를 이번 범위에서 부재 증명하지 않았다.

미확정으로 남기는 항목은 선택 범위별 AtLeftEdge 하위 판정, PH의 정확한 focus NavState, 모든 axis/legend reorder와 multi-selection 조합, source mutation과 curve commit 사이의 모든 이벤트 우선순위, 원본 전역 개수 cap이다. 지원 첫 단계는 검증된 primary 축의 before/after insertion, shared membership 이동, empty-slot 제거 및 단일 슬롯 보존이며, native key로 근거 없는 comma 삽입을 추가하지 않는다.

## 원시 증거

설치 Ptc.Controls.Plots.dll SHA=`4e5067e58e69c68e8152ce677d5ac77a6587e00f6295bda5cc8ddff18f6685eb`, MVID=`cecd333a-bf3e-4423-b74b-7ff799618db1`.

새 REA는 command/nested callback `ev_845334022e261bf875d873c6eee87684fe8bfd87b9ed7aad169e2acd53ba6b3a`, PlotEquation `ev_eda26b6104cbe44c78367474cd3fa207a9f33345a55c537a10ee42fd65b79acd`, presenter predicates `ev_328c602936f5c4859cbd76bf6ba5dd0ea6aaf083eccc607eda59c4351f55e9f4`이다. namespace 없는 nested type은 metadata의 단순 TypeDef 이름으로 선택했다. 별도 unmatched-qualified-name 시도 artifact는 완전한 nested 수집 근거로 사용하지 않았다.

`artifacts/rea/native-xy-slot-method-ledger-20261019.json`에 기존/새 token·IL SHA·raw file·설치 bytes 검증을 기록했다. 새 원시 본문은 `feedback/native-xy-slot-*-cil-20261019.txt`, 기존 본문은0151 ledger의 `native-xy-pairing-*`이다. 도움말 SHA/본문은0151 ledger를 그대로 재사용했다. 본문에서 native observation과 정적 도출 예, 미확정 항목을 구분했고 제품 코드는 변경하지 않았다.
