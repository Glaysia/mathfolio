# 명시 페이지 나누기와 공간 삽입·제거

2026-10-19. 설치된 Mathcad Prime 12.0.0.1의 보존 REA metadata와 설치 PE bytes, 도움말, XSD, 기본 Presentation 템플릿을 정적으로 조사했다. GUI·스크린샷·원본 프로그램·CLR 실행은 하지 않았다. 제품 코드는 변경하지 않았다.

## 명령과 저장 형식

빈 워크시트 삽입점에서 Ctrl+Enter는 페이지 나누기를 삽입한다. 도움말 `to_insert_or_delete_page_breaks.html`(SHA `35a1f56adb40b00d17cec2105b82615ad8dbdb18a374ef18d5ea1e12b299180b`)과 tutorial task6-2(SHA `8c3135165bb6dab8ff6d35a2c5bd16efb38eb041ad02eefb135cdff52d6cf24a`)는 `Document → Spacing → Add Page Break` 경로를 설명한다. 설치 BAML에는 같은 버튼의 tooltip이 `Insert Page Break (Ctrl+Enter)`로 들어 있다. 따라서 도움말의 Add와 리본의 Insert 표기는 같은 기능의 다른 문자열이며 별도 기능으로 만들지 않는다. BAML은 0139의 `area-enter-documentinsert-baml-20261019.json` offset6803을 재사용한다.

**활성 수식에서 Ctrl+Enter가 항상 새 페이지 나누기를 만드는 것은 아니다.** Worksheet `HandleControlEnter 0x06000a40`은 active/selected item이 없으면 InsertPageBreak, active item이 있으면 그 item의 선택을 toggle하며, 나머지 selected 경로도 별도 선택 처리다. 리본 `NewPageBreak`는 `SetupCommandBindings 0x06000adf` IL053c–0559 → action `0x06000b8e` → InsertPageBreak이며 predicate `0x06000b8f`는 `CanCreateControlOfType<IWorksheetPageBreak>`를 사용한다. Area 등 금지 컨테이너에는 0138의 기존 exclusion 규칙을 적용한다.

native XML은 worksheet namespace의 **일반 root region 안에 빈 `<pageBreak/>` payload**이다. worksheet50 XSD pageBreak는 빈 complexType이며 NextPage 같은 속성이 없다. 원본 control `GetRegionPersistentData 0x06000816`도 `worksheetRegionTypePageBreak`의 새 인스턴스를 반환한다. `RegionFactory.ConvertFromIRegionType 0x06000326` IL01d4–01e5는 이 타입을 WorksheetPageBreak/control persistent data로 복원한다. 일반 RegionFactory0325가 region ID·control 크기·actual 크기를 저장한다.

변경하지 않은 원본 `Default Templates/en-US/Presentation.mctx`를 ignored `artifacts/rea/page-break-shipped-fixture-20261019.mctx`에 복사하고 worksheet XML과 출처 해시를 동명 `.xml`/`.json`에 보존했다. 실제 두 pageBreak region은 각각 ID6/7, left0, top710.40000000000009/1420.8000000000002, height2, actualHeight2, actualWidth1056이다. 이 값들은 해당 템플릿의 관찰값이며 모든 용지의 고정 너비·페이지높이가 아니다. control ctor0810은 Height=2.0을 설정한다. payload에 임의 `nextPage`/`breakY` 속성을 추가하면 원본 스키마와 달라진다.

## 삽입은 뒤쪽 region의 좌표도 바꾼다

`InsertPageBreak 0x06000a61/0x06000b00`은 현재 insertion point로 `PageBreakCommand`를 만든다. 핵심은 생성자 `0x06000eb0`과 `InsertThePageBreak 0x06000eb4`다. 페이지 나누기를 현재 Y에 선으로만 추가하고 인쇄 시에만 강제 개행하는 계약이 아니다.

다음은 교차하는 page-wide item이 없는 일반 빈 삽입점의 확인된 전이다. H는 PageBodyHeight, g는 grid height, S=(x,y)는 현재 worksheet 본문 좌표다.

1. Worksheet GetPageIndex `0x06000ad8`은 `trunc((y+g/2)/H)`를 사용한다(H=0 예외는0). 새로운 삽입점과 break 위치 B는 `(0,(pageIndex+1)*H)`다.
2. 이동 대상은 `item.top>y`, 또는 `item.top==y && item.left>=x`인 worksheet item이다. 이 Y/X 비교는 DoubleComparer를 사용한다(`0x06000eb6`). 같은 줄에서 삽입점보다 왼쪽에 있는 영역까지 무조건 이동하지 않는다.
3. 기존 뒤쪽 pageBreak가 없으면 이 item들을 Y방향으로 `B.y-y`만큼 옮긴다.
4. 기존 뒤쪽 pageBreak가 있으면 가장 가까운 break 전 item들만 위 delta를 적용하고, 그 break 이후 item들은 H 전체만큼 옮기는 별도 MoveItemsCommand를 만든다. 다음 break 기준 분할은 `0x06000ffb`, 후속 break의 Y와 NextPage 조정은 `0x06000eab`에서 확인된다.
5. 새 break를 worksheet content의 `(0,B.y)`에 추가하고 insertion point를 B로 옮긴다. 후속 break들은 한 body page 앞으로 이동한다. NextPage는 control의 runtime field이며 native payload가 아니다.

삽입점이 page-wide item의 내부 높이와 겹치면 생성자 초반은 해당 item의 rect bottom을 사용하여 목표 페이지를 정한다. 이 조정은 단순 math-only 양성보다 복잡하다. 원본은 page-wide item을 중간에서 자르는 대신 별도 처리를 한다는 것은 확인했지만, 모든 겹침·동일좌표·tall Area 경계의 최종 레이아웃을 이번 기록만으로 완전 지원했다고 주장하지 않는다.

Undo `0x06000eb2`는 두 MoveItemsCommand를 Undo하고 새 break를 제거한 뒤 원래 insertion point를 복원하고 후속 break 위치를 되돌린다. Redo `0x06000eb3`는 동일 InsertThePageBreak를 재실행한다. 영역 이동과 break 추가를 별도 사용자 Undo 단계로 나누지 않는 단일 PageBreakCommand 계약이다.

## 삭제는 공간 제거 명령이다

도움말의 양성은 break 바로 위에서 Delete, 바로 아래에서 Backspace, 또는 Remove Space → Empty Space다. Worksheet `HandleDelete 0x06000a3c`/`HandleBackspace 0x06000a3d`는 우선 selected item 삭제를 시도한다. 선택이 없을 때 `CanDeleteVerticalSpace`를 확인하고 각각 `RemoveSpaceCommand(viewModel,1,Below=3,null)` 또는 Above=2를 생성한다. 수식 내부 문자 삭제와 worksheet crosshair 공간 제거를 섞으면 안 된다.

`RemoveSpaceCommand 0x06000e8c`는 처음에 -g offset을 만들고 그 범위를 지나는 break들을 찾는다. `GetPageBreaksToRemove 0x06000e91`/predicate `0x06000fe0`의 구간은 **upper < breakY <= lower**다. 가장 가까운 worksheet item을 이용하여 경계를 제한하는 추가 경로가 있다. 따라서 임의 근처 break를 선택하는 tolerance를 발명하지 않는다.

한 칸 제거가 break를 지나는 경우 `GetOffsetForPageBreakRemoval 0x06000e92`로 이동량을 바꾼다. reference point보다 위에 시작하는 item들의 최대 bottom(render height 및 VerticalAlignmentOffset 포함)을 구해 `max(-H, maxBottom-referenceY)`를 사용하며, 그러한 item이 없으면 -H다. 원본은 항상 H 전체를 위로 당기는 것도, break marker만 지우는 것도 아니다. 실제 moved-item 집합은 Below에서 `top>referenceY`, Above에서 `top>=referenceY`로 다르다(`0x06000e97/0x06000e98`). 다음 break가 있으면 그 전후 이동과 후속 break의 page count 보정도 분리한다.

`RemoveSpace 0x06000e90`은 MoveItemsCommand들을 수행하고 break collection에서 범위 삭제한 뒤 insertion point·empty pages를 조정한다. Undo `0x06000e8e`는 후속 break를 복원하고 removed break들을 AddRange하고 두 이동을 Undo하며 원래 insertion point를 복원한다. 공간 제거의 모든 충돌 방지 조건은 CanDeleteVerticalSpace/LimitOffsetBasedOnItems에 더 있으므로 단순 위 식만으로 혼잡한 문서의 전체 삭제 계약을 대체하지 않는다.

## 페이지와 출력 좌표

페이지 나누기 명령이 region top을 body-page 경계로 옮기므로, 일반적인 body-Y→physical-page 출력 mapping을 유지하면서 저장 좌표를 재배치하는 접근은 위 근거와 일치한다. `WorksheetPageManager.GetPageIndex 0x06000919`도 PageBodyHeight를 기준으로 페이지를 판별하며 grid 반 칸의 경계 허용 분기가 있다. 페이지 나누기마다 별도 가상 logical-Y 단절을 삽입한다는 근거는 이번 경로에 없다.

표시선은 native top과 별개다. `SetPagebreakVisualOffset 0x06000b03`은 `DisplayHeight*pageIndex-(DisplayHeight-PageHeight)/2-2*breakCollectionIndex`를 VerticalPosition으로 설정하고, control setter0812가 TranslateTransform으로 적용한다. 이 값을 native top으로 저장하면 안 된다. 출력은 [0033](0033-print-and-page-contract.md)의 페이지 snapshot 경로와 연결되며, 이번 조사로 break 편집용 선 자체가 인쇄되는지 또는 정확히 어느 resource에서 숨겨지는지는 확정하지 않았다. 임의 점선·라벨을 PDF에 넣는 근거로 사용하지 않는다.

## 증거와 구현 범위

ignored `artifacts/rea/page-break-method-ledger-20261019.json`에 raw CIL·IL SHA·도움말 해시·해결한 MethodSpec을 기록했다. `feedback/page-break-*-cil-20261019.txt`는 보존 REA metadata의 어셈블리와 각 IL 본문을 설치 bytes와 재검증한 결과다. Worksheet identity는 SHA `c1a70ec8007228ff119d4a31b36b9062132be3a10c771459c1dbe3fd0a7baddd`, MVID `44f31187-dcfc-4afd-a7d6-50fbee8dc361`, REA `ev_ca6ee361e44f5b877e72b99111bddb37dcf24e4dd6163ce516a16f3ad2b2611b`다.

주요 IL SHA: PageBreakCommand ctorEB0=`ee48b0fd23e70c7ea408a4b9fd928231564d2fa3327c57f92a77bec1fb8b1446`, insertEB4=`4d987b2e4d5baaec2e7e2eba6316ac772aa4f556b640f153ad4db6b31244659e`, UndoEB2=`6de9ca3a81ade670dc81976f4e2de82ffa9a93054522f3c7e0a34e9a8f74fbde`, removeE90=`31a19ac9b04a83864c57af49ae2059e08b7a811e00ae748ac424aeadd39c7aa8`.

최소 구현 가능한 범위는 원본 형식의 pageBreak 읽기·저장, 빈 crosshair에서 첫 삽입과 단일 명령 Undo/Redo, 단순 배치의 삭제와 좌표 복원이다. 여러 break, 교차하는 page-wide 영역, 임의 drag 이동, 보호 영역·혼잡 배치의 전체 guard, 편집선 인쇄 여부는 위 양성과 구분하여 계속 조사한다.

## 독립 문서 트랜잭션 API

`crates/mathcraft-document/src/page_break.rs`는 중앙 모듈 등록·GUI 연결과 분리한 pure candidate API다. `insert_page_break`는 CanvasDocument, PageBreakLayout(body/grid height), 이미 검증한 빈 root crosshair, 각 non-break region의 live bounds를 받는다. bounds는 region identity별 정확히 한 개여야 하며 bottom은 render height·vertical offset을 포함한 절대 Y다. Ordinary/PageWide/Unsupported 분류는 호출자가 소유한다. source XML에서 live 크기를 추측하지 않는다.

반환 PageBreakChange는 전체 candidate 문서, 새 삽입점, Canvas break identity다. 호출자는 이를 한 문서 Undo로 적용한다. 기존 Canvas left/top이 source XML의 오래된 좌표보다 우선하며 이 모듈은 preserved source·metadata·원본 archive를 변경하지 않는다. exporter가 나중에 Canvas 좌표와 package-global ID를 적용해야 한다. 새 owner는 region-id0/source_regionNone의 임시 정체성, height2와 빈 pageBreak payload를 가지며 package ID로 가장하지 않는다.

`remove_single_page_break`는 break 하나, 명시 방향·reference point, live bounds의 제한된 양성만 받는다. 전체 CanDeleteVerticalSpace를 구현했다고 주장하지 않으며 교차·충돌·비인접·다중 break는 거절한다. candidate는 입력을 mutate하지 않으므로 거절해도 source/undo/캐시를 손상하지 않는다. saved presentation은 유지하지만 기존 source signature 검증이 위치 변경의 freshness를 판별한다.

추가 설치 확인: Ptc.DoubleComparer 기본 epsilon은 1e-10이고 abs(diff)<epsilon에서만 같다고 본다(core0116/0117/011f). DoubleExtension 기본 epsilon도 1e-10(core026e)이다. 세부 IL은 위 ledger에 추가했다. 모듈의 Y/X 동률 비교는 이 comparer를 따르며 임의 픽셀 tolerance를 도입하지 않는다.

## 표시선과 인쇄 제외 후속 확인

후속 정적 추적으로 앞 절의 편집선 출력 여부 한계를 좁혔다. 원본 페이지 나누기 control은 일반 worksheet 수식·텍스트 item과 다른 collection에 들어가며, 일반 인쇄 snapshot이 그 collection을 복사하지 않는다는 양성이 확인됐다.

* `WorksheetModel.AddItemAtLocation 0x06000807` IL0001은 `IWorksheetPageBreak`(TypeRef65)를 먼저 검사한다. 일치하면 worksheet location을 설정하고 **PageBreaks** collection에 추가한 뒤 IL0022에서 반환한다. 그 뒤의 일반 `IWhiteboardItem → WhiteboardItems` 삽입 분기를 통과하지 않는다. IL SHA=`c0fb7d989daaaf5c8feecbe941feb09ee082a2ca9daa79cc99bb151a12ad879a`.
* 설치 `worksheetcontrol.baml`에는 별도 `PageBreaksControl`(offset2616, ItemsSource path PageBreaks2647)과 `WorksheetItemsCanvasControl`(2739, path WorksheetItems2780)이 sibling으로 있다. 페이지 나누기 control을 일반 math/text canvas child로 표시하는 binding이 아니다.
* `WorksheetControl.GetPageSnapshot 04a4 → WorksheetViewModel.GetPageSnapshot09f4 → WorksheetVisualHelper.CreatePageSnapshot0c46`의 snapshot은 새 PageSnapshot을 만들고 header/footer, watermark, clip을 설정한 뒤 **원본 Page.WhiteboardItems만** 순회한다(IL00fb–0106). 각 item의 print view를 새 PageBody.WhiteboardItems에 추가한다(0234–023b). PageBreaks collection을 복사하는 분기는 없다. `CreatePageSnapshot` IL SHA=`f77e4cee1dfb7f5e3124f6adf6a63fe7bb4364d0568fdc6e8a13e043b3777fb0`.
* 기존 EngineeringDocument0133의 GetPageSnapshot → EngineeringPages.AddPage 경로와 연결하면, **기본 worksheet 인쇄 snapshot에는 편집용 pageBreak 선이 포함되지 않는다**는 결론이다. 원본 프린터를 실행하거나 PDF pixels를 관찰한 것은 아니며 모든 서드파티 print extension까지 부재를 증명한 것은 아니다. Mathcraft의 기본 vector PDF에 페이지 나누기 점선을 넣지 않는 근거로 사용할 수 있다.

표시선의 실제 template도 설치 satellite resource에서 확인했다. `worksheetresources.baml` offset22476은 WorksheetPageBreak ControlTemplate이며 Line(known type350)을 가진다. IsHitTestVisible=false22493, StrokeThickness=4(known property102)22516, StrokeDashArray=2,2(22524), VerticalAlignment=Center22534, Width는 WorksheetControl ancestor의 PageLayout.PageWidth에 bind된다(22554–22588). style의 TargetType은 같은 WorksheetPageBreak22629이며 WorksheetPageLayoutMode=DraftMode일 때 Visibility=Hidden22783이다. 즉 DraftMode에서 편집선 자체를 숨기는 원본 경로가 있다. 직선 brush는 custom BAML serialized payload로 보존했으며 이번 범위에서는 brush serializer의 색 해석까지 확정하지 않는다.

`HandlePagebreaksChanged0b02`는 Add collection 이벤트의 새 break들에 SetPagebreakVisualOffset을 호출한다. 이동 명령 eab도 해당 offset을 다시 설정한다. offset은 **XAML binding이 아니라 CIL의 명시 setter**이며0812가 RenderTransform=(0,VerticalPosition)을 적용한다. 확정 식은 앞 절과 같다. control Height2와 별도 ItemsControl 배치를 고려하면 식의 -2*collectionIndex가 누적 배치 상쇄라는 해석이 가능하지만, 그 의도를 원본 주석으로 증명한 것은 아니다. 절대 좌표 canvas에 paint할 때 이 transform 값에 native top을 무조건 다시 더하거나, 원본 panel 배치를 재현하지 않은 상태에서 -2*index를 중복 적용하는 것은 근거가 없다.

원시 stream은 `artifacts/rea/page-break-visual-{worksheetresources,worksheetcontrol,worksheetpagebreak}.baml`, 상세 decoded records는 `page-break-visual-*-detailed-20261019.json`에 있다. 새 decoder는 기존 decoder에서 빠졌던 PropertyTypeReference와 custom payload 원시 bytes도 보존한다. WPF known type/property 번호는 설치 PresentationFramework.dll의 enum constants를 정적으로 확인하여 `page-break-known-wpf-20261019.json`에 저장했다. satellite 및 stream SHA, 추가 print/model 메서드 SHA는 기존 page-break ledger에 추가했다. 제품 코드는 변경하지 않았다.
