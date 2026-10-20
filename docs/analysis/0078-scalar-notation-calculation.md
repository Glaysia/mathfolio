# 퍼센트·각도·극형식 연산자의 계산 계약

2026-10-16. 입력·저장 identity는 [0077](0077-scalar-notation-contract.md)를
따른다. 설치된 도움말과 REA metadata 위치를 이용해 파일의 CIL을 정적으로
읽었다. 원본 Mathcad·GUI·CLR·COM은 실행하지 않았다. 아래 수치 예제의 실제
실행은 독립 Rust 엔진의 headless 검사이며 원본 runtime oracle이 아니다.

## 항목 구분

EA034는 DMS 세 성분을 받는 연산자가 아니라 단항 Degree `°`다.
원본 XML은 `apply(degree_op, x)`이며 frontend는 `op_mult(x, mc_deg)`로
낮춘다. 별도 affine `DMS` 함수나 임의 `dms` XML은 이번에 추가하지 않았다.
Percent 입력 연산자 `apply(percent, x)` 역시 결과의 Percent 서식과 별개다.
숫자를 입력할 때 0.01을 곱하며, 결과 서식은 그 계산 값을 바꾸지 않는다.

설치 도움말의 SHA-256은 다음과 같다. 위치는 설치 폴더의
`Help/en/PTC_Mathcad_Help/`다.

| 파일 | SHA-256 | 확인한 계약 |
| --- | --- | --- |
| `percent_operator.html` | `c01114f3dec5e3103a7ac9bfe7c0961e5dd59ce9d781c50760a56f8cc01f33bf` | 실수·복소수·벡터·행렬에 0.01을 곱함 |
| `degree_operator.html` | `4a71d14a212502884330b972d65ee0a6e3568a598f0fa2fa529de2ae177765dd` | 같은 입력 범위에 π/180을 곱하여 라디안 값으로 변환 |
| `polar_operator.html` | `3c029de9676b94fb613bca48777cef7816cb5f69580cfb655124242a97a4e339` | 크기와 라디안 각도로 복소수 생성, 각도에 Degree를 사용할 수 있음 |

## 실제 계산 경로의 정적 근거

아래 원본 파일의 SHA·MVID와 각 method의 raw CIL SHA를 검사했다.
검증된 decode는 ignored `artifacts/rea/scalar-notation-calculation-cil.json`,
`.txt`, `scalar-notation-decode.py`에 보존했다. Decode는 어셈블리를 로드하지
않으며, 원본 바이트와 구현을 제품 소스에 복사하지 않았다.

| 어셈블리 | SHA-256 | REA evidence ID |
| --- | --- | --- |
| `efimatrix.dll` | `5f037dd885d2760f4af9dc1e000c2220d08c19ef109b0c163dfe79167365c2de` | `ev_b7a909697fb7d91de7a681c28727de5dca9be2a4c1c8444094617564d7865120` |
| `eficore.dll` | `1bca7c1285a87d40ae4662f062155efe65fe4a673fe76c74811c7789fb053d93` | `ev_d1a2fa460549e186f494812827de4c4bc50d2b7ca7f1858617fe9877fc866ef8` |
| `mcdrun.dll` | `7a1f2d6926cde4af49f45f814d2ae01655562ebb1264a9af39a742edc3326009` | `ev_bd54b6d6912bc5b9065343a3e385ae3aad30e8cfe83786f472537f23b1de7042` |
| `mplrun.dll`, MatVal | `7c5fd2b212b683d5dcfb0e6a7ddf2a0450ccf93b663f70985e2e847d1f9ca798` | `ev_927878a5d43cc634a6d156680cdd0bcbe644fb3278c2e3a04b84ec785346424b` |
| 같은 파일, NumVal | 동일 | `ev_efda85b200d4ff8ada32cb0b648d128f8adf47f5936a0fb360caca310fdd61b7` |

### Percent와 Degree

`EfiMatrix.values.Update`, `0x06000001`은 정방향과 역방향 Percent 함수를
연결한다. 정방향 helper `0x0600000e`의 raw CIL SHA는
`d458db0d1df20c08beae861b1780960665675a93c43b2a97f91cf97643edd219`이며,
피연산자와 `EfiCore.values.mc__25_`를 `Efi.Matrix.mult`로 넘긴다.
`EfiCore.values..cctor`, `0x06000002`의 0.01 실수 상수와 `Update`,
`0x06000001`의 `mc__25_` 필드 연결을 함께 확인했다. helper 이름이나 `%`
문자열만으로 계산을 추정하지 않았다. 역방향 helper `0x0600000f`는 같은
상수로 나누지만, 이것을 별도의 저작 연산자로 노출하지 않았다.

Percent의 MPL interface는 `V {a} -> Value {a}`로 차원 변수를 유지한다.
Degree는 무차원 각도 단위 `deg`를 곱하므로 입력 물리 차원이 유지된다.
두 연산자는 복소수의 실수부·허수부 모두를 같은 실수 배율로 바꾸며,
직사각형·단일 셀 행렬도 행렬 shape를 유지한다.

### Polar

`EfiMatrix.values.op_polar`, `0x06000002`의 raw CIL SHA는
`4901787ba35ef48e00511589919d908f053fbcda163622b769dc63b621ad31ad`다.
그 경로는 다음과 같다.

1. 둘째 피연산자에 `check_real_v`를 적용한다. 그 결과가 scalar 실수이거나
   vector인지 검사하고, 그 밖에는 `must be vector` 오류를 발생시킨다.
2. 첫째 피연산자에 `check_non_negative_v`를 적용한다. 이 검사는 실수 변환
   후 0보다 작은지 확인하므로 음수 또는 허수가 있는 크기를 거절한다.
3. 첫째 피연산자와 `exp(i × 둘째 피연산자)`를 일반 `Efi.Matrix.mult`로
   곱한다. Polar 전용 elementwise 곱셈을 호출하지 않는다.

`mcdrun.dll`의 `check_real_v`, `0x06000007`,
`check_non_negative_v`, `0x0600000c`는 scalar 검사를 vectorize 경로로
보낸다. `check_non_negative`, `0x0600000b`는 엄밀히 `< 0`을 검사한다.
`mplrun.dll`의 `MatVal.get_IsVector`, `0x0600045b`는 `m_cols == 1`이며,
`NumVal.get_IsPureReal`, `0x06000564`와 `asReal`, `0x06000582`는 허수부가
0인지를 검사한다. 따라서 1×1 행렬도 vector 검사에 통과하며, 행벡터의
각도를 임의로 열벡터로 바꾸면 안 된다.

차원 제한과 실수 제한은 같은 검사가 아니다. 현재 `efimatrix.dll`의
ModuleInterface는 Polar 양쪽과 결과에 `V {}`를 지정한다. 비교하면 Percent는
앞서 본 차원 변수 `{a}`를 사용한다. 이에 따라 현재 구현의 Polar 피연산자는
무차원이어야 한다. `asReal`이 단위를 거절한다고 주장하지 않는다. 그것은
허수부만 검사하며, 무차원 계약은 MPL interface/type 경계의 근거다.
현재 facade의 18개 method와 전체 interface에서 별도의 단위 generic
`op_polar2` 경로는 확인되지 않았다. `2 V ∠ 90°`는 현재 명시적인 차원 오류이며,
단위를 복소수 전체에 곱하는 `(2 ∠ 90°) V`는 일반 scale 연산으로 처리할 수 있다.

배열 결과는 위 facade와 기존에 확인한 일반 곱셈 계약([0005](0005-array-semantics.md))의
조합으로 도출했다. 예를 들어 크기 `[1;2]`, 각도 `[0;π/2]`는 일반 column-vector
dot product가 둘째 벡터를 켤레 처리하므로 `1−2i` scalar다. 같은 식 전체를
벡터화하면 scalar Polar 적용을 매핑하여 `[1;2i]`가 된다. 이 예제의 원본 runtime을
실행한 것은 아니다. 원본 facade의 곱셈 호출과 기존 곱셈의 CIL 근거를 연결한
정적 해석이며, 독립 Rust의 두 경로를 별도 회귀로 검사했다.

## 독립 구현과 검사

`compat/src/lib.rs`는 정확한 namespace·arity의 `percent`, `degree_op`,
`polar`만 파싱한다. Percent와 Degree는 기존 numeric unary mapping을 사용한다.
Polar는 각도 scalar/column-vector와 각 원소의 실수·무차원 조건, 크기의
비음수 조건을 먼저 검사한 뒤 위 ordinary multiplication 경로를 사용한다.
벡터화된 식은 기존 화살표 evaluator가 각 scalar application에 적용한다.

`deg`라는 사용자가 정의한 VARIABLE은 Degree 연산자를 바꾸지 않는다.
명시적 UNIT 라벨의 `deg`와 VARIABLE 라벨도 별개다. `percent`, `degree_op`,
`polar`라는 named 사용자 함수는 그대로 함수이며, 같은 이름의 builtin alias를
추가하지 않았다. 각 피연산자의 원래 라벨·입력 AST는 bridge에서 보존한다.

`tests/scalar_notation_calculation.rs`의 8개 검사가 통과했다. 복소수·물리 차원·
직사각형·1×1 행렬, 라디안/명시적 Degree, 같은 이름의 변수와 함수,
ordinary/vectorized Polar의 차이, 음수·복소·물리 단위·잘못된 angle shape의 거절,
strict parser, 범위/잘못된 배열/nonfinite 거절과 cancellation을 포함한다.
전체 compat 테스트도 실패 없이 통과했고 기존 CPU benchmark 2개는 ignored다.
`cargo clippy -p mathcraft-compat --all-targets --no-deps --locked --offline -- -D warnings`
역시 통과했다.

원본 localized 오류 문구·인자 번호·노드 위치, nested/string 배열, nonfinite 및
underflow/overflow의 원본 bit 정확성은 재현 완료 범위가 아니다. 기존 numeric
배열 한도와 cooperative cancellation을 그대로 적용하며, 불명확한 값을 성공으로
대체하지 않는다. 양 OS 및 native export·재열기 검사는 parent integration 결과로 구분한다.
