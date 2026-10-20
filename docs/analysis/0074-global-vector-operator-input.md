# 전역 정의·벡터화·외적의 입력과 조판

2026-10-16. 원본 descriptor·저장·편집 계약은
[0073](0073-global-vector-operator-contract.md), 계산과 남은 경계는
[0075](0075-global-vector-operator-calculation.md)를 따른다. 이 단계의 수정 범위는
`formula.rs` 및 새 `formula/global_vector_operator_tests.rs`다. 원본 앱,
GUI·스크린샷·CLR·COM을 실행하지 않고 REA의 retained metadata/raw IL,
설치 도움말과 headless 테스트를 사용했다.

## 확인한 원본 표기와 키

| 명령 | AST와 native identity | 원본 표기·키 | binding power |
| --- | --- | --- | --- |
| Global Definition | Binary GlobalDefine, `<globalDefine>` | `≡` U+2261, Ctrl+Shift+Backtick(미국 배열의 `~` 키) | 10 |
| Vectorize | unary Vectorize, `apply(vectorize, expression)` | 식 전체 윗화살표 U+2192, Ctrl+Shift+Num6(미국 배열의 `^` 키) | 130 |
| Cross Product | Binary CrossProduct, `apply(crossProduct, left, right)` | `⨯` U+2A2F, Ctrl+8 | 120 |

표기는 이름만 보고 추정하지 않았다. Global Definition의 glyph는 원본
descriptor의 U+2261이며 Cross Product는 일반 곱셈의 점 또는 U+00D7 `×`와
다른 U+2A2F다. 새로운 연산자는 전용 AST로 저작하며 같은 이름의 Call을
자동으로 바꾸지 않는다. 일반 `*`의 기존 Multiply 동작도 바꾸지 않았다.
원본에 없는 plain `~`, 단독 숫자 키 또는 별도 키 alias를 추가하지 않았다.

Cross Product의 원본 Associativity=2는 enum Right다. 따라서 연속 외적의
기본 결합은 오른쪽이다. raw imported AST에서 왼쪽 자식이 다시 외적이면
보이는 괄호를 넣고, 오른쪽 외적에는 불필요한 괄호를 넣지 않는다.
외적 안의 낮은 우선순위 덧셈과 높은 우선순위 곱셈 안의 외적도 괄호로
분명하게 표시한다. source AST는 그대로 두며, 이 별도 결합 규칙을 기존
Multiply 입력 정책으로 확대하지 않았다.

## 윗화살표의 정적 배치 근거

`Ptc.Controls.EquationControl.dll` SHA는
`0b6fd580eb6cc183f17b5b5e0967226bbafc5e12efc6da809f44cd16416bf9d1`,
MVID는 `cf1d7cb5-c73f-4618-8e6b-b83959d13b98`, 원본 REA evidence ID는
`ev_389338a9845c7ea83dfd310922316cf92ca1637bdbac6602a01c4690e52b8349`다.
선택한 34개 method의 instruction, raw IL SHA 및 부모 identity를
ignored `artifacts/rea/global-vector-rendering-cil.json`에 보존했다. 이는
원본 호출 결과가 아니라 REA 위치를 사용한 supplemental static decode다.

- `VisualVectorizationOperatorNode..ctor`, `0x06000dff`는 별도의
  `HorizontalRightArrow`를 만든다.
- `MeasureStaticLayout`, `0x06000e0a`는 화살표 폭을 피연산자 폭과 마지막
  화살표 glyph 폭 중 큰 값으로 정하고 해당 크기로 arrow를 측정한다.
- `VectorizationLayoutManager.Arrange`, `0x06000551`은 arrow를 `(0,0)`,
  피연산자를 `((arrow_width - child_width)/2, 4 DIP)`에 놓는다. baseline은
  피연산자 baseline에 그 y offset을 더한 값이다.
- `ArrangeStaticLayout`, `0x06000e0b`는 이 두 rect를 실제 배치에 사용한다.
  `HorizontalRightArrow.OnRender`, `0x06000e19`는 오른쪽 arrow head와 늘어나는
  shaft를 그리는 별도 경로다.

독립 helper는 이 최소 폭, 중앙 정렬, 4 DIP offset 및 baseline 관계를
반영한다. STIX 대체 글꼴의 `→` advance를 최소 폭으로 사용하고, 세 개의
vector line으로 shaft와 오른쪽 head를 그린다. 원본 font/resource 또는
arrow 코드·바이트를 제품에 복사하지 않았다. 실제 glyph metrics와 선의
윤곽은 독립 구현이며 원본 pixel geometry 전체의 일치를 주장하지 않는다.

윗화살표는 피연산자 전체를 덮어 벡터화 범위를 보인다. 선택한 부분식만
벡터화하면 그 부분식만 감싸고, root를 선택하면 식 전체를 감싼다.
arrow primitive는 가짜 편집 노드를 만들지 않으며 source operand의 hit와
caret 위치는 실제 child offset을 따른다. 저장된 symbolic 결과의
`DisplayMath::OverArrow`도 같은 helper를 사용하되 결과 glyph는 편집 가능한
source 노드로 노출하지 않는다.

## 확정·결과·기존 입력 보존

Global Definition 입력은 기존 definition과 구분하여 전용 Core 명령을
호출한다. 이미 Define/GlobalDefine/Evaluate인 root를 임의로 다른 정의로
전환하는 toggle은 추가하지 않았다. 전역 정의의 `=`는 root 전체를 새 평가로
감싸지 않고 RHS 평가를 인식·추가한다. 빈 RHS에는 평가를 요청하지 않으며
이미 RHS 평가가 있으면 새 연산자나 Undo를 늘리지 않고 평가 명령만 전달한다.
이는 원본 `EquationModel.get_EvaluationRoot`, `0x060000f9`가
GLOBAL_DEFINITION의 child 1도 검사하는 분기로 직접 확인했다.

전역 정의의 오른쪽 배치에도 cached result를 전달하여 cold-open 결과가
정의 안에서 사라지지 않게 했다. source 단위 슬롯은 실제 NodeId를 유지한다.
단축키 입력 자체는 평가를 실행하지 않고 기존 draft/Enter parent 확정 경계를
유지한다. MatrixCells 범위에서는 새 수식 명령이 no-op이며 문서·Undo·dirty
response·계산 요청을 만들지 않는다.

원본 SpecTable의 header·단위·데이터 편집기는 모두
`AllowExpressionsOnly=true`다. `EquationModel.InsertOperator`의
`0x0600017b`, IL99–157은 DEFINITION과 GLOBAL_DEFINITION을 거절한다.
따라서 이 문맥의 `:` 및 Ctrl+Shift+Backtick도 무시한다. Core의 문맥 guard를
사용하고 실제 arena revision이 바뀐 경우에만 문서 변경을 반환하여 직접
text 입력과 위젯 경로 모두 dirty·Undo·평가 요청을 추가하지 않게 한다.

새 키는 정확한 modifier와 함께 처리한다. 기존 Ctrl+- literal subscript,
논리 명령, 일반 지수 `^`, `*` 곱셈이나 backend Copy를 다른 동작으로 추측하지
않는다. 앞 단계의 licensed egui-winit Copy predicate 수정도 유지했다.

## Headless 검증

새 9개 테스트는 다음을 검사한다.

- 정확한 세 단축키의 전용 AST, 원래 operand 보존, 한 Undo 및 평가 미요청.
- Global RHS의 빈 값 guard, 첫 평가·반복 평가의 차이, native MathML 왕복.
- 실제 `≡`/`⨯` glyph, 전체 범위 arrow와 head 방향, source hit 보존.
- 원본 arrow 최소 폭·피연산자 중앙 정렬·4 DIP y offset, readonly 측정의
  pixel snap 허용 범위, keyboard input 비소비.
- native parse 후 `i ⨯ j = k` 및 vectorized 2×2 matrix power의
  `[1,4;9,16]` 요소별 계산.
- 범위 선택 상태 no-op, 원본에 없는 unmodified shortcut 미추가.
- 외적의 오른쪽 결합과 imported raw AST의 시각 괄호, source tree 불변.
- 선택 범위 벡터화, named Call 구분, 전역 RHS cached 결과와 단위 hit,
  `OverArrow` 결과에 가짜 editable node가 생기지 않음.
- SpecTable 표현식 전용 문맥에서 두 정의 입력이 source·arena revision·
  Undo·dirty·계산 요청을 바꾸지 않음.

계산의 shape·단위·complex·global prepass 상세는 0075의 별도 계약과 회귀를
따른다. 이 문서의 위젯 검사는 원본 runtime 재실행이나 전체 navigator 상태의
동일성을 입증하지 않는다. Parent의 파일 저장·native export·재열기 및 양 OS
전체 검증은 최종 source snapshot의 integration 결과로 구분한다.

최종 좁은 검사에서 위 9개 테스트가 모두 통과했고,
`cargo clippy -p mathcraft-app --all-targets --no-deps --locked --offline -- -D warnings`도
통과했다. 그보다 앞선 전체 `formula::` 검사에서는 75개 통과·1개 ignored였다.
이후 arrow의 원본 4 DIP·최소 폭 보정과 표현식 전용 입력 guard를 넣고 위
9개를 다시 검사했으므로, 앞선 전체 수치를 최종 전체 검사로 간주하지 않는다.
