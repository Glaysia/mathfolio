# 전역 정의·벡터화·외적의 계산 계약

2026-10-16. 원본 파일과 기존 REA 증거를 정적으로 조사하고 독립 Rust 계산 엔진을 검사했다. GUI·스크린샷·원본 프로세스·CLR·COM은 실행하지 않았다. 편집 descriptor와 XML 쓰기 경계는 별도 [0073](0073-global-vector-operator-contract.md), 입력·조판은 [0074](0074-global-vector-operator-input.md)의 담당 조사와 함께 본다. 이 문서의 실행 수치는 Rust headless 테스트 결과다.

## 원본 증거

### 벡터화

설치 도움말 `Help/en/PTC_Mathcad_Help/vectorize_operator.html`의 SHA-256은 `a7388a75226cbbe7e21000eb27254b08eac9e28907d43d9665c52697d9dafefc`이다. 화살표 아래의 연산자와 함수를 원소 단위로 계산하며, 배열끼리는 같은 행·열 수여야 하고 scalar는 각 배열 원소에 적용한다고 명시한다. `using_the_vectorize_operator.html` (`3f8a744068124ecea37b73baa227b21b80094e0728d4b85d37509a3de059eda3`)은 array 인자를 요구하는 함수에 화살표를 적용하면 오류이며, nested array는 한 단계만 내려간다고 설명한다. `example_vectorize_operator.html` (`494dbba80ebf71276ceaf66e59b0a67b468196e61e1cf10e9d35e3287025b919`)은 적용된 표현식 전체와 이미 계산된 중간 변수를 구분한다.

기존 REA `ev_927878a5d43cc634a6d156680cdd0bcbe644fb3278c2e3a04b84ec785346424b`의 현재 `mplrun.dll`을 다시 해시 검증했다. 파일 SHA-256은 `7c5fd2b212b683d5dcfb0e6a7ddf2a0450ccf93b663f70985e2e847d1f9ca798`이다. `Mcd.MatVal.vectorize` (`0x06000521`, CIL SHA `4da2e2ba1a6b80c55a831643f4f01026be90f117deb0947e60969bef07d77a19`)의 파일 바이트를 직접 해독해 다음을 확인했다.

- 모든 인자를 살펴 MatVal인 인자를 찾고, 첫 배열의 Rows/Cols를 기준으로 다른 배열의 두 크기를 모두 비교한다. 차이가 나면 `unequal matrix dimensions`를 발생시킨다.
- 배열 인자가 없으면 원래 인자들로 `DimVal.Apply`를 호출한다.
- 배열이 있으면 같은 크기의 새 결과를 만들고, 열을 바깥 루프·행을 안쪽 루프로 순회한다. 각 배열 인자의 해당 셀을 가져와 함수를 호출하고 결과 셀에 저장한다. 배열이 아닌 인자를 임의로 길이 1 배열로 바꾸지 않는다.
- 함수 호출의 인자를 매핑하는 경로이며, 이미 정의한 함수의 저장 본문이나 과거 중간 변수의 계산식을 다시 쓰는 동작이 아니다.

관련 `map` 메서드와 함께 raw CIL 해시를 검증한 자료는 ignored `artifacts/rea/global-vector-calculation-cil.json`, `.txt`, `global-vector-calculation-decode.py`에 있다. 원본 코드는 제품 소스에 복제하지 않았다.

설치된 Converter의 `mpl/mcdrun.mpl:1216` 이후도 arity별 vectorize helper와 `MatVal.vectorize` 연결을 보여 준다. 이 파일의 SHA-256은 `b552105944ae643b59d2f569e6327cecb1055d92be6d450b58c41f67e61d913b`이다. Converter 소스를 Prime의 현재 전체 frontend와 같은 버전이라고 가정하지 않고, 현재 mplrun CIL·현재 도움말과 일치하는 인자 매핑의 보조 증거로만 사용했다.

### 외적

`vector_cross_product_operator.html` (`b224748b8e777f527e66077b887e2051e7a9a31bf6b3ea1892d357c39d694d56`)은 두 피연산자를 원소 3개의 열벡터로 제한하고 오른손 법칙을 명시한다. descriptor의 넓은 설명 문자열만으로 일반 행렬 외적이나 row vector 지원을 추가하지 않았다.

기존 REA `ev_474b2253bb4b6bbdd8d962be20e081847ffc0c260d63a70db2408022c77cb830`의 `efimatrix_.dll` SHA-256 `0eda05cf6f4f67a46fba769143797c73f1a71b5a626bffddfebfda9ee9eee496`와 `Efi.Matrix.crossprod` (`0x06000192`, CIL SHA `bfa85fcab6aeeb46a4fa95ffdd9cb2d30583567dd3e91411763083f09ad0e70b`)를 다시 검증했다. 이 wrapper는 각각 3원소 벡터·비중첩·uniform dimension을 검사하고, 결과에 두 입력의 차원을 곱한다. 각 벡터 내부의 단위는 같아야 하지만 두 벡터가 서로 같은 단위일 필요는 없다.

복소수에서 켤레를 취하는지는 wrapper만으로 단정하지 않았다. `__unep@?crossprod…` FieldRVA `0x3d5c8`가 가리키는 VA `0x180014c70`을 읽고 PE의 runtime-function 범위 `[0x14c70,0x14fa4)`를 확인했다. 해당 820바이트의 SHA-256은 `0dd70653e7332cbc01db26e4a90c85b154fbf857121ebff28103b5dabf96772e`이다. 설치 LLVM의 읽기 전용 disassembly에서 세 성분의 실수·허수 곱셈/가감식을 추적했으며, 각각 `a_i b_j − a_j b_i`인 **bilinear 외적**이다. 입력이나 출력의 켤레를 취하지 않는다. 예를 들어 `(1+i,0,0) × (0,1+2i,0) = (0,0,−1+3i)`다.

이 native 후속 조사는 REA의 objdump provider가 `capability_unavailable / ERR_REA_WINDOWS_NATIVE`를 반환해 설치된 `C:/Program Files/LLVM/bin/llvm-objdump.exe`로 보완했다. 따라서 새 REA native-decompiler 결과라고 부르지 않는다. ignored `global-vector-cross-native.asm`, `.json`에 도구·명령·바이트 범위·해시를 남겼다. LLVM이 표시한 인접 export `comeval` 이름은 해당 함수의 식별 근거가 아니며, FieldRVA와 unwind 범위로 entry를 정했다. 원본 수치 경계·subnormal·overflow 처리까지 동일하다는 주장은 하지 않는다.

### 전역 정의

`global_definition_operator.html` (`9ed12a6c5eaea08b687ff24208cc6eb9d1b4c41944a454b8f259e09496927672`)은 worksheet 전체에 유효한 정의, 앞선 global만 RHS에서 참조 가능, 같은 변수의 global 중복 및 local override 금지를 명시한다. `example_interleaving_of_local_and_global_definitions.html` (`1052984e8207acd55cb57ca31f303122d783789eea22e38c5c731865e7a86ce0`)은 global-only 첫 pass와 일반 정의/평가의 두 번째 pass, 위→아래·왼쪽→오른쪽 순서를 구체적으로 설명한다. 일반 local이 화면상 먼저 있어도 global의 선행 환경에는 들어가지 않는다.

## 독립 구현과 검증

`compat/src/lib.rs`가 native `vectorize` 단항 연산과 `crossProduct` 이항 연산을 인식한다. named `vectorize(...)`나 `crossProduct(...)` 내장 함수는 만들지 않았다. 원본 arity/namespace/속성을 공통 strict parser로 검사한다.

벡터화는 기존 행렬 연산의 결과에 단순히 함수를 씌우는 것이 아니다. `Scope::evaluate_vectorized`가 화살표 안의 AST를 따라 각 application을 원소 단위로 계산한다. scalar 산술, 부호·절댓값·일반 단항 연산, 거듭제곱·root, 일반 함수 호출, 논리 비교/연산, 수치 행렬 리터럴을 처리한다. 호출되는 사용자 함수의 저장 본문은 일반 함수 호출 경로에서 실행한다. 인자에 배열이 있으면 결과가 scalar여야 하며, 현재 결과 모델이 표현하지 못하는 nested 결과를 임의로 펼치지 않는다.

`arrays::vectorize`는 scalar broadcast와 정확한 Rows/Cols 일치, column-major 방문 순서, 결과의 독립 소유, 셀별 물리 차원, cooperative cancellation을 담당한다. 1×1 행렬은 scalar로 취급하지 않는다. `arrays::cross_product`는 원본의 3×1·uniform unit 검사를 적용하며, Scope의 immutable binding을 빌려 읽어 선행 전체 행렬 복제를 하지 않는다. 기존 Scope Arc/alias/캡처/체크포인트와 indexed zero-fill metadata 구조는 유지했다.

기존 global 계산 엔진은 새 검증에서 원본의 두 pass와 정렬 규칙을 만족했다. 새 글로벌 함수 overload나 계산 순서 규칙을 추가하지 않았다. global 위치/내용이 바뀌면 일반 평가가 그 위에 있어도 무효화하며, prepare 단계는 과거 게시 결과를 유지하고 실행하지 않는다. 실제 계산 시 실패한 global은 의존성 오류로 전파한다. local override는 오류를 표시하고 원래 global binding을 덮어쓰지 않는다.

새 `tests/global_vector_calculation.rs`의 10개 테스트는 다음을 검사한다.

- 일반 matrix product와 벡터화 product의 구분, 화살표 내부 여러 연산 적용, 저장된 중간 변수 경계.
- scalar broadcast, reverse division, fractional powers, 복소수 magnitude, 물리 차원 및 행·열 방향이 다른 같은 원소 수의 배열 거절.
- 사용자 함수의 scalar 인자 매핑, 함수 본문 비재작성, nested 결과·array-only 함수 오류.
- 마지막 셀에서 계산이 실패해도 일부 결과를 게시하지 않고 정의가 실패로 전파됨; alias 원본 불변.
- 외적의 오른손 방향·반교환·복소수 비켤레 공식·입력 단위 곱, 잘못된 shape/혼합 단위 오류, ORIGIN 비의존.
- global의 선행 pass, regular/later-global 참조 실패, local override 금지, 공간 순서 편집 후 앞선 결과 무효화.
- global 함수 캡처, 기존 global indexed 정의와 local 충돌·중복 global 오류.
- 중단된 재계산의 이전 게시 결과 보존·Pending 유지와 재개.

`arrays.rs`의 추가 deterministic 회귀는 두 셀 처리 후 같은 control을 취소해 callback 방문이 `[1,3]`에서 멈추는 것을 검사한다. 원본 column-major 순서와 중도 중단이 결과/입력에 부분 쓰기를 남기지 않는지 확인하며, wall-clock sleep이나 경쟁 조건에 의존하지 않는다.

최종 compat 검사: **191 passed / 0 failed / 2 ignored**, all-targets Clippy `-D warnings` 통과. ignored 로그는 `artifacts/rea/global-vector-compat-tests-20261016.log`, `global-vector-clippy-20261016.log`이다. 전체 제품의 OS별 합산 검사는 부모 통합 결과를 따른다.

## 남은 경계

- nested array 저장/반환은 기존 Value 모델에 없으므로 한 단계 nested vectorization을 완료했다고 주장하지 않는다.
- 화살표 내부의 index/range/program/numerical/symbolic 특수형은 lowering 규칙을 충분히 확인하지 못해 명시적인 미지원 오류다. 일반 함수 안에 이미 지원되는 프로그램을 정의한 뒤 scalar 인자로 호출하는 기존 경로와 구분한다.
- 배열 원소마다 형상이 다른 결과, 함수 값 자체·외부 script/data 함수, symbolic vectorization, 원본의 모든 오류 위치/문구와 수치 경계는 미완료다.
- global의 matrix-of-names LHS, 포함 worksheet 간 충돌, solve block 안의 global 차단과 원본 전체 indexed-global 입력 제약은 이번 구현 확장 범위가 아니다. indexed-global 회귀는 기존 엔진 동작의 보존이며 원본의 모든 허용 입력을 확정하는 증거가 아니다.
- 논리식은 화살표 안에서 인자를 매핑한 뒤 기존 scalar 논리 평가를 재사용한다. 원본의 모든 단락 평가/오류 우선순위를 실행 비교한 것은 아니다.

기능 목록의 ‘표현 가능’이나 descriptor 존재를 계산 구현 완료로 세지 않는다. 이 변경은 위에서 실제 구현·검증한 numeric subset을 늘린 것이다.
