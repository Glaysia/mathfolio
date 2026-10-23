# 0214 — 실제로 로드하는 WASM SDK

2026-10-24. 사용자 요청: 이 레포에서 SDK를 만들고 실제 로딩과 용량을 확인하되, 독립판의 기능·성능을 유지한다. 두 제품 플러그인은 만들지 않는다.

## 결과와 경계

`mathcraft_ui`를 `wasm32-unknown-unknown` release로 빌드하고, 실제 egui/WebRunner Canvas와 같은 Rust 계산 실행을 Web Worker에 연결했다. 더미 계산기나 TS 편집기를 만들지 않았다. 공개 인터페이스와 재현 명령은 [WASM-SDK.md](../WASM-SDK.md)에 있다.

공통화한 부분은 bytes 가져오기/내보내기, 가져온 문서 설치, 기존 수식/표/텍스트 편집, 결과/Pending·계산 명령, 저장 승인, 테마/글꼴과 벡터 PDF다. 네이티브 계산 스레드는 별도 transport로 옮겼고 실행 함수는 공통으로 유지했다. WASM 전용 메서드와 의존성은 target 조건으로 제외한다. native의 창·렌더러 선택과 단축키를 바꾸지 않았다.

SDK는 ID/revision 기반 저장 승인, dirty 문서 교체 거절, 이전 문서 작업/저장 응답 차단, 독립 세션, 명시적 dispose를 제공한다. 단순 mount/저장 결과 열기로 Worker를 시작하지 않는다. TS 코드는 로딩·타입·수명과 작은 상태 전달만 맡는다.

## 용량

Rust 1.99.0, wasm-bindgen 0.2.129, Binaryen 132.0.0 `-O2`. MB는1,000,000바이트 기준이다. 최신 파일별 바이트와 SHA-256은 `dist/wasm-sdk-size.json`에 생성한다.

| 산출물 | 실측 크기(반올림) |
| --- | ---: |
| 최적화 WASM | 13,255,658바이트(13.26MB) |
| WASM gzip | 5,425,457바이트(5.43MB) |
| JS/타입·선택 한글 글꼴·라이선스 포함 디렉터리 | 15,491,964바이트(15.49MB) |
| 배포 tar.gz | 6,151,460바이트(6.15MB) |
| Windows 일반 release exe | 26,699,776바이트 |

원시 Rust `.wasm` → wasm-bindgen 처리 → Binaryen 처리 크기를 별도로 기록한다. 압축 배포 크기는 설치 후 디렉터리 크기나 RAM 사용량이 아니다. 선택 Nanum Gothic은 약2.05MB의 외부 자산으로 WASM/네이티브 exe 안에 추가하지 않았다. STIX와 egui 기본 글꼴은 기존처럼 포함한다. 글꼴·팔레트 고지도 SDK에 넣는다.

작은 문서·두 세션·실제 표본을 오간 통합 검사에서 UI WASM 메모리 high-water는18,743,296바이트, 응답한 작은 계산 Worker는6,029,312바이트였다. 전체 프로세스 RSS·컴파일 코드·GPU 자원이나 큰 문서의 최대 메모리를 나타내지 않는다.

## 실제 브라우저 검증

REA `capture_browser_scenario`가 설치된 Chrome155를 전용 임시 프로필로 실행하고 패키지에서 SDK를 로드했다. self script/Worker/connect와 `wasm-unsafe-eval`만 허용하는 테스트 CSP를 사용했다. 일반 JS `unsafe-eval`과 SharedArrayBuffer/cross-origin isolation은 사용하지 않았다. 공급한 CJK 글꼴은 화면과 출력 컨텍스트에 함께 전달한다.

검증은 `web/example/verify.js`가 실제 WASM/WebRunner/API에 수행한다.

- 실제 Canvas 생성, 버전 불일치 거절, 문서 열기 시 계산 실행 금지.
- 스칼라·표시 단위·행렬·한글 문자열 계산과 Worker 실행 중 UI 타이머 진행.
- MCDX/자체 형식 bytes 저장·재열기, 저장 결과 캐시, malformed 입력 후 기존 문서 보존.
- 한글을 포함한 글꼴 임베딩 벡터 PDF. 페이지 bitmap을 만들지 않는지 출력 구조 검사.
- 합성 DOM 키 이벤트를 통한 작성·Undo/Redo, Enter 전 계산 금지/확정 후 계산, Ctrl+- 아래첨자 유지.
- 저장 실패·취소, 이전 저장 ID/문서 승인 거절, 쓰는 동안 새 편집의 dirty 보존.
- 실제 계산 중지·Worker 재생성, 이전 문서 결과 차단, 두 세션의 서로 다른 계산 바인딩.
- 반복 mount/dispose, 입력 agent 제거, 소유 Worker의 terminate 호출, 해제 후 명령 거절과 숨김 상태의 워크시트 frame 중지.
- `1015_LCC_S.mcdx`, `1016_LCC_S.mcdx`의 원본 영역·메모·저장 결과 열기와 자체/MCDX 내보내기.

중지 입력의 첫 버전은 For 범위 제한에 걸려 일찍 끝나는 문제가 있었다. 최적화본에서 이를 발견하고, 허용 범위 안의128×128 행렬식을 반복하는 작업으로 교체했다. 작은 동형 작업의 실제 결과가1임을 먼저 확인한 뒤 긴 작업을 시작·중지한다. 따라서 단순히 시작 메시지를 받은 것을 오래 실행되는 유효 계산으로 취급하지 않는다. 최종60개 검사가 통과했고, 중지 요청에서 UI 계산 상태가 끝날 때까지51ms를 관측했다. 타이머/poll·프레임 예약이 포함되므로 OS 스레드 종료 지연의 단독 측정은 아니다.

물리 키보드/OS IME와 실제 Obsidian·VS Code의 리소스 URI·CSP·키 중재는 이 결과에 포함하지 않는다. CDP worker-close 이벤트와 SDK의 소유 Worker terminate 호출 기록도 구분한다. 이 검사가 모든 프로세스 메모리 누수 부재를 증명하는 것은 아니다.

## 네이티브 기능·성능

Windows: fmt, workspace/all-targets/native-profile Clippy, workspace 테스트, egui-winit 키 전달 회귀, 참조 계산을 통과했다(2,195통과/105기존 ignored). 일반 release를 다시 빌드하고1015를 `--smoke-test`로 열어 exit0을 확인했다. peets에서도 격리한 소스로 fmt/Clippy/전체 테스트/egui-winit/dev 빌드를 통과했다(2,196통과/103기존 ignored). SSH stdout/stderr가 섞인 요약을 합산하지 않고 원격에서 두 스트림을 한 파일로 합친 테스트 로그를 다시 받아 개수를 확인했다. 구체적인 로그 해시는 검증 receipt에 기록한다.

네이티브 의존성 트리에 `wasm-bindgen`, `serde-wasm-bindgen`, `web-sys`가 없고, WASM 트리에 `winit`, `rfd`, `wgpu`가 없음을 확인했다. 문서 메모를 가져올 때 같은 파일을 다시 열던 부분은 전달된 archive bytes를 재사용하도록 바꿨다. 새 공통 tests는 존재하지 않는 파일 표시 이름으로도 한글 메모/수식이 보존되고, 열기가 계산을 실행하지 않으며 저장 결과가 bytes 왕복하는지 확인한다.

기준 native 소스는 커밋 `10c861715eecfb815868fddd59ba853799bb4fc4`와 코드/자산506파일을 대조해 일치함을 확인했다. 별도 baseline/current native-profile 실행체를 사용했다. 빌드·테스트·wasm-opt와 겹치지 않고,1180×900·배율1·동일 CPU 어댑터·112프레임 입력을 교차 실행했다.17–96의80개 휠 구간만 비교한다.

| 조건 | 이전 p95 ms | SDK 작업 후 p95 ms |
| --- | ---: | ---: |
| 1015 100%, 각3회/240표본 통합 | 7.8868 | 8.1313 |
| 1016 100%, 각1회 | 7.7604 | 7.5597 |
| 1015 200%, 각1회 | 5.4778 | 5.4803 |

1015 통합 변화는+3.10%로 설정한10% 회귀 기준 안이다. 모든 실행에서 source/cache/계산 generation/이력/dirty 보존 검사와 viewport/DPI 일치를 확인했다. baseline/current의1015 화면 PNG도 같은 SHA-256이었다. 이 값은 실제 native 창의 raw-input 시작 간격이고 외부 scanout FPS나 물리 휠 입력 지연을 측정한 것은 아니다. 모든 하드웨어/문서의 동일 성능을 보장하지 않는다.

## 증거와 후속 범위

로컬 원본 증거는 `artifacts/rea/wasm-sdk-20261024/`에 있다. `checks/receipt.json`, `peets-check.log`, `native-comparison.json`, `runs/*/observation.json`, `browser/rea-*.json`, browser 자체 receipt, 소스 manifest와 `validation.json`으로 재사용한다. 실패한 중간 검사도 보존한다. 최종 SDK/입력 해시를 비교해 검증한 파일과 배포 파일의 동일성을 확인한다.

Obsidian/VS Code 플러그인·실제 host 저장/backup/충돌·호스트 키 중재/물리 IME·읽기 전용 미리보기와 배포/서명 자동화는 구현하지 않았다. 자세한 SDK의 현재 한계는 [사용 문서](../WASM-SDK.md)에 연결한다.
