# XY graph-size와 region 크기의 분리 계약

2026-10-19. 설치 Prime12.0.0.1 CIL/REA metadata와 원본 embedded XSD를 정적으로 조사했다. GUI·스크린샷·원본 runtime·CLR은 실행하지 않았고 제품 Rust 파일은 수정하지 않았다. [0161](0161-native-xy-expression-layout-contract.md)의 식 목록 측정/배치, [0163](0163-native-empty-and-partial-plot-contract.md)의 native 위치 필드를 잇는다.

## 서로 다른 네 가지 크기

| Native/source 값 | 원본 작성/복원 경로 | 의미 |
| --- | --- | --- |
| owning region @width/@height | RegionFactory0325의 FrameworkElement.Width/Height, RegionPersistentData02f1/02f0의 specified 검사 | 바깥 plot control의 명시 크기 |
| owning region @actualWidth/@actualHeight |0325의 FrameworkElement.ActualWidth/ActualHeight | 바깥 region의 측정 snapshot |
| plot/xyPlot/graph-size @width/@height | PlotModel.GraphSize getter1156/setter1157, native writer1154/reader1155 | 내부 graph control의 별도 크기; import 초기 constraint와 이후 actual snapshot 역할 |
| 축/legend의 실제 크기·legendWidth | 식 desired size, table columns, margin/padding, native axisLine/@legendWidth | 내부 graph와 outer region 사이 여백/점유 공간에 참여하는 독립 측정 |

XSD는 region width/height를 optional pageUnitType으로, actualWidth/actualHeight를 optional xs:double로 구분한다. graph-size는 optional SizeType이며 **존재할 때 width/height 두 xs:double attr가 required**다(worksheet50.xsd1242–1244). graph-size에 fixed50 여백, region과의 비율, 동일값 조건은 없다. XSD의 double 허용 범위 자체를 앱 finite-positive 지원 범위와 혼동하지 않는다.

Mathcraft의 기존 `graph-height = spec.height - 50`은 이전 UI의 독립 정책이지 원본에서 회수한 등식이 아니다. 좌/우 legend, 다중 식, 단위 칸, tick/title, font가 바뀌면 외곽과 내부의 차이가 달라진다.

## 바깥 region의 import/export

PlotRegionPersistentData.Deserialize02ff는 HideFromCalculationContext를 설정한 뒤 **DeserializeDimensions를 먼저**, 그 다음 공통 Deserialize02ee를 실행한다. DeserializeDimensions02f2는 Height02f0, Width02f1 순서다. 각각 region heightSpecified/widthSpecified일 때만 FrameworkElement.Height/Width를 설정한다. 값이 없으면 해당 property를 변경하지 않는다. 이 경로에는 actualHeight/actualWidth를 대체 요청 크기로 쓰는 branch가 없다.

공통 Deserialize02ee는 위치/region 공통 속성을 적용하고 payload의 SetRegionPersistentData를 호출한다. PlotControl.SetRegionPersistentData02bd가 payload를 model.PlotXmlNode에 전달하여 graph-size와 축/목록 정보를 읽은 뒤 SetGraphSize(true)를 호출한다. 따라서 source 적용의 양성 순서는 **outer 요청 dimensions → plot payload/model graph-size → graph 초기화 요청**이다. 이 순서와 WPF의 비동기 measure 이벤트 전체 시간순서는 구분한다.

RegionFactory.ConvertToIRegionType0325는 payload GetRegionPersistentData를 받은 후 크기용 FrameworkElement를 선택한다. Width/Height가 NaN이 아니면 region width/height를 기록하고 ActualWidth/ActualHeight는 measured attrs로 별도로 기록한다. GetFrameworkElementForDimensionsSerialization02ef는 IResizableElementInfo가 명시적으로 별도 dimension element를 제공하면 그 element를 선택하고, 그렇지 않으면 control을 선택한다.

XYPlotControl → PlotControl → PlotControlBase → WPF ContentControl의 설치 TypeDef/InterfaceImpl를 확인했다. 이 chain의 자체 interfaces에는 IResizableElementInfo가 없으므로 일반 XY control의 이 writer 경로는 outer control을 기록한다. 식/legend 자체 drag 같은 별도 target까지 모든 resize gesture가 같은 target이라는 뜻은 아니다.

FrameworkElementFacade.SetSize0765도 선택한 resizable FrameworkElement의 Width/Height를 설정하고 ContentChanged를 발생시킨다. GetResizableElement0775는 위 interface가 없으면 원래 element를 그대로 반환한다. 일반 plot region의 size 변경이 model.GraphSize 한 필드만 직접 바꾸는 계약은 아니다.

## graph-size reader와 임시 축 고정

PlotModel.set_GraphRegionXmlNode1155는 graph-size가 있으면 InvariantCulture Double.Parse로 width/height를 읽어 model.GraphSize에 넣는다. 두 값이 모두 NegativeInfinity인 경우 Size.Empty를 사용하고 graph-size가 없을 때도 Size.Empty다. 이 sentinel을 유한한0 크기로 변환하거나 미지정과 동일하게 저장하지 않는다. reader의 다른 음수/비유한 Size constructor 거절 범위를 실제 원본을 실행해 조사하지 않았으므로 bounded codec은 지원 가능한 숫자만 받아야 한다.

PlotControl.SetGraphSize02ba의 일반 non-Mathcad14 경로는 deferred `_postInitializeResizeEventsCallback`을 저장하고 false를 반환한다. XY override01a4는 그 결과가false이면 FixateAxesSizeAtDeserialization01a5를 실행한다. saved graph width/height가 둘 다 양수일 때 다음을 수행한다.

* X axis.FixateSize(saved graph **height**)
* Y axis.FixateSize(saved graph **width**)

X axis가 horizontal인데 height를 받는 것이 전사 오류는 아니다. FixateSizeOfRowsInner0561은 axis orientation과 splitter/grid를 이용해 **perpendicular graph 길이의 두 grid row/column**을 pixel GridLength로 고정한다. tick-relative position에 따라 길이를 둘로 분할하고 SizeFrozen=true로 둔다. graph와 legend의 실제 grid layout을 복원하기 위한 임시 제약이다. raw width/height를 단순히 outer control에 다시 대입하는 경로가 아니다.

XY EndInit01a1은 load 시 graph ActualWidth/Height가 양수이면 다시 이 고정 helper를 요청한다. template/splitter가 아직 없을 때 FixateSize055e는 saved length만 보관하고, 준비된 후0561이 적용한다. 따라서 한 source setter의 즉시 반환만 보고 최종 measured geometry를 확정하지 않는다.

## load 후 actual snapshot과 제약 해제

PlotControlLoaded01bb는 InitializeResizeEventsAfterLoading01be를 호출한다. 01be는 graph placeholder SizeChanged→GraphSizeChanged01d9, outer SizeChanged→PlotControlSizeChanged01dd, LayoutUpdated→01da, X/Y 식 목록의 SizeChanged 등 실제 이벤트를 연결한다. graph placeholder가 아직 없으면 template 적용 뒤 연결할 callback을 보관한다.

01be의 model.GraphSize 갱신은 다음과 같다.

1. saved GraphSize가 Size.Empty면 실제 `Graph.ActualWidth/ActualHeight`를 model에 저장한다.
2. 일반 non-Mathcad14 문서에서 saved값과 graph actual이 tolerance1e−5 내에서 같으면 유지한다. 다르면 **actual**로 model.GraphSize를 갱신한다.
3. MC14 converted 문서에는 별도 branch가 있으며 saved zero/inconsistent 상태와 ForceMc14GraphSize를 처리한다. 일반 native 파일의 크기 정책으로 이 변환 branch를 사용하지 않는다.

XY.PostInitializeResizeEventsAfterLoading01a2는 ReleaseAxisSizes01a3 후 base01bf를 호출한다. frozen axes는 ReleaseAutoReSize0563으로 grid pixel lengths를 같은 비중의 star lengths로 바꾸고 SizeFrozen=false로 해제한다. base01bf는 axis/plot legend의 SetLegendWidthWhenLayoutReady를 호출한 뒤 deferred graph-size callback14e0을 실행하고 callback을 지운다.

14e0은 measure를 invalidate하고 현재 outer Width/Height를 `_referenceSizeForPostUnfixate`에, 호출의 numCells를 `_numCells`에 저장한다. OnLayoutUpdated01da는 이 기준과 현재 outer값 차이가 numCells×GridCellSize보다 커지면 기준 크기로 돌리는 보호 branch를 갖고 마지막에 reference를 해제한다. 따라서 load는 saved graph를 영구적인 hard request로만 쓰지 않으며, source outer request를 완전히 무시하는 방식도 아니다. 이 임시 제약·목록폭·실측·외곽 보정의 WPF 이벤트 순환을 모두 실행 관찰한 것은 아니므로, 충돌하는 source 두 크기에서 어느 값이 최종 픽셀에 항상 승리한다는 단일 규칙은 미확정이다.

## 편집 후 다시 저장되는 값

GraphSizeChanged01d9는 graph size의 previous/new delta를 계산하고 `_plotSizeChanging`, `_resizeInProgress`, 식 visibility 상태에 따라 outer Width/Height를 보정한다. 메서드 마지막은 **현재 Graph.ActualWidth/ActualHeight**를 model.GraphSize에 저장한다. 목록 크기 변경의 overflow 보정과 graph 최소 크기 강제는0161을 따른다. graph actual/placeholder actual과 outer actual은 같은 개념이 아니다.

PlotModel.GraphSize1156/1157 자체는 단순 backing field다. native writer1154는 이 model 값을 InvariantCulture 문자열로 graph-size에 기록한다. PlotControl.GetRegionPersistentData02be는 model.PlotXmlNode의 자식들을 저장 payload에 전달하므로 여기서 outer size나50픽셀 보정으로 graph-size를 재생성하지 않는다. 그 뒤 region wrapper writer0325는 별도로 outer Width/Height와 ActualWidth/ActualHeight를 쓴다.

따라서 편집·measure가 완료된 일반 경로에서는 내부 graph actual snapshot과 outer requested/measured snapshot이 각각 다시 저장된다. size edit 직후 아직 layout이 수렴하지 않은 순간, font 변경/resize drag 도중 snapshot의 정확한 event-order까지 재현했다는 주장은 하지 않는다.

## MC14와 XAML 별도 경로

SetGraphSize02ba는 ConvertedFromMathcad14일 때 XY saved graph 길이를0.8333333333333334로 나누는 별도 변환을 갖는다. ForceMc14GraphSize02bb는 saved graph와 actual graph/outer request의 차이로 outer Width/Height를 조정한다. schema history에도 graph-size가 MC14 conversion 관련으로 추가되었다고 기록돼 있지만 현재 일반 native writer도 이 element를 쓴다. legacy비율을 모든 native plot에 적용하지 않는다.

PlotGraphSizeForXamlSerialization02ab/02ac는 model.GraphSize getter/setter 뒤 가상 SetGraphSizeForXamlSerialization02ad를 호출한다. base02ad는 no-op이다. 이 XAML용 property와 MCDX의 region dimensions 적용 순서를 같은 setter 하나로 합치지 않는다. 이번 scope에서는 기본 XY의 native package 경로를 우선하고 다른 plot type/XAML designer 복원 전체는 미확정으로 남긴다.

## 모델/codec에 필요한 분리 — 구현 제안

원본 source를 잃지 않으려면 적어도 **outer geometry와 optional native graph geometry**를 독립적으로 표현해야 한다. 이 절은 제품 구현이 아닌 제안이다.

* 기존 width/height를 outer region 의미로 유지한다면 imported native graph-size를 별도 optional `graph_size`로 보존한다. 기존 JSON에서 필드가 없는 legacy 정책과 새 native graph source를 구분한다.
* 원본 owning width/height의 미지정 여부와 actual snapshot도 필요하면 별도 carrier에 유지한다. actual을 요청 크기로 자동 승격하거나 graph를 outer로 이름만 바꿔 저장하지 않는다.
* unchanged native source는 원래 두 크기를 보존한다. 새 graph-size를 쓰려면 앱이 실제로 렌더링한 graph rect와 outer rect를 구분해 얻어야 한다. 기존 height−50을 원본 geometry라고 인증하지 않는다.
* source에 graph-size가 없으면 그 부재를 보존할 수 있어야 한다. UI 표시용 derived size는 독립 layout 정책이며 native source shape와 같은 권위로 저장하지 않는다.
* plot region resize, graph 영역 resize, legend 폭/위치 변경의 command는 영향을 주는 geometry 필드를 명시한다. 모든 편집을 숫자 한 쌍 변경으로 숨기지 않는다. 정확한 원본 drag/Undo 경계는 이번 read-only 조사에서 구현하지 않았다.

최소한의 양성은 separate source values·reader 적용 순서·실측 갱신·별도 writer다. graph-size/outer 두 source가 불가능한 조합일 때의 WPF 최종 수렴, 모든 label/tick/font 측정, 모든 drag/aspect ratio 제약은 미확정이다. 이를 이유로 imported unknown geometry를 조용히 버리는 것도 원본 보존 계약과 다르다.

## 증거

새 REA deferred callback/linear-axis=`ev_f372f514816ef96bd605b7c7ff1c9575a54ce09487a19a37f035cd9d43b7844b`. Plot/XY/model/Worksheet/Core facade는 기존 REA identity를 재사용하고 실제 설치 bytes의 선택 CIL SHA를 재검증했다. Plots.dll SHA=`4e5067e58e69c68e8152ce677d5ac77a6587e00f6295bda5cc8ddff18f6685eb`, MVID=`cecd333a-bf3e-4423-b74b-7ff799618db1`.

Worksheet.dll SHA=`c1a70ec8007228ff119d4a31b36b9062132be3a10c771459c1dbe3fd0a7baddd`, Controls.Core.dll SHA=`654a0252f699c9c9763aa4f4102488279500ed89cb8ce285a38476c2dd24a977`. 아래는 핵심 branch의 설치 raw CIL 위치다. 전체59개 method의 SHA는 ledger에 있다.

| Assembly/token | CIL 위치 | Raw IL SHA256 |
| --- | --- | --- |
| Worksheet02ff |0007 dimensions,000e common/payload deserialize|`2271e252ead953a190767eaebac7816f90b29ebc512781c147014c2bc83f6de4`|
| Worksheet0325 |00eb–011c requested Height/Width,0121–0144 ActualHeight/Width|`3e0211b264238f34d20c6e913064a75152c7a7c1e6c02f23c834d5037dd911cc`|
| Plots01a5 |003c–0073 saved graph dimensions→axis FixateSize|`d84a98da1421190ff04ce54f778ffd24a29746b8e5bbfc8a0447f4a12f2f5b6f`|
| Plots01be |00bb–0169 empty/mismatch→actual model size,01de post-init|`60d018c5a2152b6c2d23efe5793e52afb091ae31c0909708321559bec2d05a4c`|
| Plots01d9 |019b–01bc Graph.ActualWidth/Height→model.GraphSize|`5903654f64b0dfae4ef0da55ca766f8467c03519a2933a2c42465f2936d8720a`|
| Plots1154 |0132–0195 graph-size element/model dimensions writer|`1bfda8e41e5fe0414dc16dfb1c96e12c9b03ae4498e429d97368039d2325480f`|

ignored `native-xy-geometry-method-ledger-20261019.json`에 assembly/token/IL SHA와 schema/interface 관찰을 보존했다. 원시 본문은 `feedback/native-xy-geometry-*-cil-20261019.txt`와 재사용0161/0163 ledger에 있다. 초기 exploratory whiteboard의 다른 동명 token 파일은 worksheet geometry 증거로 사용하지 않았다. 제품 소스·원본 리소스·런타임은 변경하지 않았다.
