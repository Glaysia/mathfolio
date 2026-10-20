# Area 안의 일반 Enter와 삽입 리본

2026-10-19. Mathcad Prime 12.0.0.1 설치 파일을 정적으로 확인했다. GUI·스크린샷·원본 프로세스·CLR 실행은 하지 않았다. [0138](0138-area-authoring-contract.md)의 새 Area 생성 계약을 보충하며, 제품 구현 완료를 의미하지 않는다.

## 일반 수식 Enter 이후의 위치

Area의 일반 Enter는 자식 수식을 확정한 뒤 계속 그 수식에 머무는 계약이 아니다. 수식 내부 명령이 Enter를 소비하지 않고 `EnterParagraphBreak`가 컨테이너에 도달하면, Area 내부의 다음 삽입점으로 이동한다. 프로그램 줄 분할 등 수식 내부 Enter는 [0042](0042-edit-commit-contract.md)의 선행 소비를 그대로 구분해야 한다.

Whiteboard `SetupCommandBindings` `0x0600026c` IL00bc–00d4는 `EditingCommands.EnterParagraphBreak`를 `CarriageReturnCommand`에 연결한다. Mathboard의 `SetupCommandBindings 0x060003bb`와 Area의 `BindCommands 0x060000aa`/control `0x06000017`에는 이 명령을 교체하는 분기가 없다. Area는 Whiteboard의 `HandleCarriageReturn 0x06000247`을 상속한다.

| 상태 | 설치 CIL의 전이 |
| --- | --- |
| active/selected child 없음 | `IsNullOrEmpty`가 true이면 `InsertVerticalSpace(1,1,false)`; 자식 수식 생성이 아니다. |
| active/selected child 있음 | `x = min(GetItemLocation.X)`, `y = max(GetItemRect.Bottom) + GridCellSize.Height / 2`를 계산하고 `InsertionPoint`를 설정한다. |
| Area의 위치 보정 | property setter `01b4`는 virtual `SetInsertionPoint`를 호출하므로 Area `00c5`의 grid snap 및 top/bottom 경계 보정을 거친다. y=0이면 한 grid 아래, y=ContentHeight이면 한 grid 위로 보정하는 기존 0138 계약이다. |
| 새 위치가 빈 공간 | base `SetInsertionPoint 01b5`는 saved state·선택을 지우고 crosshair를 표시하며 `ActiveItem=null`로 바꾼다. 내부 수식 편집을 떠나지만 Area 소유 삽입점은 유지한다. |
| 새 위치에 활성화 가능한 child 있음 | 그 child에 local 좌표 클릭을 전달하여 활성화한다. 언제나 빈 삽입점이라는 보장은 없다. |
| 아래 경계 도달 | `0247` 후반은 directional grid intersection의 Y와 ContentHeight를 비교하여 `InsertSpaceCommand(1,1,Empty<IWhiteboardItem>())`를 실행하는 공간 확장 경로를 가진다. 단순 좌표 clamp만으로 전체 계약을 대체할 수 없다. |

`GetItemRect`는 item location의 Y에 `VerticalAlignmentOffset`을 더한 후 render size로 rect를 만든다 (`0147`). 그러므로 계산된 수식의 glyph 높이만 사용하거나 기존 자식 top에 고정 grid 한 칸을 더하는 것은 위 계약과 다르다. 자식 비활성화 시 Mathboard `03b7`의 미확정 내용 Commit 경로는 0042에 기록되어 있다. 이 문서는 Enter 한 번이 모든 종속 계산의 완료까지 동기 보장한다는 주장을 하지 않는다.

일반 워크시트 최상위의 `WorksheetViewModel.HandleEnter 0x06000a3e`는 별도다. 선택된 영역들의 최소 X와 최대 `(sectionY + renderHeight + VerticalAlignmentOffset)`에 **grid 한 칸 전체**를 더한다. Area의 상속 Whiteboard 경로는 **반 칸을 더한 후 grid snap**한다. 두 경로를 혼합하지 않는다.

## 원본 리본 위치

설치된 en-US `MathcadPrime.resources.dll`의 `ribbonresources.baml`에는 Document RibbonTab(offset9388, label9416, KeyTip D9431)의 첫 RibbonBar로 `DocumentRegionsRibbonBar`(9458)가 있다. `documentregionsribbonbar.baml`에는 다음 정보가 있다.

| BAML offset | 내용 |
| --- | --- |
| 1968 | group Header=Regions |
| 2380 | Name=Part_DocumentRegionsRibbonBar_Area |
| 2465 / 2527 | Area / KeyTip Q |
| 2595 | Command=Worksheet:WorksheetCommands.NewCollapsibleArea |
| 2669 | ImagePosition=Top |
| 2847 / 2918 | region_area_32x32.png / region_area_16x16.png |
| 3086 | tooltip Header=Area (Ctrl+Shift+A) |

따라서 `Document → Regions → Area`는 원본 근거가 있는 노출 위치다. command 활성 조건과 empty Area 생성 동작은 0138을 사용한다. 이 BAML 확인은 임의의 선택 영역 감싸기 기능을 승인하지 않는다.

## 증거와 한계

메서드별 원시 본문과 SHA는 ignored `artifacts/rea/area-enter-method-ledger-20261019.json`, `feedback/area-enter-*-cil-20261019.txt`에 있다. decoder는 보존한 REA metadata의 설치 어셈블리 SHA 및 선택 IL SHA를 실제 설치 bytes와 재검증했다. Whiteboard 어셈블리 SHA는 `947ae0dea0c7a7fdbd1c377e4e7f2447dccd5e839111fec0c0789dafffc1cbd9`, Worksheet는 `c1a70ec8007228ff119d4a31b36b9062132be3a10c771459c1dbe3fd0a7baddd`, Area는 `63418ae37c2e16fc49398205f28279dc0d1b7d5301940458497a504309be5590`이다. REA identities는 0138과 같다.

주요 IL SHA: `0247`=`66737f1957daadf0e3cd7bc8d29d4927c6c31fcd01dac19b6eff29e1c079df89`, `0248`=`d0646323538c5c0d7056ad0041a0c1b8bf0a475b457b222c7e703e2944f13d88`, `01b5`=`79d6d02f56078be78d073c9100e7cd3d2d264158436513c46053bd2b2a2aa092`, `0a3e`=`a92ba0f6dbbb1c1c27a6c2df3c52812894d86ac290bba6e29761cd63f345cc50`.

Satellite resource assembly SHA=`5a7b18a5dbe905fc121dba6f4bb64def8f367161292db5114d3cafe0f5e927f4`. Raw prefixed BAML streams, decoded offsets and resource hashes are in `area-enter-ribbon-identities-20261019.json` and `area-enter-*-baml-20261019.json`. These are static resource records, not screen observations.

전체 routed input 우선순위, 모든 IME/수식 내부 Enter, 특수 컨트롤의 Enter, 원본 command history grouping과 자동 공간 확장의 최종 소스 geometry는 이번 범위에서 완전히 추적하지 않았다. 새 child가 commit된 뒤 빈 삽입점으로 이동하는 일반 양성과, 기존 equation 내부 Enter 소비를 구분하는 범위가 구현 가능한 계약이다.
