# 전역 정의·벡터화·외적의 원본 저작 계약

2026-10-16. 기존 계산 지원과 별개로 원본 연산자를 새 수식으로 작성하고 native MathML로 저장하는 경로를 조사·구현했다. GUI·스크린샷·원본 앱·CLR·COM 실행 없이 설치 도움말, REA metadata/raw IL, inert BAML을 읽었다. [0074](0074-global-vector-operator-input.md)는 키 입력·조판, [0075](0075-global-vector-operator-calculation.md)는 원소 계산·shape·함수/단위 및 취소 계약을 담당한다.

## 원본 표현과 키

| 원본 명령 | descriptor | 독립 AST / API | 네이티브 XML | 원본 키 / glyph / binding power |
| --- | --- | --- | --- | --- |
| Global Definition | `GLOBAL_DEFINITION` | `BinaryOp::GlobalDefine`, `insert_global_definition()` | `globalDefine(lhs,rhs)` | Ctrl+Shift+~ / ≡ U+2261 / 10 |
| Vectorize | `VECTORIZATION` | unary `NodeKind::Vectorize`, `insert_vectorize()` | `apply(vectorize,expression)` | Ctrl+Shift+^ / 위쪽 오른쪽 화살표 U+2192 / 130 |
| Cross Product | `CROSSPRODUCT` | `BinaryOp::CrossProduct`, `insert_cross_product()` | `apply(crossProduct,left,right)` | Ctrl+8 / ⨯ U+2A2F / 120 |

Vectorize는 `apply`의 boolean 속성이 아니다. 식 전체를 감싸는 unary operator이고 writer는 `<ml:apply><ml:vectorize/>…</ml:apply>`를 만든다. 외적은 일반 곱셈 glyph와 구분하며 U+00D7로 추정하지 않는다. 전역 정의는 일반 `define`의 이름만 표시상 바꾼 연산자가 아니라 별도 `globalDefine` root다.

## 정적 원본 증거

원본 `Ptc.EquationEditor.dll` SHA `d419032c0e99a62eab2f278e571a338d8c4b9662655af8ab462d482a028c0223`, retained evidence `ev_419ab24312df266f2e58ff6b91d32e0261a9a2bba9105ba09ebecb7517b43fdf`의 metadata/IL을 재사용했다. `DefaultDescriptorTable.f` `0x06000bbb`, raw IL SHA `90ff4afc15ec4d75068d5d2c0eab08be91548ff38974f8de5b54e02a25a6376a`에서 다음을 확인했다.

- CROSSPRODUCT 초기화 IL 1980–2151: XMLFormat `crossProduct`, RLFormat의 op_cross, child count 2, OperatorSymbol 10799, precedence 120. IL 2084의 Associativity=2는 동일 SHA의 metadata enum `Associativity.Right`다.
- GLOBAL_DEFINITION 초기화 IL 6776–7194: XMLFormat `globalDefine`, child count 2, OperatorSymbol 8801, precedence 10. LHS에 허용되는 원본 타입과 parent ROOT 제약을 따로 둔다. 원본은 function call을 함수 선언으로 바꾸는 child 제약도 가진다.
- VECTORIZATION 초기화 IL 10919–11091: XMLFormat `vectorize`, RLFormat vectorize, child count 1, prefix fixity, container slot 0, OperatorSymbol 8594, precedence 130이다.

`MathNodeToXmlTransform.a` `0x06001167`, raw IL SHA `3930351561a894c121a700a2d9fa21cb2d5c8966dbba30f0550897f7c2136e89`의 dispatch는 GLOBAL_DEFINITION을 `TransformDefaultNode`, CROSSPRODUCT와 VECTORIZATION을 `TransformOperatorNode`에 연결한다. `TransformOperatorNode` `0x06001177`, raw IL SHA `edd18405d02dfd8f5cc817f0fe06287202b091b678d03d13bd1cd2cb078f7ba7`의 IL 168–209는 `apply` 아래에 descriptor.XMLFormat 이름의 operator를 만들고 child들을 이어 붙인다. 따라서 vectorize의 태그/배치를 다른 표 형식에서 추정하지 않았다.

전역 정의에 결과 `=`를 붙이는 계약도 확인했다. `EquationModel.get_EvaluationRoot` `0x060000f9`, raw IL SHA `31a66c2472ea35b934f5213b5ac45699be8474aed4a4074a9a11cccd28bddd0b`는 DEFINITION뿐 아니라 GLOBAL_DEFINITION을 검사하고 child 1이 evaluation이면 그 RHS를 반환한다. 전역 정의 전체를 또 다른 eval의 operand로 감싸는 경로가 아니다.

ignored `global-vector-decode.py`는 artifact SHA 및 REA raw IL SHA를 대조한 뒤 dncil로 바이트를 해석한다. `global-vector-contract-cil.json`/`.txt`에 실제 branch offset을 보존했고 `global-vector-descriptors.json`에 세 descriptor의 범위를 남겼다. 원본 메서드를 호출하거나 원본 소스를 독립 구현에 복사하지 않았다.

### 표 셀의 expression-only context

같은 EquationEditor artifact의 `SpecTableColumn..ctor` `0x06000a40`, raw IL SHA `433c8b7db95f495ef366b65ad88abc0b1c9cee23ccc41431a1e12a97cbbc1c78`는 header EquationModel의 IL 27–28과 unit EquationModel의 IL 135–136에서 `AllowExpressionsOnly=true`를 설정한다. body cell 생성 command `a.Do` `0x06000a70`, SHA `092c27294c4a5ab40a9ccd074a846916848a0f08095f5462f6f73e668b1c91aa`의 IL 14–15도 같은 flag를 설정한다.

실제 삽입 guard는 `EquationModel.InsertOperator` `0x0600017b`, SHA `1e29c7dc18bbf50e602aa8dffa24c21341be7d44956638b6351c0a37c46e232b`다. IL 99에서 flag를 읽고 true이면 DEFINITION(112)과 GLOBAL_DEFINITION(130)을 검사하여 IL 157로 반환한다. UI 버튼 비활성화만의 계약이 아니라 공통 모델 명령의 거절 계약이다. 이 메서드들도 위 retained CIL과 evidence identity ledger에 포함했다.

독립 Editor에는 기본 false인 `set_expressions_only(bool)` / `expressions_only()`를 추가했다. 표 셀 host가 이 context를 제공하면 지역·전역 정의 직접 API와 low-level binary/infix 삽입·연산자 교체 및 정의 clipboard replacement를 모두 문서 변경 없이 거절한다. 일반 식 연산자는 계속 편집한다. 이 flag는 문서 AST나 undo Snapshot에 포함하지 않으므로 context 변경으로 history/revision이 바뀌지 않고 Undo/Redo로 context가 되돌아가지 않는다. `clone_without_history()`는 context를 보존한다. 이 회귀는 context 보호를 위한 독립 구현이며 원본의 다른 모든 expression-only key 정책까지 복원했다는 뜻은 아니다.

## BAML 명령·상수·설치 도움말 교차 확인

기존에 정적으로 추출한 `ko/Ptc.Controls.EquationControl.resources.dll` SHA `943a2278738739403e7e2517199d056a368ee3b83c9ddff14b274ecbda4e53fb`, `visualoperatordescriptortable.baml` SHA `17d8c47b27fea89b0d61cb68d6011a85ee2c6215c2bc6b9b53b3b8308f5ccd36`를 재사용했다. 추출 방법과 metadata 경계는 [0069](0069-structural-array-operators-contract.md)에 기록되어 있다.

- BAML offset 12746의 CanonicalName reference 33은 MathNodeNames.Vectorization이고 키는 CTRL+SHIFT+D6, 표시 키는 Ctrl+Shift+^다.
- offset 19008의 CanonicalName reference 60은 MathNodeNames.GlobalDefinition이고 키는 CTRL+SHIFT+OemTilde, glyph ≡다.
- offset 19637의 CanonicalName reference 62는 MathNodeNames.CrossProduct이고 키는 Ctrl+8, glyph ⨯다.
- 같은 EquationEditor SHA의 CLI Constant table field `0x04000511`, `0x04000537`, `0x0400053e`는 각각 `GLOBAL_DEFINITION`, `CROSSPRODUCT`, `VECTORIZATION`이다. 표시 이름만 보고 명령을 연결하지 않았다.

설치 도움말도 `global-vector-evidence-identities.json`에 원본 절대 경로·byte length·SHA로 기록했다.

| `Help/en/PTC_Mathcad_Help/` 파일 | SHA-256 | 사용자 계약 |
| --- | --- | --- |
| `global_definition_operator.html` | `9ed12a6c5eaea08b687ff24208cc6eb9d1b4c41944a454b8f259e09496927672` | 문서 전체에서 유효하다. RHS의 다른 변수는 앞선 전역 정의여야 한다. 같은 이름의 중복 전역 정의와 지역 덮어쓰기, SolveBlock 내부 사용은 허용하지 않는다. |
| `example_interleaving_of_local_and_global_definitions.html` | `1052984e8207acd55cb57ca31f303122d783789eea22e38c5c731865e7a86ce0` | 지역·전역 정의를 섞은 문서의 원본 예제다. |
| `vectorize_operator.html` | `a7388a75226cbbe7e21000eb27254b08eac9e28907d43d9665c52697d9dafefc` | 식 안의 연산자·함수를 원소 단위로 적용한다. 배열끼리는 크기가 같아야 하며 scalar는 각 원소에 적용한다. |
| `using_the_vectorize_operator.html` | `3f8a744068124ecea37b73baa227b21b80094e0728d4b85d37509a3de059eda3` | 일반 행렬 곱과 원소별 곱을 구분한다. nested array는 한 단계만 내려가고 scalar 벡터화는 값에 영향을 주지 않는다. 배열 인수를 요구하는 함수를 원소별 호출하면 오류가 날 수 있다. |
| `vector_cross_product_operator.html` | `b224748b8e777f527e66077b887e2051e7a9a31bf6b3ea1892d357c39d694d56` | 두 operand는 각각 길이 3의 열벡터이며 방향은 오른손 규칙을 따른다. |

외적 BAML의 번역 설명은 vector/matrix라는 더 넓은 표현을 쓰지만 이것만으로 추가 차원의 operand를 지원한다고 해석하지 않는다. 계산 지원은 위의 명시적 3-element column 계약과 0075의 증거 범위를 따른다.

## 독립 편집과 bridge 연결

`GlobalDefine`는 일반 Define과 별도의 AST enum 및 XML 태그를 가진다. 지역 정의의 기존 `insert_definition()`과 혼동하지 않는다. `insert_global_definition()`은 region root의 이름·함수·indexed name에서 시작하고 call 형태의 함수 이름/parameter들을 선언 형태로 보존한 뒤 RHS placeholder를 연다. 기존 Define/GlobalDefine/Evaluate root나 숫자·일반 식에 다시 전역 정의를 중첩하지 않는다. 기존 지역 정의를 전역으로 전환하는 toggle은 이번 증거에서 확인하지 않아 추가하지 않았다.

`insert_evaluation()`은 GlobalDefine의 RHS를 감싸거나 기존 RHS Eval을 재사용한다. bridge의 parser와 writer도 GlobalDefine root 및 그 RHS Eval만 허용한다. Program 내부 또는 Vectorize/일반 식 아래에 중첩된 globalDefine, RHS의 더 깊은 곳에 들어간 Eval은 거절하여 raw XML 보존 경계를 유지한다. 전역 선언을 로컬 define으로 조용히 바꾸지 않는다.

표시·저장용 연산자를 삽입하는 것은 계산 요청과 별개다. editor API는 AST와 undo만 바꾸며 engine을 호출하지 않는다. `=`의 명시 평가 요청과 Enter/영역 이탈의 draft commit은 상위 입력 경로의 기존 계약을 따른다. root의 LHS 및 RHS Eval 인식·실패 전역 binding 보존과 계산 목록 포함은 상위 worksheet 통합 범위다.

CrossProduct는 기존 일반 곱과 별도로 저장하며 precedence 120으로 조합한다. 원본 Associativity.Right를 확인했으므로 같은 precedence의 연속 Cross 입력은 오른쪽으로 결합한다. 이 규칙을 근거 없이 다른 기존 연산자의 결합 규칙으로 확대하지 않는다. 명시적 Group 선택과 별개로 사용자 식을 임의로 commute하지 않는다.

Vectorize는 Conjugate와 같은 종류의 범위 container지만 별도 unary AST와 XML operator를 유지한다. 벡터화 범위의 그룹 종료와 경계 구조 삭제, 원래 식/label을 보존하는 Undo를 연결했다. 복사된 문서에 표현만 화살표로 그려놓고 일반 matrix 곱으로 계산하는 방식이 아니다. 수치 단계의 element-wise interpretation과 지원 연산 범위는 0075가 담당한다.

세 신규 명령 모두 MatrixCells 선택 중에는 문서와 history를 바꾸지 않는다. 원본 matrix range-mode의 일반 수식 삽입 금지 계약을 유지한다. 이름과 명시 UNIT label, 함수 formal parameter, literal suffix를 새로운 연산자 표시 때문에 재해석하지 않는다.

## 검증과 남은 경계

editor의 `global_vector_operators.rs`에 회귀 9개를 추가했다. descriptor arity/precedence, global LHS metadata 보존, RHS Eval 재사용과 단일 Undo, 함수 call→declaration, 기존 정의/invalid LHS에 중첩 금지, Cross의 오른쪽 결합과 Multiply보다 낮은 binding power, Vectorize container의 그룹 종료/삭제/Undo, 세 명령의 matrix range no-op, expression-only 정의 거절 및 context의 clone/load/Undo/Redo 보존을 검사한다. editor 전체 58개가 통과했다.

compat의 지정 파일 `tests/structural_global_bridge.rs` 4개는 operator/XML 왕복·arity·가짜 vectorize 속성 거절, 전역 변수/함수/indexed 정의와 RHS Eval 왕복, nested global/Eval 거절, 뒤에 선언된 전역 m을 앞에서 참조하면서 명시 UNIT m은 유지하는 계산을 검사한다. editor+compat `clippy --all-targets -- -D warnings`도 통과했다. ignored 로그는 `global-vector-editor-tests.log`, `global-vector-bridge-tests.log`, `global-vector-clippy.log`에 있다. formula key event 및 전체 worksheet의 저장/열기/계산 검증은 0074와 상위 통합 결과를 참조한다.

원본이 허용하는 이름 행렬 LHS·행/열 전체 LHS 전역 정의와 모든 scoped operand 부분 선택은 이번 core의 지원 범위를 넘는다. bridge는 기존과 같은 named variable/function/indexed-name 범위를 저장하며 더 넓은 LHS는 raw/미지원 경계로 남긴다. 현재 앱 SolveBlock은 내부 구조 Editor context가 없는 별도 입력 경로이며 전역 정의 명령은 바깥 Math region을 생성하므로 내부 global authoring을 지원한다는 보고가 아니다. Vectorize의 모든 사용자 함수·special operator·중첩 자료형을 지원한다는 보고도 아니다. 미지원 계산은 0075의 범위에 맞게 명시적 오류로 남긴다.
