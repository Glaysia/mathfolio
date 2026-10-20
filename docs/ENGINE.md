# 계산 엔진 개발 상태

갱신: 2026-10-16. 기능별 현재 판정과 전체 원본 바인딩의 개별 상태는 [통합 구현 목록](FEATURES.md)을 따른다.

계산 엔진은 [Mathcad 작성 경험](DESIGN.md)을 지탱한다. 현재 우선순위는 REA로 편집 명령·문맥 라벨·계산 피드백을 추적하고 편집→계산→수정 흐름에 연결하는 것이다. Windows와 peets 모두 GUI·스크린샷을 사용하지 않고 정적 분석과 headless 검사를 진행한다.

```sh
cargo run -p mathcraft-cli --locked -- features
cargo run -p mathcraft-cli --locked -- features --all
cargo run -p mathcraft-cli --locked -- mathml-run examples/engineering.xml
cargo run -p mathcraft-cli --locked -- native-run INPUT.mcdx
cargo run -p mathcraft-cli --locked -- conformance tests/reference/mathcad12.json
```

`mathml-run`과 `native-run`은 MathML 구조를 직접 실행한다. 기존 문자열 `eval`/`run` 경로는 초기 실험 엔진이므로 새 기능의 검증에 사용하지 않는다. `features`의 `bounded-subset` 표시는 제한된 구현이며 전체 호환을 뜻하지 않는다. 분석 목록과 실제 구현 상태를 구분한다.

| 분야 | 연결한 기능 | 근거와 제한 |
|---|---|---|
| 이름·순서 | 라벨, 위치 순서, 지역/전역 정의, 정의 시 환경을 잡는 함수, 실패한 재정의 전파 | [0003](analysis/0003-calculation-semantics.md), 원본 38개 기존 실행 표본 |
| 배열 | scalar mapping, 복소 벡터 내적, 행렬곱·나눗셈·정수 power, 전치, determinant/inverse, 결합·추출, norm 계열, full-column-rank `lsolve`, 실수 대칭 `eigenvals`/`eigenvecs` | [0005](analysis/0005-array-semantics.md), [0025](analysis/0025-linear-systems.md), [0026](analysis/0026-symmetric-eigensystems.md); mixed-unit factorization·rank-deficient/underdetermined solve·일반/복소 eigensystem·대규모 spectral·SVD 미완료 |
| 프로그램 | 지역 대입, if/elseif/alsoif/else, for/while, return/break/continue, 비교/논리 | [0006](analysis/0006-programming-semantics.md); indexed assignment·try/onerror 등 미완료 |
| 수치 | root, finite adaptive integration, 0–5계 미분, 정수 bounds의 합/곱, polyroots | [0007](analysis/0007-numerics-semantics.md); 원본과 다른 독립 알고리즘이며 TOL·작업량 제한 존재 |
| 과학 함수 | 삼각·쌍곡·역함수, 복소 함수, log 기본 밑, 반올림·절삭·mod·타입 판별 | [0008](analysis/0008-engine-catalog.md); vector와 matrix 허용 범위를 구분 |
| 통계 | mean/gmean/hmean, median, 모집단/표본 var·stdev, real cvar/corr, min/max, sort/reverse/Rank | [0009](analysis/0009-statistics-semantics.md); signal covar와 별개 |
| 단위 | 원본 SI 단위 212개와 물리 상수 15개 수치·차원 | [0010](analysis/0010-unit-semantics.md); Money·affine·다른 unit system 미완료 |
| 풀이 | native solve block의 scalar 실수 guess, equality 제약, Find/Minerr, solver 정의 내보내기 | [0011](analysis/0011-solve-semantics.md); inequalities·최적화·복소/행렬 unknown 미완료 |
| 상징 | 유리수·다항식 expand/simplify, 다항식 미분, 유한점 다항식 극한, 2차 이하 solve | [0012](analysis/0012-symbolics-semantics.md), [0024](analysis/0024-symbolic-limits.md); 제한된 독립 CAS이며 전체 symbolic engine 호환 아님 |
| 설정·라벨 | SYSTEM 5개 기본값, 명시/문맥 라벨 구분, 계산용 구조 라벨 규칙 | [0013](analysis/0013-system-defaults.md), [0016](analysis/0016-contextual-labels.md); 문서별 설정 전달·전체 자유 식별자 해석 미완료 |

REA에서 얻은 21개 모듈의 874개 `mc_*`/`op_*` 바인딩 사실은 [엔진 목록](spec/mathcad12-engine-catalog.json)에 있다. EfiAffine의 DMS/FIF/hhmmss와 역방향 바인딩 6개는 이번 조사에서 추가했으며 모두 미구현이다. 기존 내부 encoding 이름과 도움말의 사용자 이름 대조도 통합 목록에 기록했다. 상징 모듈은 별도 `sym_*`/`symeval_*` 경로를 사용하므로 이 개수를 전체 CAS나 제품 기능 수로 해석하지 않는다. 원본 바이너리·리소스·전체 인터페이스·디컴파일 원문은 Git에 넣지 않는다.

기존 원본 실행 표본과 새 구현 테스트를 구분한다. 새 행렬·프로그램·수치·통계 테스트는 REA로 확인한 정적 계약과 자체 작성한 수학적 기대값을 검사하며, 이번 작업에서 원본 프로그램을 새로 실행한 비교는 아니다. 한 문서의 복합 실행 예제는 [engineering.xml](../examples/engineering.xml)이다.

편집기는 프로그램 Enter 줄·빈 줄 삭제·행렬 Tab 확장과 기본 적분/미분/합/곱·상징 구조를 엔진에 연결했다. Find/Minerr는 제한된 별도 편집 경로이며 Prime의 인라인 작성은 미완료다. 전체 제어문 작성·삭제/unwrap·상징 명령·키 전이는 남아 있다. CLI/엔진 검사와 headless 편집→계산 검사를 함께 사용한다. FFT·회귀·분포·ODE·영상·파일 IO와 전체 풀이/CAS 등은 미완료다.

계산 바인딩만으로 전체 기능을 포괄할 수 없다. 도움말에서 추가 확인한 사용자 함수/상징 overload 46개, 상징 키워드·수정자, 계산 명령의 수명 주기는 [재조사 기록](analysis/0031-feature-inventory-audit.md)과 통합 목록에 별도로 있다. `WorksheetCalculator`는 입력 무효화와 실행을 나누고, 계산된 binding delta를 재사용한다. F5의 가시 범위/활성 영역 계산과 Ctrl+F5 전체 강제 계산을 구분한다. 문서 열기는 저장 표현만 복원한다. 도표의 모든 표본도 같은 계산 문맥을 재사용한다. 평면 비활성 영역은 실행에서 제외한다. 긴 프로그램·수치·행렬·풀이 반복에는 [협력 취소와 중단행 rollback](analysis/0040-calculation-cancellation-engine.md)을 넣고 데스크톱은 [영속 worker](analysis/0041-background-calculation-ui.md)에서 실행한다. 정밀 의존성·전체 native cache·Area 전파·원본 queue 게시 시점·외부 IO 중단은 남아 있다.
