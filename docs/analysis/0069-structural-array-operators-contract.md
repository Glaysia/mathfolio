# 구조 연산자의 원본 식별자·저장·편집 계약

2026-10-16. Factorial, transpose, complex conjugate, 절대값/행렬식, Norm, 행/열 추출을 함수 호출과 구분하여 저작하는 경로를 조사·구현했다. 원본 앱·GUI·스크린샷·CLR·COM을 실행하지 않았다. 설치 도움말, shipped BAML의 inert record, REA가 반환한 metadata/raw IL을 읽었다. 수식 입력과 그림은 [0070](0070-structural-array-operators-input.md), 수치 계산과 지원 경계는 [0071](0071-structural-array-operators-calculation.md)의 범위다.

## 원본 이름과 명령의 구분

`NodeKind::Norm`이나 네이티브 `<norm/>`를 추가하지 않았다. 조사 결과 원본의 사용자 명칭과 내부 descriptor 명칭이 다르기 때문이다.

| 원본 사용자 명령 | 원본 descriptor | 네이티브 MathML operator | 독립 AST / 삽입 API |
| --- | --- | --- | --- |
| Factorial, `!` | `FACTORIAL` | `factorial` | `Factorial`, `insert_factorial` |
| Matrix Transpose, Ctrl+Shift+T | `MATRIX_TRANSPOSE` | `transpose` | `Transpose`, `insert_transpose` |
| Complex Conjugate, Ctrl+Shift+( | `CONJUGATE` | `conjugate` | `Conjugate`, `insert_conjugate` |
| Absolute Value/Determinant, `|` | `ABSOLUTEVALUE` | `absval` | 기존 `Absolute`, `insert_determinant` |
| Norm, Ctrl+Shift+\| | `DETERMINANT` | `determinant` | `Determinant`, `insert_norm` |
| Matrix Row, Ctrl+Shift+R | `MATRIX_ROW` | `matrow` | `Row`, `insert_row_operator` |
| Matrix Column, Ctrl+Shift+C | `MATRIX_COLUMN` | `matcol` | `Column`, `insert_column_operator` |

row 추출은 함수 이름을 임의로 연산자로 만든 것이 아니다. 설치 `row_and_column_operators.html` 및 `keyboard_shortcuts_for_matrix_operators.html`이 전용 행/열 연산자와 각각의 키를 명시하며 REA descriptor도 두 연산자를 별도로 선언한다. `insert_matrix_row/column`은 행렬 구조에 행/열을 추가하는 기존 API이므로 추출 API와 이름을 분리했다. factorial/transpose 등과 같은 이름의 사용자 함수도 기존 `Call`로 유지하며 descriptor operator와 혼동하지 않는다.

## REA identity와 descriptor 계약

기존 retained evidence를 재사용하고 설치 파일과 raw IL SHA를 다시 검증했다.

- `Ptc.EquationEditor.dll`, SHA `d419032c0e99a62eab2f278e571a338d8c4b9662655af8ab462d482a028c0223`, evidence `ev_419ab24312df266f2e58ff6b91d32e0261a9a2bba9105ba09ebecb7517b43fdf`.
- `Ptc.Controls.EquationControl.dll`, SHA `0b6fd580eb6cc183f17b5b5e0967226bbafc5e12efc6da809f44cd16416bf9d1`, evidence `ev_389338a9845c7ea83dfd310922316cf92ca1637bdbac6602a01c4690e52b8349`.
- `DefaultDescriptorTable.f`, token `0x06000bbb`, raw IL SHA `90ff4afc15ec4d75068d5d2c0eab08be91548ff38974f8de5b54e02a25a6376a`가 descriptor 이름, XML 이름, arity, fixity, binding power와 child 제약을 설정한다.

ignored `structural-array-operators-decode.py`는 REA method body 위치에서 바이트만 읽고 artifact/raw IL SHA를 대조한 뒤 dncil로 해석한다. 결과 `structural-array-operators-cil.json`/`.txt`는 원본 메서드 호출이 아니다. `structural-array-descriptors.json`에는 initializer의 선택 descriptor 구간과 setter offset을 따로 보존했다.

설치 도움말 7개(`factorial_operator`, `transpose_operator`, `complex_conjugate_operator`, `determinant_operator`, `norm_operator`, `row_and_column_operators`, `keyboard_shortcuts_for_matrix_operators`의 `.html`)의 원본 절대 경로·byte length·SHA도 `structural-array-evidence-identities.json`에 기록했다. 예를 들어 행/열 도움말 SHA는 `a870677758a3602c0fc4deabcefd06511fb08817a03a39a46b1eca9e61fa5b2e`, 행렬 단축키 도움말 SHA는 `bd88d52e9aca4b7aa660e9af9983daa8906c48a83bae1d318cad21bbac3ade1f`다. 설치 내용과 독립 앱 테스트 결과를 구분한다.

| descriptor 생성 IL 범위 | arity | binding power | 관찰한 배치 |
| --- | --- | --- | --- |
| MATRIX_TRANSPOSE, 3756–3932 | 1 | 160 | postfix, 표시 `T` |
| MATRIX_ROW, 3932–4172 | 2 | 160 | base와 추출 index, index의 숨은 ROW_CONTENT wrapper |
| MATRIX_COLUMN, 4172–4412 | 2 | 160 | base와 추출 index, index의 숨은 COLUMN_CONTENT wrapper |
| CONJUGATE, 11091–11263 | 1 | 130 | prefix/container, overline U+203E |
| FACTORIAL, 11263–11461 | 1 | 180 | postfix, `!` |
| ABSOLUTEVALUE, 12036–12186 | 1 | 190 | outfix |
| DETERMINANT, 12186–12336 | 1 | 190 | outfix |

Transpose의 child 0이 Row/Column이면 GROUPING으로 reparent한다(IL 3877–3919). Row와 Column의 base가 Transpose/Row/Column이면 같은 GROUPING을 추가한다(각 IL 4109–4159, 4349–4399). 따라서 위첨자 연산자를 겹쳐 저작할 때 괄호를 생략하여 다른 식처럼 보이게 하지 않는다. 독립 editor는 이 신규 연산자들의 직접 삽입에 동일한 grouping 조건을 적용한다.

원본 Row/Column은 hidden visual wrapper를 쓰지만 네이티브 식은 `apply(matrow|matcol, matrix-expression, index-expression)`의 두 operand다. 독립 PortableTree는 두 operand를 직접 저장하고 renderer가 해당 wrapper 표현을 담당한다. literal subscript나 원소 `indexer`로 바꾸지 않는다.

## Norm의 BAML·상수·시각 경로

설치 `ko/Ptc.Controls.EquationControl.resources.dll`, SHA `943a2278738739403e7e2517199d056a368ee3b83c9ddff14b274ecbda4e53fb`를 정적으로 읽었다. `Ptc.Controls.EquationControl.g.ko.resources` payload SHA는 `20b7b6f85f6dcdb212d7c811914fad21719339c0b12e6b84a3217ef2c41511c7`이고 `visualoperatordescriptortable.baml` SHA는 `17d8c47b27fea89b0d61cb68d6011a85ee2c6215c2bc6b9b53b3b8308f5ccd36`이다. WPF 객체나 markup을 실행하지 않고 resource/BAML record를 해석했다. 사용한 satellite는 일부 CLI metadata stream 경고가 있으므로 metadata 실행 가능성과 resource byte identity를 혼동하지 않는다.

`structural-array-resource.py`와 생성된 `structural-array-operator-baml.mjs`가 추출/record 해석을 재현한다. 다음을 `structural-array-operator-baml.json`에서 확인했다.

- BAML offset 11753의 CanonicalName은 type `Ptc.EquationEditor.Tree.MathNodeNames`의 `AbsoluteValue` member reference이고 DisplayName은 절대 값/행렬식, Keybinding은 `|`, FunctionalityKey는 ArithmeticOperatorsAbsoluteValueOrDeterminantCommand다.
- offset 12090의 CanonicalName은 같은 type의 `Determinant`이며 DisplayName은 Norm에 해당하는 번역, Keybinding은 Ctrl+Shift+OemPipe, FunctionalityKey는 VectorAndMatrixOperatorsNormCommand다.
- 같은 EquationEditor SHA의 CLI Constant table에서 field `0x0400052b` AbsoluteValue=`ABSOLUTEVALUE`, `0x0400052c` Determinant=`DETERMINANT`도 확인했다. 단순 member 이름 추정으로 mapping을 결정하지 않았다.
- `MathNodeToXmlTransform.a`, `0x06001167`, raw IL SHA `3930351561a894c121a700a2d9fa21cb2d5c8966dbba30f0550897f7c2136e89`는 이 두 descriptor를 각각의 XML format을 사용하는 operator writer에 연결한다. XML 이름이 없는 새 Norm 태그를 만들 근거는 없다.

그림도 descriptor의 `'||'` 문자열만 보고 추정하지 않았다. `VisualFactory.CreateAbsoluteValueNode` `0x06000699` → `VisualLinearOutfixNode.CreateAbsoluteValue`는 기존 단선 경로다. `CreateDeterminate` `0x0600069a` → `VisualLinearOutfixNode.CreateDeterminate` `0x06000895`, raw IL SHA `9cbccd757f36cb78c1e53489cca36815ce00642d6de796bc1331f4dc72d347f4`는 양쪽 fence factory에 `CreateDoubleVerticalLine`을 넣는다. 따라서 사용자 Norm 명령에 해당하는 새 `Determinant` AST는 이중선이고 `Absolute`는 단선이다.

Row/Column은 `VisualFactory.CreateMatrixRowNode/ColNode`, `0x06000688/689`가 공통 `VisualMatrixSuperscriptBinaryNode`를 만든다. content factory `0x0600068a/68b`는 각각 VerticalBrackets/HorizontalBrackets를 만든다. 실제 bracket 회전과 배치까지 추적한 정적 증거 및 독립 font layout 경계는 0070에 기록한다. 이름만 보고 일반 아래첨자로 그리지 않는다.

## 편집과 native bridge

새 전용 AST 여섯 개와 기존 Absolute로 원본의 일곱 명령을 연결했다. unary arity와 Row/Column의 두 operand를 PortableTree에서 검증하고 bridge에서 정확한 namespace·arity·operator attributes를 검사한다. native `<norm/>`, foreign namespace, 추가 속성·자식 또는 잘못된 index sequence를 받으면 raw 보존 경로로 돌릴 수 있도록 명시적으로 거절한다.

Postfix 삽입은 현재 operand 또는 명시적으로 선택한 subtree를 감싸고 비어 있지 않으면 caret를 새 mark 뒤로 둔다. 빈 수식에서는 operand placeholder 안에 남겨 값을 작성할 수 있다. Power의 exponent 안에 놓은 transpose를 전체 Power로 임의 승격하지 않는다. Row/Column은 기존 base를 첫 operand로 보존하고 새 index placeholder를 두 번째 operand로 연다. Conjugate와 Norm의 container, Row/Column index에서 그룹 종료가 가능하다.

새 unary wrapper의 경계에서 구조 Backspace는 operand를 보존하여 감싼 연산자만 벗길 수 있고 Undo로 정확한 metadata/tree를 복원한다. MatrixCells 범위 상태에서는 모든 신규 명령을 no-op으로 처리하여 선택된 여러 셀을 새 연산자 하나로 감싸지 않는다. 명령마다 한 번의 editor checkpoint를 사용한다.

여기서의 caret/wrapper 조작은 기존 독립 arena의 bounded 재구현이다. 원본 descriptor의 identity·arity·binding power·확인된 grouping 제약을 사용하지만 전체 operand 부분 선택, atom 중간의 분할 저작, hidden logical wrapper의 모든 navigator 상태를 복원한 것은 아니다. 내부 NodeId나 undo data 구조의 원본 일치를 주장하지 않는다.

## 검증과 계산 경계

editor helper `structural_operators.rs`의 6개 회귀는 descriptor arity/binding power, Norm과 |의 서로 다른 native identity, postfix caret/빈 operand, Row/Column operand 순서와 그룹 종료, Power 내부 scope, unary 삭제의 literal/label metadata 보존, 신규 명령의 MatrixCells no-op, 원본의 겹친 superscript grouping과 단일 Undo를 검사한다. editor 전체 49개가 통과했다.

compat의 별도 `tests/structural_operators.rs` 4개는 여섯 신규 operator와 기존 absval의 exact MathML 왕복·operand 순서·명시 label/contextual 보존, 잘못된 attributes/arity/namespace 및 가짜 norm 거절, 같은 이름의 사용자 함수 Call 유지, 두 명령의 다른 XML 저장을 확인한다. 계산 회귀와 파일명을 분리했으며 `structural_operator_calculation.rs`는 0071의 소유 범위다. editor+compat clippy `--all-targets -- -D warnings`도 통과했다. ignored 로그는 `structural-array-editor-all-tests.log`, `structural-array-bridge-tests.log`, `structural-array-clippy.log`에 있다.

이 문서의 UI/native mapping과 backend의 `op_det`/`op_det2` 중 정확한 type lowering은 별도 문제다. descriptor의 op_det RL 문자열만으로 차원 있는 값의 특정 native helper 호출을 확정하지 않는다. 현재 수치 지원·물리 단위와 함수 det/norm의 차이·미복원 lowering은 0071의 범위와 한계를 따른다. Vectorize, Cross Product, Percent는 이번 구조 연산자 묶음에 추가하지 않았다.
