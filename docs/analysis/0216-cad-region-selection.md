# 0.1.2-dev: CAD 영역 선택과 묶음 작업

2026-10-30. 공개 버전은 v0.1.1이며 이번 개발분으로 기존 태그나 릴리즈 파일을 교체하지 않는다.

빈 워크시트에서 왼쪽→오른쪽 드래그는 완전히 포함한 영역만, 오른쪽→왼쪽은 걸친 영역까지 선택한다. 위→아래/아래→위 방향은 관계없다. 원본 Prime 12의 선택은 양방향 교차이며, 이번 차이는 사용자가 CAD 규칙을 명시적으로 요청한 결과다. 원본과 같다고 주장하지 않는다. Ctrl 클릭/Shift 선택/Ctrl+A는 이번 구현 범위가 아니다.

선택된 영역의 본문이나 이동 손잡이를 끌면 묶음을 이동한다. 상대 위치를 유지하고, drop 때 한 번만 좌표를 확정하고 Undo를 추가한다. 왼쪽·위쪽 경계 제한과 그리드 보정도 묶음에 같은 delta를 적용한다. 오른쪽 펼친 페이지에서는 A4 너비를 넘어 이동할 수 있다. 드래그 중 종이 크기를 확장하고 가장자리에서 자동 스크롤한다. Escape로 취소하며, 포커스를 잃거나 호스트가 뷰를 비활성화하면 진행 중 제스처를 취소한다.

선택과 이동 미리보기는 문서 소스·dirty·이력·계산 generation을 바꾸지 않는다. 기존 Enter 확정과 키 의미는 유지한다. Copy/Cut/Paste/Delete와 복제·위치 이동 명령은 선택 묶음을 처리하며 Undo/Redo도 묶음을 복원한다. 붙여넣기는 모든 후보와 총 문서 예산을 먼저 검증하고 새 ID로 넣는다. 잘못된 두 번째 항목 때문에 첫 항목만 삽입하거나 ID를 소비하지 않는다.

`mathfolio-regions:` 버전 1 패킷은 상대 위치·소스·수식 형식·계산 비활성·텍스트 서식·행렬 결과 viewport를 담는다. 기존 단일 영역 클립보드도 계속 받는다. 읽기 전용 opaque 영역/접이식 Area의 복사는 지원하지 않으며, 포함돼 있으면 Copy/Cut 전체를 거부하고 원본 선택과 내용을 보존한다. Mathcad OS 전용 클립보드 포맷과의 상호 복사는 검증 범위가 아니다.

구현: `worksheet_ui/region_selection.rs`, `region_clipboard.rs`, 공통 `worksheet_ui.rs`. 네이티브와 WASM 모두 동일한 Rust 경로다. SDK에는 작은 상태의 `selected_regions`와 명시적 진단용 `inspectRegionBounds()`만 추가했다. 네이티브 렌더러·계산기·release 프로필은 변경하지 않았다.

## 회귀 검증

실제 egui 입력을 넣는 네이티브 회귀는 네 방향·0.5/1/2배 zoom·미리보기 비변경·1회 Undo·복사/잘라내기/붙여넣기/삭제·잘못된 패킷 원자성·Escape·가까운 빈 공간·세로/가로 페이지 확장을 검사한다. 수식 뒤에 텍스트를 함께 붙이는 경우의 prospective ID 검증 오류도 이 경로에서 수정했다.

WASM은 최적화한 배포 SDK를 실제 Chromium에 로드하고 DOM pointer/key/clipboard 이벤트를 WebRunner에 전달한다. 클립보드 write stub은 검사 호스트에서만 사용하며 사용자의 OS 클립보드는 변경하지 않는다. 새 15개 검사와 기존 65개 검사를 합쳐 80개를 실행한다. 실제 1015/1016 MCDX 열기·계산·PDF·저장과 기존 단축키도 유지 검사한다. 물리 IME, 실제 Obsidian/VS Code 연결과 모든 OS 클립보드 정책을 검증했다는 뜻은 아니다.

Windows는 2,205개 통과/105개 기존 ignored, peets Linux는 2,206개 통과/103개 기존 ignored다. 양쪽 모두 fmt·clippy(-D warnings, native-profile 포함)·workspace/backend 테스트·debug/release 빌드·CLI 버전·참조 conformance를 통과했다. 로그·소스 manifest·실행 영수증은 ignored `artifacts/multi-selection-20261030/`에 둔다.

양 OS release 앱에서 실제 1015 MCDX와 이름 변경 전 `.mathcraft` 문서를 열고 smoke-test를 통과했다. 최종 최적화 SDK의 Chromium 156 브라우저 검사 80개가 모두 통과했다. REA 근거는 `ev_c6b621b0a5d975cb4d8633bc412b824fb213c12999c29647ca3e7e7ac83c72de`이며 page error는 없었다. 캡처 종료 때 진행 중이던 네트워크 항목은 전체 이벤트 관찰에서 미완료로 보고하므로 이를 완전한 네트워크 캡처로 해석하지 않는다.

배포 WASM은 13,348,275 bytes, gzip 5,466,527 bytes이며 SDK tar.gz는 6,193,829 bytes다. WASM SHA-256은 `7723acf9302570b0e69d8c82059c9f6db41efb94fd23bb8add7e8abbedba8471`, SDK 패키지는 `aea742d9a54f06345503b52924d1681f3f1d59587613aa0d0ce0ebe459c07265`다. 브라우저가 실제 로드한 WASM 해시와 배포 파일 해시가 일치한다.

## 0.1.1 GitHub CI 실패 수정

[v0.1.1 실행 41021673340](https://github.com/Glaysia/mathfolio/actions/runs/41021673340)은 Ubuntu에서 PDF 출력용 한글 글꼴을 찾지 못해 테스트 3개가 실패했다. 컴파일은 성공했고 Windows 작업은 matrix fail-fast로 취소됐다. 기존 테스트를 건너뛰지 않고 CI와 Linux 준비 스크립트에 `fonts-noto-cjk`를 추가한다. CI에서는 기대하는 TTC 파일도 검사하고 `fail-fast: false`로 양 OS 결과를 독립적으로 받는다. 글꼴을 실행 파일에 새로 내장하지 않는다. v0.1.1의 실패 이력 자체는 바꾸지 않는다.
