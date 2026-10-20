# 배열 원소 정의: 원본 계약과 독립 계산 경로

2026-10-16. EA-041의 `a[0 := v` 및 두 index 정의를 조사·구현했다. 설치된 Mathcad Prime 12.0.0.1의 도움말, shipped Lisp의 제한된 구간, REA가 반환한 관리형 metadata/raw IL만 읽었다. 원본 앱·GUI·스크린샷·CLR·COM 및 원본 계산 엔진을 실행하지 않았다. 아래의 실행 검사는 독립 Rust 구현에 대한 headless 검사다.

## 확인한 사용자 계약과 저장 형태

설치 도움말 `Help/en/PTC_Mathcad_Help/`를 기준으로 한다.

| 설치 파일 | SHA-256 | 확인한 계약 |
| --- | --- | --- |
| `index_operator.html` | `3d5997e15472d892c1c55026410eef52d751ccfb6d06f22d03a587d14f8cb2ad` | index는 무차원 정수이고 기본 ORIGIN은 0이다. 이미 정의된 range로 배열을 정의할 수 있으며 마지막 원소를 먼저 정의하면 앞선 칸은 0으로 채운다. |
| `methods_for_defining_arrays.html` | `fea85703790059de1d630f1ec95034dd5d3b80faddf3794dda29cba98271c2f9` | range 두 개를 쓴 원소 정의, 행벡터·열벡터의 단일 index 갱신/확장 예제가 있다. 행벡터의 복소수·문자열 원소 갱신도 명시한다. |
| `to_set_the_array_origin.html` | `7e07229ac7f1b3019d75fa06b52375b06f933c6bea61ee79d2a1a4cb3440e712` | ORIGIN은 양수·음수 정수일 수 있다. 지역 정의는 오른쪽/아래에 적용한다. 메뉴의 0/1 선택지만이 유효 값 전체는 아니다. |
| `about_working_with_arrays.html` | `c64757f07b9d4601fb0f5cf243d53da19ed8fc93df40e7465156c0f10bdf3288` | ORIGIN보다 작은 index는 유효하지 않다. |
| `example_row_column_and_index_operators.html` | `7a2540e50594023bc5fcefd9a115eb7dd15e03841bf196a665bec218fc2dc737` | 저장된 MathML은 `define`의 LHS에 `apply/indexer`, 배열 이름, 두 index의 `sequence`를 둔다. named range 두 개로 만든 10×10 예제의 저장 결과도 포함한다. |
| `definition_operator.html` | `4ef22f7d6b263dd7561516ba004db15ea3cfda2a8a7afcbda6287951acd601c9` | 정의는 해당 지점 이후의 문서 순서에 적용한다. |
| `global_definition_operator.html` | `9ed12a6c5eaea08b687ff24208cc6eb9d1b4c41944a454b8f259e09496927672` | 같은 이름의 전역 정의는 하나여야 하며 지역 정의로 전역 이름을 덮어쓰지 못한다. |

두 index의 저장 형태는 직접 argument 세 개가 아니다. `apply(indexer, id, sequence(row, col))`이며 단일 index는 `apply(indexer, id, index)`다. 이것은 literal subscript로 저장되는 이름 suffix와 다르다. 예제의 구조적 사실만 사용했고 원본 수식 소스를 구현 파일로 복사하지 않았다.

## REA 정적 증거

새 `inspect_managed_members` 요청은 명시적으로 설치 경로 `mplrun.dll`의 `Mcd.MatVal`/`Mcd.DimVal`을 선택했다. evidence `ev_927878a5d43cc634a6d156680cdd0bcbe644fb3278c2e3a04b84ec785346424b`, artifact SHA `7c5fd2b212b683d5dcfb0e6a7ddf2a0450ccf93b663f70985e2e847d1f9ca798`, 218096 bytes. 선택 type의 metadata coverage는 complete다. 기존 `mcdrun.dll` evidence `ev_bd54b6d6912bc5b9065343a3e385ae3aad30e8cfe83786f472537f23b1de7042`, artifact SHA `7a1f2d6926cde4af49f45f814d2ae01655562ebb1264a9af39a742edc3326009`도 재사용했다.

ignored `artifacts/rea/indexed-definition-decode.py`는 원본 파일 SHA 및 REA raw IL SHA를 대조한 뒤 dncil로 바이트를 해석한다. `indexed-definition-contract-cil.json`에는 evidence ID, artifact identity, method token, raw IL SHA와 instruction offset이 있다. 원본 assembly를 로드하지 않는다.

| 메서드·token | raw IL SHA-256 | 관찰 |
| --- | --- | --- |
| `McdRun.values.to_matrix`, `0x0600002b` | `f13bb3d4bb8f796c5129ab405533e081d4e863a57ab6169b82ebf1bb05b1f928` | 기존 MatVal은 그대로 반환하며 다른 값은 빈 default matrix로 바꾼다. 원래 scalar를 새 배열의 첫 칸에 보존하는 동작은 없다. |
| `McdRun.values.set_1`, `0x0600002f` | `0d3a502112c7c8df82ebc55b03a4d11d501a08259216bde79b17384cc170c38d` | matrix 변환, 정수 index 변환, MatVal 원소 setter를 잇는다. |
| `McdRun.values.set_2`, `0x0600002c` | `a2ca1fcffbaf00f59630d1a6c37e5cd627bdf3b9edb93ec4db988128eb09885b` | 두 index setter도 같은 변환 경로다. |
| `MatVal.mcd_set_1`, `0x060004f4` | `794dbfd5dda9bf8637d8bcd948b7dd481a85aca8bbd8e6ac86d3ca45fde62b23` | 음수 offset을 거절하고 immutable이면 재할당한 뒤 setter를 호출한다. 이 저수준 경로는 cols>1도 거절한다. |
| `MatVal.mcd_set_2`, `0x060004f9` | `b4f6c880607a8f7b190b5c94c8cb19d5bab65fa728a194704b5a303a293f462b` | 두 offset의 음수 검증 후 복사 또는 확장한다. |
| `MatVal.ReAllocate(1D)`, `0x06000506` | `43ec2ed58729092daa881473fff650243cdb7908c36347e2aed4b16821c5f20c` | 기존 크기와 필요한 크기 중 큰 것을 유지한다. |
| `MatVal.ReAllocate(2D)`, `0x06000507` | `054a92dcb9acec86d5dfa8e5b9d35a6756937f60535ed3c1f7cc15122db59bf7` | 행·열 각각 기존 크기를 줄이지 않는다. |
| `MatVal` copy/resize ctor, `0x06000491` | `50d487dac988b73825a389b9b393b0ff50c69f523d213786a46c0381b33f57d2` | 실수·허수 배열과 dimension field를 복사한다. dimension 복사는 IL 69–76이다. |
| `MatVal` allocating ctor, `0x06000476` | `9148a5051215d65015e6718683365812bbc285d338af2a0b9b379112445025bb` | double 배열을 새로 만들며 dimension field를 지정하지 않는다. |
| `DimVal` base ctor, `0x060006cb` | `10d02b6f0bf17fe8b1e18ee245b3994ec19c932fe60b0b5ffc2c9e7f7e045d28` | Object constructor만 호출한다. 새 dimension field는 null이다. |
| `MatVal.Fix`, `0x06000528` | `05427cae0cf9479b0ef27e70759a2837faca7d6a3940628eb54eae34acdba341` | immutable flag만 지정하며 dimension을 바꾸지 않는다. |

`MatVal.set`의 dimension overload `0x06000511`과 복소수 overload `0x06000513`도 retained decode에 있다. null matrix dimension은 처음 들어온 값의 dimension으로 지정한다. 이후 다른 dimension의 원소는 cell별 boxed value로 둔다. 따라서 단위가 있는 첫 값 뒤의 0 padding을 무조건 무차원 0으로 바꾸거나, 뒤의 혼합 단위 갱신 때문에 배열의 기존 fill dimension을 잃으면 안 된다. 메서드별 SHA는 같은 ignored identity ledger에 보존했다.

행벡터에는 증거 층의 차이가 있다. 공개 도움말은 단일 index 행벡터 갱신을 명시하지만 위 저수준 setter는 cols>1을 거절한다. 상위 lowering/행벡터 변환 wrapper는 아직 복원하지 못했다. 독립 구현은 명시된 사용자 계약에 따라 이미 1×N인 배열의 단일 index를 열로 해석하여 갱신·확장한다. 저수준 IL 자체가 이 변환을 한다고 주장하지 않는다. 일반 `v[i]` 조회는 기존 열벡터 한정 경로를 유지하며, 행벡터는 명시적 두 index 조회 또는 전체 `v=`로 볼 수 있다. indexed definition의 inline result는 방금 지정한 행벡터 칸을 표시한다.

## shipped frontend와 범위 정의

`lisp/front12.lisp` SHA `5b15dd2d9066b253200133a5b5fd79263718a9f5f5d4e520cdadda5c104e01ee`를 정적으로 읽었다. 원본 텍스트는 저장소에 복사하지 않고 `indexed-definition-evidence-identities.json`에 범위별 byte offset/length/SHA만 기록했다.

- lines 2442–2485, byte offset 83569, range SHA `90d1404c8217e8da2c391b0fb78ab865d943f45bc501a76324052fc00dde66e6`: 전역 정의가 공통 정의 경로로 이어지고 임시 변수 추출은 subscript의 base까지 내려간다. 고유 이름의 indexed global 정의를 공통 전역 규칙에 연결할 근거다. 전역 indexed 정의의 모든 변형을 원본 실행으로 확인한 것은 아니다.
- lines 2554–2618, byte offset 87703, SHA `309997e996c6130a2781d493d55dafd4bebecefb77f1a1832ac68db2fb30ec03`: subscript 식의 변수를 평가하여 range 여부를 검출하고 정의를 dictionary 평가 경로에 전달한다. 표시 단위로 나누기 전에 물리 matrix를 보존하려는 callback 경계도 있다.
- lines 2625–2660, byte offset 90383, SHA `e2f5a834c0f27d1c71f83d35a95a673664a9c8529f6778620e7574c0da9b1fa5`: 일반 indexed define-eval은 LHS를 평가하지만 ranged subscript는 별도의 range-loop marker를 사용한다. 독립 구현은 이 경우 임의의 숫자나 결과 벡터를 만들어 표시하지 않는다.

두 named range의 Cartesian 결과가 성립하는 도움말 예제는 확인했다. 같은 range 이름을 양쪽 index에 반복하면 같은 scalar binder로 취급한다. 서로 다른 range의 순서는 독립 구현에서 안정적으로 정렬하지만, 원본의 self-dependent recurrence 실행 순서는 이 조사에서 확정하지 못했다. 따라서 범위 정의 RHS가 갱신 중인 배열 자체를 읽는 recurrence는 명시적 미지원 오류로 남긴다. 단일 scalar index에서 `a[0] := a[0] + 1` 같은 기존 값 참조 및 이전 시점에 capture된 함수 호출은 지원한다.

## 구현과 실패 경계

`Statement::IndexedDefine`는 이름 정의와 별개로 index들을 보존한다. bridge가 LHS base를 찾고 문서 좌표 순서·global prepass·checkpoint binding name에 같은 이름을 사용한다. A/m/V처럼 SI 단위와 같은 새 배열 이름도 LHS에서는 VARIABLE로 선언하며, 명시적 UNIT로 저장된 RHS 단위는 그 상태로 유지한다.

독립 `IndexedArray` builder는 두 index의 행·열 확장 시 기존 칸과 orientation을 보존한다. 새 칸은 처음 배열에 설정된 dimension의 0으로 채운다. private binding에 fill dimension을 보관하여 mixed-unit 원소를 수정한 후에도 padding의 단위가 바뀌지 않는다. 단순 alias 정의와 checkpoint 복원은 이 정보를 함께 복사한다. 이전 alias나 함수 capture의 배열을 나중 원소 정의가 변경하지 않는다.

정의는 임시 배열에서 완성한 뒤 한 번에 binding으로 출판한다. index·RHS·range 중간에서 실패하면 정의된 이름을 실패 binding으로 바꿔 이후 참조가 예전의 정상 값을 조용히 재사용하지 않게 한다. 앞선 alias는 유지한다. 취소는 이전 binding과 fill dimension을 보존하며 재시도할 수 있다. 이 atomic publication은 독립 앱의 기존 계산/취소 계약에 맞춘 결정이고, 원본 setter의 전체 실패 트랜잭션을 복원했다고 주장하지 않는다.

ORIGIN은 배열을 이동시키지 않고 그 시점의 index→offset 매핑에만 적용한다. index에 단위·허수·소수 또는 ORIGIN보다 작은 값이 있으면 거절한다. offset과 행렬 원소 수, range 작업 수에는 기존 독립 엔진의 16384 budget을 적용한다. 이것을 Mathcad의 원본 최대 크기라고 표시하지 않는다.

이미 외부에서 유입된 mixed-unit literal matrix에는 원본의 숨은 fill dimension 정보가 없다. 기존 칸 덮어쓰기는 가능하지만 차원이 다른 칸들을 가진 배열의 확장은 추정하지 않고 명시적으로 거절한다. 함수 반환 등에서 private fill metadata가 소실된 mixed array에도 같은 제한이 있다. 일반 2D 행렬을 단일 index로 펴지 않는다. 문자열 원소와 symbolic/indexed program-local 정의, range recurrence는 여전히 미구현이다.

## 검증

`crates/mathcraft-compat/tests/indexed_definitions.rs`의 11개 headless 회귀는 새 벡터와 3×6 matrix의 zero fill, 단위 혼합·복소수·행벡터 확장, scalar self-update와 이전 함수 capture, ORIGIN 음수/변경/오류, named ranges와 diagonal, alias와 checkpoint replay, global 충돌, native MathML 왕복, 표시 단위와 물리 값의 분리, SI 이름 라벨, 범위 중간 실패와 이전 alias 보존을 검사한다. private cancellation 검사 한 개는 범위 계산을 중단한 뒤 이전 1×1 배열/차원을 유지하고 재개 시 302×1 결과를 완성하는지 확인한다.

formula headless 검사는 실제 키 텍스트 `A[2` 다음 `:=7A=`를 구조 편집기로 입력하고 indexed Define AST·원본 MathML 왕복·계산 결과·한 텍스트 이벤트의 Undo/Redo를 확인한다. ORIGIN=1, range i/j와 두 index `M[i,j:=i^2+j`를 별도 region들로 저작한 뒤 `M[3,2=`의 11도 확인한다. 이 경로에는 GUI나 원본 런타임이 없다.

Windows dev에서 indexed 회귀 11/11, formula 전체 52개 통과/기존 benchmark 1개 ignored, compat+editor `clippy --all-targets -- -D warnings` 통과했다. 행벡터·SI 라벨 회귀 추가 직전의 compat 전체 검사도 실패 없이 통과했다. ignored 로그는 `indexed-definition-compat-all-tests.log`, `indexed-definition-formula-test.log`, `indexed-definition-formula-all-tests.log`, `indexed-definition-clippy.log`이며 최종 같은 소스의 전체 양OS 검증은 상위 통합 작업이 수행한다.
