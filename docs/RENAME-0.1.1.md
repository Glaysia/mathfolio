# Mathfolio 0.1.1

2026-10-30. 프로젝트와 제품 이름을 Mathfolio로 변경한다. 새 이름과 버전을
앱 제목·대화상자·CLI·Rust 크레이트·SDK·새 문서 메타데이터·배포 파일에 적용한다.

| 구성 | 현재 이름 |
| --- | --- |
| GitHub | `https://github.com/Glaysia/mathfolio` |
| 네이티브 앱 / CLI | `mathfolio` / `mathfolio-cli` |
| Rust workspace | `mathfolio-{core,editor,compat,document,app,cli}` |
| 자체 문서 기본 확장자 | `.mathfolio` |
| WASM SDK / 클래스 | `@mathfolio/wasm-sdk` / `MathfolioEditor` |
| 생성 WASM / glue | `mathfolio_bg.wasm` / `mathfolio.js` |
| 버전 | Cargo와 SDK 모두 `0.1.1`, SDK 프로토콜 `1` 유지 |

기존 `.mathcraft` 파일은 열기와 저장을 계속 지원하며, 해당 파일을 그대로
저장할 때 확장자를 강제로 바꾸지 않는다. JSON 문서 버전은 2다. 저장 결과의
해시 도메인, Area 임시 식별자와 구조화 클립보드 접두사는 기존 바이트를
유지한다. 이들은 이미 저장된 문서·결과와 통신하는 형식이므로 제품 이름과
함께 바꾸면 기존 데이터가 무효가 된다.

SDK의 `exportDocument("mathfolio")`를 기본으로 사용한다. 이전
`exportDocument("mathcraft")`와 `MathcraftEditor` 별칭도 같은 구현에 연결한다.
새 연동 프로젝트는 새 패키지·리소스 이름을 사용한다. 실제 Obsidian·VS Code
플러그인은 계속 별도 레포에서 개발한다.

`tests/reference/legacy/pre-rename.mathcraft`는 2026-10-19에 작성된 합성 문서를
그대로 고정한 독립 호환 표본이다. 이름 변경 후 같은 구현으로 재생성하지
않는다. 두 확장자의 열기·저장, 캐시 7행 유지와 새 MCDX 제품 메타데이터를
검사한다. SDK 브라우저 검사에도 기존 클래스·형식 별칭과 확장자 검사를 추가했다.

`docs/analysis`, 고정 검증 기록·원본/참조 자료와 이미 공개한 `v0.1.0`은 당시의
명칭·명령·측정 경로를 유지한다. 로컬 개발 폴더 및 설치된 MSVC의 물리 경로도
현재 존재하는 경로를 그대로 기록한다. 현재 사용법은 README와 WASM SDK 문서를 따른다.

이 변경은 계산 엔진, 수식 편집 의미, 단축키, 네이티브 렌더러 선택과 릴리즈
최적화 설정을 변경하지 않는다. 네이티브에 웹 실행 엔진을 추가하지 않는다.
