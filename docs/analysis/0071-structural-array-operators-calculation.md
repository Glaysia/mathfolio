# 구조 연산자의 계산 연결과 함수 경계

2026-10-16. 일반 수식의 원본 연산자와 같은 이름의 함수를 구분해 계산 경로를 검사했다. 원본 GUI·스크린샷·원본 프로세스·CLR·COM을 실행하지 않았다. 설치 파일과 기존 REA 증거를 정적으로 읽고, 실행 검증은 독립 Rust 엔진의 headless 테스트만 사용했다.

## 원본에서 확인한 계약

편집 descriptor/BAML 조사는 [0069](0069-structural-array-operators-contract.md)의 담당 조사와 ignored `artifacts/rea/structural-array-operators-cil.json`, `structural-array-operator-baml.json`을 참조한다. 원본 XML 이름은 `factorial`, `transpose`, `conjugate`, `determinant`, `matrow`, `matcol`, `absval`이다. `Norm` 명령은 `determinant`, 절댓값/행렬식 명령은 `absval`을 사용한다. `norm`이라는 별도 native XML 연산자를 발명하지 않는다. 함수 `norm(...)`의 식별자는 FUNCTION 라벨의 `id`이며 별도 경로다.

설치된 `efimatrix_.dll` SHA-256은 `0eda05cf6f4f67a46fba769143797c73f1a71b5a626bffddfebfda9ee9eee496`이다. 기존 REA `ev_474b2253bb4b6bbdd8d962be20e081847ffc0c260d63a70db2408022c77cb830`과 파일 해시가 일치한다. 이번에 선택한 메서드의 파일 바이트를 다시 읽어 REA에 기록된 각 CIL 해시도 검증했다. 결과는 ignored `artifacts/rea/structural-operator-calculation-evidence.json`에 보존했다.

| 메서드 | 직접 확인한 분기 |
|---|---|
| `Efi.Matrix.abs`, `0x0600018d` | 숫자는 `abs_num`, 행렬은 `abs_mat`, 그 외 타입 오류 |
| `abs_mat`, `0x06000027` | 열 수가 1이면 `Norm`; 그 외 정방행렬이면 `Det`; 나머지는 `must be square` |
| `Norm`, `0x06000025` | uniform scalar 행렬의 vector norm을 계산하고 입력 차원을 유지; 다른 물리 배열은 오류 |
| `Det`, `0x06000024` | uniform scalar 행렬의 수치 행렬식에 입력 차원의 Rows 제곱을 적용; mixed physical 행렬은 별도 `MatDim.Det` 경로 |
| `Efi.Matrix.det`, `0x0600018e` | `Efi.Matrix.abs` 호출 |

따라서 정방행렬에서 전용 연산자를 무조건 Frobenius norm으로 바꾸면 이 dispatcher의 `Det` 분기와 달라진다. 도움말의 ‘제곱 크기의 합의 제곱근’이라는 문장만으로 그 변경을 하지 않았다. 예를 들어 단위 없는 `[[1,2],[3,4]]`에 기존 공통 dispatcher를 적용하면 `-2`다.

`efimatrix.dll` facade의 interface는 [0005](0005-array-semantics.md)에 기록한 SHA/MVID/속성 blob 해시로 검증된 자료다. `op_abs`, `op_norm`, `op_det`, `op_det2`의 외부 호출 대상은 모두 `Efi.Matrix::abs`이지만 **타입 선언은 모두 같지 않다**. 특히 `op_det`는 dimensionless matrix→dimensionless scalar이고 `op_det2`는 generic이다. 하위 helper가 같다는 사실만으로 최상위 타입 계약까지 같다고 주장하지 않는다.

`eficore_.dll` SHA-256 `d9a0a8e7716b71f2ccbc14af4bc48bc52bf3f53e0f1d9bbab825947c3aeffeaf`, REA `ev_9b6b49e1d2b919b7dab3cc072596d9c2593526001613ee05faacbfee9a2bfbb6`의 `Efi.Core.fact` (`0x0600053d`, CIL SHA `2470129d43a54ac26a1534bc59ee7d3208ec62cb5d263304293a4ac8e725e7f3`)도 이번 ledger에서 검증했다. 숫자는 `numN`, 벡터는 `mapvector`, 나머지는 `Fail` 분기다. factorial을 scalar만 허용하도록 좁혀 기존 열벡터 계산을 깨뜨리지 않았다.

설치 도움말은 `Help/en/PTC_Mathcad_Help`의 다음 파일을 읽었다. 본문을 실행하거나 포함된 이미지/스크린샷을 읽지 않았다.

| 파일 / SHA-256 | 관련 명시 계약 |
|---|---|
| `row_and_column_operators.html` / `a870677758a3602c0fc4deabcefd06511fb08817a03a39a46b1eca9e61fa5b2e` | 벡터/행렬, 차원 없는 정수 인덱스, ORIGIN 기본값 0 |
| `example_row_column_and_index_operators.html` / `7a2540e50594023bc5fcefd9a115eb7dd15e03841bf196a665bec218fc2dc737` | 내장 MathML 예제의 `matcol` 결과는 N×1, `matrow` 결과는 1×N |
| `transpose_operator.html` / `f5a759d827bee54a1d809d835d02e0560b2e0264a74c9a4797b6cd6a4b389c8a` | 행/열 교환, 벡터 방향 전환 |
| `complex_conjugate_operator.html` / `fab7bb167924c3f2c742154ba158e99271abb234e74856fb965590950681f3ad` | 실수부 유지, 허수부 부호 전환 |
| `factorial_operator.html` / `c63febac5f13a9d7f9aaca492cf2f8bbe27dbdff5ccbc526cf3f781f388563f6` | 0 이상 정수, 171 미만; 분수 factorial은 gamma 사용 |
| `determinant_operator.html` / `4962d53138883ac11b276e62897278f314e285294ba1ec5036a0e1b39ce6f53d` | scalar/complex/열벡터/정방행렬, compatible units, LU determinant 언급 |
| `norm_operator.html` / `d02b9cc6bed63aff1b018a5270455e79048f08a589480aa890c0228956038ed5` | 위와 유사한 피연산자·단위 설명; norm과 determinant 설명이 혼재 |

## 이번 수정

`compat/src/lib.rs`에서 native `matrow`/`matcol` 이항 연산자를 파싱하고 `arrays::row_or_column`으로 연결했다. 좌항은 immutable Scope binding을 빌려 읽으며 원본 행렬 전체의 소유 복제를 먼저 만들지 않는다. 계산식으로 만든 좌항도 기존 평가 경로로 처리한다. 결과는 선택된 셀만 복사한 독립 벡터이고 각 셀의 단위와 복소수를 유지한다.

인덱스는 기존 공통 `index_offset`과 검증된 SYSTEM ORIGIN을 사용한다. 음수 ORIGIN도 지원하며 차원이 있거나 분수/복소수인 인덱스, 범위 밖 위치, scalar/range 피연산자는 오류다. 별도 함수 `matrow(...)`, `matcol(...)`, `column(...)`를 내장 함수로 만들지 않는다. 범위 반복으로 행렬들을 담는 nested array는 현재 결과 모델에서 지원하지 않으므로 명시적으로 실패한다.

`factorial`/`transpose`/`conjugate`/`determinant` 단항 계산도 Scope의 read-only reference 경로를 사용한다. Scope의 `Arc<Binding>`, alias, 캡처, 체크포인트, indexed zero-fill 차원 정보는 변경하지 않았다. 취소 검사와 실패한 선행 정의의 `previous_region`은 유지한다.

원본 descriptor에 없는 `<apply><norm/>…</apply>`를 native 파서에서 제거했다. 내부 `arrays::unary("norm",…)` helper는 기존 MPL 공통 dispatcher의 내부 호출 호환용으로 남지만 native 파일 형식으로 생성·인식하지 않는다. 사용자 함수 및 named `norm`은 영향을 받지 않는다.

## 검증과 남은 범위

신규 `tests/structural_operator_calculation.rs` 6개 headless 회귀는 다음을 검증한다.

- 혼합 단위·복소수 행렬의 행/열 선택, 방향, 계산식 좌항과 원본 비변경.
- ORIGIN 0/1/음수, 함수 정의 당시의 ORIGIN 캡처, 다른 ORIGIN으로 바뀐 이후의 일반 선택.
- 잘못된 타입/인덱스/arity/native opcode, named 함수 혼동 방지, 불필요한 ORIGIN 의존 방지.
- scalar magnitude, 열벡터 norm, 정방행렬 determinant 및 named `abs`/`det`/`norm`의 다른 계약.
- transpose의 비켤레 성질, conjugate 부호·차원, factorial 0/170 경계·정수/차원 검사·열벡터 mapping.
- 선택 결과의 소유 독립성, 실패 의존성 위치, cooperative cancellation.

기존 uniform physical determinant 동작은 보존했다. **`determinant` native의 물리 단위 최상위 타입 허용 범위는 아직 완전 검증되지 않았다.** 도움말의 단위 허용, `op_det`의 제한된 선언, generic `op_det2`의 관계를 모두 잇는 lowering/type-dispatch 경로를 추가 조사해야 한다. 해당 Rust 회귀는 현재 독립 구현 동작의 보존 검사이며 그 조합에 대한 원본 실행 비교가 아니다.

mixed physical matrix의 `MatDim` factorization, nested/symbolic 행렬, vectorize/cross product, 행/열 연산자를 좌변에 둔 대입, 정확한 원본 오류 문구·노드 강조, MKL과 모든 부동소수 경계의 동일성은 이번 완료 범위가 아니다. 일반 numeric 계산 연결을 원본 제품 전체 완료로 계산하지 않는다.

검사 로그는 ignored `artifacts/rea/structural-operators-compat-tests-20261016.log`, `structural-operators-clippy-20261016.log`에 있다. 계산 6개와 다른 담당자의 bridge 4개를 함께 포함한 최종 compat 전체 검사는 176 passed / 0 failed / 2 ignored였고, all-targets Clippy `-D warnings`도 통과했다. 전체 제품 합산은 부모 통합 검사를 따른다. 실제 GUI·사용자 화면의 조작 검증은 하지 않았다.
