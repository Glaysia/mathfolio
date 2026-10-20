# break·continue 직접 저작과 Ctrl+J 계약 — 2026-10-19

[FEATURES](../FEATURES.md)의 EA-076·EA-077·EA-080 조사다. 제품 코드는 수정하지 않았다. 설치 Prime 12.0.0.1 도움말과 기존 REA managed-artifact 증거를 재사용했으며 선택한 CIL을 설치 파일에서 다시 읽어 REA body SHA와 비교했다. GUI·스크린샷·원본 앱·CLR·COM 실행은 사용하지 않았다. 저작 가능 조건, 직렬화, 계산 시 유효성은 서로 다른 계약으로 다룬다.

## 근거 신원

| 기호 | 설치 파일 | SHA-256·REA Evidence |
| --- | --- | --- |
| E | `Ptc.EquationEditor.dll` | `d419032c0e99a62eab2f278e571a338d8c4b9662655af8ab462d482a028c0223`; `ev_419ab24312df266f2e58ff6b91d32e0261a9a2bba9105ba09ebecb7517b43fdf` |
| C | `Ptc.Controls.EquationControl.dll` | `0b6fd580eb6cc183f17b5b5e0967226bbafc5e12efc6da809f44cd16416bf9d1`; `ev_389338a9845c7ea83dfd310922316cf92ca1637bdbac6602a01c4690e52b8349` |
| K | `Doc/en-US/keyboard_shortcuts.pdf` | `db4cdf1f913cfad20075b3f6354085183a22bde588900709c12ca06c065b3a4f`; 설치 동봉 문서의 표시 버전은 12.0.0.0 |

파일은 `C:/Program Files/PTC/Mathcad Prime 12.0.0.1/` 아래에 있다. E·C의 MVID와 원시 REA/보조 디코더 설명은 [0002](0002-editor-semantics.md) 및 [0093](0093-label-cycle-contract.md)에 있다. 이번 조회는 `artifacts/rea/editor/{equation,control}-{members,il}.json`, `context-labels-deep/`와 SHA 검증 조회기 `artifacts/rea/feedback/label-cycle-inspect.py`를 재사용했다. 아래 token/IL은 해당 어셈블리 빌드에만 유효하다.

| 설치 도움말 | SHA-256 | 확인 내용 |
| --- | --- | --- |
| `operators_for_exiting_loops_and_programs.html` | `71ea59d760e75ce706c4ac2c23405a53128570d6e680dc68e5e4972207cbb499` | break/continue는 인자가 없음; 반복 종료/다음 반복 의미; 이름 입력 후 Ctrl+J 변환 |
| `conditional_operators.html` | `bf2f3a6803b0a6a69bfd87929a81ffa78bde1a3630678f246dacda400f9b1594` | 공백 없는 조건 연산자 이름과 Ctrl+J; if/elseif/alsoif/else의 계산·배치 의미 |
| `program_operator.html` | `477ff512864a2355d69e206e0660b1e17b8df7c38c4ce615991b57cba96c2b28` | 빈 영역·수학 영역·기존 프로그램 내부에 프로그램 생성 가능 |
| `to_define_a_program.html` | `adc169078739f2e6b29e25abe88398a5a9ebab2b5a5660888706399b2cac9dfd` | Math → Operators and Symbols → Programming 경로, Enter로 줄 추가, continue 왼쪽에서 줄 추가, for 오른쪽 바 밖에서 다음 줄 추가 |

도움말 상대 경로의 기준은 `Help/en/PTC_Mathcad_Help/`다. 도움말은 저작/계산 의미의 문서화 근거이며 새 런타임 관찰이 아니다.

## break·continue는 독립적인 0-arity leaf다

E `DefaultDescriptorTable.f`, token `0x06000bbb`, IL SHA `90ff4afc15ec4d75068d5d2c0eab08be91548ff38974f8de5b54e02a25a6376a`에서 다음을 확인했다.

| 항목 | BREAK | CONTINUE |
| --- | --- | --- |
| descriptor 생성 범위 | `IL_4542..45a5` | `IL_45aa..460d` |
| XMLFormat | `break` | `continue` |
| OperatorSExpName | `@BREAK` | `@CONTINUE` |
| OperatorText | `break` | `continue` |
| RLFormat | operand가 없는 leaf 형식 | operand가 없는 leaf 형식 |
| ChildCount | 0 | 0 |
| Fixity | None(0) | None(0) |
| Precedence | **2147483647** | **2147483647** |
| caret | 왼쪽·오른쪽 모두 허용 | 왼쪽·오른쪽 모두 허용 |
| navigable part 수 | 1 | 1 |

ChildCount/Fixity는 임의 추정이 아니다. descriptor 생성자 `0x06000f74`, IL SHA `66382848d127e9465892c3a847a444bc5dca0765d5cc0eb39d41e16f1b441f5b`에서 해당 정수 field를 변경하지 않고 두 초기화 범위에도 setter가 없음을 확인했다. `get_ChildCount` `0x06000f8e`는 그 field를 그대로 읽는다. **Precedence는 default 0이 아니라 `IL_4584/4589`, `45ec/45f1`에서 최대 정수로 명시 설정한다.**

E XML transform 등록 `0x06001167`, `IL_0abd..0b03`은 두 descriptor를 `TransformDefaultNode` `0x06001173`에 연결한다. 이 함수는 descriptor.XMLFormat을 사용한다. 따라서 native 저장은 Math50 namespace의 `<ml:break/>`, `<ml:continue/>`이며 값을 가진 identifier나 인자 placeholder가 있는 함수 호출이 아니다. 일반 program의 statement로 넣었을 때도 operand를 추가하지 않는다.

E visual-editor 등록 `g..ctor` `0x06000014`, `IL_01d8..020b`는 두 leaf를 같은 `g.ad` `0x06000023`에 연결한다. 해당 helper는 `CreateLeafOperatorEditor`를 사용하고, operator 선택 상태이면 선택을 표시하며 그 외에는 기본 caret을 표시한다. 글자를 수정할 수 있는 일반 identifier editor로 취급하지 않는다.

### 함께 확인한 프로그램 descriptor의 precedence

이번 비교에서 기존 프로그램 관련 노드의 precedence도 모두 0으로 둘 수 없음을 확인했다. 아래 값은 같은 E `0x06000bbb`의 직접 literal setter이며, 해당 메서드 CIL 시작 파일 offset은 171304다. setter token은 모두 `0x06000f9e`다.

| descriptor | 값 | literal → setter IL | literal 파일 offset / 원본 hex | setter 파일 offset |
| --- | ---: | --- | --- | ---: |
| LOCAL_ASSIGNMENT | 41 | `1c9d → 1c9f` (십진 7325 → 7327) | 178629 / `1f 29` | 178631 |
| PROGRAM | 290 | `40ab → 40b0` | 187859 / `20 22 01 00 00` | 187864 |
| WHILE_LOOP | 290 | `416b → 4170` | 188051 / `20 22 01 00 00` | 188056 |
| FOR_LOOP | 290 | `42b3 → 42b8` | 188379 / `20 22 01 00 00` | 188384 |
| BREAK | 2147483647 | `4584 → 4589` | 189100 / `20 ff ff ff 7f` | 189105 |
| CONTINUE | 2147483647 | `45ec → 45f1` | 189204 / `20 ff ff ff 7f` | 189209 |
| RETURN | 40 | `4669 → 466b` | 189329 / `1f 28` | 189331 |
| IF | 290 | `4749 → 474e` | 189553 / `20 22 01 00 00` | 189558 |

setter의 원본 opcode/token 바이트는 `6f 9e 0f 00 06`이다. 이는 해당 빌드의 직접 바이트 근거이며 다른 버전에 같은 파일 offset을 적용하지 않는다. LOCAL_ASSIGNMENT의 41은 후속 조사에서 별도로 확인해 추가했다. 290/41/40/최대 정수는 **구조 편집 parser의 결합·범위 결정값**이며 실행문의 실행 순서가 아니다. 조판에서도 부모·자식 precedence에 따라 필요 괄호를 결정하므로 모든 프로그램 노드를 0으로 취급하는 독립 구현과 차이가 생길 수 있다. 그러나 이 정적 값만으로 모든 원본 괄호·줄 배치를 재현했다고 주장하지 않는다. 반환할 합, local 정의 우변, 산술식에 들어간 프로그램, 반복 body와 새 leaf의 결합은 각각 회귀로 검증해야 한다. 표 밖의 다른 노드 값까지 확대해서 교정할 근거로 쓰지 않는다.

K의 19~20쪽은 break에 `Ctrl+Shift+{`, continue에 `Ctrl+Shift+:`, 이름 변환에 `Ctrl+J`를 문서화한다. 국가별 키보드 표기가 별도로 있다. 이 표의 문자를 임의의 물리 keycode 조합으로 바꾸거나 새로운 단축키를 추가하는 근거로 쓰지 않는다. 이번 조사에서는 해당 keycode/BAML binding 전체를 새로 대조하지 않았다.

## 루프 안에서만 저작 가능한가

두 descriptor의 **전체 초기화 범위에 `RestrictParent`, 루프 ancestor 검사, Program host 검사 등록이 없다.** 앞쪽 for descriptor의 제약 등록은 `IL_4535`에서 끝나며 break는 `4542`에서 새로 생성된다. 이 경계를 혼동하면 for의 제약을 break에도 붙였다고 오독하게 된다.

E public `InsertOperator` `0x0600017b`, IL SHA `1e29c7dc18bbf50e602aa8dffa24c21341be7d44956638b6351c0a37c46e232b`도 두 이름만을 대상으로 루프/Program을 확인하지 않는다. `AllowExpressionsOnly`의 특별 차단은 DEFINITION/GLOBAL_DEFINITION이며, symbolic evaluation/step range 등의 별도 경로가 있다. 이후 현재 mode의 일반 structural insertion으로 간다.

Ctrl+J의 helper도 아래에서 보듯 generic insertion/parser/repair를 사용하고 프로그램·루프를 찾는 직접 조건이 없다. 따라서 **break/continue에만 임의의 ‘루프 안에서만 버튼/입력 허용’ 조건을 추가할 원본 근거는 확인되지 않았다.** nested program 저작 자체는 도움말에서 허용되며, 두 leaf의 nearest-loop 계산 의미를 별개의 프로그램 저작 제한으로 바꾸면 안 된다.

다만 이 결론을 ‘어떤 위치에서도 반드시 삽입 성공’으로 확대하지 않는다. 일반 parser/repair, read-only, 선택/caret, expression-only 설정 등의 기존 조건은 남는다. 루프 밖 break/continue가 원본 계산에서 돌려주는 정확한 오류·위치·가장 가까운 루프의 제어 전파를 이번 정적 범위로 확정하지 못했다. 저작을 허용한 뒤 독립 엔진이 scope 오류를 표시하는 경로와 원본 런타임 오류의 완전 일치는 별도 검증 대상이다.

## Ctrl+J의 정확한 대상과 이름 비교

C `EquationControl.OnConvertToOperator`, `0x060001c6`는 view event로 전달하며 E presenter가 `IEquationModel.ConvertToOperator`를 연결한다. E `EquationModel.ConvertToOperator`, `0x0600016e`, IL SHA `ac0cfbf5d4870182696680a3031d4c3f642efd92becd15863d930982162bc89d`는 `Convert to operator` 명령 객체를 만들고 현재 mode의 `o()`를 호출한다.

- 기본 `b.o`, `0x0600024d`는 false다.
- Normal mode `l.o`, `0x0600007a`, IL SHA `861c6b551f4f93f173ded0aac262610b47fb14374f1951be3f6382877209b549`만 변환 helper `a8`을 실행한다. Scope·Typeover는 이 override가 없어 기본 false다. 모드 enum 신원은 [0093](0093-label-cycle-contract.md)을 따른다.
- helper `a8.bq`, `0x0600059e`, IL SHA `dbaea23c9588a86b123e3b7d67e5b4c86467ce28b38ef1873559dd97a2bb9f0c`는 현재 노드가 `IdentifierNodeInfo`이고 `HasNoLabel`일 때만 시작한다. HasNoLabel은 null 또는 None(6)이다. 명시 VARIABLE·KEYWORD 등은 변환 대상이 아니다. contextual 라벨은 reader가 명시 IdLabel과 분리하므로 해결된 contextual 라벨만 있다는 이유로 차단하지 않는다.
- `IL_0024..0060`은 DefaultDescriptorTable을 순회하며 `descriptor.OperatorText == Current.NodeInfoValueAsString`을 `String.op_Equality`로 비교한다. 대소문자 변환·trim·부분 문자열 매칭이 없다.
- caret이 단어 끝에 있는지, 일부 문자만 선택했는지를 이 helper에서 읽지 않는다. 대상은 Normal mode의 **현재 identifier 전체 값**이며 단어 조각을 정규식으로 추출하는 기능이 아니다. Scope mode의 노드/영역 선택과 Normal 내부의 문자 선택을 같은 상태로 추측하지 않는다.

실제 비교 문자열은 단순한 base 이름도 아니다. `MathNode.get_NodeInfoValueAsString` `0x06000c70` → `NodeInfoExtensions.ValueAsString` `0x0600109c` → identifier의 `FormattedString.ToString` `0x06000d7b`이다. 마지막 함수는 editable formatted string을 marker 포함으로 순회한다. `FormattedString.a` `0x06000d7e`는 형식 진입/종료 marker U+0086/U+0087과 형식 코드를 붙이며 `.cctor` `0x06000d9d`는 Subscript에 U+208A, Bold/Italic/Underline에 각각 B/I/U를 연결한다. 따라서 literal subscript를 가진 `break`의 base만 보고 변환하면 원본 비교와 다르다. 비어 있는 suffix span 등 특수 formatted 상태의 전부를 실행 확인한 것은 아니다.

## 변환 후보의 전체 확인 범위

이 빌드의 descriptor 초기화에서 비어 있지 않은 OperatorText setter는 11개이며, 고유 문자열은 10개다.

| 입력 문자열 | descriptor |
| --- | --- |
| `while` | WHILE_LOOP |
| `for` | FOR_LOOP |
| `break` | BREAK |
| `continue` | CONTINUE |
| `return` | RETURN |
| `if` | IF / IF_THEN |
| `else` | ELSE |
| `elseif` | ELSE_IF |
| `alsoif` | ALSO_IF |
| `try` | TRY |

`program`, `onerror`, 임의 함수 이름 등을 이 목록에 추가할 근거는 이 경로에서 없다. 프로그램 메뉴의 모든 항목이 이름 변환 후보인 것은 아니다.

`if` 중복은 주의가 필요하다. IF는 pair-nary이고 first=IF_THEN, middle=ELSE_IF, last=ELSE, alternate middle=ALSO_IF를 지정한다. 초기화는 IF를 `IL_4807`에서 추가한 뒤 IF_THEN을 만든다. 변환 helper에는 별도 우선순위 선택이 없고 dictionary 순회의 첫 일치에서 삽입을 시도하며, 실패해도 두 번째 동명 후보를 계속 시도하지 않는다. 이 조사에서 해당 프레임워크 dictionary의 전체 열거 구현까지 따라가지는 않았으므로 임의로 두 후보를 단순 if/else 트리와 일대일 대응시키지 않는다. 일반 parser의 pair-nary repair도 최종 형태에 관여한다.

## 이름 제거, placeholder와 cursor

변환 helper는 이름 identifier를 그대로 새 연산자의 operand로 보존하지 않는다. 보통 그 노드를 placeholder로 바꾸고, descriptor로 만든 새 노드를 generic insertion `bg.bq`, `0x060005c4`, IL SHA `25d960678b62705f5d8537a636f5d5997588222b4a40f9e3af9b9c8ab0dd39e5`에 전달한다. prefix/primal-child 연산자가 implicit parent의 첫 자식인 특수 경우에는 뒤의 기존 operand를 보존하는 별도 분기도 있다. 이 특수 분기 때문에 ‘현재 identifier만 항상 단순 교체’라는 일반 규칙으로 전체 Ctrl+J를 설명할 수 없다.

cursor helper `bg.a`, `0x060005c6`, IL SHA `3bc6ac5566ace12d0ee48c1c7b8795bbaffb44737548ba64e75b270f85ef6a69`는 fixity로 나눈다.

- break/continue: Fixity None, CarriesValue 없음. `IL_018a..0191`에서 새 leaf 오른쪽의 navigator 상태를 만든다. operand placeholder를 생성하지 않는다.
- return: Prefix. 첫 child가 placeholder이면 그 child로 이동한다(`IL_0061..009d`). 이름 `return`을 반환값으로 남기는 변환이 아니다.
- while/for/if: Special. `WalkSubTree`로 첫 placeholder를 찾아 그 안으로 이동한다(`IL_0112..0140`). while은 조건/body, for는 반복 변수/range/body 구조이며 if는 위 pair-nary 구조와 repair가 관여한다. 이 사실을 근거 없이 기존 operand를 보존하는 UI wrapper 동작과 동일시하지 않는다.

완성된 구조와 입력 중간 상태에서 모든 branch·nested body·implicit adjacency의 최종 cursor를 실행 검증한 것은 아니다. 현재 AST로 표현하지 못하는 else/elseif/alsoif/try를 다른 연산자로 묵시 변환해서는 안 된다.

## 리본에서 이미 값이 있는 위치에 삽입하는 동작

Ctrl+J의 이름 제거 경로와 리본의 일반 `InsertOperator`는 다르다. Normal mode `l.a`, `0x06000088`, IL SHA `d2e82ba51ec7275cbe5e6b911eaf06401ff6a634851661ae6462fb31e78c74e6`는 `IL_0158..01c0`에서 문자 caret/SelectionStart/SelectionEnd를 보고 value insertion에 전달하고, 그 밖에는 caret side·family·operator 경계에 따라 다른 삽입을 수행한다. generic `bg.bq`도 현재 container의 analysis list에 노드를 앞/뒤로 넣어 parse와 repair를 거친다.

따라서 populated target에서 항상 전체 교체, 항상 옆 삽입, 항상 무작업 중 하나라고 단정하지 않는다. 특히 Ctrl+J용 placeholder/identifier 교체 API를 리본의 모든 대상에 그대로 적용했다는 이유로 원본 리본 동작이 완성되었다고 세지 않는다. 제한된 placeholder 입력과 명시 이름 변환을 먼저 지원한다면 이 제한을 별도로 기록하고, 기존 operand 보존/분할·선택·scope의 원본 분기 행렬은 후속으로 재현해야 한다.

## Undo와 남은 한계

### leaf에 인접한 Backspace/Delete의 후속 조사

새 leaf를 선택했을 때만 지울 수 있게 두어서는 원본 삭제 경로를 재현하지 못한다. 기존 원시 증거에서 다음 **안쪽 방향** 삭제를 확인했다. 일반 프로그램 줄 병합 전체와는 구분한다.

C `EquationControl.OnBackspace`, `0x06000188`, IL SHA `5df471b0f29c9b6d38fdfdb21cece2759e28720c49f96d173164282d1668efe0`는 `FireDeleteLeft`를, `OnDelete`, `0x0600018a`, IL SHA `fc9f8b48f9161ed238f41aee49858cb13c06e0ff60fc7282f84287dc17355b16`는 `FireDeleteRight`를 호출한다. E `EquationModel.Delete`, `0x06000173`, IL SHA `d1f9ec8d9a4021489c69227bf6c03096daab5f25c12e669f1db7b70c6677006c`는 하나의 `Delete` 명령으로 현재 mode에 방향을 전달한다. Normal mode `l.j`, `0x06000075` → `ax.a`, `0x060004e6` → `bh.br`, `0x060005c8`가 삭제의 상태를 만든다.

E navigation-state 목록 생성기 `bq.f`, `0x0600060c`, IL SHA `42ead008bbbf5ddbf30f54951ecfcd994c966bc6d13c0ac530b3222947eec087`의 `IL_005d..00ab`는 Fixity None leaf에 대해 다음 상태를 만든다.

1. 기본 `NavigatorState(node)`는 CaretSide=Indeterminate(4)인 operator 중심 상태다(`0x0600130f`).
2. IsLeafOperator이고 AllowCaretOnRight이면 그 뒤에 Right(1)를 추가한다.
3. AllowCaretOnLeft이면 중심 상태 앞에 Left(0)를 삽입한다.

break/continue는 앞에서 확인한 두 허용 속성이 모두 true이므로 해당 leaf의 순서는 **Left → Center → Right**다. `bq.c`, `0x06000600`, IL SHA `46ca7eee9126d02c7e6e6d739fe5973aea13db14544cee65e49d6d0578359ece`는 Left 이동에서 목록 index−1, Right 이동에서 index+1을 선택한다. CarriesValue가 false인 leaf에는 문자 caret 위치를 추가하는 변환도 적용하지 않는다(`bq.b`, `0x06000602`).

삭제 단계 배열 `ap.b`, `0x0600042e`, IL SHA `b98fe5c621a8025d7612d3a83874e72e8dbdce39ea75c237217986068ee57263`는 leaf 처리 `ap.ae`를 index 5와 25에 두 차례 등록하고, 그 중간 index 21에 `ap.t`를 둔다. `ap.t`, `0x06000447`, IL SHA `f9d5b2c4750f7d13052c2480c0a391f12e19707443fbb0b0072a4159ebc6c0b3`의 `IL_0044..005a`는 navigator를 **삭제 방향으로 한 상태 이동**시키고 `IL_00ba..00bc`에서 현재 작업 상태를 갱신한다. 이는 별도의 사용자 화살표 입력을 요구하는 것이 아니라 한 Delete 처리 과정 안의 이동이다.

leaf 처리 `ap.ae`, `0x0600043c`, IL SHA `439c1eea6c3426979e41c137238144890d8191693698b406ca8df7c2986ae0c7`는 Current.IsLeafOperator와 CaretSide=4를 검사한다. 조건이 맞으면 `IL_0026..0040`에서 처리 완료 flag를 세우고 새 placeholder를 만들어 `Utilities.ReplaceNode`로 **그 leaf 자리만 교체**한다. `IL_0045..0052`는 새 placeholder의 Left 방향 navigator를 결과로 둔다. 처리 배열 순회 `0x06000432`는 완료 flag가 세워지면 이후 단계를 실행하지 않는다.

따라서 이 leaf 삭제 경로의 구현 대상은 다음과 같다.

| 시작 상태·키 | 안쪽 이동과 처리 | 결과 |
| --- | --- | --- |
| break/continue 오른쪽 caret + Backspace | Right → Center → leaf 처리 | 현재 leaf 자리에 placeholder, 그 placeholder의 caret |
| break/continue 왼쪽 caret + Delete | Left → Center → leaf 처리 | 현재 leaf 자리에 placeholder, 그 placeholder의 caret |
| 이미 operator 중심 선택인 상태의 삭제 | 첫 leaf 처리에서 바로 수행 가능 | 같은 placeholder 교체 |

위 경로는 프로그램 statement 행 자체를 삭제하거나 자식·형제 행을 합치는 처리가 아니다. 교체 전후는 상위의 한 Delete Undo 명령에 속한다. 반대로 **leaf 오른쪽에서 Delete / leaf 왼쪽에서 Backspace**는 바깥 이웃·parent·program 경계로 가는 다른 삭제 단계에 들어갈 수 있다. 이를 현재 leaf의 강제 삭제나 무조건 줄 병합으로 바꾸는 근거는 이번에 확인하지 않았다. read-only·다른 구조의 선행 삭제 handler와 프로그램 경계의 전 상태를 실행 검증한 것도 아니다.

Ctrl+J는 한 `Convert to operator` 명령, 직접 연산자 삽입은 한 `insert operator` 명령 객체를 사용한다. 공통 `e` 명령 생성·실행 `0x06000b63/0b6b`와 Undo/Redo `0x06000b68/0b69`는 편집 전후 상태 및 부수 CompoundCommand를 복원한다. 실패한 structural conversion을 문자 삭제로 먼저 확정하거나 별개의 여러 Undo로 나누는 것은 이 관찰과 맞지 않는다. 원본 사용자 이력의 모든 연속 명령 병합 정책은 이번 범위 밖이다.

검증할 독립 구현의 최소 범위는 zero-arity 검증/native roundtrip, 새 leaf의 양쪽 caret와 단일 Undo, 정확한 lowercase 이름 및 명시/contextual/None 차이, suffix 형식 보존·변환 거부, 지원하지 않는 이름의 무변경, 루프/중첩 프로그램 scope의 계산 오류와 제어 전파다. 원본 계산의 루프 밖 정확한 오류, 메뉴 populated 대상의 전 분기, if dictionary/repair의 모든 입력 조합, read-only 상위 command routing, locale별 실제 key binding은 미확인이다. 문서화된 키와 정적 코드의 부분 복구를 전체 원본 편집 일치로 표시하지 않는다.
