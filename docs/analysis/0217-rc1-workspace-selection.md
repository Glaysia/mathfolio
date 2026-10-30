# 0.1.2-rc1: 모눈 바깥에서 드래그 선택 시작

2026-10-30. 사용자 재현: 모눈/계산 본문 밖에서 사각형 선택을 시작할 수 없었다. `selection_pointer_after`가 시작 위치에 `PageGeometry::to_body`를 요구해 페이지 여백·페이지 사이·종이 바깥 워크시트 캔버스를 거부했다.

선택 시작은 보이는 워크시트 viewport의 빈 공간을 기준으로 한다. 본문 좌표 검증은 기존 내용 삽입 경로에 유지한다. 영역 본문·이동 손잡이와 페이지 펼치기 버튼은 빈 공간으로 처리하지 않는다. viewport 밖이나 popup이 열린 상태에서도 새 선택을 시작하지 않는다. CAD 방향 규칙·묶음 작업·드래그 중 소스 비변경 계약은 [0216](0216-cad-region-selection.md)을 유지한다. 네이티브와 WASM은 같은 Rust 수정을 사용한다.

수정 전 회귀 검사에서 왼쪽 여백으로 시작하는 50% 배율 선택이 실패하는 것을 확인했다(`regression-before.log`). 새 검사는 50/100/200% 배율에서 왼쪽·위쪽·오른쪽 여백과 종이 바깥 네 방향, 페이지 사이 빈틈, viewport 밖 입력 배제, 페이지 펼치기 버튼의 정상 클릭을 확인한다. 선택만으로 소스·Undo·dirty·계산 generation이 바뀌지 않는지도 검사한다.

추가 회귀에서 떠 있는 스크롤바 클릭도 사각형 선택으로 처리하는 것을 재현했다(`scrollbar-before.log`). 스크롤바는 viewport 안에 겹쳐 그려지므로 clip 검사만으로 제외되지 않았다. ScrollArea가 등록한 실제 상호작용 사각형을 해당 레이어 좌표로 검사해 시작을 제외한다. 고정 너비의 빈 띠를 임의로 막지 않는다. 새 검사는 스크롤바가 계속 스크롤하고 소스·이력은 그대로인지를 확인한다.

최종 네이티브 선택 회귀는 13개다. Windows는 전체 2,210개 통과/105개 기존 ignored, peets Linux는 2,211개 통과/103개 기존 ignored다. 양 OS에서 fmt·clippy(-D warnings, native-profile 포함)·workspace/backend 테스트·debug/release 빌드·CLI 버전·참조 conformance를 통과했다. 최종 release 앱의 실제 1015 MCDX와 이전 `.mathcraft` 문서 열기 smoke 검사도 확인했다.

최종 최적화 SDK를 Chromium 156에 로드해 기존 검사와 새 여백/종이 바깥 DOM pointer 검사를 합쳐 84개가 통과했다. REA 근거는 `ev_94c6e830bcafa32aebebdc5ac3281644feb02f95879f1c6cff312e12e61303b6`이며 page error는 없었다. 이는 합성 DOM 이벤트와 실제 WebRunner의 통합 검사이며 물리 마우스/IME 또는 Obsidian·VS Code 플러그인 검증을 뜻하지 않는다.

WASM은 13,349,819 bytes(gzip 5,466,204), SDK tar.gz는 6,193,592 bytes다. 실제 로드된 WASM SHA-256은 `92e7bf21f8ca4643d7860154a29253ba9b133c662390337dafcf11e6eed0fa3b`, SDK 패키지는 `279958f5235e9133830f5cea2c25af4e4d299ba1cf6c2420285317b3ecaf7ccf`다. 패키지 파일과 브라우저 로드 파일의 동일성을 해시로 확인한다.

로그와 소스 manifest·실행 영수증은 ignored `artifacts/rc1-20261030/`에 둔다. 사용자 요청에 따라 이 rc1 변경에는 GitHub CI를 실행하지 않으며 커밋에 `[skip ci]`를 붙인다.

## 이전 CI 및 실행 파일 정리

0.1.2-dev 커밋 `110419d`의 [GitHub CI 41053633594](https://github.com/Glaysia/mathfolio/actions/runs/41053633594)는 Windows·Ubuntu 모두 성공했다. 이는 이번 rc1의 CI 결과를 뜻하지 않는다.

로컬 `target/release`와 `target/debug`에 남은 `mathcraft.exe`, `mathcraft-cli.exe` 총 4개를 삭제했다. 현재 실행 파일 이름은 `mathfolio.exe`, `mathfolio-cli.exe`다. `mathcraft`라는 물리 프로젝트 폴더와 이전 문서/클립보드 호환 처리는 유지한다.
