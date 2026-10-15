# Mathcraft

Mathcad Prime의 워크시트·수식 편집·계산·문서 동작을 맞추는 Rust 오픈소스 독립 구현 프로젝트.

- 개발 위치: 이 Windows PC의 `C:\Users\harry\Projects\mathcraft`
- 필수 실행 환경: Windows, Kubuntu 26.04 (`ssh peets`)
- macOS: 우선 검증 대상에서 제외하되 플랫폼 종속 설계는 최소화
- 현재 단계: 원본 동작 분석과 호환성 기반 구현. 기존 행 기반 계산기 GUI는 실험용이며 목표 제품으로 인정하지 않는다.

[개발 계획](docs/PLAN.md) · [환경 조사](docs/ENVIRONMENT.md) · [분석 기록 양식](docs/OBSERVATION_TEMPLATE.md)

[REA 분석 환경](docs/REA.md) · [Mathcad Prime 12 최초 분석 결과](docs/analysis/0001-initial-inspection.md)

[프로토타입 사용법과 검증 범위](docs/PROTOTYPE.md)

[호환성 검증과 현재 한계](docs/CONFORMANCE.md) · [수식 편집 분석](docs/analysis/0002-editor-semantics.md) · [계산 분석](docs/analysis/0003-calculation-semantics.md) · [문서 분석](docs/analysis/0004-document-semantics.md)

## 개발 실행

프로젝트의 `rust-toolchain.toml`에서 Rust 1.99.0과 rustfmt, Clippy, rust-analyzer를 고정한다.

```sh
cargo run -p mathcraft-cli --locked
cargo run -p mathcraft-app --locked
cargo run -p mathcraft-cli --locked -- conformance tests/reference/mathcad12.json
cargo run -p mathcraft-cli --locked -- native-inspect INPUT.mcdx
cargo run -p mathcraft-cli --locked -- native-copy INPUT.mcdx NEW.mcdx
cargo run -p mathcraft-cli --locked -- native-run INPUT.mcdx
```

전체 검사는 Windows에서 `./scripts/check.ps1`, Linux에서 `bash scripts/check.sh`로 실행한다. 신규 환경 준비 절차와 검증 상태는 [개발 환경](docs/DEVELOPMENT.md)을 참고한다.

## 다음 작업

1. 구조 수식 편집기를 자유 배치 워크시트에 통합하고 원본 키 조작과 대조.
2. MathML 직접 실행의 범위·행렬·풀이 블록·상징 연산과 의존성 갱신 확장.
3. MCDX 보존 계층에 편집·캐시 무효화·왕복 저장을 추가하고 원본 재열기 검증.

설치 프로그램과 원본 제품 바이너리는 Git 추적에서 제외한다.
