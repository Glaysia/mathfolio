# Rust 개발 환경

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

- 빌드 소스 위치: `/home/peets/Projects/mathcraft`
- Windows 소스를 선별 복사한 검증용 디렉터리다. 원격 Git 저장소나 별도 개발 원본으로 사용하지 않는다.
- SSH 키로 접속하고 sudo 인증은 대화형 입력으로 처리했다. 비밀번호는 스크립트에 넣지 않는다.
- Rust 1.99.0, GCC 15.2.0, CMake 4.2.3 설치 확인.
- build-essential, pkg-config, X11/Wayland/OpenGL/udev/OpenSSL 개발 패키지 설치 완료.

```sh
ssh peets
cd ~/Projects/mathcraft
. "$HOME/.cargo/env"
bash scripts/check.sh
cargo run -p mathcraft-cli --locked
# GUI는 KDE 로그인 세션의 터미널에서 실행한다.
cargo run -p mathcraft-app --locked
```

신규 Ubuntu/Kubuntu 환경은 `bash scripts/setup-linux.sh`로 같은 패키지와 도구체인을 준비한다.

## 확인 결과 (2026-10-15)

| 검사 | Windows | peets |
| --- | --- | --- |
| Rust/Cargo 1.99.0 | 설치 확인 | 설치 확인 |
| rustfmt / Clippy / rust-analyzer | 설치 확인 | 설치 확인 |
| 전체 workspace fmt / Clippy | 통과 | 통과 |
| CLI 및 GUI 빌드 | 통과 | 통과 |
| CLI 실행 | 통과 | 통과 |
| GUI 화면 / IME / 클립보드 | 화면·한글 메모 표시 확인, IME·클립보드 미검증 | 미검증: KDE 사용자 GUI 세션 없음 |

자동 테스트 13개로 코어 연산·차원·함수·오류·한도, MCDX 변환·DTD 거부, 문서 저장·재열기·갱신을 확인했다. 새 Mathcad 실행과 비교한 테스트는 아직 아니다. [프로토타입 검증 범위](PROTOTYPE.md)를 참고한다.

CI는 Windows 및 Ubuntu 24.04의 검사 구성을 추가했다. 아직 원격에 push하지 않아 CI 실행 결과는 없다. Kubuntu 26.04 실기 검증은 CI와 별도로 유지한다.

설치 절차는 [Rust 공식 설치 안내](https://rust-lang.org/tools/install/)와 [Microsoft 설치 명령 문서](https://learn.microsoft.com/en-us/visualstudio/install/use-command-line-parameters-to-install-visual-studio?view=vs-2022)를 참고한다. GUI API는 [eframe 0.36.2 문서](https://docs.rs/eframe/0.36.2/eframe/)를 기준으로 작성했다.
