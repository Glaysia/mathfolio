# Prime 12 배열 연산의 정적 계약과 독립 구현

2026-10-15. 원본 프로그램·CLR·COM·GUI를 실행하지 않고 설치된 파일을 REA로 읽었다. 이 문서의 런타임 수치 검사는 새 Rust 구현의 headless 회귀 검사이며, 원본 엔진의 신규 실행 oracle은 아니다. PTC 소스는 저장소에 복제하지 않는다.

## 증거의 경계

`efimatrix_.dll`은 managed/native 혼합 어셈블리이다. SHA-256 `0eda05cf6f4f67a46fba769143797c73f1a71b5a626bffddfebfda9ee9eee496`, MVID `f0915c22-a64f-4b58-ba5f-f200230b9210`. REA `inspect_managed_members` 증거 `ev_474b2253bb4b6bbdd8d962be20e081847ffc0c260d63a70db2408022c77cb830`는 489개 MethodDef를 제공하며 coverage는 partial이다. 이를 전체 native 해석 완료로 주장하지 않는다. file-backed CIL을 별도로 해독하고 매 메서드의 raw CIL SHA-256이 REA와 같은지 검사하여 413개 메서드를 분석했다. 도구는 원본 어셈블리를 로드하지 않는다. ignored 자료는 `artifacts/rea/array-deep/efimatrix-members.json`, `efimatrix-full-il.json`, `decode.py`이다.

`efimatrix.dll`의 `Mpl.Run.ModuleInterfaceAttribute`에서 부모 에이전트가 해시 검증한 MPL interface를 읽었다. operator의 별칭과 named function의 다른 제약, ORIGIN 인자를 구분하는 데 사용했다. 원문은 ignored `artifacts/rea/engine/efimatrix-interface.txt`에만 있다. 인터페이스 수치 handle만으로 native 구현을 추측하지 않고 실제 wrapper의 분기를 함께 확인했다.

해당 facade 어셈블리 SHA-256은 `5f037dd885d2760f4af9dc1e000c2220d08c19ef109b0c163dfe79167365c2de`, MVID는 `61993031-02d1-47d5-a686-313f4f466ed2`, REA evidence는 `ev_21f7f73e57f41b16c939ad649823f910075949cb91e105748efad3e3720a4a1f`이다. interface attribute token `0x0c000003`, row file offset 2278, 전체 Blob SHA-256은 `e0995bd604385e4b4e88656b33fa507aebfd13af7bf9d0344c2b456aa5735927`이다.

별도 결과 모델 증거는 `Ptc.Calculation.Core.dll`, SHA-256 `cf42d8116d697a560f84467c8c32eb8dc1a15172205772eaf4c95a1125eb01bf`, MVID `f26e51c8-54b1-4d6f-8816-d7805b72d6c6`, REA `ev_f9228e6a66d637e383c93ba06772fbcf894a724aa1b6bde52961a553bf6e8609`이다. `EagerMatrixSimpleResultNode.get_Item` (`0x060001cb`, method header file offset 11340, raw IL hash `af0d91ce727b0f3701b9251704acc68f06a4ede849b547501f3ae532ac067ce1`)은 row×Columns+column 위치를 사용한다. 결과 XML의 column-major 순서는 0003에서 별도 확인했다. 내부 배열 저장 순서와 XML 직렬화 순서는 다르다.

## 실제 분기에서 얻은 규칙

아래 CIL 시작 offset은 method header 다음 첫 명령의 파일 offset이며 토큰은 위 어셈블리 빌드에만 유효하다.

| 경로 | 직접 확인한 분기 및 동작 |
|---|---|
| `Efi.Matrix.mult`, `0x06000193`, 176116 | scalar/array mapping과 array×array를 분리. uniform scalar matrix는 dimension을 곱하고 `ScalarMatrixMult`로, 다른 physical matrix는 `CrazyMatrixMult`로 보냄 |
| `ScalarMatrixMult`, `0x06000016`, 162756 | numeric BLAS 경계 `_gemm`에 전달. 모르는 nested matrix는 별도 오류 |
| `_gemm`, `0x060000a8`, 214936 | column vector 두 개는 dot product 경로. 복소수 경로 IL_02b1..035d의 실수/허수 dot와 가감은 둘째 벡터의 conjugate를 사용하는 공식과 결합됨. 일반 matrix는 dgemm/zgemm; 결과 shape 1×1이면 real IL_0187..01a7 및 complex IL_0486..04a2에서 scalar로 압축 |
| `CrazyMatrixMult`, `0x06000017`, 163344 | `MatDim.make`, `StripDimension`, `MMult`/`DMult`, `ApplyLinearDimension`; unit mismatch와 linear-algebra matrix 오류를 구분. 단순 각 원소 곱이 아님 |
| `Efi.Matrix.div`, `0x06000196`, 176572 | IL_0000..0038에서 **오른쪽이 square matrix인지 먼저** 검사하여 Inv→mult. scalar/array map은 그 뒤. 따라서 `2 / diag(2,4)`는 `diag(1,0.5)`이며 각 0 원소를 나누는 연산이 아님 |
| `abs_mat`, `0x06000027`, 153880 | Cols=1이면 Norm, 그렇지 않고 Rows=Cols이면 Det, 그 밖에는 square 오류 |
| `Det`, `0x06000024`, 153748 | uniform matrix의 단위는 input dimension^Rows. mixed physical matrix에는 MatDim.Det/linear-algebra 검사가 별도로 있음 |
| `Efi.Matrix.abs_f`, `0x0600018f`, 172852 | named `abs`는 scalar 검사 후 abs_num. `abs(array)`와 `\|array\|`는 같은 함수가 아님 |
| `det_f`, `0x06000190`, 172900; `norm_f`, `0x06000191`, 174308 | named det는 square 검사, named norm은 column-vector 검사. MPL named det 인터페이스는 dimensionless matrix 제약이며 operator의 generic dimension 경로와 분리 |
| `submatrix`, `0x06000186`, 227684 | 인덱스 4개에 integer/bounds 검사. 증가 방향이면 SubMatrix, 역방향에는 별도 루프 및 Abs(end-start)+1 크기. 양끝 포함 및 역방향 추출을 지원 |
| `augment`, `0x06000188`, 145032; `stack`, `0x06000189`, 227404 | 각각 row/column 수를 검사하고 scalar는 1×1로 결합. numeric 값의 단위를 제거하지 않고 block을 복사 |
| `length`, `0x06000182`, 173324; `last`, `0x0600017f`, 173252 | column-vector 검사. length는 Rows, last는 ORIGIN+Rows−1. MPL의 last/submatrix에만 ORIGIN 인자가 존재 |

설치된 `lisp/front12.lisp:1911–1922`는 solve block에서 row-shaped LHS의 RHS를 transpose하는 별도 전처리도 보여 준다. 일반 수식 transpose와 이 solve-block 전처리를 합치지 않는다. 이 구현은 solve block을 지원하지 않는다.

## Rust 구현과 통합 API

`mathcraft-compat/src/arrays.rs`는 독립 구현이다.

- `binary(op,left,right)`: scalar 및 배열 덧셈/뺄셈 broadcast, 곱셈, 나눗셈, 거듭제곱.
- `unary(op,value)`: negation, conjugate, transpose, absolute/determinant/norm operator. MPL에서 `op_abs`, `op_norm`, `op_det`가 `Efi.Matrix.abs`로 연결되므로 마지막 세 표현 연산은 동적 shape 분기를 공유한다.
- `is_builtin(name)`: 실제 지원한 named function만 분류. 원본에 없는 `inverse`/`transpose`라는 named builtin을 새로 만들지 않는다.
- `call(name,args,origin)`: abs, rows, cols, length, last, identity, augment, stack, submatrix, det, norm, norm1/norm2/norme/normi, diag, tr. 내부 검사/시험용 inverse/transpose helper 호출은 가능하나 builtin 목록에는 넣지 않는다.

사용자 함수 binding을 먼저 검사한 뒤 FUNCTION 라벨의 named builtin을 호출한다. SYSTEM ORIGIN 조회는 last/submatrix에만 필요하다. 다른 배열 함수가 실패한 ORIGIN 정의에 불필요하게 의존하면 안 된다.

column-vector×column-vector는 `sum(a_i*conj(b_i))`의 scalar이다. 일반 행렬 곱은 `sum(A_ik*B_kj)`이며 conjugate하지 않는다. scalar×array는 원소별이다. 덧셈/뺄셈은 같은 shape의 배열끼리 또는 scalar broadcast이다. 물리 dot product의 합산은 첫 항의 차원으로 시작하여 dimensionless 초기 0을 임의로 더하지 않는다. 각 합산 항의 단위가 달라지면 오류이다.

determinant는 partial-pivot LU 방식, inverse는 pivoted Gauss-Jordan의 독립 구현이다. homogeneous physical matrix의 determinant 차원은 n배, inverse는 반대 차원이다. named det의 dimensionless MPL 제약을 검사한다. mixed physical matrix의 linear-algebra dimension factoring은 아직 구현하지 않아 명시적인 미지원으로 반환한다. 원본 MKL의 pivot tolerance나 bit-for-bit 결과까지 일치한다는 주장은 하지 않는다.

norm1은 최대 absolute column sum, normi는 최대 absolute row sum, norme는 Frobenius norm이다. named norm은 column vector의 Euclidean norm이다. norm2는 입력을 scale하여 overflow/underflow를 피하고 A* A의 real symmetric 표현에 cyclic Jacobi를 적용한다. 수렴 확인을 못 하면 오류를 내며 임의 근사값으로 성공 처리하지 않는다. 32×32까지만 허용한다. 행렬 원소 수는 16,384개, 단일 곱셈 항 수는 8,388,608개로 제한한다. 정수 matrix exponent의 절댓값 한도는 1024이다. 이후 `lsolve`의 정방형·전열 랭크 과결정·복소수·균일 단위 범위를 [0025](0025-linear-systems.md)에 따라 연결했고, 실수 대칭 `eigenvals`/`eigenvecs`는 [0026](0026-symmetric-eigensystems.md)에 기록했다. 원본이 지원하는 1/60 배수의 fractional matrix power, nested/string arrays, SVD, 일반/복소 eigensystem 및 mixed-unit factorization은 아직 미지원이다.

## 검증

`tests/arrays.rs`의 18개 headless 테스트는 complex dot의 conjugation 방향, 일반 matrix 곱, scalar mapping, unit-preserving sums/dot, nonconjugating transpose, pivoted inverse×원본=identity, 단위 exponent, singular/mixed-unit 실패, power와 vector mapping 구분, square denominator의 우선순위, 결합 순서, 역방향 submatrix와 ORIGIN, length/last 차이, complex spectral norm 및 1e−200/1e200 scale, 잘못된 입력과 자원 제한, `lsolve`의 square/complex/multiple-RHS/full-column-rank least-squares/단위 전파, 실수 대칭 eigensystem의 정렬·고유쌍·단위·실패 경계와 native MathML dispatch를 검사한다. 모두 통과했다. 기존 38개 원본 reference case를 검사하는 compatibility 테스트도 이 모듈 배선 후 통과했다. 본 검사는 이번 단계에서 원본을 실행해 비교한 것이 아니다.

독립 구현에서 아직 확정하지 못한 원본 오류 family는 `unsupported`와 구체 detail로 반환한다. 새로운 localized 오류 키를 추측해서 원본 호환으로 주장하지 않는다. 이미 확정된 물리 차원 불일치와 0 나눗셈만 기존 공통 family를 재사용한다.

설치 도움말의 보조 계약은 `multiplication_and_dot_product_operator.html`, `addition_operator.html`, `division_operators.html`, `joining_and_subdividing_arrays.html`, `norm_of_a_matrix.html`, `functions_for_creating_arrays.html`에서 읽었다. 도움말의 일반 설명과 실제 wrapper 분기가 충돌하는 경우에는 위의 CIL 분기를 우선하여 차이를 기록했다.
