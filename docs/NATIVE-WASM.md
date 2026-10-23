# 네이티브·WASM 공통 구현 계획

2026-10-23 사용자 합의. 독립 실행판은 브라우저 엔진 없는 Rust 네이티브 앱으로 유지하고, 같은 Rust 구현을 WASM으로 빌드하여 별도 Obsidian·VS Code TS 플러그인에서 사용한다. 이 문서는 구조·작업 순서·검증 기준이며, 아래 목표 구조가 이미 구현됐다는 뜻은 아니다. 화면 설계는 [UI 개선 계획](UI-MODERNIZATION.md), 성능 기준은 [PERFORMANCE.md](PERFORMANCE.md)를 따른다.

## 고정할 원칙

- **네이티브 제품에 JS/TS 실행 엔진을 넣지 않는다.** Electron, Chromium/CEF, WebView, Node.js, V8과 다른 JS 인터프리터를 독립 실행판의 직접·전이 런타임 의존성으로 추가하지 않는다. TS는 플러그인·웹 바인딩에만 둔다. 웹 산출물 제작에 쓰는 개발 도구와 제품 실행 의존성은 구분한다.
- **두 실행 환경, 하나의 제품 구현.** 계산·단위·문서·편집·확정·Undo/Redo·조판·서식·명령 처리는 Rust를 단일 기준으로 삼는다. egui 화면도 공통으로 유지한다. TS에 별도 계산기, 문서 모델, 수식 편집기나 DOM 기반 대체 UI를 만들지 않는다.
- **호스트 차이만 어댑터로 분리한다.** 파일·인쇄·클립보드·글꼴 공급·작업 실행·창 수명은 각 환경에서 구현하되, 계산·편집 의미를 바꾸지 않는다.
- **기존 동작과 성능을 보존한다.** Enter 확정, 저장 결과/Pending, F5/Ctrl+F5, 기존 키, 단위와 MCDX 보존이 우선이다. UI 개편을 이유로 문서를 다시 계산하거나 입력마다 계산을 확정하지 않는다.
- Obsidian·VS Code 플러그인의 본 제품은 별도 프로젝트로 유지한다. Mathcraft 저장소는 공통 Rust 구현, WASM SDK와 작은 호스트 검증 도구를 제공한다. CLI·MCP·skill 확장은 [보관 계획](AGENT-INTEGRATION.md)으로 남긴다.

## 현재 상태 (2026-10-24)

| 확인한 상태 | 후속 범위 |
| --- | --- |
| `mathcraft-core/editor/compat/document`와 같은 egui 화면을 native/WASM에서 사용한다. | 별도 TS 계산기/문서 모델을 만들지 않는다. |
| WASM target의 창/파일 대화상자/직접 OS 저장·인쇄를 제외하고 bytes API와 호스트 요청을 연결했다. | Vault/workspace IO·외부 변경 충돌·backup은 각 플러그인이 연결한다. |
| 같은 Rust 계산 실행을 native persistent thread와 WASM Worker에서 사용한다. Worker 실제 중지·재생성·이전 generation 차단을 브라우저에서 검사했다. | 소스 delta 전송·공용 Worker pool은 후속 범위다. |
| Canvas mount/dispose, 타입이 있는 TS SDK, 글꼴·테마·저장 승인·벡터 PDF API를 제공한다. | 물리 IME·호스트 단축키·리소스 URI/CSP·미리보기는 실제 플러그인에서 검증한다. |
| native 회귀와 실제 Windows 전후 스크롤 비교를 수행했다. native에는 JS 실행 엔진을 추가하지 않았다. | 기존 성능/기능 검사를 유지한다. |

실제 사용/빌드는 [WASM SDK](WASM-SDK.md), 실측·증거는 [0214](analysis/0214-wasm-sdk.md)를 따른다. 아래 단계는 전체 내장 로드맵으로 보존한다. SDK의 브라우저 검증과 두 제품 플러그인의 검증은 서로 다른 완료 조건이다.

## 목표 의존 구조

```text
mathcraft-core / mathcraft-editor
                 ↓
mathcraft-compat / mathcraft-document
                 ↓
공통 Rust 편집 세션 + egui 화면 + 계산 작업 계약
                 ↑                         ↑
네이티브 어댑터                         WASM 바인딩
eframe 창·OS IO·계산 thread             WebRunner·Canvas·계산 Worker
                 ↑                         ↑
Windows / Kubuntu 독립 실행판           공통 TS SDK
                                           ↑
                               Obsidian / VS Code 별도 플러그인
```

위 이름은 책임 구분이며 새 crate 이름을 확정한 것은 아니다. 기존 crate 안에서 작은 모듈로 경계를 검증한 후, 실제 의존성 차단에 필요한 부분만 crate로 추출한다. 코어가 Obsidian·VS Code·DOM·OS 창 타입을 참조하는 역방향 의존성은 허용하지 않는다.

| 계층 | 포함할 책임 | 경계 밖의 책임 |
| --- | --- | --- |
| 공통 모델·엔진 | 원본 문서 표현 보존, 계산·단위, 수식 편집, 변경 이력, 계산·저장 상태, 벡터 출력 생성 | OS 경로, Vault, Webview 메시지, 파일 선택 창 |
| 공통 편집 세션 | 문서 revision, 미확정 입력, 선택·포커스, 명령 적용, 결과 게시 판단 | 실제 파일 쓰기, 창 종료, host별 Undo 원장 |
| 공통 egui UI | 종이·수식·도구·속성·도움말·테마 토큰, 입력 의미·가시 영역 캐시 | 호스트 앱 API, 네이티브 창 생성, JS 호출 직접 실행 |
| 네이티브 어댑터 | 창·렌더러, 파일 IO/원자적 저장, OS 글꼴·클립보드·인쇄, Rust 계산 thread | 웹 UI·JS 런타임 |
| WASM 어댑터 | bytes API, Canvas 부착/해제, 호스트 요청/응답, Worker 전달, 브라우저 입력·IME 접점 | 계산·편집 로직의 TS 재작성 |
| 공통 TS SDK / 호스트 플러그인 | WASM 로딩, host IO·테마·크기·활성 상태·이벤트 연결 | Rust와 경쟁하는 별도 문서 상태나 자동 계산 규칙 |

## 호스트 API 계약

테마 경계의 실제 Rust 구현과 모드/Nord·색 토큰 계약은 [HOST-THEMES.md](HOST-THEMES.md)를 따른다. host CSS 수집·WASM 바인딩·플러그인 실행은 여전히 후속이다.

세션 단위로 명시적인 API를 제공한다. 아래 이름은 설계 예시이며 구현·타입 생성 때 확정한다.

| 조작 | 계약 |
| --- | --- |
| `mount` / `dispose` | Canvas와 편집 세션을 연결하고, 해제 시 listener·repaint·timer·Worker·렌더러 리소스를 정리한다. 해제 후 도착한 응답은 무시한다. |
| `loadDocument(bytes, format, metadata)` | 파일 경로 없이 기존 importer를 호출한다. 제목·수정일 등 필요한 metadata는 호스트가 전달한다. 실패하면 현재 문서와 미저장 입력을 보존한다. |
| `dispatch(command)` | 버튼·도움말·호스트 메뉴가 기존 Rust 명령 핸들러를 호출한다. 명령 ID·문맥·활성 조건·설명은 공통 catalog에서 얻는다. |
| `exportDocument(format)` / `ackSaved(revision)` | 특정 revision의 저장 bytes를 반환한다. 호스트가 실제 쓰기에 성공한 뒤에만 해당 revision을 저장됨으로 승인한다. 쓰는 동안 새 편집이 생기면 dirty를 유지한다. |
| `setTheme` / `setViewport` / `setActive` | 호스트 테마·크기·DPI·가시성·활성 상태를 반영한다. 문서 원래 색·서식은 테마에 따라 바꾸지 않는다. |
| `provideFont` | 호스트가 적법하게 공급한 글꼴 bytes를 설치한다. native OS 경로 로더와 분리하며 한글·수학 글리프와 라이선스 고지를 검증한다. |
| 상태 구독 / 호스트 요청 | dirty·revision·선택 문맥·계산 중/오류 같은 작은 상태만 변경 시 알린다. open/save/print/외부 링크 요청에는 request ID를 부여하고 호스트가 결과를 회신한다. |

저장에 필요한 canonical 문서와 편집 이력은 Rust 세션이 소유한다. Worker는 revision이 붙은 계산용 복제본을 가지며 독립적으로 문서를 수정하지 않는다. 매 프레임 전체 문서 JSON이나 AST를 TS로 왕복시키지 않는다. 전체 bytes 이동은 열기·명시 저장·내보내기와 복구 시점에 한정하고, Worker의 최초 입력 이후에는 revision에 맞는 재사용·변경 전달을 검증한다.

호스트 프로토콜 버전과 WASM/TS SDK 버전을 묶는다. 불일치·미지원 기능·실패·취소를 구분한 결과를 제공하고, 생성한 `.d.ts`와 실제 바인딩이 일치하는지 검사한다. host capability에 없는 인쇄·쓰기 기능은 이유를 표시하며 실행 가능한 척하지 않는다.

## 계산과 반응성

계산 작업의 입력·출력·오류·generation 계약은 공통 Rust로 둔다. 네이티브는 기존 persistent Rust thread를, WASM은 별도 Web Worker 안의 같은 계산 엔진을 사용한다. UI 스레드에서 긴 동기 계산을 실행하지 않는다.

- 결과에는 session·문서 revision·작업 generation을 붙인다. Undo·재열기·다른 문서 전환·새 계산 뒤 도착한 낡은 결과가 현재 상태를 덮어쓰지 않게 한다.
- F5·Ctrl+F5·자동/수동·중지/재개와 미확정 입력의 기존 의미를 공통 계약 테스트로 고정한다. 단순 스크롤·포커스·테마 변경은 계산 제출 사유가 아니다.
- **Worker에 취소 메시지를 보내는 것만으로 중지를 구현했다고 하지 않는다.** 공유 메모리 없는 Worker에서 동기 WASM이 실행 중이면 메시지 처리가 밀릴 수 있다. 초기 경로는 작업 generation 폐기와 Worker 종료/재생성으로 실제 중지를 보장하고, 확정된 Rust 세션으로 계산 상태를 재구성한다. 재개에 필요한 체크포인트와 표시 상태를 검증한다. 복원 비용이 기준을 넘으면 Rust 계산의 협력적 작업 분할을 먼저 구현한다.
- SharedArrayBuffer·cross-origin isolation을 필수 조건으로 삼지 않는다. 원자적 취소나 WASM 멀티스레드는 두 호스트의 실제 지원을 확인한 뒤 선택 최적화로 검토한다.
- 문서별 작업과 캐시를 격리한다. 여러 탭·미리보기의 동시 계산 수를 제한하고, 숨김 상태에서 불필요한 repaint를 멈춘다. 취소/복구 중에도 UI의 선택·Undo·미저장 문서는 유지한다.

## UI/UX와 키 입력

[UI 개선 계획](UI-MODERNIZATION.md)의 문서 중심 도구 모음·문맥 속성·공통 시각 토큰·검색 도움말을 **Rust egui에서** 구현한다. Excalidraw는 사용 경험의 참고이며 React/HTML/CSS로 화면을 다시 작성하는 지시가 아니다. 웹 호스트 DOM은 Canvas container와 필수 입력·접근성 연동을 담당한다.

- 버튼·메뉴·도움말은 같은 명령 정의와 활성 조건을 사용한다. 기존 기능에 계속 접근할 수 있게 하고, 주요 도구를 줄이면서 고급 기능을 누락하지 않는다.
- 기존 키를 재배정하지 않는다. `Ctrl+-` 이름 아래첨자, `Ctrl+Y` Redo, 기존 `Ctrl+Shift+Z` 무한대 입력, F1 도움말, Ctrl+F1 도구 모음 접기를 구분한다. Enter 확정·행렬 Tab·IME와 문맥별 키는 실제 처리 코드 기준으로 안내한다.
- 편집기 안에서도 검색·속성 입력·도움말이 키를 소유할 때 수식이나 전역 명령이 함께 실행되지 않게 한다. 포커스를 잃으면 눌림 상태를 정리한다.
- 플러그인에서는 활성 편집 표면에서 처리한 키만 소비한다. VS Code host keybinding이 Webview보다 먼저 처리하는 경우를 실제로 확인해, 해당 편집기 문맥으로 범위를 좁힌 연결을 한다. Obsidian의 전역 단축키도 덮어쓰지 않는다.
- 휠·가로 스크롤·IME 조합·클립보드·고해상도 글꼴·DPI·키보드 포커스·접근성 이름을 양 실행 환경에서 검사한다. 읽기 전용 미리보기는 편집 명령을 소비하지 않는다.

## 두 플러그인의 최소 검증 범위

| 호스트 | 검증할 연결 | 완료 판정 |
| --- | --- | --- |
| Obsidian Desktop | custom View와 Vault bytes IO, 테마·크기·포커스, 노트 안 읽기 전용 미리보기 | 열기→편집→확정→저장→재열기, 노트 전환·동시 탭·외부 변경 충돌·닫기 정리가 동작한다. |
| VS Code Desktop | 바이너리 문서용 Custom Editor/Webview, workspace 파일 IO, save/revert/backup·호스트 Undo 연결, 테마·수명 | 같은 문서 흐름과 host Save/Undo/Redo·분할 화면·숨김/복원·종료 복구가 동작한다. |

VS Code host Undo와 Rust Undo를 중복 기록하지 않는다. 첫 검증에서는 문서당 하나의 편집 세션·하나의 쓰기 권한을 기준으로 삼고, 다른 뷰는 읽기 전용 또는 동일 세션 관찰로 연결한다. 별도 세션 두 개가 같은 파일을 조용히 덮어쓰지 않게 한다. 외부 파일 수정 시 충돌을 알리고 기존 미저장 상태를 보존한다.

웹 자산은 플러그인에 포함하여 오프라인으로 로드한다. VS Code CSP·리소스 URI와 Obsidian의 Worker/WASM 자산 로딩을 실제로 확인한다. 임의 CDN·localhost Rust 서버·네이티브 헬퍼 설치를 정상 편집의 필수 조건으로 삼지 않는다. 브라우저판 VS Code·Obsidian Mobile·마켓 배포는 이번 Desktop 검증 완료 조건에 포함하지 않는다.

PDF의 벡터 생성은 공통 Rust에 유지한다. 네이티브는 OS 인쇄를 실행하고 웹 호스트는 반환된 PDF bytes를 저장하거나 지원하는 출력 경로에 연결한다. Canvas 캡처로 PDF를 대체하지 않는다.

## 실행 순서와 단계별 통과 조건

| 순서 | 구현 범위 | 다음 단계로 넘어갈 증거 |
| --- | --- | --- |
| 0. 기준선 보존 | 현재 Windows/peets release, 1015·1016 표본과 입력/저장/스크롤 자료를 고정한다. 진행 중 library 추출을 검증한다. | 기존 회귀 검사 통과, 동일 입력·계산·저장 결과, 스크롤 기준 자료와 실행체 해시 |
| 1. native 경계 분리 | 공통 세션·bytes importer/exporter·호스트 요청·계산 작업 계약을 추출한다. 기존 native 핸들러를 어댑터에 연결한다. | native에서 열기/저장/인쇄/Undo/취소가 유지되고 공통 코드에 호스트 역참조가 없음 |
| 2. 실제 WASM 수직 검증 | 실제 공통 egui 화면을 브라우저에 부착한다. 한글/수식·입력/IME·열기/저장 bytes·Worker 계산/중지를 연결한다. 더미 계산기로 대신하지 않는다. | WASM release 빌드와 브라우저 실행, native와 같은 문서/명령 결과, Worker 중지·반복 mount/dispose 검사 |
| 3. 호스트 계약 검증 | 생성 TS 타입·SDK와 양 Desktop 최소 어댑터를 연결한다. 파일·키·복구·수명 문제를 먼저 드러낸다. | 두 호스트의 실제 열기→편집→저장→재열기 및 포커스/취소/닫기 기록 |
| 4. 현대적인 UI 적용 | 검증된 공통 화면에 토큰·도구 모음·속성·도움말·툴팁을 작은 단위로 적용한다. | 기능 접근성·기존 키·수식 조판·사용감과 Windows 스크롤 유지 |
| 5. 통합·배포 경계 고정 | native 두 OS와 두 호스트에서 같은 cohort를 돌리고 SDK 버전·빌드·제약을 기록한다. | 아래 완료 기준과 재현 가능한 빌드/검증 문서, 별도 플러그인 프로젝트가 사용할 SDK 산출물 |

0~1은 네이티브 동작을 유지하는 추출부터 한다. 2~3에서 내장에 필요한 경계가 검증되기 전에 큰 UI 재작성을 시작하지 않는다. 문제가 생기면 기존 네이티브 경로를 유지하며 경계를 고치고, JS 엔진 도입이나 TS 편집기 재작성으로 우회하지 않는다.

## 검증과 완료 기준

1. **네이티브 순수성:** Windows/Kubuntu 배포 파일·직접/전이 런타임 의존성·로드 모듈/자식 프로세스를 검사한다. JS 실행 엔진·브라우저·WebView·웹 자산 서버 없이 시작·열기·편집·계산·저장·인쇄가 동작한다. Cargo feature/target별 의존성 검사도 자동화한다.
2. **의미 일치:** 같은 파일과 명령 시퀀스에서 native/WASM의 구조 수식·단위·결과·오류·Pending·확정·Undo/Redo를 비교한다. ZIP metadata 등 정상 차이는 구분하고, MCDX 의미·보존 필드·재열기 결과를 검사한다. 화면 픽셀만으로 계산/저장 동등성을 판정하지 않는다.
3. **회귀:** 기존 native 테스트·참조 계산·문서 왕복·벡터 PDF 검사를 유지한다. 새 호스트 계약에는 저장 실패/취소, 오래된 저장 승인·계산 응답, 잘못된 입력 bytes, 해제 뒤 callback, 다중 세션 격리 검사를 추가한다.
4. **성능:** [0211](analysis/0211-windows-scroll-renderer-fix.md)의 native release와 같은 조건으로 p50/p95/p99·최대 간격·UI 준비 비용·메모리를 비교한다. 새 UI의 p95가 기준보다 10% 이상 악화하면 동일 조건에서 반복하여 원인을 해소하거나 근거를 기록하기 전에는 완료로 처리하지 않는다. 이 10%는 앞으로 적용할 회귀 기준이며 과거 측정 결과가 아니다. WASM/호스트 성능은 별도 기준선을 만들고 긴 계산 중에도 UI가 반응하는지 확인한다. 실제 표시와 CPU/headless 수치를 구분한다.
5. **수명·입력:** 반복 열기/닫기 뒤 살아 있는 Worker·listener·세션 수가 기준으로 돌아오는지 검사한다. 두 호스트에서 IME·기존 단축키·테마·DPI·리사이즈·포커스·스크롤을 실제 실행해 확인한다.
6. **산출물:** 독립판 release, WASM/JS 바인딩/생성 TS 타입/Worker/필요 자산을 묶은 버전 고정 SDK, 최소 호스트 예제, 의존성·글꼴 고지, 실행체/입력 해시와 검사 결과를 남긴다. SDK는 UI와 Worker의 WASM 인스턴스별 메모리도 계측한다. 기능/테스트 개수를 완성률로 환산하지 않는다.

단계별 상태는 미착수·진행·검증 완료로 기록하고, 플랫폼별 실제 실행 증거를 연결한다. 타깃 설치·빌드 성공·브라우저 실행·플러그인 실행·사용감 확인은 각각 다른 완료 조건이다.

## 설계 근거

- [eframe 0.36.2](https://docs.rs/eframe/0.36.2/eframe/): 같은 App의 native 실행과 WebRunner 경로, WASM 바인딩 빌드 안내.
- [wasm-bindgen 공식 가이드](https://rustwasm.github.io/docs/wasm-bindgen/): Rust/Web 바인딩과 Worker 통합. 바인딩은 WASM 타깃에만 둔다.
- [VS Code Custom Editor](https://code.visualstudio.com/api/extension-guides/custom-editors): 문서·편집 뷰·저장/Undo/backup의 host 계약. 단순 Webview만 띄운 것을 편집기 통합 완료로 보지 않는다.
- [Obsidian 공식 API](https://github.com/obsidianmd/obsidian-api/blob/master/obsidian.d.ts): custom View·Vault·Markdown renderer·lifecycle 접점.
- [PhotoCraft](https://github.com/storytold/photocraft#under-the-hood)와 [craft-ui](https://github.com/storytold/craft-libs#readme)는 native/WASM 공통 Rust와 egui 구성의 참고 사례다. 해당 프로젝트의 기능 완성도나 구현을 Mathcraft의 검증 증거로 대신하지 않는다.
