# WASM SDK

2026-10-30. 작업 버전 **0.1.2-rc1 / 프로토콜 1** (공개 릴리즈 0.1.1). 공통 Rust 워크시트·egui 화면·계산기를 실제 브라우저 Canvas에 올리는 SDK다. Obsidian·VS Code 플러그인은 이 SDK를 사용하는 **별도 레포**에서 만든다. 독립판에는 JS 실행 엔진을 추가하지 않았다.

## 빌드와 실행

Rust 1.99.0, Python 3.11 이상, Node/npm을 개발 도구로 사용한다.

```sh
rustup target add wasm32-unknown-unknown
cargo install wasm-bindgen-cli --version 0.2.129 --locked
npm ci --prefix web
python scripts/build-wasm.py
python web/serve.py --fixtures artifacts/rea/unit-selected-sources --output artifacts/rea/wasm-sdk-20261024/browser
```

예제: `http://127.0.0.1:8765/example/`. `?scene=1015_LCC_S.mcdx`로 실제 문서를 열고 `?test=1`로 통합 검사를 실행한다. fixtures 폴더가 없으면 생성된 자체 문서 검증까지는 가능하지만 실제 문서 두 개의 검사는 실패한다. 전체 검증은 해당 입력을 준비한 환경에서 한다. 서버는 테스트 전용 loopback 도구이며 SDK나 플러그인의 실행 의존성이 아니다.

산출물은 `.gitignore`된 다음 경로에 생성된다.

| 경로 | 내용 |
| --- | --- |
| `dist/wasm-sdk/` | `sdk.js`, 생성 TS 타입, WASM/glue, Worker, 선택 한글 글꼴, 라이선스 |
| `dist/mathfolio-wasm-sdk-0.1.2-rc1.tar.gz` | 현재 작업 버전을 다른 레포에서 가져갈 패키지 |
| `dist/wasm-sdk-size.json` | 파일별 바이트·SHA-256, 최적화 전후·gzip·패키지 크기와 도구 버전 |

빌드는 Cargo.lock과 wasm-bindgen CLI 버전 일치를 검사한다. Binaryen 132.0.0의 `-O2`를 적용하고, 더 커지면 최적화 전 파일을 유지한다. 입력·도구·옵션·출력 해시가 일치할 때만 최적화 캐시를 재사용한다. `--skip-opt`는 개발용 비최적화 빌드다. 네이티브 release 프로필은 변경하지 않았다.

## 별도 호스트에서 사용

```ts
import { MathfolioEditor } from "@mathfolio/wasm-sdk";

const editor = await MathfolioEditor.mount({
  canvas,
  wasm: wasmResourceUrl,
  workerUrl: workerResourceUrl,
  fonts: [{ name: "Nanum Gothic", bytes: koreanFontBytes }],
});
editor.setThemeMode("nord"); // 또는 follow_host + setTheme(semanticTokens)
editor.loadDocument("계산서.mcdx", documentBytes);

const stopState = editor.subscribe(state => updateHostDirtyIndicator(state.dirty));
const stopRequests = editor.onRequest(request => handleHostRequest(request));

const packet = editor.prepareSave("계산서.mcdx");
try {
  await writeThroughHost(packet.bytes);
  editor.acknowledgeSave(packet.id, true);
} catch (error) {
  editor.acknowledgeSave(packet.id, false);
  throw error;
}

// 뷰를 숨길 때 false, 다시 보일 때 true. Canvas CSS 크기/DPI는 자동 관찰한다.
editor.setActive(false);
// 탭/뷰 종료 시:
stopState(); stopRequests(); editor.dispose();
```

파일 IO·Vault/workspace 충돌 확인·호스트 저장/복구 정책은 플러그인 책임이다. `prepareSave`는 bytes와 ID/revision을 만들고 실제 파일은 쓰지 않는다. 호스트 쓰기가 성공한 후에만 승인한다. 이전 문서·이전 저장 ID는 거절하고, 쓰는 동안 새 편집이 생기면 dirty를 유지한다. 다운로드 시작은 쓰기 성공으로 취급하지 않는다.

`loadDocument(name, bytes, discardUnsaved)`는 dirty 문서 교체를 기본 거절한다. 명시적 버리기 승인과 입력 검증 후에만 기존 문서를 교체하고, 이전 확인창·속성 초안·계산 전송을 정리한다. `name`은 표시·내보내기 메타데이터이며 filesystem 권한이 아니다. 저장 결과를 여는 것만으로 계산하지 않는다.

`exportDocument("mathfolio" | "mcdx")`는 기존 저장 명령처럼 편집을 확정하고 현재 결과 캐시를 직렬화한다. 저장됨으로 표시하지는 않는다. `exportPdf()`는 계산을 실행하지 않고 기존 표현을 벡터 PDF bytes로 반환한다. 현재 SDK API는 PDF 전체 페이지를 출력하며, OS 인쇄 대화상자·스풀러는 호스트 쪽에서 연결한다.

명령은 `calculate_now`, `calculate_worksheet`, `stop`, `undo`, `redo`, `new`, `open`, `save`, `close`다. 기존 UI/키의 같은 핸들러에 연결한다. UI의 열기·저장·다른 이름 저장·MCDX 내보내기·PDF·인쇄·닫기는 ID가 붙은 `onRequest`로 전달된다. 호스트의 닫기를 브라우저 전체 창 닫기로 바꾸지 않는다.

## 자산·테마·입력

- Canvas에 실제 CSS 너비/높이와 `touch-action: none`을 준다. WebRunner가 resize/DPI를 반영한다. 같은 Rust UI를 그리며 TS에 별도 수식 편집기나 문서 모델을 만들지 않는다.
- `provideFont`에 해당하는 `mount({fonts})`는 mount 전에 bytes를 받는다. 기본 수학 글꼴은 STIX이며, 선택 CJK 자산은 `fonts/nanum/NanumGothic-Regular.ttf`다. 이 글꼴은 WASM 밖에 있으므로 호스트가 적법한 다른 글꼴로 대체할 수 있다. 화면과 PDF에 같은 공급 글꼴을 쓴다.
- `setThemeMode`는 `follow_host`, `light`, `dark`, `nord`다. `setTheme`는 [의미별 RGBA 계약](HOST-THEMES.md)을 따른다. 실제 호스트 CSS 변수 수집·변경 이벤트·설정 저장은 플러그인에 둔다. 문서의 수식·표·강조 색은 유지한다.
- Enter 확정, F5/Ctrl+F5, Ctrl+- 문자 아래첨자, Ctrl+Y Redo 등 기존 의미를 유지한다. 호스트가 먼저 잡는 단축키·실제 OS 한글 IME·클립보드는 두 플러그인에서 추가로 검증한다. 브라우저 검사의 한국어 키 입력은 합성 DOM 이벤트이며 물리 IME 검증을 뜻하지 않는다.
- `subscribe`는 최대10Hz의 작은 상태만 알린다. 호스트 요청은 `onRequest`에서 별도로 전달한다. 문서/AST 전체를 프레임마다 직렬화하지 않는다. `inspectDocument`와 `inspectResults`는 명시적 진단용 snapshot이다.
- `state().selected_regions`는 선택한 영역 ID 목록이다. 드래그 다중 선택·묶음 이동·Copy/Cut/Paste/Delete·Undo/Redo는 네이티브와 같은 Rust 경로를 사용한다. `inspectRegionBounds()`는 마지막으로 측정한 영역의 Canvas 좌표(egui points)를 명시적으로 조회하는 진단 API다. 초기 프레임 이전이나 아직 그리지 않은 영역은 빠질 수 있고, 이동 미리보기 중에는 확정 좌표를 기준으로 한다. 매 프레임 호출하는 API가 아니다. [선택 규칙과 제한](analysis/0216-cad-region-selection.md).

## 계산과 수명

문서·편집·Undo와 최종 결과의 소유자는 Rust 세션이다. UI WASM을 공유하는 여러 세션과 각 계산 Worker의 바인딩은 분리한다. Worker는 첫 계산 작업에서 시작하며 단순 mount/저장 결과 열기로 시작하지 않는다.

동기 계산 중인 Worker는 취소 메시지를 기다리지 않고 `terminate()`한다. 다음 명시 계산이 Worker를 재생성한다. 이전 표시값은 pending으로 보존하되 이전 바인딩을 fresh로 복구하지 않는다. 새 문서/작업 generation과 다른 응답은 현재 결과를 덮어쓰지 못한다. 작업별 전체 확정 계산 입력을 보내며, 소스 delta 프로토콜이나 여러 탭의 공용 Worker pool은 이번 SDK 범위가 아니다.

`dispose()`는 반복 호출 가능하며 이벤트·입력 agent·SDK 타이머·소유 Worker를 정리한다. 이후의 상태 조회/명령은 거절한다. 플러그인이 직접 등록한 Vault/DOM 이벤트는 플러그인도 해제해야 한다.

`memoryBytes()`는 공유 UI WASM 메모리의 크기다. 프로세스 RSS나 뷰별 메모리가 아니며 해제 후 high-water 크기가 줄지 않을 수 있다. state의 `worker_memory_bytes`는 마지막 응답 Worker의 WASM 메모리이고 최초 응답 전에는 모른다. 디스크 용량과 구분한다.

## 호스트 경계와 남은 검증

생성 파일은 ESM이다. 번들러가 `import.meta.url`을 바꾸는 호스트에서는 WASM/Worker 리소스 URL을 명시적으로 전달한다. 기본 Worker는 같은 폴더의 glue/WASM을 로드한다. Blob 또는 전용 Worker entry를 쓰는 호스트는 그 entry에서도 동일 버전의 모듈과 WASM URL을 초기화해야 한다.

검증 호스트는 self script/Worker/connect와 `wasm-unsafe-eval`을 허용하는 CSP에서 실행했다. 일반 JS `unsafe-eval`, CDN, Node 헬퍼, SharedArrayBuffer나 cross-origin isolation은 요구하지 않는다. Obsidian·VS Code의 실제 리소스 URI·Worker/CSP·단축키 중재·Vault/Custom Editor 저장/backup은 각 플러그인 레포에서 검증한다. Obsidian Mobile/VS Code Web은 이번 검증 대상이 아니다.

원본 전체 호환 범위는 [기능 목록](FEATURES.md)을 그대로 따른다. SDK에서 파일 수정시각/로캘 저장 날짜 주입, PDF 페이지 범위 API, 읽기 전용 미리보기 API와 공용 계산 스케줄러는 후속 범위다. 두 호스트 플러그인이 이미 완성됐다고 주장하지 않는다.

0.1.1의 실측 용량·브라우저/네이티브 검증은 [0215 기록](analysis/0215-mathfolio-0.1.1.md), 0.1.2-dev의 다중 선택은 [0216 기록](analysis/0216-cad-region-selection.md)에 보존한다. 0.1.2-rc1의 워크시트 바깥 선택 수정과 검증은 [0217 기록](analysis/0217-rc1-workspace-selection.md)을 따른다.
