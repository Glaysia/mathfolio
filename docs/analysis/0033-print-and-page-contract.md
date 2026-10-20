# Prime 12 인쇄·용지·비활성 영역 계약

2026-10-16. 대상은 설치된 **PTC Mathcad Prime 12.0.0.1**이다. 설치 도움말, DLL 메타데이터와 CIL, DLL 내 XSD를 읽었다. 원본 실행, GUI, 스크린샷, COM, CLR 로드, 실물 출력은 사용하지 않았다. 아래는 독립 구현을 위한 관찰 기록이며, 이 문서만으로 Mathcraft의 구현 완료나 출력 품질을 주장하지 않는다. 현재 구현 상태는 [통합 기능 목록](../FEATURES.md)을 기준으로 한다.

## 증거의 식별과 재현

ignored `artifacts/rea/printing-source-ledger.json`에 도움말 7개와 XSD 2개, 해시 검증한 CIL 메서드 40개의 식별자를 기록했다. `printing-decode.py`는 각 원본 파일 SHA와 REA의 IL SHA를 검사한 뒤 필요한 메서드를 디코딩한다. `printing-source-ledger.py`는 원본 도움말 및 내장 리소스의 파일 범위까지 다시 검사한다. 코드를 실행하거나 리플렉션으로 읽는 방식이 아니다. 원본 메서드 본문은 저장소 문서에 복제하지 않는다.

| 원본·REA Evidence | 정체성·사용 범위 |
| --- | --- |
| `MathcadPrime.dll`, `ev_3df9115ea4951deb8e8d6af008e1012ba7b29a66fe86d6a95f9c67c4d0143b00` | SHA-256 `68c0d708abf70dd687e7996de6f6d7105f29a1ffd7e47fd719e161076fcc140f`; `SpiritMainWindow` 명령 연결 |
| 같은 DLL, `ev_e80904386e4d5ac2acd7555beb151f1440294c8038e9d5903f6d640ef5af213e` | `Spirit.AppCommands` 정적 초기화 |
| `Ptc.Controls.dll`, `ev_9b70b5ea25aee75f94f3de2d771e6f18c65568a5bdfcebf56dd709ee365a702b` | SHA-256 `4f6e1d7cbdeaf5752d3c02d6b70b74d34e9c4d7c4cc66497c90c49feebb905fd`; `EngineeringDocument` 인쇄 체인 |
| `Ptc.Controls.Worksheet.dll`, `ev_ca6ee361e44f5b877e72b99111bddb37dcf24e4dd6163ce516a16f3ad2b2611b` | SHA-256 `c1a70ec8007228ff119d4a31b36b9062132be3a10c771459c1dbe3fd0a7baddd`; 페이지 준비·paginator |
| `Ptc.Controls.Core.dll`, `ev_f208e991f9e1e1e33663525464eb86305e6bee7352187fed8011d74326426e64` | SHA-256 `654a0252f699c9c9763aa4f4102488279500ed89cb8ce285a38476c2dd24a977`; 용지 크기·여백 상수 |
| `Ptc.Core.dll`, `ev_8ebe82dd9876db1a7a4243a15f754c914ecad3a7a7477f71c8e3a69d362ae721` | SHA-256 `ace9265338ef2aecf73b3e62540e026c5f0966df61a00127a2083110f4a2a4ab`; PrintDialog·여백 직렬화 |
| 같은 DLL 내 XSD 리소스, `ev_733639bc33098dab0c4d11199cf3e2e1224402d74e9ca171f32c3355e310c704` | `presentation10.xsd` SHA `9463d5b00f2b1255cd5459e02cadfa5dfde59ed7182a453b2c212a14fa0afc4a`; `worksheet50.xsd` SHA `b4a73084e11db4dbcba52c603e2791a3026b088e150cb482114b9a315f355ba3` |

도움말은 설치 `Help/en/PTC_Mathcad_Help/` 기준이다.

| 파일 | SHA-256 | 관찰 범위 |
| --- | --- | --- |
| `to_print_worksheets.html` | `35e9843a38ae3b7343a809c6aca2896de881a15a0f9f452c63b6c8cc72003768` | 인쇄 대화상자·페이지 범위·선택 |
| `to_customize_the_worksheet.html` | `76cd28a3046e5d512bdc234012168e558ab4fc2b33d5635745b02cfc731b0763` | 용지·여백·보기·출력 제외 |
| `to_create_and_edit_headers_and_footers.html` | `e31b8565a26908b42024d6bd90cc8e6d59e303674b42ff7383d67ee067894e2b` | 머리글·바닥글·번호 |
| `about_controlling_region_calculation.html` | `e9c3025f61e9a9bde4092c91613789eb3cf457814d92ce43e86f176d6773a005` | 인쇄 시 계산·비활성 영역 |
| `to_insert_or_delete_page_breaks.html` | `35a1f56adb40b00d17cec2105b82615ad8dbdb18a374ef18d5ea1e12b299180b` | 명시 페이지 나눔 |
| `to_collapse_and_exapnd_an_area.html` | `d7f978ee02264f02a7d78a6be39847844dd24fddf71a902f80a2fd87e716307f` | 접힌 Area 출력 제외 |
| `to_control_area_calculation.html` | `72ab26a4ad6ec8b7e4d054631e8b88cbac6d3ed8b0c5f949076d3016e64c9fea` | Area 및 내부 개별 영역 비활성화 |

## 명령과 출력 범위

**관찰:** `Spirit.AppCommands::.cctor` (`0x06000083`, IL `0x0069`)는 WPF `ApplicationCommands.Print`를 `PrintEngineeringDocument`에 넣는다. `SpiritMainWindow.BindCommands` (`0x060004ec`, IL `0x0348` 이후)는 이를 `HandlePrintDocument`에 연결한다. 핸들러 (`0x06000581`)는 현재 문서의 `Print`를 호출한다. 따라서 Ctrl+P는 표준 WPF Print 명령에 대응한다는 해석이다. 이 조사에서는 원본 키보드 입력을 실행하지 않았다.

`EngineeringDocument.Print` (`0x06000129`)는 기존 인쇄 작업 처리 뒤 PrintDialog를 만들고 페이지 최솟값 1·최댓값 문서 페이지 수를 지정한다. 선택 영역이 없을 때 Selection은 활성화되지 않는다. 도움말의 출력 범위는 다음과 같다.

| 선택 | 계약 |
| --- | --- |
| All | 기본값. 워크시트 모든 페이지 |
| Current Page | 상태 표시줄에 표시된 활성 페이지 |
| Pages | 단일 번호 또는 `1-3`처럼 연속 범위. 쉼표는 오류 |
| Selection | 선택된 영역이 하나라도 걸친 **페이지 전체**. 영역만 잘라 출력하는 동작이 아님 |

`CreatePrintableDocument` (`0x06000133`, IL `0x02cd` 이후)는 Current·Range·Selection·All을 별도 분기한다. Selection은 선택 항목을 페이지로 매핑하고 정렬·중복 제거해 페이지 snapshot을 추가한다. PDF와 XPS는 도움말에서 각각 OS의 PDF·XPS 프린터를 선택하는 방식으로 설명한다. 별도 Print Preview 명령의 존재는 이 조사에서 확인하지 않았다. Page 보기는 도움말상 인쇄 모양을 보여 주는 보기다.

**관찰:** 수동 계산 모드에서 인쇄하면 결과가 최신 상태가 아닐 수 있으며, 사용자가 먼저 계산해야 한다고 도움말에 명시되어 있다. 확인한 Print → PrintDocument → CreatePrintableDocument 경로는 페이지 snapshot을 만들며 강제 재계산 명령을 호출하지 않는다. 따라서 인쇄를 누를 때 임의로 전체 계산을 추가하면 원본 계약과 달라진다.

## 페이지 크기·여백·배율

`SetPrintDialogPageLayoutOptionsFromDocument` (`0x06000130`)는 문서의 용지 크기와 방향을 프린터 PrintTicket에 설정한다. `EngPagesPaginator.get_PageSize` (`0x06000fa4`)는 문서의 `PageSizeDips`를 반환하고 setter (`0x06000fa5`)는 NotSupportedException을 낸다. 이 경로에서는 추가 fit-to-width 배율을 확인하지 않았다. 따라서 독립 구현의 기본 출력은 문서 물리 크기를 보존해야 한다는 결론이다. 프린터 드라이버 자체 배율·인쇄 가능 영역 처리는 이번 정적 조사로 확정하지 않는다.

| 용지 코드 | 세로 방향 원본 치수 | 표준 여백 |
| --- | --- | --- |
| A1 | 59.4 × 84.1 cm | 각 2.5 cm |
| A2 | 42 × 59.4 cm | 각 2.5 cm |
| A3 | 29.7 × 42 cm | 각 2.5 cm |
| A4 | 21 × 29.7 cm | 각 2.5 cm |
| A5 | 14.8 × 21 cm | 각 2.5 cm |
| Letter | 8.5 × 11 in | 각 1 in |
| Legal | 8.5 × 14 in | 각 1 in |
| Ledger | 11 × 17 in | 각 1 in |

치수와 여백은 `WorksheetPageLayout.SetupDefaultPageSizeInformation` (`0x0600087e`) 및 상수 메서드에서 확인했다. Narrow는 표준 ×0.5, Wide는 ×2, Legacy는 용지 계열과 관계없이 각 1.2 in이다. 세로·가로 방향은 별도 속성이다. 내부 좌표는 96 DIP/in이다. 표에 적은 용지 크기를 다른 단위의 임의 숫자로 추측해 저장하면 안 된다.

`pageModel`의 `paper-code` 기본값은 Letter, `orientation` 기본값은 Portrait다. `page-width`·`page-height`는 XSD 주석상 points 단위의 정보용 출력값이며 용지 변경은 `paper-code`로 한다. 화면·인쇄 DIP 좌표와 이 정보용 값을 혼동하면 안 된다.

Custom은 `margin-type="Custom"` 및 `page-margin` 문자열이다. `presentationPageModel.get_PageMargin` (`0x06000dc3`)는 ThicknessConverter.ConvertFromInvariantString, setter (`0x06000dc4`)는 ConvertToInvariantString을 사용한다. 즉 WPF Thickness 형식이며 네 값의 순서는 **left, top, right, bottom**이다. 기본 숫자의 단위는 DIP다. 해당 converter 전체의 단위 접미사·오류 허용 범위를 Mathcraft가 지원한다고 주장하려면 별도 회귀가 필요하다. Last Custom Setting은 도움말상 현재 앱 세션에서 다른 열린 문서와 공유하는 최근 설정이다.

## 인쇄 표시 규칙과 페이지 흐름

**관찰:** 오른쪽 Draft 공간의 수식도 워크시트 계산에 참여하지만 출력하지 않는다. Page 보기에서는 그 영역과 Draft 내용을 숨기며, Draft 보기에서는 오른쪽 수직 점선 너머 내용과 가로 스크롤을 제공한다. 인쇄를 위해 오른쪽 공간을 새 페이지 열로 바꾸거나 전체를 한 장에 축소하는 것은 현재 도움말의 동작이 아니다.

내장 XSD에는 과거 `print-single-page-width` 속성과 poster 열 출력 주석이 남아 있다. 현재 Prime 12 도움말과 의미가 충돌하므로 **XSD 주석만으로 현재 UI의 poster 출력 기능을 추가하지 않는다**. 현재 속성의 실제 사용 여부는 미확인이다.

`CreatePrintableDocument`는 문서의 페이지 레이아웃·수식 서식과 함께 다음 값을 출력 객체에 복사한다.

| 문서 설정 | 인쇄 계약·XSD 기본값 |
| --- | --- |
| `show-grid`, `show-hf-grid` | 본문 및 머리글/바닥글 눈금 출력; true / false |
| `useMajorGridLines`, `useHFMajorGridLines` | 주 눈금선 선택; 둘 다 false |
| `show-page-frame`, `show-header-frame`, `show-footer-frame` | 세 프레임 모두 출력에 반영; 기본 false |
| 배경 Paper Color / Gray / White | 화면 배경이며 인쇄 배경색으로 출력하지 않음 |
| 시작 페이지 번호·전체 수 offset | 문서 전체 기준 필드 설정을 전달. 부분 출력에서 1부터 다시 붙이는 것으로 바꾸지 않음 |
| 첫 페이지 별도 머리글/바닥글 | 별도 플래그와 문서 설정을 전달 |
| 파일명·파일 경로·저장 날짜 | 문서의 해당 표시 정보를 전달 |
| 접힌 Area·접힘 아이콘 | 출력 제외 |

명시 페이지 나눔은 포인터 위치에서 삽입한다. 바로 위 Delete 또는 바로 아래 Backspace, 혹은 Remove Space → Empty Space로 제거한다. 이 조사에서는 나눔을 가로지르는 복잡한 도표·표·Area의 모든 배치 전이를 복원하지 않았다.

## 비활성 영역과 다음 계산 구현 기준

`worksheet50.xsd`의 `disableCalcAttr`는 `disable-calc` boolean, 기본 false다. 도움말과 EquationControl의 상태 변경 이벤트를 대조하면 다음 기준을 확보할 수 있다.

1. Disable Region은 선택된 영역에 적용하며 즉시 흐리게 표시한다.
2. 비활성 영역은 편집 가능하나 그 수정이 이후 계산에 적용되지 않는다.
3. Auto Calculation, Calculate Now, Calculate Worksheet 모두 비활성 영역을 실행하지 않는다.
4. 비활성 정의·계산 결과를 필요로 하는 후속 영역은 오류를 보고한다. 이전 정의 값을 계속 유효한 새 바인딩으로 공급하면 안 된다.
5. Area 전체 비활성화와 Area 안의 개별 영역 비활성화는 범위가 다르다.

`EquationControl.HandleCalcIsDisabledChanged` (`0x06000220`)와 SpecTable 대응 메서드 (`0x0600029b`)는 내용·영역 변경 이벤트를 일으킨다. 이는 상태 변경이 계산 입력에 전달되는 정적 근거이며, 엔진 전체의 정밀 의존성 전파가 복원되었다는 뜻은 아니다. 비활성 영역의 표시 결과 캐시 유지·모든 영역 종류·중첩 상위 Area 토글 경계는 구현과 회귀를 별도로 검증해야 한다.

## 확인하지 못한 항목

프린터별 대화상자·스풀 성공·실물 인쇄 품질, 현재 버전의 poster 속성 사용 여부, 드라이버의 인쇄 가능 영역 보정, 전체 Rich Text·모든 원본 도표의 인쇄 fidelity, 모든 페이지 나눔 경계, 정밀 의존성 전파와 계산 취소의 전체 엔진 계약은 미확인이다. 정적 증거가 없다는 이유만으로 기능이 원본에 없다고 결론 내리지 않는다.
