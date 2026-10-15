# Mathcraft

Mathcad의 워크시트 편집과 계산 동작을 분석하고 Rust로 독립 구현하는 프로젝트.

- 개발 위치: 이 Windows PC의 `C:\Users\harry\Projects\mathcraft`
- 필수 실행 환경: Windows, Kubuntu 26.04 (`ssh peets`)
- macOS: 우선 검증 대상에서 제외하되 플랫폼 종속 설계는 최소화
- 현재 단계: 복소수·단위 계산 코어, 워크시트 GUI, 그래프, 자체 문서 저장 및 MCDX 부분 가져오기 프로토타입.

[개발 계획](docs/PLAN.md) · [환경 조사](docs/ENVIRONMENT.md) · [분석 기록 양식](docs/OBSERVATION_TEMPLATE.md)

[REA 분석 환경](docs/REA.md) · [Mathcad Prime 12 최초 분석 결과](docs/analysis/0001-initial-inspection.md)

[프로토타입 사용법과 검증 범위](docs/PROTOTYPE.md)

## 개발 실행

프로젝트의 `rust-toolchain.toml`에서 Rust 1.99.0과 rustfmt, Clippy, rust-analyzer를 고정한다.

```sh
cargo run -p mathcraft-cli --locked
cargo run -p mathcraft-app --locked
```

전체 검사는 Windows에서 `./scripts/check.ps1`, Linux에서 `bash scripts/check.sh`로 실행한다. 신규 환경 준비 절차와 검증 상태는 [개발 환경](docs/DEVELOPMENT.md)을 참고한다.

## 다음 작업

1. 확인한 Mathcad 설치 모듈에서 계산 엔진과 의존성 처리 경로 추적.
2. 최소 워크시트 기준 사례를 원본에서 실행해 입출력과 화면을 기록.
3. 프로토타입의 평가 순서·정밀도를 원본 실행 결과로 검증하고 미지원 기능 확장.

설치 프로그램과 원본 제품 바이너리는 Git 추적에서 제외한다.
