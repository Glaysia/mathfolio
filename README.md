# Mathcraft

Mathcad Prime의 워크시트·수식 편집·계산·문서 동작을 맞추는 Rust 오픈소스 독립 구현 프로젝트.

- 개발 위치: 이 Windows PC의 `C:\Users\harry\Projects\mathcraft`
- 필수 실행 환경: Windows, Kubuntu 26.04 (`ssh peets`)
- macOS: 우선 검증 대상에서 제외하되 플랫폼 종속 설계는 최소화
- 현재 단계: REA로 원본의 수식 작성·편집·계산 상태 전이를 추적하고 Rust에 연결 중. 원본 전체 호환은 미완료다.

제품의 중심은 종이에 쓰듯 수식을 작성하고 고치는 경험이다. 계산 엔진은 그 흐름을 지탱한다. [작성 경험의 기준](docs/DESIGN.md)에 원본 근거와 구현·검증 범위를 연결했다.

[계산 엔진 상태와 실행법](docs/ENGINE.md) · [원본 계산 바인딩 목록](docs/spec/mathcad12-engine-catalog.json)

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
cargo run -p mathcraft-cli --locked -- features
cargo run -p mathcraft-cli --locked -- mathml-run examples/engineering.xml
```

전체 검사는 Windows에서 `./scripts/check.ps1`, Linux에서 `bash scripts/check.sh`로 실행한다. 신규 환경 준비 절차와 검증 상태는 [개발 환경](docs/DEVELOPMENT.md)을 참고한다.

## 다음 작업

1. 원본 편집 명령·선택·커서·라벨·피드백 상태를 REA로 추적하고 작성 시나리오에 연결.
2. 편집→계산→수정→Undo→저장 흐름의 재현 테스트를 확대.
3. Windows와 peets에서 GUI 없이 상태 전이·MathML·MCDX 및 기존 참조 결과를 검증.

설치 프로그램과 원본 제품 바이너리는 Git 추적에서 제외한다.
