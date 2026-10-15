# Mathcad Prime 12 문서·배치 의미 조사

조사일: 2026-10-15. 대상은 `C:\Program Files\PTC\Mathcad Prime 12.0.0.1`의 실제 배포 DLL, 기본 템플릿 7개, 기존 `detune.mcdx`다. 이 문서의 근거는 **실행 없는 REA CIL/메타데이터 검사와 패키지 XML 검사**다. 원본 앱 실행 결과를 관찰했다는 의미가 아니다. 계산 순서의 최종 규칙은 별도 런타임 차등 검사로 확인해야 한다.

현재 구현의 수식 목록·숫자 캐시 가져오기만으로는 원본 워크시트의 편집·배치·파일 호환성을 충족하지 못한다. 특히 문서 구조와 수식 의미를 텍스트 줄로 평탄화하면 이후 동일한 편집 경험을 구현하기 어렵다.

## 증거와 재현 경계

상세 결과·추출 자원은 Git에서 제외된 `artifacts/rea/document-deep/`에 있다. 원본 DLL과 문서를 변경하지 않았다. REA에서 대상 어셈블리를 로드하거나 실행하지 않았다. 토큰은 아래 빌드의 모듈에만 유효하다.

| 표기 | 원본 | SHA-256 | REA Evidence ID |
| --- | --- | --- | --- |
| W | Ptc.Controls.Worksheet.dll | `c1a70ec8007228ff119d4a31b36b9062132be3a10c771459c1dbe3fd0a7baddd` | `ev_ca6ee361e44f5b877e72b99111bddb37dcf24e4dd6163ce516a16f3ad2b2611b` |
| B | Ptc.Controls.Whiteboard.dll | `947ae0dea0c7a7fdbd1c377e4e7f2447dccd5e839111fec0c0789dafffc1cbd9` | `ev_b5601734c03d7bc9573770a236f1d7d0f7c2f9203276c86b4dff938362dc44ab` |
| C | Ptc.Core.dll | `ace9265338ef2aecf73b3e62540e026c5f0966df61a00127a2083110f4a2a4ab` | `ev_8ebe82dd9876db1a7a4243a15f754c914ecad3a7a7477f71c8e3a69d362ae721` |
| K | Ptc.Controls.Core.dll | `654a0252f699c9c9763aa4f4102488279500ed89cb8ce285a38476c2dd24a977` | `ev_f208e991f9e1e1e33663525464eb86305e6bee7352187fed8011d74326426e64` |

전체 REA 출력은 `worksheet-members.json`, `whiteboard-members.json`, `core-members.json`, `controls-core-members.json`에, 작은 보조 뷰는 `*-selected-evidence.json`에 있다. 검사 범위는 W 371개 타입·4,096개 메서드, B 117개 타입·1,329개 메서드, C 897개 타입·5,062개 메서드, K 616개 타입·4,382개 메서드다. 이는 기록된 메타데이터 행의 개수이며 모든 구현을 개별 해독했다는 뜻은 아니다.

再実行例:

```powershell
rea inspect-managed-members 'C:\Program Files\PTC\Mathcad Prime 12.0.0.1\Ptc.Controls.Worksheet.dll' --json
rea inspect-managed-members 'C:\Program Files\PTC\Mathcad Prime 12.0.0.1\Ptc.Controls.Whiteboard.dll' --json
rea inspect-managed-members 'C:\Program Files\PTC\Mathcad Prime 12.0.0.1\Ptc.Core.dll' --json
rea inspect-managed-members 'C:\Program Files\PTC\Mathcad Prime 12.0.0.1\Ptc.Controls.Core.dll' --json
```

## 현재 형식 스키마를 실제 배포 DLL에서 확보

**관찰:** C의 manifest resource에 `Ptc.Schemas.Mathcad.worksheet50.xsd`, `Math50.xsd`, `calculation10.xsd`, `presentation10.xsd`, `result10.xsd` 등이 실제로 들어 있다. `Mathcad Converter/schema/worksheet30.xsd`는 Mathcad 14용 과거 형식이므로 Prime 12 명세로 대체 사용하면 안 된다.

REA artifact 결과 `artifacts/rea/mathcad/Ptc.Core.dll.artifact.json`의 resources RVA·resource offset·길이를 PE 섹션의 파일 위치로 매핑해 데이터만 추출했다. 각 추출물의 SHA-256과 길이는 REA가 보고한 값과 일치한다. 추출 당시 어셈블리 실행·리플렉션은 사용하지 않았다. `schemas/manifest.json`에 원본 해시, 원본 REA ID, 파일 범위, 추출 해시를 보존했다.

| 자원 | 바이트 | SHA-256 |
| --- | ---: | --- |
| worksheet50.xsd | 138,898 | `b4a73084e11db4dbcba52c603e2791a3026b088e150cb482114b9a315f355ba3` |
| Math50.xsd | 79,889 | `a10afb4dd442a4f5dd77b6f1df903c1d2e5b6f4d6a701aae38b0cefee21f6ca3` |
| presentation10.xsd | 34,779 | `9463d5b00f2b1255cd5459e02cadfa5dfde59ed7182a453b2c212a14fa0afc4a` |
| result10.xsd | 10,363 | `f2d598e3cb62fecd5dd986b781856f4d5ffdb1ee530cc872ac4969984d208bcf` |

스키마는 저장 문법·기본값·작성자 주석의 근거다. 선언된 모든 필드가 현재 실행 경로에서 사용된다는 보장은 없다. 특히 호환성용 과거 속성들이 포함된다. 배포 자원 원문을 프로젝트 코드에 복사하지 않고, 관찰된 파일 문법과 동작을 독립적으로 구현한다.

## 패키지와 참조

**관찰:** MCDX는 단일 XML 파일이 아니라 ZIP/OPC 패키지다. `document-package-samples.json`에 `detune.mcdx`와 en-US 템플릿 7개의 입력 해시, ZIP 항목, 관계, 영역 구조, 문서 설정을 기록했다.

- 루트 `_rels/.rels`, 각 part의 `.rels`, `[Content_Types].xml`이 존재한다. 본문·머리글·바닥글은 별도 part다. 실제 표본의 여러 worksheet 계열 part가 동일한 officeDocument 관계 종류를 쓰므로 관계 종류 하나만 보고 첫 항목을 선택해서는 안 된다.
- `text/@item-idref`는 **그 text를 소유하는 part**의 관계 ID다. 본문의 `mathcad/_rels/worksheet.xml.rels`와 머리글의 `mathcad/_rels/header.xml.rels`를 구분해야 한다.
- K `McdxSerializer.PackFlowDocument` (`0x06000b37`)는 별도 `application/zip` part를 만들고 flowDocument 관계를 추가하며 `SaveTextRangeAsXamlPackage`를 호출한다. 관찰된 경로는 `/mathcad/xaml/FlowDocument{n}.XamlPackage`다. 이 내부 ZIP의 XAML·서식·이미지를 단순 문자열과 구분해야 한다.
- K `PackImage`는 이미지 해시 및 `PackPartAndGetId`를 호출한다. `PackIncludedWorksheet` (`0x06000b2f`)는 include 관계와 캐시 경로 등록을 사용한다. Excel/OLE/chart도 별도 part 및 개별 관계 종류다. part 이름에서 객체 의미를 추측해 직접 접근하는 것은 충분하지 않다.
- C `PackageOperationsProvider`의 실제 메서드는 본문/헤더/푸터뿐 아니라 `/mathcad/headers.xml`, `/mathcad/footers.xml`도 다룬다. 현재 스키마의 headersContainer/footersContainer는 일반 페이지와 첫 페이지의 내용을 각각 담을 수 있다.

**구현 요구:** URI/관계/콘텐츠 타입 기반 package 계층과 원본 part 보존 계층이 필요하다. 지원하지 않는 영역·외부 객체를 누락시킨 상태로 성공적으로 저장했다고 보고하면 안 된다. 미지원 XML과 관계·바이너리 payload를 보존하거나 해당 저장 경로를 명시적으로 제한해야 한다.

## 영역은 중첩 트리다

**관찰:** 현재 worksheet50 스키마는 math/text/picture 이외에 Area, pageBreak, solveblock, plot, reference, spec-table, Excel, chart, OLE, 보호된 part 등의 구조를 포함한다. text 안에도 embedded region이 들어갈 수 있다. header/footer에는 자동 페이지 번호·날짜·파일명·경로용 `fieldText`가 존재한다.

Area는 자식 regions를 가진다. `IsCollapsed`, `IsBorderVisible`, `AreaHeight`를 별도로 저장한다. Company Standard Definitions 템플릿에는 Area 1개와 그 안의 영역 14개가 실제로 들어 있다. Report 템플릿에는 Area 6개가 있다. Specification에는 spec-table 1개, Presentation에는 pageBreak 2개가 실제로 관찰된다. 이는 단지 타입 이름으로 추측한 기능 목록이 아니다.

W `ContainerRegionPersistentData.Deserialize` 및 개별 region persistent data 타입들이 존재한다. K serialization helper는 본문·헤더·푸터·첫 페이지 헤더·첫 페이지 푸터 목록을 따로 관리한다.

**구현 요구:** 문서 모델을 `Document -> section -> region tree -> typed payload`로 구성한다. 영역의 parent, 고유 ID, local position, 형식, 크기 지정 여부, content AST, formatting, 관계 ID를 함께 보존한다. 전역 `descendants(region)` 순회로 모든 자식을 단일 목록에 넣으면 Area 내부 좌표 및 scope/context를 잃는다. Area가 접혔을 때 계산하는지, Area 위치 변경이 자식의 엔진 위치를 어떻게 바꾸는지는 아직 런타임 확인 대상이다.

## 배치 단위·크기·그리드

**관찰:** 현재 worksheet50의 `pageUnitType`은 `xs:double`이다. region은 `top`, `left`, 선택적 `width/height`, 측정된 `actualWidth/actualHeight`, 선택적 `align-x/align-y`를 구분한다. C `regionInWhiteboardBase.get_Location` (`0x06000a1a`)는 left와 top을 읽어 `System.Windows.Point`를 만든다. W `RegionPersistentData.DeserializeLocation` (`0x060002eb`)는 이 위치를 받아 사용한다. W의 DeserializeWidth/Height는 각각 지정 여부를 검사하고 WPF FrameworkElement Width/Height를 설정한다. 미지정과 0은 다른 상태다.

단위 변환에 대한 CIL 관찰:

| 메서드 | 토큰 | 연산 |
| --- | --- | --- |
| C WpfUtils.InchesToDips | `0x060005f1` | 입력 × 96 |
| C WpfUtils.DipsToInches | `0x060005f2` | 입력 ÷ 96 |
| C WpfUtils.CmToDips | `0x060005f3` | 입력 × 37.79527559055118 |
| C WpfUtils.DipsToCm | `0x060005f4` | 입력 ÷ 37.79527559055118 |

K WorksheetPageLayout의 MetricGridCellSize (`0x06000880`)는 0.5 cm를 각각 DIP 변환한다. USGridCellSize (`0x06000883`)는 0.2 inch를 DIP 변환한다. Fine 값은 각각 Standard Size의 가로·세로에 0.5를 곱한다 (`0x06000881/884`, ScaledSize `0x06000892`). 따라서 관찰된 standard grid는 metric 약 18.8976377953 DIP, US 19.2 DIP이고 fine은 각각 절반이다. 실제 `detune`의 첫 text top은 18.89763779527559, en-US 템플릿은 19.2의 배수 위치들이 관찰된다.

**추론:** 위 직접 위치 대입, 단위 변환, 표본 좌표는 현재 region 좌표를 DIP 단위로 다루는 설계를 강하게 지지한다. 스키마 presentation의 일부 오래된 설명에는 points 표현이 남아 있어 모든 숫자 필드에 같은 단위를 일괄 적용해서는 안 된다. 종이 크기, 폰트 크기, 화면 확대, 디스플레이 DPI를 각각 구분해야 한다.

**구현 요구:** f64 좌표와 별도 변환 계층을 사용하고 OS DPI를 문서 좌표에 굽지 않는다. A4/Letter, 방향, 여백, Standard/Fine grid, 헤더/푸터 grid, 첫 페이지와 전체 페이지 offset 등을 문서 설정에 보존한다. 실제 환경에서 종이 크기·확대·인쇄와의 대응은 추가 검증한다.

## 앵커와 화면 사각형, 위치 정렬

**관찰:** B `WhiteboardItemManager.GetItemRect` (`0x06000147`)는 item location을 구한 후 `Point.Offset(0, IWhiteboardItem.VerticalAlignmentOffset)`를 적용하고 실제 render size로 사각형을 만든다. location과 화면 사각형의 top이 같은 값이라고 단정할 수 없다. 실제 Company Standard Definitions의 일반 수식(높이 25.6)과 분수 수식(높이 47.2)이 같은 top 57.600000000000009를 갖는다.

W `WorksheetItemManager.GetItemWorksheetLocation` (`0x060006e3/e4`)은 WorksheetLeft/WorksheetTop getter를 B `WhiteboardItemManager.GetItemLocation`에 전달한다. B의 위치 getter는 Point를 구성하며 이 경로에는 VerticalAlignmentOffset 호출이 없다. 스키마의 align-x/y 설명은 alignment offset과 계산 순서의 관련성을 명시하지만, 저장 top에 무조건 align-y를 더해야 한다는 수식을 확립하지는 않는다.

W `WorksheetItemPositionComparer.Compare` (`0x0600070c`)는 같은 object reference면 0, 아니면 두 worksheet location을 구해서 B `PositionComparer.Compare` (`0x06000150`)에 넘긴다. B 메서드는 Point.Y를 먼저, 비교가 동률이면 X를 비교한다. C `DoubleComparer.Compare(double,double)` (`0x06000117`)는 절대 차이와 epsilon을 비교하며, 기본 생성자 (`0x06000116`)의 epsilon은 `1e-10`이다. 기본 comparer의 정적 초기화가 이 생성자를 호출한다 (`0x0600011f`). B의 두 좌표가 모두 동률인 분기는 -1로 끝난다. 따라서 서로 다른 동일 위치 객체에 대해 일반적인 정렬 comparator의 equality 성질을 가정해서는 안 된다.

W `WorksheetModel.AddItemAtLocation` (`0x06000807`)은 위치를 설정하고 `List.BinarySearch` 후 WhiteboardItems에 Insert한다. `SetItemWorksheetLocation` (`0x06000808`)은 이웃 위치와 PositionComparer를 비교하고 ObservableCollection.Move를 호출한다. 이는 XML 저장 순서와 공간 위치 기반 collection 순서가 독립적일 수 있음을 보여준다. 이 정렬이 엔진의 모든 평가 규칙을 확정하는 근거는 아니다.

`z-order`는 현재 스키마에도 남아 있지만, **regionUnusedAttrs라는 과거 Mathcad 14 잔존 속성 그룹**에 속한다. 설명에 draw order가 있어도 Prime 12의 실제 renderer가 이를 사용한다고 확정할 수 없다.

**필수 차등 검사:** XML 순서 뒤집기, 같은 Y의 좌우 정의/참조, 1e-10보다 작은/큰 좌표 차이, 완전히 같은 위치, 크기가 다른 수식의 같은 anchor, align-x/y만 변경, 영역을 가로/세로 이동, Area 내부/외부, 페이지 경계. 원본 저장 전후 좌표와 계산 결과를 함께 기록한다. 재정의와 전방 참조의 순서는 이 검사 및 계산 엔진 조사로 결정한다.

## 결과 캐시는 값 하나가 아니다

**관찰:** worksheet의 `math/@resultRef`와 result document의 `resultData/@result-id`는 region-id와 별도의 ID 공간이다. result10은 숫자 결과, symbolic 결과, 2D plot 결과를 구분하며 결과 본체가 없는 항목도 허용한다. 상태는 `Synchronized` 또는 `Pending`이다. `detune` 표본의 66개 결과는 모두 Synchronized지만 이것이 원본을 지금 재계산했다는 뜻은 아니다.

K `ResultSerializer.WriteXml` (`0x06000bb5`)은 IResultManager.Results를 순회하며 dictionary key, SerializationTag, SynchronizationStatus, ResultNode, Errors, Warnings를 읽어 쓴다. ReadXml (`0x06000bbe`)은 EngineResult를 만들고 IResultManager.SetEngineResult를 호출한다. K `ResultDataSerializer.Read`는 결과 콘텐츠 타입으로 part를 찾는다. Write는 결과 part 생성 및 일부 예외 상황에서 part 삭제 호출을 가진다. 정확한 오류 복구 정책은 아직 확인하지 않았다.

result10에서 확인한 진단 데이터:

- engineError의 errorCode, errorArgument, 선택적 localized resource-string, error-id, previous-error-id, error-node-tag, error-tag-arg, error-node-handle.
- redefinition Warning의 nodeId, oldType(Value/Matrix/Function), 선택적 oldLocation.

**구현 요구:** computed value, cached value, synchronization state, engine diagnostic, node mapping, result formatting을 별도로 모델링한다. 캐시 일치율을 계산 기능 전체의 호환율로 보고하지 않는다. Pending 캐시를 정답 fixture로 채택하지 않는다. errors/warnings·표현식별 진단 위치도 차등 검사 결과에 포함한다.

## 현 importer의 차이와 우선순위

조사 당시 `crates/mathcraft-document/src/mcdx.rs`는 수식을 문자열로 변환하고 전역 region descendants를 순회하며 left/top만 읽는다. 숫자 복소수 캐시 일부를 해석하고 math/text 이외의 영역을 unsupported로 만든다. 이 기능은 표본 열람용 subset이며 원본 파일의 의미와 편집 상태를 유지하는 기반으로 충분하지 않다.

1. 원본 namespace를 보존하는 typed math AST·identifier의 label/서브스크립트·표현 서식 모델부터 정의한다. 소스 수식을 평문 문자열로 변환해 저장하지 않는다.
2. 중첩 region tree, section별 relationships, 문서 설정과 미지원 payload 보존을 구현한다.
3. anchor·render bounds·grid·paper/page layout을 분리하고 원본의 공간 배치를 유지한다.
4. 원본 런타임에서 위치/재정의/범위/비활성화 검사를 실행해 평가 위치 규칙을 확정하고 테스트에 고정한다.
5. 결과의 타입·상태·오류·경고를 보존하고, 읽기/편집/저장/다시 열기의 차등 검사를 수행한다.

아직 미확정: 원본의 완전한 page mapping/print clipping, 같은 좌표의 엔진 tie-break, align-x/y의 현재 코드 적용 방식, nested container의 정확한 엔진 위치/비활성화/접기 규칙, 미지원 part를 포함한 완전한 round trip, 실제 z-order 동작. 이 항목들은 별도 증거 없이 구현 규칙으로 확정하지 않는다.

## 독립 구현한 무손실 패키지 기반

후속 구현으로 `crates/mathcraft-document/src/native.rs`에 기존 importer와 분리된 읽기 전용 `NativePackage`를 추가했다. 실제 제품 코드 실행 없이 Rust 코드와 독립 synthetic fixture로 구현했다.

- `open`, `original_part`, `parts`, `worksheet_part_name`, `worksheet_xml`로 원본 part 바이트와 해석용 XML 뷰를 구분한다. XML은 UTF-8/UTF-16을 읽되 저장할 때 원본 바이트를 다시 사용한다.
- `regions`는 part, 구조적 path, parent region path, 원본 ID, 위치·지정 크기·actual size·alignment 값과 원본 XML을 반환한다. 수식의 element/namespace/attribute/direct text/raw XML을 보존하므로 VARIABLE/UNIT label 및 literal subscript를 같은 문자열로 평탄화하지 않는다. 화면 배치나 계산 순서를 임의로 계산하지 않는다.
- `caches`는 저장된 result-id/status/tag, result/symResult 등 종류, 오류 인자·노드 속성·localized resource-string, 재정의 경고와 원본 XML을 반환한다. 캐시를 재계산 결과로 취급하지 않는다.
- `write_new`는 새 `.mcdx` 파일을 create_new로만 생성한다. 원본 경로는 원본이 삭제된 뒤에도 재사용하지 않는다. 모든 part 이름/바이트/압축 방식과 ZIP comment를 유지한다. ZIP 컨테이너 전체의 바이트 동일성이나 모든 ZIP extra field 보존은 보장하지 않는다.
- ZIP reader의 이름 map이 중복 항목을 덮어쓰기 전에 central directory를 검사한다. 최대 4,096개 항목, 항목당 32 MiB, 합계 256 MiB, XML당 8 MiB, 노드 250,000개, XML 깊이 128로 제한한다. DTD, 중복 이름, 다중 디스크, ZIP64, 암호화, 미지원 압축·파일명 인코딩은 명시적으로 거절한다. 파일 추출·외부 관계 접속·프로그램 실행을 하지 않는다.

검증: synthetic 영역 중첩·anchor/크기 미지정 구분·label/서브스크립트, Pending 캐시와 진단 보존, 모든 part의 content-level round trip, UTF-16 원본 바이트 보존, 미지원 XML part의 DTD 차단, 중복 ZIP 이름, 선언된 크기·항목 수 초과 차단 테스트 7개를 추가했다. `cargo test -p mathcraft-document --locked` 전체 13개 통과 및 해당 crate의 `cargo clippy --all-targets --locked -- -D warnings` 통과. 실제 원본 앱에서 다시 열어 본 호환성 검증은 이 테스트 결과에 포함하지 않는다.

## 부록: 로드할 때 그리드에 맞추는 경로

후속 정적 조사로 **로드된 본문 위치를 그대로 정렬하는 것이 원본 동작과 다를 수 있는 직접적인 코드 경로**를 확인했다. 이 조사 중 원본 앱·COM·GUI는 실행하지 않았다. 근거는 앞의 W/K REA Evidence와 그 오프셋으로 지정한 원본 바이트다. `grid-load-supplemental-il.json`은 dncil로 디코딩한 보조 CIL이며, 164개 메서드의 IL 원문 SHA-256을 REA의 `body.il_sha256`과 대조해 일치시켰다. 원본 소스 복구나 실행 관찰로 표현하지 않는다. `grid-enum-constants.json`은 원본 Constant 테이블의 페이지/그리드 enum 값이다.

### 문서 로드 경로

1. W `WorksheetViewModel.ApplyWorksheetData` (`0x060009be`)는 저장된 LayoutSize가 알려진 enum 값이면 `WorksheetPageLayout.SetPageSize`로 적용하고, GridSize도 알려진 enum이면 적용한다.
2. W `<ApplyWorksheetData>b__5` (`0x06000d1f`)는 `IRegionPersistentData.Location`을 읽고 **AddItemAtLocation(control, location, true)**를 호출한다.
3. W `WorksheetControlContent.AddItemAtLocation` (`0x060005e5`)의 세 번째 인자가 true인 분기는 `_worksheetGrid.GetNearestIntersectionPointDisregardingBounds`를 호출한 다음 결과 위치로 model에 항목을 넣는다. 무시하는 방향 배열은 Right/Down이다. 왼쪽/위쪽 제한까지 모두 무시한다는 의미는 아니다.
4. K `AlignmentGrid.GetNearestIntersectionPointDisregardingBounds` (`0x06000132`)는 grid Width/Height를 `GetCandidateIntersectionPoint`에 전달하고 이후 `Clip`을 적용한다. `Clip` (`0x06000133`)은 조건에 따라 grid 최댓값과 0으로 제한한다. restricted-area override, page-wide sizable, 특수 영역은 추가 동작을 가질 수 있다.

따라서 raw XML 위치는 파일 자체의 관찰값으로 유지하고, **원본이 로드한 문서의 계산 위치**를 재현할 때는 별도 정규화 단계를 거쳐야 한다. 무손실 NativePackage는 이 변형을 원본 바이트에 적용하지 않는다. raw 평가와 저장 후 평가의 차이는 계산기 산술 오류와 로드 시 좌표 변경을 구분해 기록해야 한다.

### 반올림의 직접 근거

K `AlignmentGrid.GetCandidateIntersectionPoint` (`0x06000134`)는 X/Y 각각에 CIL `rem`을 수행하고 반 칸과 비교한다. **유한한 비음수 좌표** v, 양수 grid 간격 g에서는 다음과 같다.

```text
r = v % g
offset = (r > g / 2) ? (g - r) : (-r)
candidate = v + offset
```

중간값 동률은 작은 좌표 쪽으로 간다. `Math.Round`/은행가 반올림을 호출하는 코드가 아니다. 부동소수점 경계까지 맞추려면 위의 remainder와 덧셈 순서를 보존해야 한다. 음수·NaN·범위 밖 좌표의 처리까지 이 단순 공식으로 일반화하지 않는다.

Letter Standard 간격은 `0.2 * 96.0` DIP다. 따라서 일반적인 양수 위치 20과 20+1e-8은 모두 같은 약 19.2 위치로 들어갈 수 있다. 이 경우 처음 raw XML의 엄격한 Y 순서가 사라지고 X 순서가 결과에 영향을 줄 수 있다. 이는 알려진 파일 로드 경로로 설명되는 호환성 항목이다.

### Metric/US 선택

K `WorksheetPageLayout.SetupDefaultPageSizeInformation` (`0x0600087e`)은 **종이 종류별 Dictionary**에 다음 값을 등록한다. 저장된 paper 종류를 알 때 매번 현재 OS locale로 grid 단위를 선택하는 구조가 아니다.

| PageSize enum | 값 | Standard grid | Fine grid |
| --- | --- | --- | --- |
| A1, A2, A3, A4, A5 | 0, 1, 2, 3, 4 | `0.5 * 37.79527559055118` DIP | Standard × 0.5 |
| Letter, Legal, Ledger | 5, 6, 7 | `0.2 * 96.0` DIP | Standard × 0.5 |

`set_PageSize` (`0x06000860`)은 현재 PageSize로 이 Dictionary를 조회하고 GridSize가 Fine(1)이면 FineGridCellSize, 아니면 GridCellSize를 선택한다. `set_GridSize` (`0x06000869`)도 같은 Dictionary를 사용한다. `GetDefaultPageSizeForLocale` (`0x0600082d`)는 현재 thread의 **CurrentUICulture LCID**로 초기 기본 종이를 선택하는 별도 경로다. 저장된 paper/grid가 없거나 invalid인 경우의 전체 fallback 순서는 이 부록에서 확정하지 않는다.

특히 US 간격의 이진 f64 값은 `19.2` 리터럴과 `0.2 * 96.0` 계산이 한 ULP 다를 수 있다. 원본의 뒤쪽 표현은 19.200000000000003이 될 수 있으므로 간격을 표시용 소수 문자열로 반올림해 구현 값으로 다시 사용하지 않는다. grid 변경 이벤트 W `<SetupDefaultPageLayout>b__530_0` (`0x06000b42`)는 기존 위치를 이전 grid 간격으로 나누고 새 grid 간격으로 다시 변환하는 재배치 경로도 가진다.

주요 메서드의 REA raw IL SHA-256:

| 모듈/토큰 | 해시 |
| --- | --- |
| K `0x06000134` | `8550306ea628d0cebcd5e4f1c24ddb63032ca89c9c46394a57efda61e9121f89` |
| K `0x06000860` | `5ac7efb3cd3bb5de5ef7f270f31b1b7f99cf44a8e257bb780a970d5e18c133f1` |
| K `0x0600087e` | `6a9214639edd4b6db96a34b0c15981074f0738e0cfceab539d993d8f32a147bc` |
| W `0x060005e5` | `f634000b5de35e7e12cc2ef45ef53bbf62f8c6901ce2ddaa25191e04872c1a38` |
| W `0x06000d1f` | `a9e31d8f146bf8acc58ad3c5916824ee0464583c6f8a8061e8e420027c74fc93` |

## 후속 구현: 자유 배치 캔버스의 저장 계약

`crates/mathcraft-document/src/canvas.rs`는 `.mathcraft` version 2의 독립 제품 문서 모델이다. 기본값은 빈 워크시트, 816×1056 문서 좌표, grid 19.2이며 영역별 ID/좌표와 `Math(PortableTree)`, `Text`, `Opaque{xml,kind}`를 저장한다. 기본 종이/grid는 이 버전의 제품 정책이며 모든 Mathcad 문서의 기본값이라는 주장이 아니다. `source_region`은 원본 영역의 출처를 연결한다.

현재 편집 모델은 **flat canvas**만 지원한다. Mathcad의 중첩 영역·머리글/바닥글·별도 part를 완전히 재구성하는 편집 모델은 아직 아니다. 가져온 원본 MCDX ZIP 전체를 `native_archive: Option<Vec<u8>>`에 16 MiB까지 바이트 그대로 저장해 unknown part가 사라지는 것을 방지한다. 이 보존은 원본 파일에 편집 내용을 반영해 왕복 저장했다는 뜻이 아니다.

수식은 PortableTree의 semantic shape만 저장하고 caret, arena ID, undo/redo history는 포함하지 않는다. 저장은 모든 데이터를 검증한 다음 같은 디렉터리에 새 임시 파일을 만들고 동기화 후 원자적 rename한다. 검증·쓰기·rename 실패 시 기존 출력 파일을 유지하며 소유한 임시 파일만 정리한다. `.mcdx` 경로에는 이 형식을 쓰지 않는다.

제품의 자원 제한: 영역 4,096개, 단일 텍스트 1 MiB, 전체 텍스트 8 MiB, 전체 수식 노드 65,536개, 캔버스 수식 깊이 48. 깊이 제한은 serde_json의 bounded parser로 저장 후 다시 읽기가 가능하게 하려는 정책이며 원본 제품의 한계가 아니다. 유효한 PortableTree 구조, 유한 좌표·양의 문서 크기/grid, 중복되지 않는 ID와 next_id도 검증한다.

synthetic 테스트 6개를 추가해 수식·label·배치·opaque XML·원본 archive bytes의 저장/열기, 교체 저장, 중간값 snap, 실패한 이동의 원자성, invalid 저장과 rename 실패 시 이전 파일 보존/임시 파일 정리, 잘못된 버전/ID/트리/예산 거부, 지원 최대 깊이의 저장 왕복을 확인했다. `cargo test -p mathcraft-document --offline` 전체 19개 및 `cargo clippy -p mathcraft-document --all-targets --locked -- -D warnings` 통과. 원본 앱 실행·GUI·COM은 사용하지 않았다.
