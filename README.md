# Mathcraft

Mathcad의 워크시트 편집과 계산 동작을 분석하고 Rust로 독립 구현하는 프로젝트.

- 개발 위치: 이 Windows PC의 `C:\Users\harry\Projects\mathcraft`
- 필수 실행 환경: Windows, Kubuntu 26.04 (`ssh peets`)
- macOS: 우선 검증 대상에서 제외하되 플랫폼 종속 설계는 최소화
- 현재 단계: 계획 및 리포지토리 초기화. 계산 엔진과 GUI는 아직 구현하지 않음.

[개발 계획](docs/PLAN.md) · [환경 조사](docs/ENVIRONMENT.md) · [분석 기록 양식](docs/OBSERVATION_TEMPLATE.md)

## 첫 작업

1. Windows Rust 도구체인과 C++ 빌드 도구 확인 및 설치.
2. Mathcad가 설치된 분석 환경 연결, 실제 제품 버전과 실행 파일 확인.
3. 최소 워크시트 기준 사례를 원본에서 실행해 입출력과 화면을 기록.
4. 기준 사례에 맞춰 Rust 계산 코어부터 구현.

설치 프로그램과 원본 제품 바이너리는 Git 추적에서 제외한다.
