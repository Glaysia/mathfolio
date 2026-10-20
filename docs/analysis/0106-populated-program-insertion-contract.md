# 선택한 식에 프로그램 연산자 삽입 — 원본 계약

2026-10-19. Prime 12.0.0.1의 기존 REA metadata 및 원시 CIL을 재사용했다. 원본 GUI·스크린샷·CLR·COM·원본 프로세스 실행은 사용하지 않았다. 이 문서는 **Normal 텍스트 선택**, **Scope 선택**, **Ctrl+J implicit-parent 변환**을 구분한다. 같은 수식을 가리켜도 세 경로의 결과는 다르다. 조사의 실행 자료와 메서드 목록은 ignored `artifacts/rea/populated-program-insertion-contract-20261019.md` 및 `artifacts/rea/feedback/populated-*-cil.txt`에 보존한다.

## 신원과 범위

설치 파일 `Ptc.EquationEditor.dll` SHA-256은 `d419032c0e99a62eab2f278e571a338d8c4b9662655af8ab462d482a028c0223`, MVID는 `d8627dca-3258-470c-b4d1-c00bbe2f2441`이다. REA evidence는 `ev_419ab24312df266f2e58ff6b91d32e0261a9a2bba9105ba09ebecb7517b43fdf`다. 아래 토큰은 모두 이 E assembly 기준이다. retained `editor/equation-{members,il}.json`과 `context-labels-deep/equation-il.json`을 사용하고 `feedback/label-cycle-inspect.py`로 선택 메서드의 설치 IL을 재대조했다. 같은 숫자 토큰을 갖는 Controls assembly 메서드와 혼동하지 않는다.

프로그램 descriptor의 child 순서·BODY repair는 [0096](0096-program-control-authoring-contract.md), [0099](0099-conditional-branch-contract.md), [0103](0103-try-control-contract.md)를 따른다. RETURN은 Prefix(1), WHILE/FOR/IF/TRY는 Special(5)이다. primal은 각각 WHILE 1, FOR 2, IF 1, TRY 0이며 모두 BODY다. 방향 enum은 Left=0, Right=1, Indeterminate=4다.

## 입력 모드가 삽입 정책을 결정한다

| 원본 상태 | 원본 경로 / 확정된 동작 | 독립 구현에 적용할 경계 |
| --- | --- | --- |
| Normal, 값 atom 전체의 명시적 UTF16 `[0,len]` 선택 | `l.a 0088 → ax04d9 → ax04ee/04ed → bi05cc → a2` replacement | 기존 값을 버리고 새 빈 template로 교체한다. Scope 보존과 다르다. |
| Normal, 값 atom에서 범위 없는 Center | NavigatorState ctor1316이 EnterSide에 따라 collapsed text caret 0 또는 len을 만든다. | 전체 선택으로 간주하여 값을 지우면 안 된다. |
| Normal, collapsed text caret0 / len | ax04d9가 Left / Right 삽입으로 분기한다. | 전체 target을 condition/iterable에 감싸는 근거가 아니다. |
| Normal, 내부 text caret 또는 부분 text range | bk05d4가 앞·뒤 문자열을 분할하고 analysis handle 사이에 새 연산자를 삽입한 뒤 parse/repair한다. | implicit adjacency AST 없이 일반 곱셈으로 대체하지 않는다. |
| Normal, 값이 아닌 노드의 Center | ax04ed의 replacement pipeline. 같은 family, pair subelement, outfix, parenthesize 등 별도 분기가 있다. | 임의 compound wrapping의 근거가 아니다. |
| Scope, 완전한 단일 subtree | m.u/v0287/0288 → ax04e4/04e5 → bl05d9 | 아래 보존·방향 표가 확정된 양성 경로다. |
| Ctrl+J, keyword가 native implicit-parent의 첫 child | a8.bq059e가 별도 operand 보존 수행 | explicit multiply와 구별해야 한다. |

Normal entry `0088 IL0158..01c5`는 CarriesValue/HasCaretPositions 및 DistinctNodeEdges 조건 아래 SelectionStart/End를 전달한다. `ax04d9 IL000e..0024`의 전체 범위 분기와 collapsed 경계 분기는 별개다. `NavigatorState .ctor1316 IL0000..0049`는 Range 미지정 atom을 collapsed text caret로 바꾼다. 따라서 자체 editor의 `Center + value_range=None`을 명시적 전체 선택으로 소급 해석하지 않는다.

## Scope: 선택한 내용을 primal BODY로 보존

`bl05d9 IL0345..0392`는 Special이고 primal이 있으며 그 slot이 PH일 때 selected segment parse 결과를 **그 primal에** 넣는다. WHILE 조건0이나 FOR iterable1에 넣지 않는다. 기존 Program을 BODY로 쓸 때 descriptor repair로 BODY가 되므로 statement children을 보존하며 불필요한 Program 중첩을 만들지 않는다.

| 새 연산자 | Scope Left | Scope Right | 선택 식의 위치 / 최종 cursor |
| --- | --- | --- | --- |
| WHILE | 허용 | 허용 | BODY child1. 양방향 preorder 첫 PH, 보통 조건 child0 |
| FOR | 허용 | 허용 | BODY child2. 양방향 preorder 첫 PH, 보통 binder child0 |
| IF | 허용 | 허용 | BODY child1. 양방향 preorder 첫 PH, 보통 test child0 |
| TRY | 허용 | 허용 | attempt BODY child0. Left는 preorder 첫 PH; Right는 역순 첫 PH, 보통 handler의 마지막 PH |
| RETURN | 허용 | 거부 | child0에 보존. Left 성공 시 Return Center에서 한 번 Right 이동한 상태 |

`bl05dd`는 Special+primal을 effective Outfix(4)로 분류한다. RETURN은 effective Prefix(1)이다. `bl05da →05db/05dc`는 Outfix 양쪽을 허용하지만 Prefix의 Right에는 AllowCaretOnRight를 요구한다. RETURN은 Left만 true이고 Right는 descriptor 기본 false다. Scope Right에 단지 unary니까 같은 child0이라고 추정해 허용하지 않는다.

scope 방향은 `m.ax0275 → ak.v03ce → al.d040b`에서 anchor의 Opposite로 얻는다. anchor 저장 `ak03ff IL0058..005e`가 최초 방향의 Opposite이므로 둘이 상쇄된다. 따라서 이 표의 Left/Right는 자체 `select_subtree_from(expr, Direction::Left/Right)` 및 `scope_direction()`과 직접 대응한다. 텍스트에서 Space로 Scope를 시작할 때 `ak03e9`는 caret가 UTF16 midpoint 이전이면 Left, 나머지는 Right를 선택한다.

Scope entry는 IgnoreOperandsOnOvertype=true일 때 다른 경계 삽입 경로를 사용하지만 여기 다섯 descriptor는 false다. getterfa4는 field ah를 읽고 ctorf74는 ah를 설정하지 않는다. DFTbbb의 해당 setter는 IL1a6b/1c13/334d에만 있으며 프로그램 descriptor 범위40b0..4dfb에는 없다.

### RETURN의 첫 navigable state

Return Scope Left의 끝은 `05d9 IL0664..068e`: `new bq(Return)` 다음 `bq.b(Right)`다. 무조건 leaf로 내려가거나 operand Right에 놓는 방식이 아니다. `bq.f060c`와 fixity별 helper가 다음 첫 상태를 정의한다.

| operand fixity | 첫 상태 생성 규칙 |
| --- | --- |
| None(0) atom | 자신의 Center. ctor1316이 값 atom의 collapsed UTF16 caret0을 만든다. |
| Prefix(1), 060e | 자신의 Left를 항상 먼저 등록한다. |
| Postfix(2), 060f / Infix(3), 060d | IsContainerSlot(0) 또는 AllowCaretLeft면 자신의 Left, 아니면 child0의 첫 상태 |
| Outfix(4), 0610 | PreFenceNavigatable이면 Left(AllowLeft일 때), 이어 Center(part0); 아니면 children의 첫 상태 |
| Special(5), 0611 | AllowLeft면 자신의 Left. 그렇지 않으면 part0 position이0인 경우 자신의 Center(part0), 그 다음 child0 |

PreFenceNavigatable getterfe2는 field a7, ctorf74 IL00f6..00f8은 a7=true다. 따라서 Group/Matrix 등 알려진 Outfix에 AllowLeft가 있으면 자체 Left다. Add/Multiply는 ContainerSlot0/AllowLeft가 없어 child0으로 내려간다. `73*V`의 Return Scope Left는 **73의 UTF16 text caret0**이 된다. 이 알고리즘은 검증된 descriptor 속성으로 적용하고 알려지지 않은 노드까지 first_leaf로 일반화하지 않는다.

## 명시적 전체 atom 선택은 replacement

`bi05cc`는 replacement pipeline `a2`를 사용한다. iterator054f는 pair/same descriptor/compatible replacement/leaf 등 순차 handler를 돌린다. compatibility0541은 arity·fixity·leaf status를 비교한다. Number 또는 Identifier의 arity0/None/non-leaf와 새 Special/Prefix는 compatible preserving replacement가 아니다. 최종 `a2.c053e IL0000..0032`는 old ChildCount==0 및 **new ChildCount>0 또는 new IsLeafOperator**이면 처리한다.

`053e IL0101..0125`는 기존 analysis handle의 Node를 새 operator로 교체하고 parse/repair한다. 선택 값은 새 primal로 복사하지 않는다. 고립된 완전한 값 atom 또는 evaluation operand의 완전한 값 atom을 명시적으로 전체 선택한 경우의 양성 경로다. 주변 generic container의 모든 reparsing 결과까지 검증한 주장은 아니다.

같은 양성 범위는 Program/BODY의 **직접 statement인 atom**에도 적용된다. `MathNode.get_ContainerRoot 0x06000c56 IL0008..001c`는 parent.IsContainerSlot(Index)가 true이면 자신을 반환한다. `IsContainerSlot 0x06000ca9 IL0000..0012`는 AllChildSlotsAreContainerSlots일 때 유효 child index를 container로 취급하며 PROGRAM/BODY는 이 속성이 true다. 따라서 한 statement atom의 전체 값 replacement는 다른 statement와 adjacency parse를 공유하지 않는다. statement 안 Add의 일부 atom까지 확장한 근거는 아니다.

| 새 연산자 | replacement 후 cursor |
| --- | --- |
| RETURN | child0의 Left-entry 상태(`053e IL01b5..01d3`), 새 PH면 PH Center |
| WHILE / FOR / IF / TRY | subtree의 preorder 첫 PH(`0324..038f`), 각각 조건 / binder / test / attempt PH |
| BREAK / CONTINUE | 0-arity leaf도 위 guard 허용. FixityNone switch의 `0395..03ad`는 `bq.c(Right,newnode)` → 해당 leaf의 Right 상태 |

Break/Continue의 이 경로는 loop/program ancestry를 새로 제한하지 않는다. 기존 operator 서브트리 전체, 숫자의 부분 선택, suffix 부분 선택, 범위 없는 Center까지 넓힐 근거는 아니다.

## Ctrl+J의 native implicit-parent 보존

`a8.bq059e`는 Normal mode의 HasNoLabel identifier가 descriptor OperatorText와 일치하는 변환이다. 이름/라벨/lookup의 기존 계약은 0096·0099·0103을 따른다. `IL007f..00bb`의 보존 조건은 다음 세 조건이다: parent.IsImplicit이고 keyword.IndexInParent==0, 새 descriptor가 pair-nary subelement가 아니며, 새 descriptor에 primal이 있거나 Prefix이다.

조건이 참이면 implicit parent child1을 보존하고 **그 parent 전체를 PH로 교체**한다(`00c4..00d7`). 거짓이면 keyword identifier만 PH로 교체한다. 새 operator를 Right로 삽입한 다음 primal 또는 prefix child0에 보존 operand를 넣고 repair한다(`0133..019b`). 반환 cursor가 교체한 이전 PH를 가리켜 더 이상 새 operator 내부가 아니면 보존한 slot의 Left-entry로 복구한다(`01a6..01c5`).

따라서 native implicit `return x / while x / for x / if x / try x`의 보존 위치는 RETURN0 / WHILE BODY1 / FOR BODY2 / IF BODY1 / TRY attempt BODY0이다. 입력 단어만 비슷한 explicit `keyword*x`는 이 계약이 아니다. 현 AST가 explicit multiply와 implicit adjacency를 구분하지 못하면 이 경로는 unsupported로 남겨야 한다. primal만 보고 임의 곱셈의 한쪽을 이동하면 원본 분기를 위반한다.

## 확인하지 않은 범위

Normal Left/Right populated placement는 bg05c4에서 analysis handles와 generic parser를 사용한다. text split/부분 range/compound center/parent precedence/implicit adjacency의 모든 최종 AST는 이번 조사에서 복원하지 않았다. 이미 확정한 Scope·전체 값 replacement와 섞지 않는다. scope의 arbitrary linear segment, 행렬 다중 셀, 다중 subtree 역시 단일 완전 subtree의 양성 경로와 구분한다. 이 문서는 원본 일반 삽입 전체 구현 완료를 주장하지 않는다.

원본 명령 수준은 `l.a0088`의 한 top-level 삽입 및 scope model command다. 독립 구현은 후보 트리와 portable budget을 먼저 확인한 뒤 한 history checkpoint로 반영할 수 있다. 실패·지원하지 않는 선택에는 checkpoint를 만들지 않는 것이 기존 원본형 명령 계약에 부합하며, native 내부 parsing의 중간 handle/repair를 별도 사용자 Undo로 노출할 근거는 없다.

## 재현용 원시 IL SHA-256

아래는 method body의 원시 IL SHA다. 표의 값은 retained REA와 설치 파일 선택 검사에 대응하며 전체 assembly SHA와 구분한다.

| E token | 메서드 | IL SHA-256 |
| --- | --- | --- |
| `0x06000088` | `l::a` | `d2e82ba51ec7275cbe5e6b911eaf06401ff6a634851661ae6462fb31e78c74e6` |
| `0x06000287` | `m::u` | `125510e868ab657e41add426c5e3d0fe564e0f17f5a1eece960dd6e105ff7771` |
| `0x06000288` | `m::v` | `4c8ec5fb879a5907fb2113d557d8539e431ae1a6772bcdc395224dab348a76b4` |
| `0x06000275` | `m::ax` | `95767587842daea0b98cde759eb70a695abbc1fa7012e4b2c9e0ee89967b65b7` |
| `0x060003ce` | `ak::v` | `b6c3b92ccf03fda9a78ed32f052ebf80eb1b351353879dcb3f1d80c83fd27d27` |
| `0x0600040b` | `al::d` | `a9c0c22518dbf84a45dea1e938deefd1e0fa20025411f9d42bdd54c00bf0e4c6` |
| `0x060003ff` | `ak::a` | `36b2ce3946cb574bd91dfab1934ea94875ac4c39c69a8b958518a03a9190d7b3` |
| `0x060003e9` | `ak::a` | `c34c9a116d93d7d4426aebc8c5b824ed5d29ef9c9bd22231e866a76d5b5b03af` |
| `0x060004d9` | `ax::a` | `0f370e2df8a25d14436c655db42b7b8b4ffde38843443bb088e6a5074245b978` |
| `0x060004ed` | `ax::a` | `b0590a34c1d70661c8f97674720dc95c8d3cf72f0ca60210b4a679cef52fcdf6` |
| `0x060004ee` | `ax::a` | `b161e07ba4890e8f795dcb2535753b80b9a1f798257b2e92fa4298c0f83e6484` |
| `0x060004e4` | `ax::a` | `e4e531d860df8312985f30104a08f8b0deedd8fa7be3a55ca78aa8e5622c7bed` |
| `0x060004e5` | `ax::a` | `9cbd4d7ed3f0a5082346b21fe4dd5f48969b70a1fa842b82b5b4ce4266455bd5` |
| `0x060005c4` | `bg::bq` | `25d960678b62705f5d8537a636f5d5997588222b4a40f9e3af9b9c8ab0dd39e5` |
| `0x060005c6` | `bg::a` | `3bc6ac5566ace12d0ee48c1c7b8795bbaffb44737548ba64e75b270f85ef6a69` |
| `0x060005cc` | `bi::bq` | `0af1fdaceb0f0d11c1c25a9a7823d340777aadbc4100755d5f98c396ff2ea707` |
| `0x060005d4` | `bk::bq` | `87158c560d81ce05a28fc3e890b330777f49848e1ca74e655f86ffccf16a7adc` |
| `0x060005d9` | `bl::bq` | `f148d85443e73a00d367e1c62e214985432ced2e469f475b955181ba75379dbe` |
| `0x060005da` | `bl::a` | `68aba3c90bbc2edaee3eab69752741a541cface1c895e3f518c01258343e6b80` |
| `0x060005db` | `bl::c` | `9d234508502b6b3a7ecf2314ede48d1cb0f63bb786ae04157bf1cd273f70be77` |
| `0x060005dc` | `bl::b` | `f882ad5784cb428f5953aa92fe53c0569457ebf6ae41fb1b8f21fff0fcadb459` |
| `0x060005dd` | `bl::a` | `70d4a59d9839e3a5a2bb6c8611b13c90859f7b257d9c2f576b2cbd0d1ff17e31` |
| `0x0600053e` | `a2::c` | `5efe79dc7a2840ea0604dce868f545e85f757ee60a5fb9a6859247dc1804ad9f` |
| `0x06000541` | `a2::a` | `96b127d7ddccc88d7f6ae85c3263dc0012e8705abe09300e8bc4a7d6c3a2c3c4` |
| `0x0600054f` | `e::b` | `51c854c86b09bb912ef7e674b7a39061a788dd87dd404aed08d226a61e39dbad` |
| `0x0600059e` | `a8::bq` | `dbaea23c9588a86b123e3b7d67e5b4c86467ce28b38ef1873559dd97a2bb9f0c` |
| `0x06000601` | `bq::b` | `92526cd90824e55bbed4663f7257b82fa5743c0349052161c48131aa2fc49a82` |
| `0x0600060c` | `bq::f` | `42ead008bbbf5ddbf30f54951ecfcd994c966bc6d13c0ac530b3222947eec087` |
| `0x0600060d` | `bq::e` | `9c0e87635657600ad6ce47cdbcf86351e5eb7e38032f08c473f3971ebb2e6b72` |
| `0x0600060e` | `bq::d` | `b6376032d55ea5236dcbcf20ccffe63af1007cdfb621fc12919b11ac7f060b7b` |
| `0x0600060f` | `bq::c` | `11b0ac08a2f346c6e450b962526993361615f39785f68306b9fe74a3976f7f37` |
| `0x06000610` | `bq::b` | `ec1aa15375ce83262f1480211c51130661aa34a82175c48ba7a296220832ac49` |
| `0x06000611` | `bq::a` | `66566f7338caa5db7796fecad83dd8bf87ff44b4d8b84075b44d1f867b7c7485` |
| `0x06000f74` | `Ptc.EquationEditor.Tree.MathNodeDescriptor::.ctor` | `66382848d127e9465892c3a847a444bc5dca0765d5cc0eb39d41e16f1b441f5b` |
| `0x06000fa4` | `Ptc.EquationEditor.Tree.MathNodeDescriptor::get_IgnoreOperandsOnOvertype` | `e8d431a7f44df1aaaa322083ce63a849e3d335e12ef28a1f86009f4ca656c8e5` |
| `0x06000fe2` | `Ptc.EquationEditor.Tree.MathNodeDescriptor::get_PreFenceNavigatable` | `6e74001d108709ce3857eee016a7b669d9da8ceb57a03236c4a8cb30e8fd5511` |
| `0x06001316` | `Ptc.EquationEditor.Tree.Navigation.NavigatorState::.ctor` | `b3ae8dd499a4709ceb7741175397c7d410435a69e3a52f8e9eaee2e872bada50` |
| `0x06000c56` | `Ptc.EquationEditor.Tree.MathNode::get_ContainerRoot` | `972c9d3809b54161a88ed249607f37b22dd36a55cfec4b1d32eed4a1733afffd` |
| `0x06000ca9` | `Ptc.EquationEditor.Tree.MathNode::IsContainerSlot` | `b0c823e4f923a7c1a62b3040bc80b346a649e1628dcded85cd749625cfe12f89` |
