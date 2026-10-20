# XY 축 식 개수·공유 소유권·작성 명령

2026-10-19. 설치 Mathcad Prime12.0.0.1의 정적 REA와 실제 PE bytes, 설치 도움말로 조사했다. GUI·스크린샷·원본 runtime·CLR은 실행하지 않았고 제품 Rust 파일은 변경하지 않았다. [0146](0146-native-xy-plot-export-contract.md)의 N/N source subset과 [0149](0149-native-xy-domain-and-commit-contract.md)의 commit 계약을 잇는다.

## 불균등 목록의 확인된 pairing

두 축이 각각 하나 이상의 식을 가질 때, 원본은 순번대로 짝짓고 짧은 쪽 **마지막 식을 재사용**한다. 식들을 Cartesian product로 조합하거나 처음 식으로 되돌아가는 방식이 아니다. 완성된 일반 축 목록의 대응은 다음과 같이 표현할 수 있다.

`trace i = (X[min(i,nx−1)], Y[min(i,ny−1)])`, `0 ≤ i < max(nx,ny)`.

| x 목록 | y 목록 | 대응 |
| --- | --- | --- |
| x0 | y0,y1,y2 | (x0,y0),(x0,y1),(x0,y2) |
| x0,x1 | y0,y1,y2 | (x0,y0),(x1,y1),(x1,y2) |
| x0,x1,x2 | y0 | (x0,y0),(x1,y0),(x2,y0) |

이 규칙은 도움말만의 추측이 아니다. PlotPresenter.HandleEquationAdded `0x06000a94` IL001c–0073은 반대 축의 equation count가0이면 null로 반환하고, 새 식 순번이 반대 축 count 이상이면 **count−1**을 선택하며 아니면 같은 순번을 선택한다. source 식과 선택한 반대 축의 실제 `IEquationItem`을 가져온다. MethodSpec61=Count,62=ElementAt도 설치 metadata로 해결했다. IL SHA=`e5c8e3274d926f91822617d58e3028511883ea43bebce3617e611aea5b541290`.

PrimaryYAxis_EquationAdded0a8e와 xAxis_EquationAdded0a8f는 각각 source/other axis의 순서를 정하고 AddItemCommand를 Undo buffer로 실행한다. HandleEquationAdded는 새 trace 생성이 필요한 분기에서 두 실제 식의 AddTrace를 호출한다. 이후 RearrangeTraces0a9c가 equation별 trace 소유권을 조정한다.

## Native reader의 실제 loop

PlotModel.createPlot `0x06001150`은 xAxis plotEquations와 rank1 yAxis plotEquations를 독립적으로 읽는다. index를0부터 시작하여 `index < max(xCount,yCount)` 동안 반복하고, xCount>index인 경우에만 x AddEquation, yCount>index인 경우에만 y AddEquation을 호출한다. 짧은 축에 가짜 식을 추가하거나 XML 식 목록을 미리 N/N으로 늘리지 않는다. IL SHA=`5bc32b4586ea09cd880da2b5520169844b5175236445b4ac6809224ec9301f95`.

그 후 set_GraphRegionXmlNode1155가 native traces를 순회하며 이미 구성된 model trace의 같은 ordinal에 style/resultRef XML을 전달한다. 이 reader와 위 presenter의 재사용 규칙이 서로 연결된다. 원본 저장 시 GetAxisXmlNode1152는 각 axis의 실제 equation 목록을 그대로 쓴다. 공유 식을 trace 수만큼 복제하여 저장하는 계약이 아니다.

## 공유는 문자열 중복 제거가 아니다

식 하나에 여러 trace가 실제로 연결된다. equation의 get_Traces가 여러 항목을 소유하고, model trace가 같은 IEquationItem을 참조한다. 따라서 shared-X native source를 독립된 `(x0,y0),(x0,y1)` 두 PlotTraceSpec으로만 펼치면 계산 결과는 같아도 작성 의미가 달라진다.

* 원본 x0 식 편집·Enter는 그 식을 공유한 모든 trace에 영향을 준다. 두 개의 독립 x0 문자열을 만들면 한 필드만 수정되는 다른 모델이 된다.
* 식 삽입·삭제는 trace association을 바꾼다. 원본은 각 trace의 style·identity를 무조건 삭제한 뒤 처음부터 새로 만드는 코드가 아니다.
* 원본 목록을 N/N으로 바꿔 저장하면 다음 원본 편집에서도 공유 관계가 사라진다. 값이 같다는 이유만으로 식을 dedup하거나 반대로 복제하는 것은 원본 UX 보존과 다르다.

현재 source helper의 strict N/N guard를 이 문서만 보고 제거하지 않는다. 불균등 import/edit 지원에는 독립 x/y source 목록과 공유 association, 또는 원본 소유권을 보존할 별도 모델이 필요하다. 그 모델이 없으면 Opaque로 보존하는 제한이 정직하다.

RearrangeTraces0a9c는 순서대로 인접 equation을 본다. 현재 식의 trace 수가0이고 다음 식에 trace가 있으면 MoveFirstTrace0a9d를 사용하고,1개이면 유지하며,2개 이상이면 MoveAllButFirstTrace0a9e를 사용한다. 두 helper는 ITraceModel.SetEquation과 view SetEquation, RemoveTraceFromEquation/AddTraceToEquation을 통해 소유권을 이동한다. 이 조정은 마지막 식이 여러 trace를 공유하는 구조의 양성이다. 다만 임의 중간 삽입·삭제·여러 trace 선택·legend 재정렬을 포함한 전체 identity continuity와 Undo 후 커서 복원은 아직 완전히 검증하지 않았다. 위 min-index 대응을 이 모든 편집 과정의 완전 구현 증거로 과장하지 않는다.

## 빈 목록·빈 식·기본값

설치 도움말은 적어도 x 식 하나와 y 식 하나가 있어야 한다고 설명한다. 실제 EquationList.RequestRemoveItem00c0은 슬롯이 하나뿐이면 제거하지 않는다. 마지막 식이 비어 있지 않으면 ClearPlotEquationCommand로 내용만 지우며, 이미 빈 마지막 슬롯은 남긴다. XML schema가 empty plotEquations를 허용한다고 해서 작성 UI에 zero-slot 축을 추가하지 않는다.

반대 축의 model equation 목록이0개인 AddEquation은 위0a94에서 null로 끝나며,0개를 자동으로 x 변수나 수치0으로 바꾸는 분기는 없다. **빈 목록**과 **placeholder를 가진 식 슬롯 하나**도 다르다.

TraceModel.ContainsEmptyEquation1232는 x 또는 y equation의 IsPlotEquationEmpty를 검사한다. CommitIfEquationsNotEmpty1243은 이 값이 false일 때만 Commit한다. SetEquation123f는 식을 연결하고 IsCommitted=false로 바꾸며 특정 synchronization 상태에서만 이 조건부 commit을 호출한다. 새 placeholder를 넣었다고 가짜 default 식으로 계산하는 것이 아니다. 빈 식을 포함한 미완성 native plot의 모든 error/cache 표시 정책은 후속 범위다.

## 실제 작성 키와 위치

설치 `to_add_or_delete_an_axis_expression.html` SHA=`6680cf86b60e75d25cf4f114af5a0347de490218fa3b2339e50d1debda4a0d7e`는 다음을 명시한다.

* **Shift+Enter**, 또는 Plots → Traces → Add Trace가 새 axis expression placeholder를 추가한다.
* 현재 식 왼쪽에 커서를 두면 위에, 그 밖에는 아래에 추가한다. AddEquationByPosition00d3은 실제 AtLeftEdge이고 현재 식이 `@PLACEHOLDER`가 아닌 경우에만 현재 index 앞에 넣는다. 빈 PH 슬롯의 왼쪽이라는 이유만으로 같은 규칙을 추측하지 않는다.
* 내용을 지운 뒤 Backspace/Delete 또는 Remove Trace로 슬롯을 제거한다. 유일한 슬롯은 남긴다.

EquationCommands cctor0025는 InsertRow를 WPF EnterLineBreak에 연결하고 ShiftEnter gesture를 추가한다(IL02b5–02ba/0660–0674). EquationList.BindCommands0094는 InsertRow를 HandleInsertItemToLegendCommand00d5에 연결한다. handler는 ContainsLabels가 false일 때 AddEquationByPosition을 호출하고 Dispatcher를 통해 새 항목 포커스를 예약한다. 따라서 Shift+Enter의 axis-list 작성 경로는 실제 코드로 확인된다. **쉼표를 새 trace 구분자로 쓰는 경로는 이번 원본 근거에서 확인하지 못했다.** 일반 식 안 함수 인자/행렬 구분자를 축 식 목록 구분자로 재해석하지 않는다.

`to_add_or_delete_a_trace.html` SHA=`56e2c06dc32991e95d024498c5e2473f8b2ee3b3089c2baff3349a3db1f1c71ac`는 식 추가 후 **Enter**로 새 trace를 표시하고, 식 삭제 후에도 Enter로 평가하기 전에는 trace가 사라지지 않는다고 설명한다. Enter는 새 목록 슬롯을 추가하는 Shift+Enter와 다르다. 상세 dirty/commit 분리는0149를 따른다.

Tab/Shift+Tab은 EquationCommands의 Traverse/BackwardsTraverse이며 각각 WPF TabForward/TabBackward와 gesture로 연결된다. 이번 확인으로 이를 axis expression 추가 명령이라고 바꿀 근거는 없다. EquationList의 좌우/상하 이동은 별도 navigation command와 equation 경계 handler00c9를 사용한다. Tab이 expression/scaling/다음 축/worksheet의 어느 위치로 넘어가는지 모든 상태의 routed focus 순서를 이번 범위에서 완전히 추적하지 않았다.

## 증거와 구현 판단

새 REA `ev_2dcdf19ac1bb62b5912c3d25f60550c428721a04d6482ba35515ca9ef6e28c66`은 PlotPresenter/AxisPresenter/EquationList를 확인했다. assembly SHA=`4e5067e58e69c68e8152ce677d5ac77a6587e00f6295bda5cc8ddff18f6685eb`, MVID=`cecd333a-bf3e-4423-b74b-7ff799618db1`이다. source reader는0146의 기존 REA/원시 본문을 재사용했다. EquationCommands는 기존 EquationControl.dll SHA=`0b6fd580eb6cc183f17b5b5e0967226bbafc5e12efc6da809f44cd16416bf9d1`와 cctor IL SHA를 재검증했다.

ignored `artifacts/rea/native-xy-pairing-method-ledger-20261019.json`에 raw token/IL SHA·도움말·해결 MethodSpec을 기록했다. 본문은 `feedback/native-xy-pairing-*-cil-20261019.txt`, fresh metadata는 동명 presenter-members JSON이다. 제품 소스·r150 snapshot은 변경하지 않았다.

지원 가능한 다음 단계는 원본 축 목록 모델 도입 후 마지막 식 재사용·공유 편집·Shift+Enter 추가·Enter commit을 함께 구현하는 것이다. min-index 계산만 추가하여 원본 shared source를 독립 trace 문자열로 바꾸는 것은 이번 계약을 만족하지 않는다.
