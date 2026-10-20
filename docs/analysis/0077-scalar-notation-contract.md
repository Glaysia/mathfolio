# 백분율·도·극형식 연산자 저작 계약

2026-10-16. EA-033/034/035의 원본 descriptor, 설치 도움말, BAML 키와 native writer를 정적으로 대조하고 editor/bridge를 연결했다. 원본 앱·GUI·스크린샷·CLR·COM·원본 계산 실행은 사용하지 않았다. 원본 메서드 구현을 독립 Rust 소스로 복사하지 않았다.

## 이름과 표현

| 원본 descriptor | 독립 AST / API | arity·fixity·binding power | native XML | 원본 키·glyph |
| --- | --- | --- | --- | --- |
| PERCENT | `NodeKind::Percent` / `insert_percent()` | 1, suffix, 180 | `apply(percent,x)` | `%` U+0025 |
| DEGREE | `NodeKind::Degree` / `insert_degree()` | 1, suffix, 180 | `apply(degree_op,x)` | `°` U+00B0; 도움말은 Alt+0176 입력도 안내 |
| POLAR | `BinaryOp::Polar` / `insert_polar()` | 2, infix, 120, Left | `apply(polar,magnitude,angle)` | Ctrl+Shift+P / `∠` U+2220 |

EA-034의 기존 목록명 “도·분·초 연산자”는 확인된 원본 descriptor보다 범위를 넓혀 부른 것이다. 이번 원본은 unary Degree다. 세 칸의 DMS 입력 노드를 만들지 않는다. 별도 `DMS` affine 변환 함수와 구분하며 그 함수를 이번 편집 연산자 구현에 합산하지 않는다.

입력 Percent는 operand에 0.01을 곱하는 연산자다. 결과 서식의 Percent는 결과 표시를 100배 하고 % 기호로 표현하는 별도 기능이므로 AST나 native XML을 공유한다고 해석하지 않는다. Degree는 operand에 π/180을 곱하며 극형식 angle을 degree로 입력할 때도 이 노드를 angle에 붙인다. Polar의 angle 속성을 임의로 `degrees`로 저장하지 않는다. 단위·벡터·복소수의 수치 계약은 별도 계산 조사와 상위 통합 문서를 따른다.

## 정적 증거와 범위

원본 `Ptc.EquationEditor.dll` SHA `d419032c0e99a62eab2f278e571a338d8c4b9662655af8ab462d482a028c0223`, retained REA evidence `ev_419ab24312df266f2e58ff6b91d32e0261a9a2bba9105ba09ebecb7517b43fdf`를 재사용했다. `DefaultDescriptorTable.f` token `0x06000bbb`, raw IL SHA `90ff4afc15ec4d75068d5d2c0eab08be91548ff38974f8de5b54e02a25a6376a`에서 다음을 관찰했다.

- POLAR IL 3058–3226: XMLFormat `polar`, RL op_polar, symbol 8736, child count 2, infix 3, associativity 1(Left), precedence 120. 이 descriptor block에는 별도의 ProhibitChild 호출이 없다.
- PERCENT IL 11461–11646: symbol 37, RL op_percent, suffix 2, child count 1, AllowCaretOnRight=true, precedence 180, XMLFormat `percent`. child 0이 EXPONENTIATION 또는 NTHROOT이면 GROUPING으로 감싸는 ProhibitChild/ReparentChild rule이 있다.
- DEGREE IL 11651–11792: symbol 176, RL은 operand와 `mc_deg`의 op_mult, XMLFormat `degree_op`, suffix 2, child count 1, AllowCaretOnRight=true, precedence 180. 이 block에 Percent와 같은 child grouping rule은 없다.

`MathNodeToXmlTransform.a` `0x06001167`, raw IL SHA `3930351561a894c121a700a2d9fa21cb2d5c8966dbba30f0550897f7c2136e89`의 dispatch는 PERCENT(IL1702/1717), POLAR(2310/2325), DEGREE(2462/2477)를 모두 `TransformOperatorNode`에 연결한다. 해당 writer `0x06001177`, raw IL SHA `edd18405d02dfd8f5cc817f0fe06287202b091b678d03d13bd1cd2cb078f7ba7`는 `apply` 아래에 descriptor.XMLFormat을 가진 operator와 operand들을 만든다. 따라서 degree를 단위 id로 바꾸거나 polar를 함수 이름으로 저장하지 않는다.

키와 표시 이름은 정적으로 추출한 `ko/Ptc.Controls.EquationControl.resources.dll` SHA `943a2278738739403e7e2517199d056a368ee3b83c9ddff14b274ecbda4e53fb`, inert `visualoperatordescriptortable.baml` SHA `17d8c47b27fea89b0d61cb68d6011a85ee2c6215c2bc6b9b53b3b8308f5ccd36`에서 대조했다. offset 9975의 CanonicalName 참조 22는 Polar, offset 13500 참조 36은 Percent, offset 14186 참조 39는 Degree다. 각각 Ctrl+Shift+P, %, ° 키를 가진다. 기존 [0069](0069-structural-array-operators-contract.md)의 resource 추출 절차를 재사용했으며 XAML/BAML 객체를 실행하지 않았다.

설치 `Help/en/PTC_Mathcad_Help/`의 파일과 SHA는 다음과 같다.

| 도움말 | SHA-256 | 명시된 계약 |
| --- | --- | --- |
| `percent_operator.html` | `c01114f3dec5e3103a7ac9bfe7c0961e5dd59ce9d781c50760a56f8cc01f33bf` | scalar/vector/matrix/complex 각 원소에 0.01 적용. 결과 Percent 서식과 별도. |
| `degree_operator.html` | `4a71d14a212502884330b972d65ee0a6e3568a598f0fa2fa529de2ae177765dd` | scalar/vector/matrix/complex 각 원소에 π/180 적용. degree→radian 변환. |
| `polar_operator.html` | `3c029de9676b94fb613bca48777cef7816cb5f69580cfb655124242a97a4e339` | 실수 magnitude와 radian angle. degree 입력에는 Degree 연산자 사용. `-x∠y`의 magnitude 부호, 뒤의 덧셈과 angle 곱셈의 구분. |

ignored `artifacts/rea/scalar-notation-editor-evidence.py`는 원본 artifact SHA를 다시 검사한 뒤 기존 retained raw IL/descriptor와 BAML의 필요한 범위·도움말 해시를 `scalar-notation-editor-evidence.json`으로 모은다. `scalar-notation-descriptors.json`은 세 descriptor 범위를 보존한다. 이는 정적 관찰이며 실제 입력 동작이나 계산 결과를 원본 앱에서 관측했다는 주장은 아니다.

## 독립 편집·저장 구현

`editor/src/scalar_notation.rs`의 세 API는 위 원본 identity로 구조 노드를 만든다. Percent/Degree는 기존 postfix editor helper를 재사용하여 현재 operand를 보존하고 채워진 식에서는 새 기호 오른쪽에 caret을 놓으며 빈 식에서는 operand placeholder에 caret을 둔다. Percent의 Power/NthRoot child grouping rule을 추가하고 Degree에 그 rule을 임의로 적용하지 않았다. 구조 삭제는 operand를 보존하며 하나의 삽입/삭제 명령을 각각 Undo로 복구한다.

Polar는 확인된 precedence 120과 Left 결합을 사용한다. 독립 linear-entry에서 `-x∠y`는 negative magnitude를 유지하고, 뒤의 `*z`는 angle에, 뒤의 `+z`는 Polar 밖에 결합한다. 이는 복원한 binding power를 사용하는 독립 편집 정책이다. 원본의 모든 caret/부분 선택 mode가 같다는 주장은 아니다.

CrossProduct도 precedence 120이지만 Right다. 실제 Rust `prepare_infix()`는 같은 operator인지 비교하지 않고 **들어오는** operator의 결합 방향을 사용한다. headless 실제 API 입력에서 `a∠b⨯c`는 `Polar(a,Cross(b,c))`, `a⨯b∠c`는 `Polar(Cross(a,b),c)`가 되며 Undo로 직전 식을 복원한다. 일반 소스 표현의 혼합 결합과 원본의 모든 selection/descriptor 교체 경로는 구분한다.

후속 정적 감사에서 원본 `a3.a` `0x0600055e`, raw IL SHA `f8b10b6317b97fff459d28f262dc8bce14012e97d6fd22cc008cfdf5f6d43092`는 child/parent가 둘 다 infix이고 precedence가 같으면 parent의 associativity와 child 위치를 검사한다(IL70–144). parent Left의 오른쪽 child 또는 parent Right의 왼쪽 child는 group으로 감싼다(IL263–276). 이는 Polar의 오른쪽 Cross도 포함한다. 확인된 caller `a2.m` `0x06000533`와 `a2.k` `0x06000535`는 descriptor 교체 후 이전 container child들을 보호하는 경로다. 이 사실을 raw renderer 전반 또는 평범한 신규 키 입력에 그대로 적용한다고 확정하지 않는다. 해당 group-repair와 전체 infix insertion/reduction processor의 연결은 미확정이며, 모든 같은 precedence 식에 일괄 괄호를 추가하는 근거로 사용하지 않았다. 재사용한 동일 REA artifact의 13개 메서드 범위는 ignored `scalar-notation-associativity-cil.json`과 SHA 검증 decoder에 보존했다.

세 명령은 MatrixCells 범위 선택에서 문서/history를 바꾸지 않는다. expression-only 표 셀에서는 일반 식 연산자로 사용할 수 있다. 정의 명령을 다시 허용하거나 계산을 자동 실행하지 않는다. 기존 explicit label, contextual flag, literal subscript와 operand node identity를 유지한다.

`compat/src/editor_bridge.rs`는 정확한 세 operator 이름과 arity를 읽고 쓴다. 알 수 없는 operator attribute나 가짜 `degree`/`dms` operator를 조용히 변환하지 않고 raw 보존 경계로 오류를 반환한다. Percent 결과 서식이나 임의의 polar angle mode attribute를 추가하지 않는다.

## 검증과 한계

새 editor 회귀 7개가 descriptor/arity/precedence, postfix metadata·caret·구조 삭제·Undo, Percent의 grouping, Degree의 grouping 비추가, 빈 operand, Polar Left 결합과 negation/addition/multiplication 조합, Polar/Cross의 실제 혼합 API 입력과 Undo, MatrixCells guard와 expression-only 사용을 검사한다. 기존 editor 전체 63개와 마지막 신규 binding 회귀 2개가 통과했다(총 65개 구성). compat의 `tests/scalar_notation_bridge.rs` 3개는 native roundtrip·정확한 arity·가짜 속성/태그 거절·label/literal subscript 보존을 검사하며 모두 통과했다. editor+compat all-targets clippy `-D warnings`도 통과했다.

상위 formula 키 입력·원본 글꼴 조판·계산/저장/export 검증은 별도 통합 결과를 따른다. 이번 범위는 DMS/FIF/hhmmss 함수, 결과 서식의 Percent/Polar 구현 전체, 원본의 모든 nested-array 및 symbolic 해석을 완성한 것으로 보고하지 않는다.
