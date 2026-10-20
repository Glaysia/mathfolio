# 행·열 연산자 좌변 정의의 계산 계약

2026-10-19. Mathcad Prime 12.0.0.1의 설치 파일을 REA로 정적으로 조사하고 독립 Rust 계산 경로를 연결했다. GUI·스크린샷·원본 Mathcad 프로세스·CLR·COM은 사용하지 않았다. 수치 회귀는 Mathcraft의 headless 실행이며 원본 실행 비교가 아니다.

## 원본 허용과 저장 구조

[0081](0081-array-slice-definition-contract.md)의 descriptor 조사에서 일반 정의·전역 정의·프로그램 local assignment의 좌변에 MATRIX_ROW/MATRIX_COLUMN이 명시적으로 허용된다. 저장은 기존 `apply(matrow|matcol, id, index)`를 `define`, `globalDefine`, `localDefine` 좌변에 둔다. 임의의 새 native 태그나 명명 함수를 만들지 않았다.

설치 `lisp/front12.lisp` SHA-256 `5b15dd2d9066b253200133a5b5fd79263718a9f5f5d4e520cdadda5c104e01ee`의 `make-temp-pairs`(2442–2485), `make-lhs`(3255–3282), `get-defined-vars`(3306–3323)는 행·열 좌변의 base를 정의 이름으로 취급하고 인덱스 식은 정의 이름으로 바꾸지 않는다. 기존 전역 prepass·같은 이름 충돌·실패 binding도 base 이름으로 처리할 근거다. 각 구간의 byte offset·length·SHA는 ignored `array-slice-source-identities.json`에 남겼다.

## REA 관리형 setter 증거

새 REA `inspect_managed_members`는 설치 `mplrun.dll`의 `Mcd.MatVal`만 선택했다. parent evidence `ev_26d43421eb8c0e5f92f001dcbe07ce4d97019ab0c72ce4c62824e1592f07d146`, artifact SHA-256 `7c5fd2b212b683d5dcfb0e6a7ddf2a0450ccf93b663f70985e2e847d1f9ca798`, MVID `c5528d9a-5ff3-4413-9e46-fa0f8138b8fe`다. selected metadata coverage는 complete다. 핵심 메서드 8개의 retained view evidence는 `ev_93a0042d04cc8bd0a0639d12496b84cc3988f25d6587a6bcf546b66d90dc8d5b`다.

ignored `decode-array-slice.py`는 같은 DLL identity의 기존 REA full member evidence를 재사용해 raw 파일/CIL SHA를 검증한 뒤 dncil로 해석한다. `array-slice-contract-cil.json/.txt`는 token·offset·원시 IL SHA·분기를 보존하고, `array-slice-rea-selected.json`은 새 retained view를 보존한다. 원본 assembly를 로드하지 않는다.

| 메서드·token | raw IL SHA-256 | 직접 관찰한 계약 |
|---|---|---|
| `get_IsVector`, `0x0600045b` | `1f00b19691cc4ba48ad67ba814def0c9932259fb60e8f2bf218bd715f2e4c5b6` | 열 수가 1인지 검사한다. 행벡터를 포함하는 일반적인 vector 판정으로 바꾸지 않는다. |
| `mcd_setcolumn`, `0x060004fc` | `2f217a85f94c4974e904fa18b4637c205af2f82a185293b91676cb30ffac9888` | 음수 offset 거절. 비행렬 target은 RHS.Rows × (index+1)로 새로 생성한다. scalar RHS는 Singleton으로 바꾸며 이전 scalar target을 보존하지 않는다. immutable target은 ReAllocate 후 갱신한다. |
| `setcolumn`, `0x060004fd` | `0db54b876ff7ed6c7a14acb89d4e6c56080699dfe23b2b31c44520812a2869a4` | RHS가 column vector여야 한다. Stretch 후 prefix만 복사한다. 길이가 짧다고 기존 나머지 칸을 지우거나 broadcast하지 않는다. |
| `setcol`, `0x060004fe` | `9e538a65c705a0f5138a12fa9e6e44c86545e92ada349fa3eed3fa85405aa2c2` | dimension이 비어 있으면 RHS dimension을 채택하고, 다르면 cell별 pointer 경로를 사용한다. 실수/허수 배열을 모두 복사한다. |
| `mcd_setrow`, `0x060004ff` | `261dfb67b539427ce789045e8b8b109e6677921837abeb234fa99ab5aff238ac` | 비행렬 target은 (index+1) × RHS.Cols로 생성한다. scalar Singleton 및 immutable ReAllocate 분기가 있다. |
| `setrow`, `0x06000500` | `6aa4ddf5b8c2c03046e7136f58bed9a681b6ee40c115d5a9b7cc5e13433106f2` | `setblock(row,0,RHS)`로 전달한다. column setter와 달리 IsVector 검사가 없다. |
| `setblock`, `0x06000501` | `75ff70fd684a9c116b9063422fb316f7c9256270e6c9f38405d4b97e75eb27f2` | scalar는 한 칸, matrix는 RHS.Rows × RHS.Cols 블록을 해당 행·열 위치에 복사한다. 필요한 크기로 확장하며 앞선/나머지 셀은 유지한다. |
| `setmatrix`, `0x06000502` | `bcfec77ac49227fc855abe7c49e727fc53f7622e3c0d4577d38d75f1d87906c3` | 실수·허수 및 dimension/pointer 표현을 복사한다. |

`ReAllocate(2D)`와 copy/resize ctor의 max-size·dimension 보존은 [0063](0063-indexed-definition-contract.md)의 동일 DLL 증거를 재사용했다. `Stretch`, `0x0600053b`도 이번 decode에 포함된다. **Row RHS의 일반 matrix block 허용은 이 저수준 setter의 직접 증거다. Prime 12 frontend의 모든 상위 typing/lowering 분기까지 확인한 것은 아니다.** 도움말은 이 경우를 별도로 설명하지 않는다. block 회귀는 해당 setter 계약의 독립 재구현 검사로 구분한다.

## 독립 구현

`Statement::IndexedDefine`가 기존 원소 index와 행/열 slice 종류를 함께 보존한다. 공통 `Scope::define_array_at_depth`는 원본 base 이름에 대해 기존 source snapshot을 읽고 임시 배열을 완성한 뒤 한 번에 새 binding을 출판한다. 원소 정의의 기존 wrapper와 checkpoint·global prepass·중복/지역 충돌·실패 전파는 같은 경로를 사용한다.

- `matcol`은 scalar 또는 N×1 RHS를 허용하고 행 0부터 지정 열에 복사한다.
- `matrow`는 scalar 한 칸 또는 matrix 블록을 지정 행·열 0부터 복사한다. 짧은 입력은 나머지를 유지하며 긴 입력은 두 축을 필요한 만큼 확장한다.
- ORIGIN은 기존 SYSTEM binding의 정수 검증과 index→offset 변환을 재사용한다. 음수 ORIGIN도 가능하며 단위·허수·분수 index 및 ORIGIN보다 작은 index는 거절한다.
- 새 homogeneous physical 배열의 padding은 RHS의 차원을 유지한다. 기존 mixed 배열은 private zero-fill metadata를 유지한다. 이름으로 전달한 RHS의 같은 metadata도 이어받는다. 외부 mixed literal처럼 숨은 fill basis를 알 수 없을 때는 이를 추정하지 않고 padding이 필요한 확장을 명시적으로 거절한다. 완전히 덮어쓰는 새 block에는 padding이 없으므로 그 복사는 가능하다.
- alias·함수 capture·체크포인트는 immutable binding을 공유하고 이후 갱신이 이전 배열을 바꾸지 않는다. 프로그램의 일반 배열 alias도 fill metadata를 함께 유지하도록 보완했다.
- 실패는 해당 base를 failed binding으로 바꾸고 이전 alias를 보존한다. 취소는 새 binding을 출판하지 않아 이전 배열과 fill metadata를 그대로 유지한다. 원본 setter의 오류 트랜잭션 전체가 아니라 독립 앱의 기존 원자적 publication/취소 계약을 유지한 것이다.
- inline `:= … =`는 갱신 후 해당 행/열 전체를 조회한다. 따라서 prefix만 갱신했으면 보존한 나머지 셀도 결과에 들어간다. 물리 배열은 표시 단위로부터 분리해 보존한다.

`program.rs`의 local assignment도 같은 primitive에 연결했다. 명명 변수의 기존 assignment와 함께 named index/행/열 target을 지원하고, index/RHS의 이름 검증을 방문한다. index/RHS 계산은 현재 evaluation depth를 전달하므로 프로그램 assignment를 통해 함수 호출 깊이 제한을 초기화하지 않는다. caller의 배열·ORIGIN은 함수 정의 당시 capture한 scope에서 읽으며 지역 변경은 caller에 출판하지 않는다.

16384 array element/work budget은 Mathcraft의 기존 한계이지 Prime의 최대 크기라는 주장이 아니다. nested/string/symbolic arrays, 복잡한 좌변 중첩, self-dependent ranged recurrence, 모든 원본 오류 문구·강조 노드, 모든 상위 typing 분기는 여전히 완료 범위가 아니다.

Row/Column slice는 scalar index만 지원한다. `front12.lisp`의 `gather-subscripts-1`(2993–3012)는 `subscript`를 특수 처리하지만 `op_matrow`/`op_matcol`을 같은 range 탐지 경로에 넣지는 않는다. 따라서 기존 indexed range 반복 기능을 slice에 그대로 확장하지 않고 range-valued slice index는 명시적으로 거절한다. 원본 lowering/inline range 결과까지 확인되지 않은 기능을 새로 발명하지 않기 위한 경계다. 기존 일반 indexed range 정의는 유지하며 프로그램 `for` 반복 중 scalar가 된 index는 일반 scalar 계약으로 처리한다.

## 회귀 검사

`tests/array_slice_definitions.rs`의 headless 회귀 11개는 새 target/scalar replacement, 방향·prefix 보존·두 축 확장, Row matrix block/Column 거절, 음수 ORIGIN·단위·복소수·alias/function capture, global prepass/충돌/실패, range 경계, checkpoint/inline 결과, 잘못된 index·budget, 프로그램 행/열/원소 assignment, mixed fill metadata 전파, 중첩 프로그램 호출 깊이를 검사한다.

`indexed_definition_tests::cancellation_during_slice_growth_keeps_the_published_array_and_can_resume`는 작업 중 취소가 기존 `Arc<Binding>` identity와 physical fill basis를 유지하며 재개가 100×81 배열을 완성하는지 확인한다. 새 문제를 가리는 skip이나 원본 GUI 검사는 없다. Windows compat 전체 검사와 Clippy 로그는 ignored `array-slice-compat-tests-20261019.log`, `array-slice-compat-clippy-20261019.log`에 기록한다. 최종 양OS·문서 native export/PDF 통합 결과는 상위 검증 문서를 따른다.
