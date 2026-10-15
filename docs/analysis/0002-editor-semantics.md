# Mathcad Prime 12 수식 편집기: 구조와 동작 명세 조사

조사일: 2026-10-15. 대상: 이 PC에 설치된 Prime **12.0.0.1**. 이 문서는 구조적 편집기를 새로 구현하기 위한 관찰 기록이다. 타입 이름의 존재를 기능 재현이나 실행 검증으로 간주하지 않는다.

## 결과와 범위

Prime의 수식 편집은 문자열 입력란 위에 수식을 그리는 방식으로 설명되지 않는다. 실제 CIL에는 **수식 트리, 별도 시각 트리, 노드와 문자 사이를 오가는 캐럿, 범위 선택, 덮어쓰기, 연산자별 삽입 제약과 트리 변환, 명령 단위 undo**가 연결되어 있다. 현 프로토타입에 기능 버튼을 더하는 것으로는 이 동작에 도달하지 못한다.

이번에 확인한 것은 정적 바이트·메타데이터·CIL과 제품에 동봉된 설명서다. 원본 UI에서 키 입력 후 커서와 트리가 어떻게 변했는지를 실행 관찰한 증거는 이 보고서에 없다. 특히 수식 범위 확장 순서와 선택 후 삽입의 모든 조합은 미해결이다.

## 증거 식별

원본은 수정하거나 CLR에 로드하지 않았다. REA 6.5 CLI `inspect-managed-members`의 완전한 JSON을 보관했다. 명령은 두 어셈블리를 각각 한 번 전수 조사하고, `Ptc.Core.dll`은 방향 타입만 선택했다.

| 약칭 | 원본·REA 식별 | 조사량 |
|---|---|---|
| E | `Ptc.EquationEditor.dll`, SHA-256 `d419032c0e99a62eab2f278e571a338d8c4b9662655af8ab462d482a028c0223`, MVID `d8627dca-3258-470c-b4d1-c00bbe2f2441`, `ev_419ab24312df266f2e58ff6b91d32e0261a9a2bba9105ba09ebecb7517b43fdf` | 492 types, 4,909 methods, CIL body 3,797개 |
| C | `Ptc.Controls.EquationControl.dll`, SHA-256 `0b6fd580eb6cc183f17b5b5e0967226bbafc5e12efc6da809f44cd16416bf9d1`, MVID `cf1d7cb5-c73f-4618-8e6b-b83959d13b98`, `ev_389338a9845c7ea83dfd310922316cf92ca1637bdbac6602a01c4690e52b8349` | 500 types, 5,552 methods, CIL body 5,290개 |
| D | `Ptc.Core.dll`, `ev_2c6cdb7524e4d39ddcdbac9f66b877be23f828bb4c8900cd7539cf0799bc88ca` | `Ptc.Direction`, `Ptc.DirectionOps` 선택. `Ptc.Text.FormattedString`은 불일치했으며 실제 타입은 E의 `Ptc.EquationEditor.Tree.FormattedString`이다. |
| K | `Doc/en-US/keyboard_shortcuts.pdf`, SHA-256 `db4cdf1f913cfad20075b3f6354085183a22bde588900709c12ca06c065b3a4f` | 제품 동봉 12.0.0.0 설명서, 전 21쪽 텍스트 추출. 바이너리 패치 버전과 다르므로 단축키 실행 확인은 별도 필요. |

원시 증거는 `artifacts/rea/editor/`에 있다. `equation-members.json`, `control-members.json`, `core-direction-members.json`은 REA Evidence 그대로다. `equation-il.json`, `control-il.json`은 **보조 디코더** dnfile 0.18.0 / dncil 1.0.2로 해당 바이트를 읽은 결과다. 모든 보조 CIL 메서드에 원본 REA body hash·normalized hash·metadata token·file offset을 연결했다. C# 원본 복구나 실제 실행 결과가 아니다. 상세 버전과 원본 해시는 `manifest.json`에 기록한다.

아래 `E 0x060... / IL_xxxx` 표기는 해당 어셈블리의 빌드에만 유효한 메서드 토큰과 CIL 본문 시작 기준 오프셋이다. 보조 디코더는 분기·switch 목적지를 헤더 길이를 뺀 같은 좌표로 정규화했다. REA 메타데이터 coverage는 두 파일 모두 complete/issues 없음이지만, 이를 전체 제품 동작의 분석 완료로 읽으면 안 된다. 여러 private 타입이 `a`, `b`처럼 중복 난독화 이름을 가지므로 **이름만으로 호출 대상을 특정하지 않고 토큰을 사용한다.**

## 1. 연산자 설명 테이블의 실제 초기화

**관찰:** E `DefaultDescriptorTable.f`, 토큰 `0x06000bbb`는 `MathNodeDescriptor` 94개를 생성하고 속성 setter와 삽입 제약을 설정한다. 단순 타입 목록을 넘어서 생성자 인자와 실제 초기화 값을 확인했다. 추출기는 `descriptor-assignments.json`에 직접 literal setter만 기록한다. helper 호출, 복사 생성자, 기본값은 이 파일에서 추정하지 않는다.

| 노드 | 자식 수 | 결합 | 우선순위 | XML 표기 | S-expression 표기 |
|---|---:|---|---:|---|---|
| `DEFINITION` | 2 | Right | 10 | `define` | `:=` |
| `GLOBAL_DEFINITION` | 2 | Right | 10 | `globalDefine` | `@GLOBAL_DEF` |
| `EVALUATION` | 2 | Left | 20 | `eval` | `=` |
| `ADDITION` | 2 | None | 100 | `plus` | `+` |
| `SUBTRACTION` | 2 | Left | 100 | `minus` | `-` |
| `MULTIPLICATION` | 2 | None | 140 | `mult` | `*` |
| `INLINE_DIVISION` | 2 | Left | 140 | 직접 setter 미확인 | `÷` |
| `DIVISION` | 2 | Left | **143** | `div` | `/` |
| `NEGATION` | 1 | Prefix, 결합 직접 setter 없음 | 150 | `neg` | `@NEG` |
| `EXPONENTIATION` | 2 | Right | 160 | `pow` | `^` |
| `MATRIX_INDEX` | 2 | Left | 170 | `indexer` | `@INDEX` |
| `GROUPING` | 1 | Outfix, 결합 직접 setter 없음 | 299 | `parens` | `@PARENS` |

결합 열의 None은 enum 값 0의 실제 이름이다. 임의로 Left로 바꾸지 않았다. enum 값은 원본 Constant 테이블을 보조 파서로 확인했다: `Fixity None/Prefix/Suffix/Infix/Outfix/Special = 0/1/2/3/4/5`, `Associativity None/Left/Right = 0/1/2`, `LineSpan None/MonoLine/SplitLine/VariantLine/Container = 0/1/2/3/4`. 근거: `enum-constants.txt`, E의 해당 field token. 이는 런타임 객체 반사가 아니다.

근거 좌표 예: 덧셈 생성 `IL_0592`, 곱셈 `IL_070e`, inline division `IL_09c8`, fraction `IL_0a6f`, 지수 `IL_0ca4`, index `IL_0d7d`, 정의 `IL_18ee`, 평가 `IL_1f96`, 부호 `IL_24aa`, 괄호 `IL_2e1a`.

**구현상 의미(추론):** fraction과 inline division을 같은 연산 결과로 계산하더라도 편집 트리와 배치 정보에서는 구분해야 한다. 괄호, 줄바꿈 연산자, placeholder를 지운 계산용 AST만으로 편집 상태를 보존할 수 없다. 위 우선순위 숫자는 편집 설명 테이블의 값이며, 숫자만 복사해 텍스트 Pratt parser를 만들었다고 Prime 삽입 동작이 재현되는 것은 아니다.

MathNodeNames의 Constant 테이블에는 일반 계산 노드 외에도 `OP_PLACEHOLDER`, `PLACEHOLDER`, `RPLACEHOLDER`, `RESULT`, `EMPTY_FUNCTION_NAME`, `COLLAPSED_MATRIX`, `SCALE`, `RSCALE`, `SYMBOLIC_KEYWORD_STACK`, `MATRIX_ROW_CONTENT`, `MATRIX_COL_CONTENT`, 줄바꿈 연산자, 프로그램 제어 노드가 구별되어 있다. 전체 선언 값은 `enum-constants.txt`에 보관했다.

## 2. 삽입 제약은 트리 변환 규칙이다

**관찰:** 지수 descriptor는 다음을 실제로 초기화한다.

- `ChildCount = 2`, `NavigablePartCount = 1`, navigable position `[1]`, container slot `[1]`.
- `AllowCaretOnRight = true`, `ShowOnlyWhenFocused = true`.
- 자식 slot 0에 `EXPONENTIATION` 또는 `DIVISION`이 오면 `ProhibitChild(Rule(...), ReparentChild("GROUPING", 0, false))`를 등록한다.

좌표: E `0x06000bbb`, `IL_0ccf..0d6b`. 이는 지수 연산의 밑과 지수 부분에 서로 다른 편집 규칙을 선언한 직접 증거다. **예를 들어 어떤 입력 순서에서 괄호가 보이는지는 아직 실행으로 확정하지 않았다.** 등록된 repair 객체의 존재를 모든 편집 경로에서 실행되었다는 증거로 쓰지 않는다.

Matrix index는 slot `[1]`을 container로 지정하고, slot 0의 `ARGUMENT_SEPARATOR`를 제한하며, slot `[1]`의 자손에 자동 identifier labeling 제한을 건다. 좌표: E `0x06000bbb`, `IL_0de1..0e50`.

Placeholder는 precedence `Int32.MaxValue`, `IsScopable = false`, 좌우 caret 허용이며, `EMPTY_FUNCTION_NAME`에는 부모를 함께 제거하는 플래그들이 설정된다. E `0x06000bbb`, `IL_0006..0066`, `IL_00e2..012b`. 삭제가 문자열 한 글자 삭제로 끝나지 않는 이유다.

정의·전역 정의는 `IgnoreOperandsOnOvertype = true`; 평가는 `IsEvaluation = true` 및 삭제 시 자식으로 대체하는 설정을 가진다. 정확한 전후 트리는 각 선택 상태를 원본에서 재생하여 확인해야 한다.

## 3. 캐럿은 문자 인덱스 하나가 아니다

**관찰:** `NavigatorState`에는 다음 실제 필드가 있다: `Current`, `CaretSide`, `NodeSide`, `OperatorPartIndex`, `EnterSide`, `ValueRange`, `CaretInsertionType` (E fields `0x040006ca..0x040006d0`). E `0x06001316` 생성자는 이 정보를 함께 받는다. 선택 시작과 끝은 별도 getter로 노출된다.

`CaretInsertionTypes`는 Normal=0, Subscript=1이다. `ToggleCaretInsertionType` (E `0x06001323`)는 이 둘을 전환하고 현재 노드·방향·연산자 파트·값 범위를 새 NavigatorState에 보존한다.

`DefaultCaretInsertionType` (E `0x06001328`)은 `IdentifierNodeInfo` 여부를 검사한다. 비 identifier는 Normal을 반환한다. identifier에서는 현재 위치에 이웃한 문자의 **FormattedString subscript depth**를 조사한다. 보조 메서드 E `0x06001329`, `IL_0036..0049`는 조사할 문자 위치를 계산하고 `FormattedString.GetSubscriptDepthAtPosition`의 0/비0에 따라 Normal/Subscript를 반환한다. 경계 범위를 벗어나면 예외 분기도 있다.

**구현상 의미(추론):** identifier의 문자 아래첨자와 배열 인덱스는 서로 다른 데이터다. `a`의 일부 글자를 아래첨자로 쓰는 것과 `a[i]`로 원소를 고르는 것은 구별해야 한다. 이동할 때 아래첨자 입력 상태를 주변 글자에서 복원해야 한다. plain `String + byte_offset` 또는 `TextEdit` 선택 범위만으로는 이 상태를 표현하지 못한다.

수평 이동은 E `EquationModel.MoveLeft/MoveRight` (`0x06000153/155`) → mode 객체의 가상 메서드 `b.z/y` (`0x0600020e/20f`) → 방향별 `b.k`로 간다. 현재 mode 구현 세 개(`TypeDef 0x02000014`, `0x02000020`, `0x02000022`)가 이를 재정의한다. 현재 구현 하나 `l.k` (`0x06000076`)는 다음을 실제 분기한다.

- 수평 방향, 현재 노드의 `CarriesValue`, `HasCaretPositions`, 현재 caret side.
- 문자 내부의 위치와 value length를 비교해 한 칸 이동하거나 노드 바깥 side로 전환.
- identifier면 이동 후 `DefaultCaretInsertionType`을 다시 계산.

이는 노드 간 이동과 노드 내부 문자 이동이 함께 존재한다는 CIL 증거다. mode의 사용자 친화적 이름과 모든 up/down 전이는 아직 해석하지 못했다. 부모 방향 타입은 D에서 Left=0, Right=1, Up/Above=2, Down/Below=3, Indeterminate=4로 확인했다 (`core-direction-constants.json`).

## 4. 키 입력은 상태별 명령으로 해석된다

**관찰:** C `GestureBinder.Bind` (`0x06000495`)는 실제로 다음 바인딩을 추가한다.

| 텍스트 입력 | 실행 대상으로 연결되는 메서드/노드 |
|---|---|
| Space | `VisualMathRegionNode.FireExpandScope` |
| `)` | `VisualMathRegionNode.FireExitGroupRight` |
| `[` | insert `OVERLOADED_BRACKET` |
| `"` | insert string command |

나머지 연산자 입력은 `VisualOperatorDescriptorTable.Default`를 순회하며 `Keybinding`과 `MultiKeyStrokesBinding`을 등록한다 (`IL_0058..00b3`). 따라서 키 배치를 단순 ASCII 연산자 map 하나로 표현했다고 동일하다고 말할 수 없다.

C `GestureManager.Execute` (`0x060004a8/4a9`)는 현재 frame의 gesture 중 `Matches`인 첫 항목을 실행해 true를 반환하고, 없으면 parent manager의 Execute로 넘긴다. `GestureBinder.FireInsertOperatorWrapper` (`0x0600049c`)에는 기능 enable 상태와 command `CanExecute` 검사도 있다. 기능 enable 검사는 원본 제품의 동작 경로 확인일 뿐, 이 프로젝트에 제품 라이선스 제어를 재현하라는 요구가 아니다.

명령 `EquationControl.Keypress(string)` (C `0x06000116`)는 문자열의 문자들을 차례로 `VisualMathRegionNode.AutomateKeypress`에 보낸다. `CreateWithTextInput` (C `0x0600009f`)도 같은 경로를 사용한다. 따라서 문서 편집 API의 입력 동작은 parser에 완성된 문자열을 한 번 전달하는 것과 같다고 가정하면 안 된다.

K에 문서화된 기본 키: Home/End는 현재 expression 시작/끝, Ctrl+Left/Right는 선택 expression 또는 단어 경계, 좌우 화살표는 placeholder 이동, Ctrl+-는 문자 subscript, Ctrl+G는 직전 Latin 문자의 Greek 전환, Ctrl+Q는 identifier label 전환. `/`는 fraction, Ctrl+/는 inline division, `^`는 exponent, `(`는 짝 괄호를 삽입한다. 설명서는 국가별 키보드 차이도 명시한다. 이 항목들은 **문서화 사실**이며 이번 조사에서 모든 키를 실행 검증한 것은 아니다.

## 5. `=`는 단순 evaluation 토큰 삽입이 아니다

E `EquationModel.ProcessEqualsKey` (`0x060001a0`)의 CIL은 다음 순서다.

1. `UndoStackDepth`를 읽고 `InsertOperator("LOCAL_RANGE_VAR_DEF")`를 먼저 시도한다.
2. 이 시도 후 undo 깊이가 증가했다면 바로 반환한다.
3. 현재 `EvaluationRoot`의 descriptor가 `EVALUATION`인지 계산한다.
4. expression-only 상태가 아니고 기존 evaluation도 아니면 undo 명령 `Insert Evaluation`을 통해 evaluation 삽입을 시도한다.
5. 삽입 여부와 기존 evaluation 여부에 따라 `RequestEvaluation` delegate를 호출한다.

좌표: E `0x060001a0` / `IL_0000..008a`. 실제 evaluation 삽입 callback은 E `0x060001ef`이며 `DefaultDescriptorTable["EVALUATION"]`를 mode에 전달한다. 따라서 원본의 `=`는 범위 변수 정의 컨텍스트, 기존 평가 노드, undo 명령 성공 여부에 따라 달라지는 입력이다. 함수 이름만 보고 이렇게 추측한 것이 아니라 literal·분기·실제 호출을 읽었다. 원본 UI 사례별 전후 문서는 추가해야 한다.

`InsertOperator` (E `0x0600017b`)도 단순 생성자가 아니다. descriptor lookup/후보 검색, 표현식 전용 컨텍스트에서 definition/global-definition 차단, `STEPRANGE` 사전 처리, symbolic-evaluation 전용 경로, 명령 객체를 통한 실제 삽입을 수행한다. 두 mode의 삽입 구현(E `0x06000287`, `0x060002bd`)은 이전 연산자 치환 상태와 multi-key 실패 상태를 확인하며, matrix cell typeover에는 별도 실패 처리가 있다. raw CIL은 `mode-overrides-il.txt`에 보관했다.

## 6. 분수의 실제 배치 계산

**관찰:** C `VisualFractionOperatorNode.MeasureStaticLayout` (`0x060006f6`)은 두 자식을 무한 가용 크기로 측정하고 다음 값을 계산한다. 아래 수식은 읽은 CIL의 의미를 분석자가 다시 표기한 것이다.

```text
W = max(numerator.width, denominator.width) + 2 * fraction_overhang
H = numerator.height + denominator.height + 2 * vertical_pad
text_baseline = numerator.height + vertical_pad - font.baseline_adjustment
```

`ArrangeStaticLayout` (`0x060006f7`)은 분자와 분모를 가로 중앙에 정렬한다. 분자 y=0, 분모 y=`numerator.height + 2 * vertical_pad`, 분수선 y=`numerator.height + vertical_pad - bar.height / 2`로 배치한다.

`get_EffectiveBaselineY` (`0x060006f2`)는 denominator의 effective baseline에 `DenominatorY()`를 더한다. 텍스트 baseline, effective baseline, script baseline이 구분된다. `FractionOverhang`과 `FractionBarVerticalPad`는 dependency property에서 읽으므로 전역 상수라고 단정하지 않는다. 테마·글꼴·배율별 실제 값은 추가 조사 대상이다.

`GetNodeAtPoint`, measure/arrange, matrix 전용 layout manager, exponent·root·integral·derivative 전용 manager가 따로 존재한다. **타입 목록만으로 이들의 세부 배치가 복구된 것은 아니다.** 이번에 식과 호출까지 읽은 범위는 위 fraction 경로와 caret 상태다.

## 재구현에 필요한 작업과 검증 기준

아래는 원본 코드의 복제가 아니라 관찰한 기능을 충족하기 위한 구현 제안이다. 현재 Mathcraft가 충족했다는 뜻이 아니다.

| 작업 | 필요한 상태/자료 | 원본과 비교할 시나리오 |
|---|---|---|
| 손실 없는 편집 트리 | 불완전 노드, placeholder, 괄호, fraction/inline division, 문자 subscript, node ID | 빈 분수·빈 지수·미완성 함수 저장 후 다시 열기 |
| 계산 AST와 편집 트리 분리 | 계산 의미와 눈에 보이는 구조 양쪽 보존 | 같은 결과인 `/`와 inline division의 입력/직렬화 차이 |
| 구조적 caret | node/side/operator part/value range/insertion mode | 숫자·identifier 내부, 분자↔분모, 지수 안↔밖 이동 |
| expression scope | 노드 범위와 시각적 highlight | `a+b*c`에서 Space를 연속 입력한 선택 범위 순서 |
| 선택·typeover | 선택 노드·제외 operands·matrix cell 집합 | 분수 일부, matrix 사각 선택 후 연산자/문자 입력 |
| 삽입 규칙 | child/container slot, context constraints, repair operation | 지수의 밑에 분수/지수를 삽입한 괄호와 caret 위치 |
| 입력 상태기계 | multi-key operators와 context command routing | `<`→`=`, `>`→`=`, 범위 변수를 포함한 `=` |
| 측정·배치·hit test | font metrics, text/effective/script baseline, ink bounds | 중첩 분수·지수·행렬을 100/125/150%에서 클릭해 같은 노드 선택 |
| undo | 트리·caret·scope·결과 요청을 함께 복구하는 command | `=`의 컨텍스트 동작 후 undo 한 번의 전후 상태 |
| identifier label | 단위/변수/상수 분류와 문자 스타일 분리 | 같은 표기의 단위·사용자 변수·literal subscript 구별 |
| 직렬화 | 구조적 편집 상태와 결과 표현 보존 | 가져오기→편집→저장→재열기 후 AST와 위치 비교 |

다음 원본 관찰은 **빈 문서에서 입력 시퀀스 하나씩 → 각 단계의 화면/노드 XML/캐럿/undo 상태 기록**으로 진행해야 한다. 최소 시퀀스: `a+b*c`와 반복 Space, `1/2`, `(1+2)/3`, `x^2` 좌우 이동, identifier subscript 전환, matrix index와 문자 subscript 비교, `=`를 일반 수식·이미 평가된 수식·합/곱 범위 변수에서 각각 입력. 여기서 차이가 남으면 숫자 계산 결과가 같더라도 해당 항목은 미완료다.

## 남은 불확실성과 재사용 방법

- 정적 제약과 repair 등록은 확인했지만, 입력 순서별 실제 트리 변환과 세 개 mode의 모든 상태 전이는 확인하지 않았다.
- 다국어/한글 IME 조합, Ctrl shortcut의 물리키/문자키 차이는 미검증이다. C `EquationControl.OnGotKeyboardFocus`는 `ImeHandler.HandleImeFocus`를 호출하지만 이 사실만으로 한글 입력 지원이 입증되지 않는다.
- 테마·글꼴별 수치, pixel 단위 geometry, 모든 calculus/program layout은 미해결이다.
- symbolic engine, 수치 오차, 단위/함수 전체 범위는 이 보고서의 담당 범위를 벗어난다.
- 보조 도구로 원본을 실행하지 않았으며 결과를 제품 parity 점수로 환산하지 않았다.

새 분석은 `manifest.json`의 digest와 MVID가 같은지 확인한 뒤 기존 Evidence와 token을 재사용한다. `decode_editor.py`는 분석할 바이트만 읽어 보조 IL을 생성한다. `focus_editor.py`, `extract_descriptor.py`, `inspect_modes.py`, `layout_inspect.py`는 저장된 Evidence에서 관심 부분을 추출한다. generated evidence는 Git에서 제외된 `artifacts/`에 두고, 독립 구현 명세와 검증 결과만 프로젝트 문서에 유지한다.

## 기계 판독 계약과 최소 구조 편집 코어

[mathcad12-editor-contract.json](../spec/mathcad12-editor-contract.json)에 94개 literal descriptor의 초기화를 원본 Evidence와 연결했다. 직접 scalar 대입 815개와 배열 대입 145개를 기록했으며, helper·변환·생성자에서 나온 미해독 속성 대입 145개는 unknown으로 남겼다. 숫자 배열에서 대입되지 않은 원소의 0은 CLR `newarr`의 정의된 초기화 결과이고, 생략된 descriptor flag의 기본값을 추정하여 채운 것이 아니다. 94개는 두 method의 literal 초기화 범위에 한정되며 제품 전체 descriptor 완전성을 뜻하지 않는다.

`mathcraft-editor`는 이 계약에 근거한 독립 구조 편집 코어의 첫 범위다. 계산 문자열을 노드 이름으로 바꾸는 방식 대신 append-only arena와 안정 node ID, 부모/자식, 불완전 operand, caret side·value range·insertion mode, 전체 subtree 선택을 저장한다. undo 후 새로 편집한 분기에서도 과거 ID를 새 노드에 재사용하지 않는다. 부분 operand 범위와 matrix cell 선택은 아직 없다.

분수·지수는 기존 subtree를 보존하면서 감싸고 placeholder를 만든다. 지수의 밑이 fraction/power라면 확인한 grouping 제약을 적용한다. **어느 subtree를 감싸는지와 최초 caret 위치는 현재 독립 구현 정책이며 원본 입력 전이와 비교하지 않았다.** Space에 대응하는 최소 범위 확장은 현재 노드를 선택한 뒤 부모로 올라가며 placeholder를 건너뛴다. 괄호 나가기는 가장 가까운 Group 오른쪽에 caret을 둔다. 두 정책 역시 원본 command 연결은 확인했으나 정확한 실행 전이는 미검증이다.

`layout_fraction`에는 위에서 확인한 분수 배치식을 구현했다. 테마·글꼴 수치는 호출자가 제공하며 기본값을 가정하지 않는다. 지수의 실제 baseline/스크립트 배치와 일반 visual renderer는 아직 구현하지 않았다.

현재 7개 테스트는 불완전 분수, 중첩 지수 grouping, 선택 branch 수정, caret/선택 undo·redo, ID 비재사용, UTF-16 surrogate 경계, 원본 분수 배치식의 불변식을 검증한다. 원본 키 입력과 비교한 테스트가 아니므로 Prime 편집기와 동일하다는 결론은 내릴 수 없다. 사용 경계와 다음 비교 시나리오는 [명세 README](../spec/README.md)에 정리했다.

추가로 `mathcraft-app/src/formula.rs`에 AST를 직접 측정하고 그리는 egui 위젯을 구현했다. stacked fraction, 올라간 지수, 크기를 조절한 괄호, 공통 baseline, 노드 hit test와 구조 caret/선택을 표시한다. `/`, `^`, `(`, `)`, Space와 원자 문자 편집을 연결했다. 이것은 기존 문자열 입력란에 수식 문자열을 그대로 출력하는 렌더러가 아니다. 분수 배치식은 관찰한 식을 사용하지만 실제 font/pad/overhang 수치, 지수 크기와 위치, 괄호 비율은 **독립 시각 정책**이다. 제품의 실제 pixel geometry가 복구되었다고 주장하지 않는다.

원본과 사용자 GUI를 실행하지 않고 egui `Context::run_ui`만 사용한 테스트 3개로 중첩 분수·지수의 서로 겹치지 않는 operand geometry, 노드 hit, tessellation 결과, focus를 가진 프레임의 입력과 Tab 유지, UTF-16 문자 삭제를 확인했다. 아직 원본 IME, 다중 선택, matrix, context `=`, 모든 수평/수직 navigation과 원본의 키 입력 이력은 구현하지 않았다.
