# Rust 개발 환경

**UI 개편 전 기능 체크포인트:** [현재 보존 범위](CHECKPOINT-2026-10-20.md). 다음 작업은 [현대적인 UI와 기존 단축키 안내 개선](UI-MODERNIZATION.md)이다. 기존 r222 고정 검증은 [입력 행렬 편집·조판·인쇄](analysis/0207-typed-source-matrix-window.md)에 보존한다. 아래 이름 붙은 이전 검증은 당시 이력이다.

현재 실행 안내 갱신: 2026-10-20. 기능 상태는 [통합 목록](FEATURES.md), 날짜별 검사 범위는 [CONFORMANCE](CONFORMANCE.md)를 참고한다.

이전 r125 [구조 삭제·서명 통합](analysis/0125-structural-deletion-integration.md)은 Windows/peets 각각 workspace1009+backend2=1011개 통과·실패0개·51개 ignored다. 동일187파일 archive를 `/home/peets/Projects/mathfolio-structural-deletion-20261019-r125`에 검증했으며 source·release·계측 identity와 남은 한계는 통합 기록을 따른다. GUI·일반 앱 main은 실행하지 않았다.

## 도구체인

- Rust/Cargo: 1.99.0, edition 2024
- 도구: rustfmt, Clippy, rust-analyzer
- Windows 타깃: `x86_64-pc-windows-msvc`
- Linux 타깃: `x86_64-unknown-linux-gnu`
- GUI: eframe 0.36.2, Windows WGPU / Linux Glow, X11/Wayland 기능 활성화
- `Cargo.lock`을 추적하며 검증 명령은 `--locked`로 실행한다.

## Windows

Rust와 rust-analyzer 설치, 사용자 PATH 등록은 완료했다. 설치 전 열려 있던 터미널은 다시 열거나 `$env:PATH = "$env:USERPROFILE\.cargo\bin;$env:PATH"`를 실행한다.

Visual Studio Build Tools 2022와 C++ workload, Windows SDK 설치를 완료했다. 기존 등록 경로의 폴더가 없어 별도 경로에 새로 설치했다.

- 설치 경로: `C:\BuildTools\Mathcraft2022`
- Build Tools: 17.14.37710.0
- MSVC: 14.44.35207
- Windows SDK: 10.0.26100.0
- 설치 종료 코드: 0, 재부팅 요구 없음
- 일반 PowerShell에서 Rust가 MSVC 링커를 자동으로 찾으며 빌드 성공.

새 Windows 환경에서는 관리자 PowerShell에서 프로젝트 디렉터리로 이동한 후 다음 명령으로 설치와 검증을 수행한다.

```powershell
./scripts/setup-windows.ps1
```

스크립트는 기존 Visual Studio 설치가 있으면 C++ workload를 추가한다. 설치 완료 후 자동 재부팅하지 않는다.

## peets / Kubuntu 26.04

- 빌드 소스 위치: `/home/peets/Projects/mathfolio`
- Windows 소스를 선별 복사한 검증용 디렉터리다. 원격 Git 저장소나 별도 개발 원본으로 사용하지 않는다.
- SSH 키로 접속하고 sudo 인증은 대화형 입력으로 처리했다. 비밀번호는 스크립트에 넣지 않는다.
- Rust 1.99.0, GCC 15.2.0, CMake 4.2.3 설치 확인.
- build-essential, pkg-config, X11/Wayland/OpenGL/udev/OpenSSL 개발 패키지 설치 완료.

```sh
ssh peets
cd ~/Projects/mathfolio
. "$HOME/.cargo/env"
bash scripts/check.sh
cargo run -p mathfolio-cli --locked
# GUI는 KDE 로그인 세션의 터미널에서 실행한다.
cargo run --release -p mathfolio-app --locked
```

신규 Ubuntu/Kubuntu 환경은 `bash scripts/setup-linux.sh`로 같은 패키지와 도구체인을 준비한다.

## 설치 당시 확인 결과 (2026-10-15)

| 검사 | Windows | peets |
| --- | --- | --- |
| Rust/Cargo 1.99.0 | 설치 확인 | 설치 확인 |
| rustfmt / Clippy / rust-analyzer | 설치 확인 | 설치 확인 |
| 전체 workspace fmt / Clippy | 통과 | 통과 |
| CLI 및 GUI 빌드 | 통과 | 통과 |
| CLI 실행 | 통과 | 통과 |
| GUI 화면 / IME / 클립보드 | 화면·한글 메모 표시 확인, IME·클립보드 미검증 | 미검증: KDE 사용자 GUI 세션 없음 |

초기 설치 확인 시점에는 자동 테스트 67개였다. 이 수치는 현재 테스트 수가 아니다. Windows와 peets에서 자유 배치 워크시트의 클릭·연속 입력·정의/평가·위치 이동·undo·저장/재열기, PortableTree 보존, 구조 수식 headless 위젯, MathML 직접 실행·오류 전파, 표시 단위와 물리량 분리, 원본 ZIP parts 보존을 확인했다. 별도로 새 Mathcad 실행 결과 38개에 대한 제한된 참조 비교도 두 OS에서 통과했다. 측정 범위와 미완료 부분은 [호환성 검증](CONFORMANCE.md)을 참고한다. 데스크톱 창이나 원본 앱은 이 후속 검사에서 실행하지 않았다.

CI는 Windows 및 Ubuntu 24.04의 검사 구성을 추가했다. 아직 원격에 push하지 않아 CI 실행 결과는 없다. Kubuntu 26.04 실기 검증은 CI와 별도로 유지한다.

설치 절차는 [Rust 공식 설치 안내](https://rust-lang.org/tools/install/)와 [Microsoft 설치 명령 문서](https://learn.microsoft.com/en-us/visualstudio/install/use-command-line-parameters-to-install-visual-studio?view=vs-2022)를 참고한다. GUI API는 [eframe 0.36.2 문서](https://docs.rs/eframe/0.36.2/eframe/)를 기준으로 작성했다.

현재 Windows 실행은 `scripts/run.ps1` 또는 `cargo run --release -p mathfolio-app --locked`를 사용한다. debug 실행을 기본 성능 기준으로 삼지 않는다. Windows WGPU / Linux Glow의 현재 backend 구성과 성능 변경 이후 최신 소스의 peets 재검증은 별도이며, 위 설치 당시 통과 기록으로 대체하지 않는다. [성능 분석](analysis/0030-rendering-performance.md) 참고.

2026-10-16 저장 결과/계산 명령 변경 이후 Windows release와 peets에서 workspace 321개 테스트, fmt·전체 타깃 Clippy를 통과했다. peets의 최신 소스 workspace 빌드도 확인했다. 소스 위치·검증 범위·남은 차이는 [검증 기록](CONFORMANCE.md)을 따른다. GUI 성능·IME·클립보드 실기 검증은 포함하지 않는다.

같은 날 인쇄·성능·프로그램/행렬 키 입력·비활성화·저장 날짜 변경 이후에는 두 OS에서 358개 테스트(별도 실행 3개 ignored), fmt·Clippy·workspace 빌드를 통과했다. 인쇄 조판은 기본 앱 의존성을 재사용하며 Linux 날짜 변환에 기존 버전 libc를 직접 사용한다. Linux 실제 인쇄에는 CUPS `lp`/`lpr`가 필요하고, Windows 전송은 시스템 PrintDocument를 사용한다. 테스트는 실제 프린터를 호출하지 않았다.

이후 백그라운드 계산·협력 중지/재개·Auto OFF 대기 요청·텍스트 편집 중 명시 계산 보존을 추가한 최신 소스는 양쪽에서 377개 통과·3개 ignored, fmt·전체 타깃 Clippy·workspace 빌드를 통과했다. peets 격리 소스는 `/home/peets/Projects/mathfolio-background-20261016-r41`이다. Windows 최신 실행 파일은 `target/release/mathfolio.exe`다. 로그 경로·미완료 범위는 [최신 검증 기록](CONFORMANCE.md)을 따른다.

그 뒤 일반 입력/Enter 확정 분리와 결과 서식·원시 정밀도 캐시를 통합한 시점에는 두 OS에서 **405개 통과·3개 ignored**, fmt·Clippy·workspace 빌드까지 통과했다. 그 시점 MCDX 7개의 28 XML part도 설치본 XSD 검증을 통과했다.

이전 표·색상·벡터 PDF 통합은 양 OS 각각 430개 통과·8개 ignored, 표 셀 편집·열 계산·DPI/캐시 수정은 454개 통과·12개 ignored, 원본 단축키·표 명령·이동 확정 수정은 494개 통과·15개 ignored였다. 배열 원소 정의·표 clipboard 교정·계산 준비 2차 수정은 526개 통과·18개 ignored였다. 일반 행렬 선택·명령과 배열 참조 복제 개선을 포함한 r68 소스는 **Windows/peets 각각 549개 통과·20개 ignored**, fmt·Clippy·workspace 빌드 통과다. Windows 실행 파일은 release 빌드이며 당시 peets 소스는 `/home/peets/Projects/mathfolio-matrix-workflow-20261016-r68`이다. 실제 문서 및 새 인덱스·일반 행렬의 계산·저장/재열기·MCDX export는 별도 검사로 구분한다. 벡터 PDF는 renderer를 변경하지 않은 r64 기록을 유지한다. 원본 코드·GUI 실행과 구분한 최신 범위·파일 해시는 [CONFORMANCE](CONFORMANCE.md)를 따른다.

그 뒤 전용 구조 연산자와 입력 backend를 연결한 r72를 검사했다. **Windows/peets 각각 workspace 581개 + backend 2개 = 583개 통과·21개 ignored**, fmt·workspace 및 backend Clippy·빌드 통과다. peets 격리 소스는 `/home/peets/Projects/mathfolio-structural-operators-20261016-r72`다. `vendor/egui-winit`은 0.36.2의 C Copy 판정만 좁게 수정한 source patch이며 원본 해시·라이선스·변경 기록을 포함한다. 두 플랫폼 검사 스크립트는 `cargo test -p egui-winit --lib --locked`도 실행한다. 이번 소스의 실제 문서/연산자 벡터 PDF 6개와 native 파일 10개는 별도 검사이며 최신 해시와 한계는 [CONFORMANCE](CONFORMANCE.md)를 따른다.

r76은 전역 정의·벡터화·외적·표 셀 표현식 전용 context를 연결했다. 당시 Windows/peets 각각 workspace 621개 + backend 2개 = **623개 통과·실패 0개·22개 ignored**이며 fmt·Clippy·빌드와 기존 참조 38개를 통과했다. 당시 peets 소스는 `/home/peets/Projects/mathfolio-global-vector-20261016-r76`다. 별도 native probe 6건/OS와 PDF probe 4건/OS 및 설치 XSD 검사를 수행했다.

r89는 프로그램 지역 함수의 원본 `{` 입력·계산·native 왕복과 정확한 JSON 서명 cache를 연결했다. 당시 Windows/peets 각각 workspace 722개 + backend 2개 = **724개 통과·실패 0개·27개 ignored**다. 수정 준비는 줄었지만 cold 준비와 보존 payload는 늘었으며 [0088](analysis/0088-calculation-signature-cost.md)에 기록했다.

r93은 실제 열기 뒤 첫 입력·이전 입력 지연 준비와 첫 행렬 Tab 포커스를 교정했다. 당시 Windows/peets 각각 workspace 738개 + backend 2개 = **740개 통과·실패 0개·28개 ignored**다. 첫 Text 비용과 Enter로 이동한 비용을 [0090](analysis/0090-first-open-input-cost.md)·[0092](analysis/0092-committed-input-integration.md)에 함께 기록했다.

r94는 원본 Ctrl+Q 라벨 순환·자동 리본·None 저장과 제한된 추론을 연결했다. Windows/peets 각각 workspace 764개 + backend 2개 = **766개 통과·실패 0개·29개 ignored**, fmt·전체 타깃 Clippy·별도 backend Clippy·workspace 빌드 통과다. peets 소스는 `/home/peets/Projects/mathfolio-label-cycle-20261019-r94`다. native 10건/OS·PDF 8건/OS·설치 XSD와 actual-open 36case 및 Windows 5회 교대 성능 측정을 수행했다. 실행 파일·소스 해시·남은 Enter 지연·검증 한계는 [0094](analysis/0094-label-cycle-integration.md)·[CONFORMANCE](CONFORMANCE.md)를 따른다.

r98은 원본 프로그램 Ctrl+J 5개 단어·break/continue 빈 자리 삽입·삭제·native 왕복과 제한된 cold XML batch를 연결했다. Windows/peets 각각 **workspace 791개 + backend 2개 = 793개 통과·실패 0개·31개 ignored**다. native 11건/OS·PDF 9건/OS·설치 XSD·actual-open 36case와 Windows r94→r98 5회 교대 성능 검사를 수행했다. peets root는 `/home/peets/Projects/mathfolio-program-controls-20261019-r98`다. source/release identity·남은 범위·성능은 [0098](analysis/0098-program-controls-integration.md)·[CONFORMANCE](CONFORMANCE.md)를 따른다.

r101은 다중 조건 분기·원본 BODY/행 배치·Ctrl+J if와 인접 분기 단어·native 저장·XML 직렬화 비용을 연결했다. Windows/peets 각각 **822 + backend 2 = 824개 통과·실패 0개·33개 ignored**다. native 12건/OS·PDF 10건/OS·설치 XSD·actual-open 36case와 Windows r98→r101 5회 교대 성능을 확인했다. peets root는 `/home/peets/Projects/mathfolio-conditional-branches-20261019-r101`다. source/release identity·남은 공백은 [0101](analysis/0101-conditional-branch-integration.md)·[CONFORMANCE](CONFORMANCE.md)를 따른다.

r105는 Try/on error와 표시 DOM 재사용을 연결했다. Windows/peets 각각 **workspace 862 + backend 2 = 864개 통과·실패 0개·36개 ignored**다. native 13건/OS·PDF 11건/OS·설치 XSD·actual-open 36case·Windows r101→r105 5회 교대 성능을 확인했다. peets root는 `/home/peets/Projects/mathfolio-try-control-20261019-r105`다. source/release identity와 지원 한계는 [0105](analysis/0105-try-control-integration.md)·[0102](analysis/0102-display-preparation-cost.md)를 따른다.

r109는 원본 선택 모드별 프로그램 삽입과 입력/paint 중복 제거를 연결했다. 당시 Windows/peets 각각 **workspace 889 + backend 2 = 891개 통과·실패 0개·40개 ignored**다. native14/PDF12건/OS·XSD·actual-open36case·Windows r105→r1095회 교대 성능과 별도 tessellation mesh/CPU를 확인했다. peets root는 `/home/peets/Projects/mathfolio-program-insertion-20261019-r109`다. tar 전체 regular file manifest와 source/release identity·한계는 [0109](analysis/0109-program-insertion-integration.md)·[0107](analysis/0107-frame-interaction-cost.md)를 따른다.

r113은 [Scale/확정 준비 통합](analysis/0113-implicit-scale-integration.md)이다. Windows/peets 각각 **workspace 918 + backend 2 = 920개 통과·실패 0개·43개 ignored**와 source/native/PDF/원본 reference를 확인했다. exact source/display/cache/error oracle와 실제 r109→r1135회 교대360case를 기록한다. peets root `/home/peets/Projects/mathfolio-implicit-scale-20261019-r113`와 전체 소스/성능/release identity를 보존한다.

최신 r121 [일반 연산자 입력·문서 열기](analysis/0121-infix-entry-integration.md)는 Windows/peets 각각 **workspace981 + backend2 = 983개 통과·실패0개·47개 ignored**와 source181파일/native17/PDF15건/OS·reference를 확인했다. r117→r121 첫5회 교대360case와 행렬 cache 증가 확인용 별도360case·모든 증가 표본을 기록한다. peets root `/home/peets/Projects/mathfolio-infix-entry-20261019-r121`와 source/release identity를 보존한다. 이전 r117 검증은 [0117](analysis/0117-parenthesis-integration.md)에 남긴다.
