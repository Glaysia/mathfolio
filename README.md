# Mathcraft

Mathcad의 워크시트 편집과 계산 동작을 분석하고 Rust로 독립 구현하는 프로젝트.

- 개발 위치: 이 Windows PC의 `C:\Users\harry\Projects\mathcraft`
- 필수 실행 환경: Windows, Kubuntu 26.04 (`ssh peets`)
- macOS: 우선 검증 대상에서 제외하되 플랫폼 종속 설계는 최소화
- 현재 단계: Rust 개발 환경 준비 및 Cargo workspace 구성. CLI와 텍스트 입력 GUI 골격을 추가했으며 계산 엔진과 문서 저장은 아직 구현하지 않음.

[개발 계획](docs/PLAN.md) · [환경 조사](docs/ENVIRONMENT.md) · [분석 기록 양식](docs/OBSERVATION_TEMPLATE.md)

## 개발 실행

프로젝트의 `rust-toolchain.toml`에서 Rust 1.99.0과 rustfmt, Clippy, rust-analyzer를 고정한다.

```sh
cargo run -p mathcraft-cli --locked
cargo run -p mathcraft-app --locked
```

전체 검사는 Windows에서 `./scripts/check.ps1`, Linux에서 `bash scripts/check.sh`로 실행한다. 신규 환경 준비 절차와 검증 상태는 [개발 환경](docs/DEVELOPMENT.md)을 참고한다.

## 다음 작업

1. Windows MSVC 설치를 완료하고 양쪽 OS에서 빌드 검증.
2. Mathcad가 설치된 분석 환경 연결, 실제 제품 버전과 실행 파일 확인.
3. 최소 워크시트 기준 사례를 원본에서 실행해 입출력과 화면을 기록.
4. 기준 사례에 맞춰 Rust 계산 코어부터 구현.

설치 프로그램과 원본 제품 바이너리는 Git 추적에서 제외한다.
