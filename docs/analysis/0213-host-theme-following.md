# 호스트 색상 연동과 Nord

2026-10-23. Obsidian/VS Code의 테마 색상이 자연스럽게 적용되는 옵션과 사용자의 Nord 선호를 반영했다. UI의 `테마` 메뉴에 호스트 따르기·밝게·어둡게·Nord를 제공한다. 작은 화면에서는 파일 메뉴 안에 있다.

색은 dark/light 여부만이 아니라 배경·표면·글자·보조 글자·테두리·강조·주 버튼의 배경/전경·hover·선택·focus·오류/경고로 나눠 받는다. 고대비·UI 글자 크기·모서리도 받는다. Nord 프리셋은 공식 팔레트의 Rust 의미 매핑이며 출처·MIT 고지를 보존했다. 호스트 따르기는 특정 Nord 이름이나 색으로 강제하지 않고 실제 전달된 토큰을 적용한다.

공개 Rust API와 wire 값, 두 호스트의 CSS 변수 매핑 기준은 [HOST-THEMES.md](../HOST-THEMES.md)를 따른다. 초기 모드는 호스트 따르기이며 미연결 시 기본 테마를 표시한다. 수동 선택은 live host 갱신으로 덮어쓰지 않고, 다시 호스트 따르기를 선택하면 최신 값을 적용한다. 동일 토큰은 style 재설정·추가 repaint를 요청하지 않는다. 모드는 표면별로 격리하고 잘못된 입력은 원자적으로 거절한다.

테마 데이터는 문서 바깥의 UI context에 둔다. 미확정 수식·문서 색·Undo·계산 generation·확대율을 바꾸지 않는다. host CSS 수집·변경 이벤트·모드 설정 저장은 각 플러그인 레포에서 구현하며 이번 저장소에는 플러그인·DOM/CSS 해석기·JS 실행 엔진을 넣지 않았다. 네이티브 모드 선택의 디스크 저장과 실제 WASM/두 플러그인 실행은 이번 변경에서 검증하지 않았다.

새 테스트 8개는 custom 색과 고대비, 실제 계산 버튼의 전경/배경 적용, 수동 override/최신 값 복원, invalid/동일 입력, 직렬화와 표면 격리, RGBA 투명도 보존, 독립 Nord 모드, 작성 중 live theme 변경의 문서 불변을 검사한다. Windows fmt·workspace/all-targets/native-profile Clippy·workspace/egui-winit 테스트·참조 계산을 통과했다. peets도 같은 코드의 `scripts/check.sh`를 통과했다. 플랫폼별 receipt와 실행체 정보는 `artifacts/rea/host-theme-20261023/`에 둔다. GUI 색상 일치를 두 호스트에서 실측했다는 뜻은 아니다.
