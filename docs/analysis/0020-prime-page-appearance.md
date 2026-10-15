# Prime 12 페이지·영역 표시의 정적 근거

2026-10-15. GUI, 스크린샷, CLR 로드, COM, 원본 실행 없이 배포 DLL의 REA 메타데이터·CIL과 템플릿 ZIP XML을 조사했다. 아래 값은 이 빌드의 정적 관찰이다. 모든 테마·문서·확대율에서 보이는 최종 픽셀을 관찰했다는 뜻은 아니다.

## 증거 식별

| 표기 | 원본 / SHA-256 | REA Evidence |
| --- | --- | --- |
| K | Ptc.Controls.Core.dll / `654a0252f699c9c9763aa4f4102488279500ed89cb8ce285a38476c2dd24a977` | `ev_f208e991f9e1e1e33663525464eb86305e6bee7352187fed8011d74326426e64` |
| W | Ptc.Controls.Worksheet.dll / `c1a70ec8007228ff119d4a31b36b9062132be3a10c771459c1dbe3fd0a7baddd` | `ev_ca6ee361e44f5b877e72b99111bddb37dcf24e4dd6163ce516a16f3ad2b2611b` |
| B | Ptc.Controls.Whiteboard.dll / `947ae0dea0c7a7fdbd1c377e4e7f2447dccd5e839111fec0c0789dafffc1cbd9` | `ev_b5601734c03d7bc9573770a236f1d7d0f7c2f9203276c86b4dff938362dc44ab` |

K의 MVID는 `b5e8e4fd-c134-4713-b1b1-eea8cddcdd9f`다. 이번 K의 네 타입 집중 재검사는 `ev_fb568a82f2a8cbf7c8bad2c295413fe20cd07de8a44ae29e775e512b8cc82f53`이며 그 summary view는 `ev_89e37aa97af03c9e894ea6e36883d95eb829692f3d178af44a89dfbc3556e029`다. 네 타입 213개 메서드 범위이며 어셈블리 전체의 실행 검사가 아니다.

이전 보조 디코딩은 ignored `artifacts/rea/document-deep/grid-supplemental-il.json`에 있다. 이번 `artifacts/rea/prime-appearance/page-state-verified.json`은 10개 메서드를 dncil로 다시 디코딩하고 원본 파일 SHA 및 각 raw CIL SHA를 REA 결과와 비교해 일치를 확인했다. 대상 코드는 실행하지 않았다. 보조 뷰의 instruction offset은 IL 기준이고 dncil의 branch operand는 header 포함 좌표를 유지하므로 raw branch 숫자를 그대로 혼용하지 않는다.

## 단위와 종이

K `WorksheetPageLayout.SetupDefaultPageSizeInformation` (`0x0600087e`, raw IL SHA `6a9214639edd4b6db96a34b0c15981074f0738e0cfceab539d993d8f32a147bc`)는 A4를 21×29.7 cm, Letter를 8.5×11 inch로 등록한다. WPF 변환은 cm×37.79527559055118, inch×96이다. 따라서 세로 종이 크기는 다음과 같다.

| 종이 | 폭×높이 DIP | Standard grid | Fine grid |
| --- | --- | --- | --- |
| A4 | 약 793.7007874×1122.5196850 | 0.5 cm ≈18.8976378 DIP | Standard÷2 |
| Letter | 816×1056 | 0.2 inch ≈19.2 DIP | Standard÷2 |

이 단위를 print point로 다루면 안 된다. 1 inch=96 DIP와 72 print points의 구분이 필요하다. 화면 DPI와 zoom도 문서 좌표와 별도다. 특히 US grid는 표시 문자열 19.2 대신 원본의 연산 `0.2*96.0`을 보존하면 한 ULP 차이를 피할 수 있다.

`GetDefaultPageSizeForLocale` (`K 0x0600082d`, raw SHA `2111bbcf27711e0fbff9d51c5be39470b1e039e2d05e990a00df80a76f97bc58`)는 A4로 시작하고 현재 thread **CurrentUICulture LCID**가 1033, 2058, 3084, 4105, 8202, 9226, 11274, 13321, 13322일 때 Letter로 바꾼다. Korean LCID 1042는 A4 경로다. OS 지역 설정이라는 넓은 표현보다 UI culture라는 정확한 구분이 필요하다. 저장 문서·템플릿의 paper 설정은 별도로 적용된다.

실제 `Default Templates/en-US/Blank Worksheet.mctx` SHA `bfb7df488665071536d610884ffb4e3ee3d03f81c7294fe2c658b2c2a24eac8e`의 `mathcad/settings/presentation.xml`에는 Letter, Standard grid, Portrait, Standard margin이 명시돼 있다. 이는 en-US 템플릿의 설정이며 Korean 사용자나 모든 새 문서의 보편적 기본값이 아니다.

## 여백과 본문

K `WorksheetPageLayout` 생성자 `0x0600082e`는 MarginType 1(Standard)을 선택한다. Standard metric margin `0x06000888`은 모든 방향 2.5 cm≈94.488189 DIP, US margin `0x06000889`는 모든 방향 1 inch=96 DIP다. Narrow는 Standard×0.5 (`0x06000890`), Wide는 ×2 (`0x0600088f`)다. 임의 사용자 여백은 별도 정책이다.

`set_PageWidth` (`0x06000844`)와 `set_PageMargin` (`0x06000846`)은 본문 폭을 `paperWidth-leftMargin-rightMargin`으로 계산한다. `set_PageHeight` (`0x0600083e`)는 본문 높이 후보를 `paperHeight-topMargin-bottomMargin`으로 전달한다. **본문 높이는 단순한 뺄셈 결과가 아니다.** `set_PageBodyHeight` (`0x0600083c`, raw SHA `589b0a4a762142a44e4bfc0d2804e12c8fa2cbf3e3aa90ebc6339390437a8925`)는 `Math.Round(candidate/gridY)*gridY`로 칸 수를 맞춘다. 폭 setter `0x0600083a`는 이런 반올림을 하지 않는다.

`set_PageHeight`는 `DisplayHeight=paperHeight+gridY`도 설정한다(raw SHA `f8c5f41694120077600cd2a05be44753fa1e35be41e6e9d5091d546b6c415da9`). 따라서 페이지 표시 간격에 임의의 큰 카드 간격을 넣기보다 원본의 display-height 경로를 먼저 맞춰야 한다. 실제 items panel이 이 높이를 쓰는 최종 바인딩은 BAML 조사와 결합해야 한다.

`get_InitialInsertionPoint` (`0x06000867`)는 종이 종류의 값을 가져온다. metric/US getter `0x06000893/894`는 **(0, Standard grid height)**를 만든다. Fine을 선택했다고 이 getter가 Fine 간격을 쓰는 구조는 아니다. 좌표는 기존 문서 조사에서 확인한 whiteboard 위치와 함께 다뤄야 한다. 원본 표본에는 left=0인 일반 본문 영역이 존재하고 본문 폭/머리글/바닥글이 분리돼 있으므로, region 좌표를 종이 외곽 기준으로 취급하면 원본 배치를 맞추기 어렵다. 아래 BAML 후속 근거로 일반 PageView 본문이 종이의 왼쪽·위 여백 뒤에 놓이는 것을 확인했다. nested Area 및 여러 페이지의 worksheet 좌표를 종이 좌표로 변환하는 전체 경로까지 같은 식으로 일반화하지 않는다.

## 머리글·바닥글

K getter `0x0600088b–8e`의 Thickness(left,top,right,bottom): metric은 `(0.5,0.25,0.5,0.25)` cm, US는 `(0.25,0.125,0.25,0.125)` inch다. 각각 DIP로 변환한다. FooterMargin은 HeaderMargin getter를 재사용한다. 이는 종이의 본문 여백과 다른 값이다.

`set_PageMargin`은 HeaderHeight=pageMargin.Top, FooterHeight=pageMargin.Bottom을 설정하고, `set_PageWidth`는 HeaderWidth와 FooterWidth를 paperWidth로 설정한다. 머리글 내부 body의 폭/높이는 다시 header/footer margin을 반영한다. HeaderFooterGridCellSize는 Standard grid의 절반 (`0x06000882/885`)이며 본문의 Standard/Fine 선택과 구분된다.

W `WorksheetPageControl`/`WorksheetPageViewModel`은 일반/첫 페이지 header/footer와 각 zombie 상태, HasPageBreak, NonPrintableSpaceIndicator를 각각 가진다. pageBreak를 단순 수평선이나 추가 top padding으로만 재구성했다고 주장할 수 없다. 실제 pageBreak pagination과 clip/print의 전체 전이는 아직 미확정이다.

## 배경·그리드·표시 기본 상태

K `BackgroundTypeColors..cctor` (`0x06000146`, raw SHA `a60352a20019c1cd4d3ee08968878201e9ddd7076a39605c45ccfea7a864974d`)는 Paper=`#FFF6FAF6`, Gray=`#FFC0C0C0`를 만든다. `WorksheetPageLayout` 생성자는 Paper를 `_pageBackgroundColor`에 넣는다. **이 정적 Paper 값이 최종 화면 모든 부분의 배경이라고 단정하지 않는다.** 아래 BAML 후속 근거는 PageView의 영역 배경이 WorksheetControl.PageBackgroundColor에 바인딩됨을 보인다. 문서 값과 white region brush의 우선순위는 추가로 구분해야 한다. 인쇄 배경도 이 값만으로 확정하지 않는다.

W `WorksheetViewModel.SetupDefaultPageLayout` (`0x06000ade`, raw SHA `5ce67c276903514ebdf1eb5dd643470240afbc8b2965eba5fa949cb1bc741887`)의 초기 정책은 다음과 같다.

- DisplayGrid=true, DisplayFrame=false.
- DisplayHeaderFrame=false, DisplayFooterFrame=false.
- Header/Footer IsCrosshairVisible=false.
- WorksheetPageLayoutMode=0. enum의 이름은 이 기록만으로 새로 추측하지 않는다.
- ContentAreaWidth와 WidthWithNonprintableSpace는 PageBodyWidth로 시작한다.

초기 상태와 저장된 문서의 view 설정을 구분한다. 이 메서드 뒤의 load/apply 경로는 값들을 덮어쓸 수 있다. 화면의 모든 worksheet가 무조건 grid를 보인다는 주장은 아니다.

W `PrintGridSettings..cctor` (`0x060000b9`)는 major thickness1, minor0.8, major color`#FFBBDDFE`, minor`#FFDDEEFE`, frame Black을 만든다. DirectRenderingAlignmentGridBorder의 dependency metadata(`0x060000a8`)도 이 값을 초기값으로 참조하지만, 명칭 그대로 print 경로가 섞인 값이다. screen BAML override를 확인하지 않고 현재 제품의 화면 선 두께로 바로 고정하면 안 된다.

## 영역 테두리와 선택 상태

K `RegionBorderConstants..cctor` (`0x06000599`, raw SHA `2029775f010a6ade8730f02253da90d07f647155a7e76c70e2e4f77b6bd73c2a`)의 관찰값:

| 역할 | 값 |
| --- | --- |
| BorderBrush | ARGB(255,78,126,155), `#FF4E7E9B` |
| BorderLineStrokeDashArray | [8,4] |
| BorderCornerRadius | 1 DIP |
| ResizeDraggerTransparentZoneThickness | 7.5 DIP |
| SelectedDraggerBackground | LightCyan |
| SelectedBackgroundBrush | DodgerBlue |
| DraggersMarginThickness | -10 DIP |
| AlignmentPointMarginThickness | -5 DIP |

`RegionBorder.SetBorderLook` (`0x06000607`, raw SHA `d0b6f12e501332f89d45b3b16846692728ed520b43bedf9daa5248d910ca60a6`)는 투명 fill의 BorderShape, stroke thickness **0.5 DIP**를 구성한다. IsActive일 때만 dash collection을 주고 아니면 null을 준다. WPF dash 숫자는 StrokeThickness 단위이므로 8 DIP/4 DIP의 고정 길이라고 다시 해석하면 안 된다.

`SetBackgroundSelectedLook` (`0x06000604`)는 foreground opacity0.4, background opacity0으로 바꾼다. `SetBackgroundActiveLook` (`0x06000605`)는 foreground0, background=ActiveRegionBackgroundOpacity다. 그 dependency property 초기값은 **0.7** (`RegionBorder..cctor 0x060005b5 IL779`)이다. `SetBackgroundDefaultLook` (`0x06000606`)는 둘 다0이다. 이름의 foreground는 텍스트 자체를 의미한다고 확정할 수 없고 template overlay를 가리키므로 텍스트 불투명도를40%로 낮추는 구현으로 오독하지 않는다.

`UpdateBackgroundAndForeground` (`0x06000603`)는 IsActive를 먼저 분기하고 active에서 Control.Background가 SolidColorBrush.Transparent인지 검사한다. 활성/선택/기본 상태를 같은 강조 카드 하나로 합치면 원본 상태 모델을 잃는다. `UpdateBorderVisibilityState` (`0x06000608`)는 IsActive, IsSelected, ForceBorder, BorderVisibility, ShowBusy 및 리소스의 hidden visibility를 조합한다. inactive region마다 항상 사각 카드 테두리를 그린다는 근거는 없다.

Crosshair는 K `Ptc.Controls.Core.Crosshair.InitializeComponent` (`0x060001b3`)가 `/Ptc.Controls.Core;component/crosshair.xaml`을 로드하는 구조다. 아래 BAML 후속 조사에서 실제 선 길이·색을 확보했다. 마우스 resize 커서는 K에서 SizeWE/NS/NESW/NWSE/All 등의 system cursor를 참조한다. math editing caret의 색·모양은 EquationEditor의 별도 표시 경로와 구분해야 한다.

## 적용 경계와 남은 질문

페이지 종이와 본문을 분리하고, 실제 여백·표준 grid·초기 삽입 위치·얇은 활성 테두리를 사용하는 방향은 직접 정적 근거가 있다. 다만 다음 항목은 이 문서의 미확정 영역이다: 일반 페이지 사이 items-panel 배치, layoutmode0의 이름, pageBreak pagination·print clipping의 전체 경로, 사용자 DPI/zoom에서의 최종 픽셀 출력. 이 기록은 원본과의 외형 동일성 검증 결과가 아니다.


## 후속 BAML: PageView의 본문 원점과 Crosshair

리본 조사에서 확보한 ignored `artifacts/rea/ribbon/worksheet-baml-selected.json`을 독립적으로 확인했다. 소스는 `ko/Ptc.Controls.Worksheet.resources.dll`, SHA `1615d5e8b0363e6a039a1006b62b548e97edb11dcdfbc385864783781e32e831`, REA `ev_5b0a628daab9dff2230879be5292f1f8361bd85a456b7545c5d57e325edf83f7`다. `.NET resources` 안 `worksheetresources.baml` payload SHA는 `3d28afdd33bf7d40d32cbb07e76b964f603306afe2fc098d8a2cc491d9aced24`다. 기록된 오프셋은 BAML payload 내부 byte offset이다. 실행·XAML 로드는 사용하지 않았다.

BAML의 음수 KnownProperties/Types도 설치된 PresentationFramework.dll 메타데이터로 확인했다. SHA `6249f64ba1f8c69964818340c2f032be8368769064f20e23ee34790a17ae64d4`, REA `ev_e280dfe9be2514555b712fa5321192f0ecc1b56845fe2bb69f757ed01580561c`, ignored `ribbon/known-properties.json`: property61=Grid.Column,62=Grid.ColumnSpan,63=Grid.Row, type254=Grid,90=ColumnDefinition,541=RowDefinition,102=ContentPresenter다.

PageView template의 외부 Grid는 PageWidth(@17317), DisplayHeight(@17395)를 바인딩하고 RowDefinitions가 Top margin(@17484), PageBodyHeight(@17572), Bottom margin(@17660), 추가 행 순서다. ColumnDefinitions는 Left margin(@17760), 중앙 열, Right margin(@17854)이다. Page body를 담는 내부 Grid(@18432)는 외부 row1, column0, span3이며 내부 열도 Left margin(@18516), `*`(@18598), Right margin(@18618)이다. body를 감싸는 Border(@18743)는 내부 column1(@18768)에 있고 그 자식 ContentPresenter(@18784)는 `PageBody` template binding(@18818)을 사용한다. 따라서 **일반 PageView에서 본문 내부 (x,y)의 종이 내부 기준 위치는 (LeftMargin+x, TopMargin+y)**다. 이 식은 page의 화면 배치 offset, zoom, 여러 페이지로의 worksheet 변환보다 안쪽의 국소 좌표 매핑이다. 본문 저장 좌표에 margin을 다시 기록하면 안 된다.

같은 template의 header/body/footer 배경 SolidColorBrush는 `PageBackgroundColor`에 바인딩된다(@18359/@19125/@19652). 따라서 위의 Paper 정적 기본값과 화면 배경 사이 연결이 확인됐다. 페이지 전체의 모든 pixel을 그 brush가 덮는다는 의미는 아니다.

`pagebodycanvas.baml` SHA `a4ba0c365da7667df32b7ba7b34490855c1ece8d25fe752018d9c226dab44c87`는 Crosshair Canvas.Left를 `InsertionPoint.X`(@1309), Top을 `InsertionPoint.Y`(@1352), Visibility를 `IsCrosshairVisible`(@1384)에 바인딩한다. 이 원점은 본문 안에 놓인다. Crosshair 자원은 `Ptc.Controls.Core`의 satellite REA `ev_ed414eec9a6501d2d8e1ae3c0157db56b74bbd6acc3aa34922e45771ba68ba77`에서 추출됐다. `crosshair.baml` SHA `2f2b97a17f781bb30b6c03063fe5f8782d9d4b79cfb411b5750431f47ead9c85`의 관찰값은 width11/height11, 가로선(-5.5,0)→(5.5,0), 세로선(0,-5.5)→(0,5.5), StrokeThickness1이다. 두 선의 custom brush payload에는 `#0080FF`가 들어 있다. 단순히 가로/세로11 크기의 박스를 삽입점 오른쪽 아래에 그리는 것이 아니라, 두 선이 삽입점을 중심으로 교차한다. Crosshair 원본 색은 EquationEditor caret과도 일치하지만 두 상태의 표시 수명은 별개다.


## 후속 BAML: 화면 grid의 실제 override

화면 grid는 위 PrintGridSettings 초기값을 그대로 쓰지 않는다. 같은 `worksheetresources.baml`에서 화면 MinorLinesColor는 **`#FFBDDEF8`**, MajorLinesColor는 **`#FFB5DAF9`**, MinorLinesThickness는 **0.6 DIP**, MajorLinesThickness는 **1.2 DIP**다. 각 Color resource offset은16155/16174이고 TextWithConverter 기록은16159/16178, Double resource offset은16205/16218이고 TextWithConverter 기록은16209/16222다. DirectRenderingAlignmentGridBorder(@16363)의 PropertyWithStaticResourceId 기록이 MajorColor(@16388), MinorColor(@16393), MajorThickness(@16398), MinorThickness(@16403)에 이 자원을 연결한다. 리본 분석의 record decoder는 이를 문자열 키35/36/38/39와 값 offset 및 optimized static reference까지 추적했다. raw payload hex, typed 값, 키와 assignment를 ignored `artifacts/rea/ribbon/worksheet-grid-resources.json`에서 확인했다. **인쇄용 색·두께와 화면용 색·두께를 구분해야 한다.**

MajorGridLines는 칸 수가 아니라 거리다. K metric getter `0x06000886`은2cm≈75.59055 DIP, US getter `0x06000887`은1inch=96 DIP를 반환한다. Standard 기준 metric4칸/US5칸, Fine 기준 metric8칸/US10칸에 해당한다. 본문의 BAML은 PageLayout.MajorGridLines(@16691), UseMajorGridLines(@16899)를 바인딩한다.

실제 배포 `presentation10.xsd` pageModel의 `useMajorGridLines` 기본값은false(line160), `useHFMajorGridLines`도false(line165), show-grid는true(line150), show-hf-grid는false(line155)다. en-US Blank template도 major 속성을 생략한다. W viewmodel의 `_useMajorGrid`는 생성자에서 따로 true로 초기화되지 않으며, ApplyWorksheetData `0x060009be` IL513–518이 저장 설정을 setter에 전달한다.

W `get_ActualMajorLinePen` (`0x060000ae`, raw SHA `b62b4a83b73791268a608cb60aa26e4d8fd0dc6f2b286986bdc95dc1ee58ea9d`)은 UseMajorGridLines=false면 MinorLinePen, true면 MajorLinePen을 반환한다. OnRender `0x060000b8`가 이 pen과 MinorLinePen을 DirectDcDrawing `0x060000b3`에 전달한다. 따라서 기본 화면은 **0.6 DIP의 연한 파란 실선 grid**이며, 매4/5칸마다 진한 선을 강제로 넣는 상태가 아니다. major 설정을 켜면1.2 DIP의 다른 색 선을 강조한다. 원본은 교점 dot grid가 아니라 DrawLine 기반 사각 grid를 사용한다.
