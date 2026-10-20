# Implicit adjacency / SCALE 원본 계약

2026-10-19. Prime 12.0.0.1의 기존 REA 정적 CIL과 동봉 도움말·Math50 schema를 조사했다. 원본 프로세스, GUI, 스크린샷, CLR, COM을 실행하지 않았다. [0106](0106-populated-program-insertion-contract.md)의 Scope/Normal 전체 값 선택과 이번 adjacency 경로를 구분한다. 상세 조사 ledger는 ignored `artifacts/rea/implicit-adjacency-contract-20261019.md`다.

## 신원

E=`Ptc.EquationEditor.dll`, SHA-256 `d419032c0e99a62eab2f278e571a338d8c4b9662655af8ab462d482a028c0223`, MVID `d8627dca-3258-470c-b4d1-c00bbe2f2441`, REA `ev_419ab24312df266f2e58ff6b91d32e0261a9a2bba9105ba09ebecb7517b43fdf`. retained `editor/equation-{members,il}.json` 및 `context-labels-deep/equation-il.json`을 재사용했다. 아래 token은 모두 E 기준이다. focused 원시 IL은 설치 파일 및 retained REA body SHA와 대조한다.

## IMPLICIT는 독립 descriptor 이름이 아니다

`MathNode.get_IsImplicit 0x06000ced`는 descriptor의 bool getterfb9에 위임한다. getter/setterfb9/fba는 field `0x040004ba a0`를 읽고 쓴다. DFT 전체의 setter cross-reference에서 true 설정은 **SCALE(0bbb IL0bbe), OP_PLACEHOLDER(IL35d4) 두 곳뿐**이다. 이 설치의 기본 descriptor table에 문자열 이름 `IMPLICIT`라는 별도 노드는 없다. 기존 BinaryScale을 두고 모든 Multiply에 임의 provenance를 덧붙여야 한다는 결론은 이 근거에서 나오지 않는다.

| 원본 descriptor | arity / fixity / precedence / associativity raw | IsImplicit / Temporal | container·용도 |
| --- | --- | --- | --- |
| SCALE | 2 / Infix3 / 142 / 0 | true / 기본 false | 명시 container slot 없음, AllChildSlotsAreContainerSlots 기본 false. 입력 식의 실제 scaling 연산자 |
| OP_PLACEHOLDER | 2 / Infix3 / 200 / 0 | true / true | 명시 container slot 없음. NoVisual. parse 중 adjacency를 표현하는 임시 연산자 |
| RSCALE | 2 / Infix3 / 21 / 0 | **false** / 기본 false | readonly slot0; RESULT/RPLACEHOLDER 기본 자식; parent EVALUATION 제한. 결과와 표시 단위를 위한 내부 wrapper |
| MULTIPLICATION | explicit multiplication descriptor | 기본 false | SCALE로 소급 분류하지 않는다. 동일 수치 결과가 provenance 근거가 아니다. |

SCALE 선언 `0bbb IL0b14..0bed`는 XMLFormat `scale`, S-expression `@SCALE`, RL `(op_scale:0x{1:x8} {2} {3})`다. OP_PLACEHOLDER `3571..372c`는 XMLFormat `unknownOp`, S-expression `@OP_PLACEHOLDER`다. RSCALE `1d90..1f14`는 `@RSCALE`이며 SCALE의 별칭이 아니다.

SCALE의 OperatorSymbol은 **8728 = U+2218 `∘`**(`0b38..0b44`), SymbolMargin=-1, ShowOnlyWhenFocused=true(`0bb3..0bb6`)다. descriptor의 OperatorVisualRepresentation `**`는 화면에 별표 두 개를 그리는 근거가 아니다. RSCALE도 해당 symbol 및 focused-only 설정을 갖지만 별도의 result-side wrapper다.

## 명시 Scale와 자동 Scale는 같은 연산자다

동봉 `scaling_operator.html`은 Ctrl+Shift+U 수동 입력과 `5y` 자동 삽입을 명시한다. `xy`는 별도 scaling 연산자를 넣지 않으면 **하나의 변수**다. 오른쪽 y가 함수면 y(x), 아니면 x*y다. operand는 scalar/array/function/unit를 허용한다. Scale의 의미를 무조건 일반 multiplication으로 낮추면 함수 RHS 동작을 잃는다. 활성 math region에서만 symbol이 보이며 결과와 단위 사이 scaling은 삭제할 수 없다는 도움말 설명은 RSCALE의 readonly/result-side 구성과도 구별해서 읽어야 한다.

이 문구는 함수 RHS 적용을 뒷받침하지만 정적 DFT에는 `op_scale` 이름만 있다. 실제 native engine이 label만으로 함수를 판별한다거나 undefined identifier도 함수로 추정한다는 구현은 찾지 못했다. 독립 엔진은 정의된 함수 값 등 기존 resolution 결과로 이 기능을 구현할 수 있으나 새 label 추정 규칙을 원본 사실로 주장하지 않는다.

## Analysis list가 adjacency를 만드는 지점

원본 parser는 완성 AST에 곱셈 노드를 무조건 추가하는 방식이 아니라 `MathNodeAnalysisHandle` linked list의 effective fixity를 검사한다. handler iterator `c.b 0x0600049c`는 앞선 merge/normalization 및 placeholder 처리 뒤 `IL0195..01a1`에서 `at.a048c("OP_PLACEHOLDER")`를 등록한다. closure0499는 실제 `at.a048d`를 호출한다.

`048d IL0000..0013`의 조건은 다음과 같다.

| 검사 | helper | 삽입을 막는 상태 |
| --- | --- | --- |
| 직전 handle | as.b047e→047f | null, effective Prefix1 또는 Infix3 |
| 현재 handle | as.a0482→0483 | null, effective Postfix2 또는 Infix3 |

두 검사 모두 false면 새 OP_PLACEHOLDER를 current 앞에 AddBefore한다(`048d IL0021..0054`). 따라서 두 완성된 atom 또는 적절한 prefix/postfix/outfix 경계 사이에서 임시 implicit 연산자가 생긴다. fixity 검사는 실제 descriptor fixity가 아니라 **analysis handle의 EffectiveFixity**다. Special의 analysis flattening까지 무시하고 모든 Special을 atom으로 취급하는 근거는 아니다.

앞선 `at.b0493`는 인접한 비명시 값 둘을 결합해 valid한 값이 되면 merge할 수 있다. formatted identifier와 label도 보존한다. 그래서 `x`와 `y`의 문자 입력은 `xy` 한 identifier로 합쳐질 수 있으며, 이미 독립된 operand 두 개라는 조건과 문자 한 번 입력을 구분한다. numeric+identifier의 문자열 `2m`은 NUMERIC 또는 IDENTIFIER 전체 값 regex를 만족하지 않아 그 merge의 양성 결과가 아니며 이후 adjacency 경로가 필요하다. 이 형식 판단은 동봉 도움말의 `5y` 예와 일치한다.

### 임시 OP_PLACEHOLDER가 최종 연산자로 복구되는 규칙

DFT `35d9..3708`은 네 조합을 APPLY로 변환하도록 ProhibitChildren/TransformTo를 선언한다. RHS는 모두 GROUPING이다.

| left child | right child | repair 결과 |
| --- | --- | --- |
| PLACEHOLDER | GROUPING | APPLY |
| IDENTIFIER | GROUPING | APPLY |
| PRIME | GROUPING | APPLY |
| MATRIX_INDEX | GROUPING | APPLY |
| 위 조합이 아닌 나머지 | 해당 expression | 마지막 RestrictChild(empty permitted descriptor set) + TransformTo **SCALE** (`370d..3724`) |

이 규칙은 함수처럼 보이는 syntax를 APPLY로 고르는 editor repair다. RHS identifier가 실제 함수 값인지 검사해 SCALE를 APPLY로 바꾸는 규칙은 아니다. 함수 RHS의 Scale 의미는 engine의 `op_scale`/도움말 계약에서 별도로 다룬다. `2`와 `m` 두 atom, `keyword`와 `x` 두 atom은 RHS GROUPING이 아니므로 SCALE로 복구되는 양성 형태다. `f` 다음 GROUPING은 APPLY 예외다. `2` 다음 GROUPING은 네 left descriptor에 해당하지 않아 같은 APPLY 예외를 적용할 근거가 없다.

## Native XML은 SCALE를 보존한다

E writer 등록1167 `IL0674..0694`는 SCALE를 TransformScaleNode1181에 보낸다. 1181은 TransformOperatorNode1177에 위임한다. 1177 `IL00a8..00d1`은 `<apply>` 아래 descriptor XMLFormat operator를 쓰고 두 자식을 순서대로 변환한다.

```xml
<ml:apply xmlns:ml="http://schemas.mathsoft.com/math50">
  <ml:scale/>
  <ml:real>2</ml:real>
  <ml:id labels="VARIABLE">m</ml:id>
</ml:apply>
```

generic operator reader1267은 first child operator 이름으로 XMLFormat descriptor를 찾고1266으로 operands를 읽는다(`0481..04d0`). 따라서 `<scale/>`가 SCALE이고 `<mult/>`가 MULTIPLICATION인 구분을 보존할 수 있다. import된 mult를 `2m`처럼 보인다는 이유로 Scale로 바꿀 근거는 없다. OP_PLACEHOLDER의 writer 등록은1177을 사용하므로 미복구 draft는 `<apply><unknownOp/>…</apply>` 형태를 가질 수 있지만, 정상 authoring repair는 APPLY 또는 SCALE로 안정화한다. 임시 node를 portable 완성 AST로 영구 보존해야 한다는 결론은 아니다.

RSCALE writer117f는 `<result>`를 만들고 children을 변환한다. Evaluation writer1189는 별도로 unitOverride/result 처리를 한다. RSCALE라는 native `<rscale/>`를 발명하거나 일반 `<scale/>`와 혼동하지 않는다.

## Ctrl+J에 적용되는 것은 실제 IsImplicit parent

[0106](0106-populated-program-insertion-contract.md)의 a8.bq059e `IL007f..00bb`는 parent.IsImplicit 및 keyword가 parent child0인지를 검사한다. 안정화된 **SCALE(keyword,rhs)**는 이 조건을 만족한다. 명시적으로 Ctrl+Shift+U로 만든 Scale도 같은 descriptor이므로 별도 자동입력 provenance를 요구하지 않는다. MULTIPLICATION(keyword,rhs)는 만족하지 않는다.

| 실제 parent 형태 | Ctrl+J 보존 결과 |
| --- | --- |
| Scale(return,rhs) | implicit parent 전체를 교체하고 rhs를 Return child0에 보존 |
| Scale(while,rhs) | rhs를 While BODY1에 보존 |
| Scale(for,rhs) | rhs를 For BODY2에 보존 |
| Scale(if,rhs) | rhs를 If BODY1에 보존 |
| Scale(try,rhs) | rhs를 Try attempt BODY0에 보존 |
| Multiply(keyword,rhs) | 위 implicit-parent 보존 분기의 근거 없음 |
| 단일 identifier `returnx` 등 | exact keyword 일치 아님. 임의 분할하지 않는다. |

059e `IL0171..0196`은 기존 primal PH를 rhs로 교체하고 **navigator 없이 repair**한다. `01a6..01ca`는 이전 cursor가 새 operator descendant인지 검사해, 아니라면 repaired primal의 Left-entry로 복구한다. While/For/If의 header PH는 살아 있으면 유지된다. Return의 PH와 Try의 초기 attempt PH는 교체되므로 Return은 보존 operand 첫 native state, **Try는 repaired attempt BODY의 Left**다. Try의 handler PH로 보내는 것은 이 분기의 원본 계약이 아니다.

## 남은 범위와 구현에 적용할 한계

Normal 문자 입력 l.x008b는 값 내부 text selection을 ax04de→bo05e3/05e4로, 노드 옆의 operand 삽입은 ax04d5로 보낸다. 값 editing은 regex 검증과 실패 처리가 있고, parser merge/temporary operator 단계는 별도로 있다. `5y` 자동 Scale의 사용자 계약 및 인접 operand→SCALE repair는 확정했지만 **모든 한 문자 입력의 실패 후 retry, 중간/부분 UTF16 range 분할, literal subscript depth, Special 주변 handle flattening의 최종 AST**는 아직 복원하지 않았다. partial text를 지운 다음 임의 Multiply를 끼워 넣는 식으로 구현하지 않는다.

확정된 구현 후보는 기존 BinaryScale의 native/descriptor 의미 유지, 자동 numeric→name 입력의 Scale 생성, 수동 Scale, 실제 Scale(keyword,rhs)의 Ctrl+J 보존이다. 현 소스의 함수 Scale 미지원은 계산 공백이며 multiplication과 동일하다고 축소할 수 없다. 이번 조사는 제품 Rust 코드·spec을 수정하지 않았다.

## 동봉 자료 SHA-256

| 자료 | SHA-256 |
| --- | --- |
| scaling_operator.html | `d9a0b7b9c1e0849486ff948f79405c2c5801137e41b5629ddef8da94de2d6657` |
| Math50.xsd | `a10afb4dd442a4f5dd77b6f1df903c1d2e5b6f4d6a701aae38b0cefee21f6ca3` |

## 원시 IL SHA-256

전체 assembly SHA와 별개인 각 method body 원시 IL SHA다.

| E token | 메서드 | IL SHA-256 |
| --- | --- | --- |
| `0x06000bbb` | `Ptc.EquationEditor.Tree.DefaultDescriptorTable::f` | `90ff4afc15ec4d75068d5d2c0eab08be91548ff38974f8de5b54e02a25a6376a` |
| `0x06000ced` | `Ptc.EquationEditor.Tree.MathNode::get_IsImplicit` | `ca1d69af2efe441161e043ce70ff955d4d5a80588ac3dcab4c80fc64e6273243` |
| `0x06000fb9` | `Ptc.EquationEditor.Tree.MathNodeDescriptor::get_IsImplicit` | `105f8a0373f22ec95b7e3cf4cdae408f4851928c11b67b589557c645feb76196` |
| `0x06000fba` | `Ptc.EquationEditor.Tree.MathNodeDescriptor::set_IsImplicit` | `f1a94f4fc4e061a177603a3983b5d46416bc455baa64f9d76d4a245e94235ab2` |
| `0x06000f74` | `Ptc.EquationEditor.Tree.MathNodeDescriptor::.ctor` | `66382848d127e9465892c3a847a444bc5dca0765d5cc0eb39d41e16f1b441f5b` |
| `0x0600049c` | `c::b` | `f8503bb4aa0b49d324ea47d333cc90706be3954919a5bec833e1afaec200860d` |
| `0x0600048c` | `at::a` | `b1a1a4f9d60ff71f40d7d9501ea25d24065c675e2b36dbd1336e294fe6d8fd0f` |
| `0x06000499` | `b::a` | `a1f63bd6f5582e1753f765c5832a04e6f6c75ddcb6ea7cc695e869a118d5781e` |
| `0x0600048d` | `at::a` | `e9b7e7559e2ad286df08fab72711304e214a43efd12bd1fcc3297e1ca98d63e9` |
| `0x0600047e` | `as::b` | `b0f0a0d7502d643eb9e60ad8b555e2cd586caf7b603de72e31592b54f31e01a8` |
| `0x0600047f` | `as::b` | `078900818f9e8740ee42b8f7b89c6dfbc9441ca436f8d0def3ec5d6beec3ada7` |
| `0x06000482` | `as::a` | `66ecf07938d449885204f0a3fd1fb05a67ed4ae6ec0fe6b4e3d130347b52996b` |
| `0x06000483` | `as::a` | `fbc1096146a6ecce2f498d75504852021dd54bfb4a140deac66d82b3e5987a28` |
| `0x06000493` | `at::b` | `15926f51041e45c0e74368556914ed4c20d4a380f309a4fd109d98a91fe4c3e3` |
| `0x0600048e` | `at::f` | `4bf649b8f171d3c1ebd71000e2cbd0023f7b7b8bb978a04b60e19f5480b6d3af` |
| `0x0600048f` | `at::e` | `45fdb781405c3d81e37f8b8f45d2637091dfaff2936f32b702559e530933e921` |
| `0x060005e3` | `bo::bq` | `1ea16e8a8023940ae9be624707e906dd65667bbe39640d73535e5ca5afa527f2` |
| `0x060005e4` | `bo::a` | `fee94c9ce630e0264515fd5e63e90f1f3bde25939a301a721ec9f29a777c581b` |
| `0x060004de` | `ax::b` | `a08d41a4f57446715cac38eccd04267c477230543345b170558e0f2e913d861c` |
| `0x060004d5` | `ax::a` | `b5be9228eb51f7c4dbf360a8645890c3121e92443d928f30afce7f29f6df3f3e` |
| `0x0600008b` | `l::x` | `6176ec1930ed1449e482d8f89d363d47f08b02959a440e770a8ebf043e025aa3` |
| `0x06001167` | `Ptc.EquationEditor.Tree.Transform.MathNodeToXmlTransform::a` | `3930351561a894c121a700a2d9fa21cb2d5c8966dbba30f0550897f7c2136e89` |
| `0x06001181` | `Ptc.EquationEditor.Tree.Transform.MathNodeToXmlTransform::TransformScaleNode` | `5b095c2fba66b2e94d6f4172a8dbcc875f54d1642e1d891b881bf178c09c6a78` |
| `0x06001177` | `Ptc.EquationEditor.Tree.Transform.MathNodeToXmlTransform::TransformOperatorNode` | `edd18405d02dfd8f5cc817f0fe06287202b091b678d03d13bd1cd2cb078f7ba7` |
| `0x0600117f` | `Ptc.EquationEditor.Tree.Transform.MathNodeToXmlTransform::TransformRScaleNode` | `cc3ef2d9e9ab51eff0e71a0927bb7e286d00c6d63dc456cd9823db53ebbb0adb` |
| `0x06001189` | `Ptc.EquationEditor.Tree.Transform.MathNodeToXmlTransform::TransformEvaluation` | `b8db2199ab05cba3768159ffccb8025e5541111745137017deb7a050a46d54d3` |
| `0x06001267` | `Ptc.EquationEditor.Tree.Transform.XmlNodeToMathNodeTransform::b` | `9096d817b19d266a14486cac265d224441640986d88a8f1eabaf5ca720cba5ae` |
| `0x06001266` | `Ptc.EquationEditor.Tree.Transform.XmlNodeToMathNodeTransform::a` | `d7620b2e6a974ccdbe6a0fba306d981963a7abfc04b4f6eadc5751a7e9e3c31e` |
| `0x0600123f` | `Ptc.EquationEditor.Tree.Transform.XmlNodeToMathNodeTransform::a` | `92534579059fb7a57e28456d409547440bbe30028b4999040af022839df19d59` |
| `0x0600059e` | `a8::bq` | `dbaea23c9588a86b123e3b7d67e5b4c86467ce28b38ef1873559dd97a2bb9f0c` |
| `0x060005fa` | `bq::c` | `7e3832027219873ebc87952c74ad2077d0c01a3ec7a8d927019f6d0c772accab` |
| `0x06000601` | `bq::b` | `92526cd90824e55bbed4663f7257b82fa5743c0349052161c48131aa2fc49a82` |
